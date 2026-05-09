//! python-sql-model generator orchestration.
//!
//! Two-phase: a pure pre-pass walks `Schemas` and produces an immutable
//! [`plan::SqlPlan`]; a render pass consumes the plan and writes Python files
//! via vendored MiniJinja templates.

#![allow(dead_code)] // render_table, render_enum, helpers — wired up in Task 11.

pub(crate) mod plan;

use crate::error::Error;
use minijinja::{Environment, context};
use plan::{SqlField, TablePlan};
use serde::Serialize;
use serde_json::Map as JsonMap;
use std::collections::{BTreeMap, BTreeSet};

#[allow(unused_imports)]
pub(crate) use plan::SqlPlan;

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
pub(crate) fn render_table(
    env: &Environment<'_>,
    table: &TablePlan,
    depth: usize,
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
                raw_imports.extend(target_raw_imports(target_class, depth));
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
                raw_imports.extend(target_raw_imports(target_class, depth));
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

fn target_raw_imports(target_class: &str, depth: usize) -> impl IntoIterator<Item = RawImport> {
    let target_table = target_class.to_ascii_lowercase();
    [RawImport {
        from_: format!("{}com.{}", ".".repeat(depth), target_table),
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

    #[test]
    fn render_table_object_emits_sqlmodel_class() {
        let env = make_environment(None).unwrap();
        let plan = fixture_plan();
        let angebot = plan
            .tables
            .get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table");
        let body = render_table(&env, angebot, 2).expect("render");
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
}
