//! python-sql-model generator orchestration.
//!
//! Two-phase: a pure pre-pass walks `Schemas` and produces an immutable
//! [`plan::SqlPlan`]; a render pass consumes the plan and writes Python files
//! via vendored MiniJinja templates.

pub(crate) mod plan;

use bo4e_schemas::Schemas;
use crate::error::Error;
use crate::naming::module_file_name;
use minijinja::{Environment, context};
use plan::{JunctionTable, SqlField, SqlPlan, TablePlan};
use serde::Serialize;
use serde_json::Map as JsonMap;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
        (self.from_.as_str(), self.import_.as_str(), self.alias.as_deref())
            .cmp(&(other.from_.as_str(), other.import_.as_str(), other.alias.as_deref()))
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
            SqlImport { from_, import_, alias }
        })
        .collect();
    result.sort_by(|a, b| a.cmp(b));
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

    // `SQL.fields` must be a dict-like value so the template's `.items()` call works.
    // We use `serde_json::Map` (insertion-ordered) serialized via MiniJinja's JSON bridge.
    let mut fields_map: JsonMap<String, serde_json::Value> = JsonMap::new();

    for sql_field in &table.sql_fields {
        match sql_field {
            SqlField::Scalar { name, type_, default, docstring, .. } => {
                let definition = match default {
                    Some(d) if d.starts_with("Field(") => d.clone(),
                    Some(d) => format!("Field(default={d})"),
                    None => "Field(...)".to_string(),
                };
                fields_map.insert(
                    name.clone(),
                    serde_json::to_value(SqlFieldRow {
                        annotation: type_.clone(),
                        definition,
                        description: docstring.clone(),
                    })
                    .unwrap(),
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
                fields_map.insert(
                    name.clone(),
                    serde_json::to_value(SqlFieldRow {
                        annotation,
                        definition,
                        description: docstring.clone(),
                    })
                    .unwrap(),
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
                let target_module = class_to_module.get(target_class.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                raw_imports.extend(target_raw_imports(target_class, target_module, depth));
                fields_map.insert(
                    name.clone(),
                    serde_json::to_value(SqlFieldRow {
                        annotation,
                        definition,
                        description: docstring.clone(),
                    })
                    .unwrap(),
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
                let target_module = class_to_module.get(target_class.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                raw_imports.extend(target_raw_imports(target_class, target_module, depth));
                raw_imports.insert(RawImport {
                    from_: format!("{}many", ".".repeat(depth)),
                    name: link_class.clone(),
                    alias: None,
                });
                fields_map.insert(
                    name.clone(),
                    serde_json::to_value(SqlFieldRow {
                        annotation,
                        definition,
                        description: docstring.clone(),
                    })
                    .unwrap(),
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
                fields_map.insert(
                    name.clone(),
                    serde_json::to_value(SqlFieldRow {
                        annotation,
                        definition,
                        description: docstring.clone(),
                    })
                    .unwrap(),
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
                fields_map.insert(
                    name.clone(),
                    serde_json::to_value(SqlFieldRow {
                        annotation,
                        definition,
                        description: docstring.clone(),
                    })
                    .unwrap(),
                );
            }
            SqlField::AnyColumn {
                name,
                is_array,
                nullable,
                docstring,
            } => {
                let annotation = if *is_array {
                    if *nullable {
                        "list[Any] | None".to_string()
                    } else {
                        "list[Any]".to_string()
                    }
                } else if *nullable {
                    "Any | None".to_string()
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
                fields_map.insert(
                    name.clone(),
                    serde_json::to_value(SqlFieldRow {
                        annotation,
                        definition,
                        description: docstring.clone(),
                    })
                    .unwrap(),
                );
            }
        }
    }

    let imports_vec = group_imports(raw_imports);

    // Convert the insertion-ordered JsonMap to a MiniJinja Value so `.items()` works
    // in the template's `for field_name, field in SQL.fields.items()` loop.
    let fields_jinja: minijinja::Value =
        minijinja::Value::from_serialize(&fields_map);

    let tpl = env.get_template("python/sql_model/BaseModel.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => table.class_name.clone(),
        base_class => "SQLModel",
        description => table.description.clone().unwrap_or_else(|| table.class_name.clone()),
        fields => Vec::<String>::new(),
        methods => Vec::<String>::new(),
        config => None::<String>,
        SQL => context! {
            imports => imports_vec,
            fields => fields_jinja,
        },
    })?;
    Ok(rendered)
}

fn render_enum(env: &Environment<'_>, table: &TablePlan) -> Result<String, Error> {
    let members: Vec<minijinja::Value> = table
        .enum_members
        .iter()
        .map(|v| {
            minijinja::Value::from_serialize(&context! {
                name => v.clone(),
                default => format!("\"{v}\""),
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
    Ok(format!(
        "from enum import StrEnum\n\n{}",
        rendered.trim_start_matches('\n')
    ))
}

fn target_raw_imports(target_class: &str, target_module: &[String], depth: usize) -> impl IntoIterator<Item = RawImport> {
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
pub(crate) fn render_many(env: &Environment<'_>, junctions: &[JunctionTable]) -> Result<String, Error> {
    if junctions.is_empty() {
        return Ok(String::new());
    }
    let links: Vec<minijinja::Value> = junctions.iter().map(|j| {
        context! {
            table_name => j.class_name.clone(),
            cls1 => j.owner_class.clone(),
            cls2 => j.target_class.clone(),
            rel_field_name1 => j.source_field.clone(),
            id_field_name1 => j.owner_id_field.clone(),
            id_field_name2 => j.target_id_field.clone(),
        }
    }).map(minijinja::Value::from_serialize).collect();

    let tpl = env.get_template("python/sql_model/ManyLinks.jinja2")?;
    let rendered = tpl.render(context! { links => links })?;
    Ok(rendered)
}

/// Render `<output>/__init__.py` re-exporting every class and every junction.
pub(crate) fn render_init(env: &Environment<'_>, plan: &SqlPlan) -> Result<String, Error> {
    let classes: Vec<minijinja::Value> = plan.tables.values().map(|t| {
        let module_path: Vec<String> = t.module.iter()
            .take(t.module.len().saturating_sub(1))
            .map(|s| s.to_ascii_lowercase())
            .chain(std::iter::once(crate::naming::module_file_name(&t.module)))
            .collect();
        context! {
            name => t.class_name.clone(),
            module_path => module_path,
        }
    }).map(minijinja::Value::from_serialize).collect();

    let links: Vec<String> = plan.junctions.iter().map(|j| j.class_name.clone()).collect();

    let tpl = env.get_template("python/sql_model/__init__.jinja2")?;
    let rendered = tpl.render(context! {
        classes => classes,
        links => links,
    })?;
    Ok(rendered)
}

pub(crate) fn render_version(version: &str) -> String {
    format!("__version__: str = \"{version}\"\n")
}

/// Orchestrate the entire SQL model code generation: walk the plan, render each table,
/// write per-class files at the right paths, and write root-level __init__, __version__,
/// and per-subpackage __init__ files.
pub(crate) fn generate_sql_model(
    schemas: &Schemas,
    output_dir: &Path,
    env: &Environment<'_>,
) -> Result<Vec<PathBuf>, Error> {
    std::fs::create_dir_all(output_dir)?;
    let mut written: Vec<PathBuf> = Vec::new();
    let plan = plan::build_plan(schemas);

    // Build a class_name → parent-directory-segments (lowercased) lookup.
    let class_to_module: BTreeMap<String, Vec<String>> = plan.tables.values()
        .map(|t| {
            let parents: Vec<String> = t.module.iter()
                .take(t.module.len().saturating_sub(1))
                .map(|s| s.to_ascii_lowercase())
                .collect();
            (t.class_name.clone(), parents)
        })
        .collect();

    // ── Per-class files ────────────────────────────────────────────────────────
    for table in plan.tables.values() {
        let path_segments: Vec<String> = table.module.iter()
            .take(table.module.len().saturating_sub(1))
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let mut out_dir = output_dir.to_path_buf();
        for seg in &path_segments {
            out_dir.push(seg);
        }
        std::fs::create_dir_all(&out_dir)?;
        let file_name = format!("{}.py", module_file_name(&table.module));
        let depth = path_segments.len() + 1;
        let body = render_table(env, table, depth, &class_to_module)?;
        let out_path = out_dir.join(&file_name);
        std::fs::write(&out_path, body)?;
        written.push(out_path);
    }

    // ── many.py at the root (only if there are junctions) ──────────────────────
    if !plan.junctions.is_empty() {
        let many = render_many(env, &plan.junctions)?;
        let many_path = output_dir.join("many.py");
        std::fs::write(&many_path, many)?;
        written.push(many_path);
    }

    // ── __init__.py + __version__.py at the root ───────────────────────────────
    let init_body = render_init(env, &plan)?;
    let init_path = output_dir.join("__init__.py");
    std::fs::write(&init_path, init_body)?;
    written.push(init_path);

    let version_path = output_dir.join("__version__.py");
    std::fs::write(&version_path, render_version(&schemas.version.to_string()))?;
    written.push(version_path);

    // ── Empty __init__.py per first-level subdirectory ─────────────────────────
    let mut subdirs: BTreeSet<String> = BTreeSet::new();
    for table in plan.tables.values() {
        if table.module.len() > 1 {
            subdirs.insert(table.module[0].to_ascii_lowercase());
        }
    }
    for sub in subdirs {
        let p = output_dir.join(&sub).join("__init__.py");
        if !p.exists() {
            std::fs::write(&p, "")?;
            written.push(p);
        }
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::make_environment;

    fn fixture_plan() -> plan::SqlPlan {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bo4e_sql_min");
        let schemas = bo4e_schemas::io::schemas::read_schemas(&path)
            .expect("read bo4e_sql_min")
            .schemas;
        plan::build_plan(&schemas)
    }

    fn fixture_class_to_module(plan: &plan::SqlPlan) -> BTreeMap<String, Vec<String>> {
        plan.tables.values()
            .map(|t| {
                let parents: Vec<String> = t.module.iter()
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
            body.contains(
                "id: uuid_pkg.UUID = Field(default_factory=uuid_pkg.uuid4, primary_key=True"
            ),
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
                "adressen: list[Adresse] = Relationship(link_model=AngebotAdressenLink)"
            ),
            "got:\n{body}"
        );
        assert!(body.contains("_typ: Typ | None = Field"), "got:\n{body}");
        assert!(
            body.contains("werte: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))"),
            "got:\n{body}"
        );
        assert!(
            body.contains(
                "extras: Any | None = Field(sa_column=Column(PickleType, nullable=True))"
            ),
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
        assert!(
            body.contains("from ..enum.typ import Typ"),
            "got:\n{body}"
        );
    }

    #[test]
    fn render_table_bo_to_bo_relationship_uses_bo_module() {
        use plan::{SqlField, TablePlan};
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
                    name: "id".to_string(),
                    type_: "uuid_pkg.UUID".to_string(),
                    nullable: false,
                    default: Some(
                        "Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias=\"_id\", title=\"Id\")".to_string(),
                    ),
                    title: None,
                    docstring: Some("Primary key.".to_string()),
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
        assert!(body.contains("class AngebotAdressenLink(SQLModel, table=True):"), "got:\n{body}");
        assert!(body.contains("angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"angebot.id\""));
        assert!(body.contains("adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"adresse.id\""));
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
    }

    #[test]
    fn render_version_emits_constant() {
        let body = render_version("202401.4.0");
        assert_eq!(body.trim(), "__version__: str = \"202401.4.0\"");
    }
}
