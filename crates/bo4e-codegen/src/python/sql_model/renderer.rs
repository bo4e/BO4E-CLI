use super::plan::{JunctionTable, SqlField, SqlPlan, TablePlan};
use crate::error::Error;
use crate::naming::sanitize_member_name;
use crate::python::python_attr_name;
use minijinja::{Environment, context};
use serde::Serialize;
use serde_json::Map as JsonMap;
use std::collections::{BTreeMap, BTreeSet};

/// `model_config` line shared by every generated SQLModel class.
const MODEL_CONFIG: &str = "model_config = ConfigDict(alias_generator=to_camel, \
                            populate_by_name=True, use_attribute_docstrings=True)";

/// Inject `alias="<json_name>"` as the first argument inside an existing
/// `Field(...)` (or `Field()`) call. Pydantic v2 forbids leading-underscore
/// attribute names, so the renderer renames `_typ` → `typ` and uses the alias
/// to keep the JSON wire format identical.
fn inject_alias(field_def: &str, alias: &str) -> String {
    let Some(open) = field_def.find('(') else {
        return field_def.to_string();
    };
    let prefix = &field_def[..open + 1];
    let rest = &field_def[open + 1..];
    // `alias` is the original JSON property name. Validator restricts
    // property names to identifier-shaped characters, so escape is
    // technically defensive — but routing through the shared helper
    // keeps every string-into-Python-source site uniform and prevents
    // drift if a future relaxation widens the allowed name set.
    let alias_lit = crate::python::python_string_literal(alias);
    if rest.starts_with(')') {
        format!("{prefix}alias={alias_lit})")
    } else if let Some(after_ellipsis) = rest.strip_prefix("...") {
        format!("{prefix}..., alias={alias_lit}{after_ellipsis}")
    } else {
        format!("{prefix}alias={alias_lit}, {rest}")
    }
}

/// Insert one row into the `SQL.fields` map, applying the leading-underscore
/// rename + alias injection. Centralised so every `SqlField` variant gets the
/// same treatment without each match arm having to remember.
fn insert_field(
    fields_map: &mut JsonMap<String, serde_json::Value>,
    name: &str,
    annotation: String,
    definition: String,
    description: Option<String>,
) {
    let py_name = python_attr_name(name);
    let definition = if py_name != name {
        inject_alias(&definition, name)
    } else {
        definition
    };
    fields_map.insert(
        py_name,
        serde_json::to_value(SqlFieldRow {
            annotation,
            definition,
            description,
        })
        .unwrap(),
    );
}

/// Per-field row passed to `BaseModel.jinja2`'s `SQL.fields` map.
#[derive(Debug, Serialize)]
struct SqlFieldRow {
    annotation: String,
    definition: String,
    description: Option<String>,
}

/// One SQL import passed to `BaseModel.jinja2`'s `SQL.imports` loop.
/// Note: trailing underscores match the template's `{{ import.from_ }}` and
/// `{{ import.import_ }}` references directly — no serde rename needed.
///
/// A `None` `from_` means a bare `import X` statement (rendered as
/// `from_: ""`, `import_: "X"`). When `from_` is set, multiple imports
/// from the same module are grouped into one comma-separated `import_`.
#[derive(Debug, Serialize)]
struct SqlImport {
    from_: String,
    import_: String,
    alias: Option<String>,
}

impl Ord for SqlImport {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.from_.as_str(),
            self.import_.as_str(),
            self.alias.as_deref(),
        )
            .cmp(&(
                other.from_.as_str(),
                other.import_.as_str(),
                other.alias.as_deref(),
            ))
    }
}
impl PartialOrd for SqlImport {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for SqlImport {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}
impl Eq for SqlImport {}

/// Raw import entry before grouping; `alias` only set when the module itself is aliased
/// (e.g. `import uuid as uuid_pkg`), not when the name is aliased.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RawImport {
    /// Module path, e.g. `"sqlmodel"`, `"uuid"`.
    from_: String,
    /// Single name to import, e.g. `"Field"`, `"uuid"`.
    name: String,
    /// Module-level alias (only for bare `import X as Y`).
    alias: Option<String>,
}

/// Convert the language-neutral [`crate::imports::Import`] values
/// produced by [`crate::python::types::map_pydantic`] into the
/// SQL-renderer's `RawImport` shape. Sibling imports turn into
/// relative `from .X.Y import Z` lines using `depth` super-dots.
/// Named imports map 1:1.
fn raw_imports_from(
    imports: &BTreeSet<crate::imports::Import>,
    depth: usize,
) -> BTreeSet<RawImport> {
    use crate::imports::Import;
    let mut out = BTreeSet::new();
    for imp in imports {
        match imp {
            Import::Named { module, name } => {
                out.insert(RawImport {
                    from_: module.clone(),
                    name: name.clone(),
                    alias: None,
                });
            }
            Import::Sibling { module, name } => {
                let dots = ".".repeat(depth);
                let path = module
                    .iter()
                    .take(module.len().saturating_sub(1))
                    .cloned()
                    .chain(std::iter::once(crate::layout::module_file_name(module)))
                    .collect::<Vec<_>>()
                    .join(".");
                out.insert(RawImport {
                    from_: format!("{dots}{path}"),
                    name: name.clone(),
                    alias: None,
                });
            }
        }
    }
    out
}

/// Convert a set of `RawImport`s to grouped, sorted `SqlImport`s suitable for the template.
/// Imports that share the same `from_` and no alias are combined on one line.
/// Imports with an alias are always kept separate.
fn group_imports(raw: BTreeSet<RawImport>) -> Vec<SqlImport> {
    // group by (from_, alias) — aliased imports stay separate
    let mut grouped: BTreeMap<(String, Option<String>), BTreeSet<String>> = BTreeMap::new();
    for r in raw {
        grouped
            .entry((r.from_, r.alias))
            .or_default()
            .insert(r.name);
    }
    let mut result: Vec<SqlImport> = grouped
        .into_iter()
        .map(|((from_, alias), names)| {
            let import_ = names.into_iter().collect::<Vec<_>>().join(", ");
            SqlImport {
                from_,
                import_,
                alias,
            }
        })
        .collect();
    result.sort();
    result
}

/// Render a table's source as a Python module body.
/// `depth` is the relative-import depth (1 = root-level module, 2 = one subdir, …).
/// `class_to_module` maps class names to their parent directory segments (lowercased),
/// e.g. `"Angebot" → ["bo"]`, `"Adresse" → ["com"]`.
pub(crate) fn render_table(
    env: &Environment<'_>,
    table: &TablePlan,
    depth: usize,
    class_to_module: &BTreeMap<String, Vec<String>>,
) -> Result<String, Error> {
    if table.is_enum {
        return render_enum(env, table);
    }

    // Collect raw imports; they'll be grouped (by module) before rendering.
    let mut raw_imports: BTreeSet<RawImport> = BTreeSet::new();
    raw_imports.insert(RawImport {
        from_: "uuid".into(),
        name: "uuid".into(),
        alias: Some("uuid_pkg".into()),
    });
    raw_imports.insert(RawImport {
        from_: "sqlmodel".into(),
        name: "Field".into(),
        alias: None,
    });
    raw_imports.insert(RawImport {
        from_: "sqlmodel".into(),
        name: "SQLModel".into(),
        alias: None,
    });
    raw_imports.insert(RawImport {
        from_: "pydantic".into(),
        name: "ConfigDict".into(),
        alias: None,
    });
    raw_imports.insert(RawImport {
        from_: "pydantic.alias_generators".into(),
        name: "to_camel".into(),
        alias: None,
    });

    // `SQL.fields` must be a dict-like value so the template's `.items()` call works.
    // We use `serde_json::Map` (insertion-ordered) serialized via MiniJinja's JSON bridge.
    let mut fields_map: JsonMap<String, serde_json::Value> = JsonMap::new();

    for sql_field in &table.sql_fields {
        match sql_field {
            SqlField::Scalar {
                name,
                type_,
                default,
                docstring,
                imports,
                ..
            } => {
                // Schema-driven: the default comes from the property's
                // declared `default` literal. No field-name special cases.
                let definition = match default {
                    Some(d) if d.starts_with("Field(") => d.clone(),
                    Some(d) => format!("Field(default={d})"),
                    None => "Field(...)".to_string(),
                };
                // Pull through every import the type mapper attached:
                // `typing.Literal` for single-variant narrowing,
                // `datetime.date` / `time` / `datetime`, `uuid.UUID`,
                // `decimal.Decimal`. Without this the rendered type or
                // default expression would reference undefined names.
                raw_imports.extend(raw_imports_from(imports, depth));
                insert_field(
                    &mut fields_map,
                    name,
                    type_.clone(),
                    definition,
                    docstring.clone(),
                );
            }
            SqlField::ForeignKey {
                name,
                target_table,
                nullable,
                ondelete,
                docstring,
                ..
            } => {
                let annotation = if *nullable {
                    "uuid_pkg.UUID | None".to_string()
                } else {
                    "uuid_pkg.UUID".to_string()
                };
                let definition = if *nullable {
                    let mut args = format!("default=None, foreign_key=\"{target_table}.id\"");
                    if let Some(od) = ondelete {
                        args.push_str(&format!(", ondelete=\"{od}\""));
                    }
                    format!("Field({args})")
                } else {
                    format!("Field(..., foreign_key=\"{target_table}.id\")")
                };
                insert_field(
                    &mut fields_map,
                    name,
                    annotation,
                    definition,
                    docstring.clone(),
                );
            }
            SqlField::Relationship {
                name,
                target_class,
                owner_class,
                fk_field_name,
                nullable,
                docstring,
            } => {
                let annotation = if *nullable {
                    format!("{target_class} | None")
                } else {
                    target_class.clone()
                };
                let definition = format!(
                    "Relationship(sa_relationship_kwargs={{\"foreign_keys\": [\"{owner_class}.{fk_field_name}\"]}})"
                );
                raw_imports.insert(RawImport {
                    from_: "sqlmodel".into(),
                    name: "Relationship".into(),
                    alias: None,
                });
                let target_module = class_to_module
                    .get(target_class.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                raw_imports.extend(target_raw_imports(target_class, target_module, depth));
                insert_field(
                    &mut fields_map,
                    name,
                    annotation,
                    definition,
                    docstring.clone(),
                );
            }
            SqlField::ManyRelationship {
                name,
                target_class,
                link_class,
                nullable,
                docstring,
            } => {
                let annotation = if *nullable {
                    format!("list[{target_class}] | None")
                } else {
                    format!("list[{target_class}]")
                };
                let definition = format!("Relationship(link_model={link_class})");
                raw_imports.insert(RawImport {
                    from_: "sqlmodel".into(),
                    name: "Relationship".into(),
                    alias: None,
                });
                let target_module = class_to_module
                    .get(target_class.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                raw_imports.extend(target_raw_imports(target_class, target_module, depth));
                raw_imports.insert(RawImport {
                    from_: format!("{}many", ".".repeat(depth)),
                    name: link_class.clone(),
                    alias: None,
                });
                insert_field(
                    &mut fields_map,
                    name,
                    annotation,
                    definition,
                    docstring.clone(),
                );
            }
            SqlField::EnumColumn {
                name,
                enum_class,
                is_list,
                nullable,
                default,
                docstring,
                ..
            } => {
                let mut annotation = if *is_list {
                    format!("list[{enum_class}]")
                } else {
                    enum_class.clone()
                };
                if *nullable {
                    annotation.push_str(" | None");
                }
                let enum_table_name = enum_class.to_ascii_lowercase();
                let sa_column = if *is_list {
                    format!("Column(ARRAY(Enum({enum_class}, name=\"{enum_table_name}\")))")
                } else {
                    format!("Column(Enum({enum_class}, name=\"{enum_table_name}\"))")
                };
                let mut args = String::new();
                if let Some(d) = default {
                    args.push_str(&format!("default={d}, "));
                }
                args.push_str(&format!("sa_column={sa_column}"));
                let definition = format!("Field({args})");
                raw_imports.insert(RawImport {
                    from_: "sqlalchemy".into(),
                    name: "Column".into(),
                    alias: None,
                });
                raw_imports.insert(RawImport {
                    from_: "sqlalchemy".into(),
                    name: "Enum".into(),
                    alias: None,
                });
                if *is_list {
                    raw_imports.insert(RawImport {
                        from_: "sqlalchemy".into(),
                        name: "ARRAY".into(),
                        alias: None,
                    });
                }
                raw_imports.extend(enum_raw_imports(enum_class, depth));
                insert_field(
                    &mut fields_map,
                    name,
                    annotation,
                    definition,
                    docstring.clone(),
                );
            }
            SqlField::ScalarArray {
                name,
                py_inner,
                sa_inner,
                nullable,
                docstring,
                ..
            } => {
                let annotation = if *nullable {
                    format!("list[{py_inner}] | None")
                } else {
                    format!("list[{py_inner}]")
                };
                let definition = format!("Field(sa_column=Column(ARRAY({sa_inner})))");
                raw_imports.insert(RawImport {
                    from_: "sqlalchemy".into(),
                    name: "ARRAY".into(),
                    alias: None,
                });
                raw_imports.insert(RawImport {
                    from_: "sqlalchemy".into(),
                    name: "Column".into(),
                    alias: None,
                });
                raw_imports.insert(RawImport {
                    from_: "sqlalchemy".into(),
                    name: (*sa_inner).into(),
                    alias: None,
                });
                if py_inner == "Decimal" {
                    raw_imports.insert(RawImport {
                        from_: "decimal".into(),
                        name: "Decimal".into(),
                        alias: None,
                    });
                }
                insert_field(
                    &mut fields_map,
                    name,
                    annotation,
                    definition,
                    docstring.clone(),
                );
            }
            SqlField::AnyColumn {
                name,
                is_array,
                nullable,
                docstring,
            } => {
                // `Any` already covers None; only widen the outer `list[Any]` when nullable.
                let annotation = if *is_array {
                    if *nullable {
                        "list[Any] | None".to_string()
                    } else {
                        "list[Any]".to_string()
                    }
                } else {
                    "Any".to_string()
                };
                let definition = if *is_array {
                    format!(
                        "Field(sa_column=Column(ARRAY(PickleType), nullable={}))",
                        py_bool(*nullable)
                    )
                } else {
                    format!(
                        "Field(sa_column=Column(PickleType, nullable={}))",
                        py_bool(*nullable)
                    )
                };
                raw_imports.insert(RawImport {
                    from_: "typing".into(),
                    name: "Any".into(),
                    alias: None,
                });
                raw_imports.insert(RawImport {
                    from_: "sqlalchemy".into(),
                    name: "Column".into(),
                    alias: None,
                });
                raw_imports.insert(RawImport {
                    from_: "sqlalchemy".into(),
                    name: "PickleType".into(),
                    alias: None,
                });
                if *is_array {
                    raw_imports.insert(RawImport {
                        from_: "sqlalchemy".into(),
                        name: "ARRAY".into(),
                        alias: None,
                    });
                }
                insert_field(
                    &mut fields_map,
                    name,
                    annotation,
                    definition,
                    docstring.clone(),
                );
            }
        }
    }

    let imports_vec = group_imports(raw_imports);

    // Convert the insertion-ordered JsonMap to a MiniJinja Value so `.items()` works
    // in the template's `for field_name, field in SQL.fields.items()` loop.
    let fields_jinja: minijinja::Value = minijinja::Value::from_serialize(&fields_map);

    let tpl = env.get_template("python/sql_model/BaseModel.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => table.class_name.clone(),
        base_class => "SQLModel",
        description => table.description.clone().unwrap_or_else(|| table.class_name.clone()),
        fields => Vec::<String>::new(),
        methods => Vec::<String>::new(),
        model_config => MODEL_CONFIG,
        config => None::<String>,
        SQL => context! {
            imports => imports_vec,
            fields => fields_jinja,
        },
    })?;
    Ok(prepend_module_docstring(&table.class_name, &rendered))
}

/// Prepend `"""Contains class X."""` to a rendered module body, mirroring the
/// pydantic generator's `stitch()`. The body still contains its own import block,
/// so we just push the docstring on top with a blank line.
fn prepend_module_docstring(class_name: &str, body: &str) -> String {
    let docstring = format!("\"\"\"Contains class {class_name}.\"\"\"\n");
    let body_trimmed = body.trim_start_matches('\n');
    format!("{docstring}\n{body_trimmed}")
}

fn render_enum(env: &Environment<'_>, table: &TablePlan) -> Result<String, Error> {
    let members: Vec<minijinja::Value> = table
        .enum_members
        .iter()
        .map(|v| {
            minijinja::Value::from_serialize(&context! {
                name => sanitize_member_name(v),
                default => crate::python::python_string_literal(v),
                docstring => None::<String>,
            })
        })
        .collect();

    let tpl = env.get_template("python/sql_model/Enum.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => table.class_name.clone(),
        base_class => "StrEnum",
        description => table.description.clone(),
        fields => members,
    })?;
    let body = format!(
        "from enum import StrEnum\n\n{}",
        rendered.trim_start_matches('\n')
    );
    Ok(prepend_module_docstring(&table.class_name, &body))
}

fn target_raw_imports(
    target_class: &str,
    target_module: &[String],
    depth: usize,
) -> impl IntoIterator<Item = RawImport> {
    let target_table = target_class.to_ascii_lowercase();
    let module_path = if target_module.is_empty() {
        target_table.clone()
    } else {
        format!("{}.{}", target_module.join("."), target_table)
    };
    [RawImport {
        from_: format!("{}{}", ".".repeat(depth), module_path),
        name: target_class.to_string(),
        alias: None,
    }]
}

fn enum_raw_imports(enum_class: &str, depth: usize) -> impl IntoIterator<Item = RawImport> {
    let enum_table = enum_class.to_ascii_lowercase();
    [RawImport {
        from_: format!("{}enum.{}", ".".repeat(depth), enum_table),
        name: enum_class.to_string(),
        alias: None,
    }]
}

fn py_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// Render `<output>/many.py`. Returns an empty string when there are no junctions
/// (caller should not write the file in that case).
pub(crate) fn render_many(
    env: &Environment<'_>,
    junctions: &[JunctionTable],
) -> Result<String, Error> {
    if junctions.is_empty() {
        return Ok(String::new());
    }
    let links: Vec<minijinja::Value> = junctions
        .iter()
        .map(|j| {
            context! {
                table_name => j.class_name.clone(),
                cls1 => j.owner_class.clone(),
                cls2 => j.target_class.clone(),
                rel_field_name1 => j.source_field.clone(),
                id_field_name1 => j.owner_id_field.clone(),
                id_field_name2 => j.target_id_field.clone(),
            }
        })
        .map(minijinja::Value::from_serialize)
        .collect();

    let tpl = env.get_template("python/sql_model/ManyLinks.jinja2")?;
    let rendered = tpl.render(context! { links => links })?;
    Ok(rendered)
}

/// Render `<output>/__init__.py` re-exporting every class and every junction.
pub(crate) fn render_init(env: &Environment<'_>, plan: &SqlPlan) -> Result<String, Error> {
    let classes: Vec<minijinja::Value> = plan
        .tables
        .values()
        .map(|t| {
            let module_path: Vec<String> = t
                .module
                .iter()
                .take(t.module.len().saturating_sub(1))
                .map(|s| s.to_ascii_lowercase())
                .chain(std::iter::once(crate::layout::module_file_name(&t.module)))
                .collect();
            context! {
                name => t.class_name.clone(),
                module_path => module_path,
            }
        })
        .map(minijinja::Value::from_serialize)
        .collect();

    let links: Vec<String> = plan
        .junctions
        .iter()
        .map(|j| j.class_name.clone())
        .collect();

    let all_names: Vec<String> = plan
        .tables
        .values()
        .map(|t| t.class_name.clone())
        .chain(links.iter().cloned())
        .collect();

    let tpl = env.get_template("python/sql_model/__init__.jinja2")?;
    let rendered = tpl.render(context! {
        classes => classes,
        links => links,
        all_names => all_names,
    })?;
    Ok(rendered)
}

pub(crate) fn render_version(version: &str) -> String {
    format!(
        "__version__: str = {}\n",
        crate::python::python_string_literal(version)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::make_environment;

    fn fixture_plan() -> super::super::plan::SqlPlan {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bo4e_sql_min");
        let schemas = bo4e_schemas::io::schemas::read_schemas(&path)
            .expect("read bo4e_sql_min")
            .schemas;
        super::super::plan::build_plan(&schemas).expect("plan builds for fixture")
    }

    fn fixture_class_to_module(
        plan: &super::super::plan::SqlPlan,
    ) -> BTreeMap<String, Vec<String>> {
        plan.tables
            .values()
            .map(|t| {
                let parents: Vec<String> = t
                    .module
                    .iter()
                    .take(t.module.len().saturating_sub(1))
                    .map(|s| s.to_ascii_lowercase())
                    .collect();
                (t.class_name.clone(), parents)
            })
            .collect()
    }

    #[test]
    fn render_table_object_emits_sqlmodel_class() {
        let env = make_environment(None).unwrap();
        let plan = fixture_plan();
        let class_to_module = fixture_class_to_module(&plan);
        let angebot = plan
            .tables
            .get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table");
        let body = render_table(&env, angebot, 2, &class_to_module).expect("render");
        assert!(
            body.contains("class Angebot(SQLModel, table=True):"),
            "got:\n{body}"
        );
        assert!(
            body.contains("id_: uuid_pkg.UUID = Field(alias=\"_id\", default_factory=uuid_pkg.uuid4, primary_key=True"),
            "got:\n{body}"
        );
        assert!(
            body.contains(
                "adresse_id: uuid_pkg.UUID | None = Field(default=None, foreign_key=\"adresse.id\""
            ),
            "got:\n{body}"
        );
        assert!(
            body.contains("adresse: Adresse | None = Relationship("),
            "got:\n{body}"
        );
        assert!(
            body.contains(
                "adressen: list[Adresse] | None = Relationship(link_model=AngebotAdressenLink)"
            ),
            "got:\n{body}"
        );
        assert!(
            body.contains(
                "typ: Typ | None = Field(alias=\"_typ\", default=Typ.ANGEBOT, sa_column="
            ),
            "got:\n{body}"
        );
        assert!(
            body.contains("werte: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))"),
            "got:\n{body}"
        );
        assert!(
            body.contains("extras: Any = Field(sa_column=Column(PickleType, nullable=True))")
                && !body.contains("extras: Any | None"),
            "got:\n{body}"
        );
        assert!(
            body.contains("model_config = ConfigDict(alias_generator=to_camel, populate_by_name=True, use_attribute_docstrings=True)"),
            "got:\n{body}"
        );
        assert!(
            body.starts_with("\"\"\"Contains class Angebot.\"\"\""),
            "got:\n{body}"
        );
        assert!(
            body.contains(
                "anhaenge: list[Any] = Field(sa_column=Column(ARRAY(PickleType), nullable=False))"
            ),
            "got:\n{body}"
        );
        assert!(body.contains("import uuid as uuid_pkg"), "got:\n{body}");
        assert!(body.contains("from typing import Any"), "got:\n{body}");
        assert!(
            body.contains("from sqlmodel import Field, Relationship, SQLModel"),
            "got:\n{body}"
        );
        assert!(
            body.contains("from ..com.adresse import Adresse"),
            "got:\n{body}"
        );
        assert!(
            body.contains("from ..many import AngebotAdressenLink"),
            "got:\n{body}"
        );
        assert!(body.contains("from ..enum.typ import Typ"), "got:\n{body}");
    }

    #[test]
    fn render_table_bo_to_bo_relationship_uses_bo_module() {
        use super::super::plan::{SqlField, TablePlan};
        let env = make_environment(None).unwrap();

        // Hypothetical Angebot with a Relationship to Geschaeftspartner (in bo/).
        let angebot_table = TablePlan {
            module: vec!["bo".to_string(), "Angebot".to_string()],
            class_name: "Angebot".to_string(),
            is_enum: false,
            description: None,
            enum_members: vec![],
            sql_fields: vec![
                SqlField::Scalar {
                    name: "_id".to_string(),
                    type_: "uuid_pkg.UUID".to_string(),
                    nullable: false,
                    default: Some(
                        "Field(default_factory=uuid_pkg.uuid4, primary_key=True, title=\"Id\")"
                            .to_string(),
                    ),
                    title: None,
                    docstring: Some("Primary key.".to_string()),
                    imports: std::collections::BTreeSet::new(),
                },
                SqlField::ForeignKey {
                    name: "geschaeftspartner_id".to_string(),
                    target_class: "Geschaeftspartner".to_string(),
                    target_table: "geschaeftspartner".to_string(),
                    nullable: true,
                    ondelete: Some("SET NULL".to_string()),
                    docstring: None,
                },
                SqlField::Relationship {
                    name: "geschaeftspartner".to_string(),
                    target_class: "Geschaeftspartner".to_string(),
                    owner_class: "Angebot".to_string(),
                    fk_field_name: "geschaeftspartner_id".to_string(),
                    nullable: true,
                    docstring: None,
                },
            ],
        };

        // class_to_module: Geschaeftspartner lives in bo/.
        let mut class_to_module: BTreeMap<String, Vec<String>> = BTreeMap::new();
        class_to_module.insert("Geschaeftspartner".to_string(), vec!["bo".to_string()]);

        let body = render_table(&env, &angebot_table, 2, &class_to_module).expect("render");

        assert!(
            body.contains("from ..bo.geschaeftspartner import Geschaeftspartner"),
            "expected bo import, got:\n{body}"
        );
        assert!(
            !body.contains("from ..com.geschaeftspartner import Geschaeftspartner"),
            "must NOT contain com import, got:\n{body}"
        );
    }

    #[test]
    fn render_many_py_emits_one_class_per_junction() {
        let env = make_environment(None).unwrap();
        let plan = fixture_plan();
        let body = render_many(&env, &plan.junctions).expect("render");
        assert!(
            body.contains("class AngebotAdressenLink(SQLModel, table=True):"),
            "got:\n{body}"
        );
        assert!(body.contains(
            "angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"angebot.id\""
        ));
        assert!(body.contains(
            "adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"adresse.id\""
        ));
    }

    #[test]
    fn render_init_includes_classes_and_links() {
        let env = make_environment(None).unwrap();
        let plan = fixture_plan();
        let body = render_init(&env, &plan).expect("render");
        assert!(body.contains("from .bo.angebot import Angebot"));
        assert!(body.contains("from .com.adresse import Adresse"));
        assert!(body.contains("from .enum.typ import Typ"));
        assert!(body.contains("from .many import AngebotAdressenLink"));
        assert!(body.contains("from .__version__ import __version__"));
        assert!(body.contains("__all__ = ["));
        assert!(body.contains("\"__version__\","));
        assert!(body.contains("\"Angebot\","));
        assert!(body.contains("\"AngebotAdressenLink\","));
    }

    #[test]
    fn render_version_emits_constant() {
        let body = render_version("202401.4.0");
        assert_eq!(body.trim(), "__version__: str = \"202401.4.0\"");
    }
}
