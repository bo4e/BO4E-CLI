//! SQL-model build plan: an immutable description of all tables, fields, and junctions
//! produced by walking a [`bo4e_schemas::Schemas`].
//!
//! `build_plan` is pure — it has no side effects and writes no files. The renderer in
//! [`super`] consumes the plan and produces source.

#![allow(dead_code)] // Filled in across Tasks 6, 7. Wired up in Task 11.

use std::collections::BTreeMap;

use crate::naming::to_snake_case;
use crate::python::types::map_pydantic;
use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::{ObjectSchema, PrimitiveValue, SchemaRootType, SchemaType};

/// All tables and junctions produced by the pre-pass.
#[derive(Debug)]
pub(crate) struct SqlPlan {
    /// All BO/COM/enum tables, keyed by their module path
    /// (e.g. `["bo", "Angebot"]` matching `bo4e_schemas::models::schema_meta::Schema::module`).
    pub(crate) tables: BTreeMap<Vec<String>, TablePlan>,
    /// All M:N junction tables that need to land in `<output>/many.py`.
    pub(crate) junctions: Vec<JunctionTable>,
}

#[derive(Debug)]
pub(crate) struct TablePlan {
    /// Same module-path key as in `SqlPlan.tables` (e.g. `["bo", "Angebot"]`).
    pub(crate) module: Vec<String>,
    pub(crate) class_name: String,
    pub(crate) is_enum: bool,
    /// Schema-level `description`, used for the class docstring.
    pub(crate) description: Option<String>,
    /// For enum tables, the StrEnum members. Empty for object tables.
    pub(crate) enum_members: Vec<String>,
    /// For object tables, the fields in JSON-property insertion order. Empty for enum tables.
    pub(crate) sql_fields: Vec<SqlField>,
}

/// One field on a `TablePlan`. The pre-pass classifies every JSON property into
/// exactly one of these variants.
#[derive(Debug)]
pub(crate) enum SqlField {
    /// Plain scalar; renders as `name: type_ = Field(default=...)`.
    Scalar {
        name: String,
        /// Type expression as it appears inline (already includes `| None` for nullable).
        type_: String,
        nullable: bool,
        /// Already-quoted Python default expression, or `None` for required.
        default: Option<String>,
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>_id: UUID = Field(default=None, foreign_key="adresse.id")`.
    /// Sibling of a `Relationship` entry that follows immediately in `sql_fields`.
    ForeignKey {
        /// The FK column name, already `_id`-suffixed (e.g. `"adresse_id"`).
        name: String,
        target_class: String,
        target_table: String,
        nullable: bool,
        /// `Some("SET NULL")` when nullable, `None` when required.
        ondelete: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: Adresse | None = Relationship(sa_relationship_kwargs={...})`.
    /// Sibling of a `ForeignKey` entry that precedes it in `sql_fields`.
    Relationship {
        name: String,
        target_class: String,
        owner_class: String,
        /// The matching FK field name on the owner (`"adresse_id"`).
        fk_field_name: String,
        nullable: bool,
        docstring: Option<String>,
    },
    /// `<name>: list[Adresse] = Relationship(link_model=AngebotAdressenLink)`.
    /// The junction class is appended to `SqlPlan.junctions`.
    ManyRelationship {
        name: String,
        target_class: String,
        link_class: String,
        nullable: bool,
        docstring: Option<String>,
    },
    /// `<name>: Typ | None = Field(default=Typ.ANGEBOT, sa_column=Column(Enum(Typ, name="typ")))`.
    EnumColumn {
        name: String,
        enum_class: String,
        is_list: bool,
        nullable: bool,
        /// e.g. `Some("Typ.ANGEBOT")` or `None`.
        default: Option<String>,
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))`.
    ScalarArray {
        name: String,
        py_inner: String,
        sa_inner: &'static str,
        nullable: bool,
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: Any | None = Field(sa_column=Column(PickleType, nullable=True))`.
    /// Or with `ARRAY(PickleType)` when `is_array`.
    AnyColumn {
        name: String,
        is_array: bool,
        nullable: bool,
        docstring: Option<String>,
    },
}

#[derive(Debug)]
pub(crate) struct JunctionTable {
    /// Class + lower-cased table name (e.g. `"AngebotAdressenLink"`).
    pub(crate) class_name: String,
    pub(crate) owner_class: String,
    pub(crate) owner_table: String,
    pub(crate) owner_id_field: String,
    pub(crate) target_class: String,
    pub(crate) target_table: String,
    pub(crate) target_id_field: String,
    /// The source field on the owner (diagnostic only — appears in the junction class docstring).
    pub(crate) source_field: String,
}

/// Build the immutable plan from a parsed `Schemas`. Pure — no I/O, no template rendering.
///
/// Filled in across Tasks 6 and 7.
pub(crate) fn build_plan(schemas: &Schemas) -> SqlPlan {
    let mut tables: BTreeMap<Vec<String>, TablePlan> = BTreeMap::new();
    let junctions: Vec<JunctionTable> = Vec::new();

    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();
        let parsed = schema.schema().expect("schema parsed").clone();
        drop(schema);

        let table = match &parsed {
            SchemaRootType::StrEnum(e) => TablePlan {
                module: module.clone(),
                class_name: class_name.clone(),
                is_enum: true,
                description: e.str_enum.base.description.clone(),
                enum_members: e.str_enum.enum_values.clone(),
                sql_fields: Vec::new(),
            },
            SchemaRootType::Object(o) => {
                let id_field = synth_id_field(&o.object);
                let mut fields = vec![id_field];
                for (prop_name, prop_schema) in o.object.properties.iter() {
                    if prop_name == "_id" {
                        continue;
                    }
                    if let Some(field) = simple_scalar_field(prop_name, prop_schema) {
                        fields.push(field);
                    }
                }
                TablePlan {
                    module: module.clone(),
                    class_name: class_name.clone(),
                    is_enum: false,
                    description: o.object.base.description.clone(),
                    enum_members: Vec::new(),
                    sql_fields: fields,
                }
            }
        };
        tables.insert(module, table);
    }

    SqlPlan { tables, junctions }
}

fn synth_id_field(obj: &ObjectSchema) -> SqlField {
    let title = obj
        .properties
        .get("_id")
        .and_then(|s| literal_title(s))
        .unwrap_or_else(|| "Primary key ID-Field".to_string());
    let default = format!(
        "Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias=\"_id\", title=\"{title}\")"
    );
    SqlField::Scalar {
        name: "id".to_string(),
        type_: "uuid_pkg.UUID".to_string(),
        nullable: false,
        default: Some(default),
        title: None,
        docstring: Some("The primary key of the table as a UUID4.".to_string()),
    }
}

fn simple_scalar_field(prop_name: &str, schema: &SchemaType) -> Option<SqlField> {
    if !is_simple_scalar(schema) {
        return None;
    }
    let mapped = map_pydantic(schema);
    let nullable = mapped.rendered.contains("| None");
    let type_ = mapped.rendered.clone();
    let default = if nullable {
        Some(literal_default(schema).unwrap_or_else(|| "None".to_string()))
    } else {
        literal_default(schema)
    };
    Some(SqlField::Scalar {
        name: to_snake_case(prop_name),
        type_,
        nullable,
        default,
        title: literal_title(schema),
        docstring: literal_description(schema),
    })
}

fn is_simple_scalar(schema: &SchemaType) -> bool {
    match schema {
        SchemaType::StringSchema(_)
        | SchemaType::IntegerSchema(_)
        | SchemaType::NumberSchema(_)
        | SchemaType::BooleanSchema(_)
        | SchemaType::DecimalSchema(_)
        | SchemaType::ConstantSchema(_) => true,
        SchemaType::AnyOf(a) => {
            a.any_of.iter().all(|t| matches!(t,
                SchemaType::StringSchema(_)
                | SchemaType::IntegerSchema(_)
                | SchemaType::NumberSchema(_)
                | SchemaType::BooleanSchema(_)
                | SchemaType::DecimalSchema(_)
                | SchemaType::ConstantSchema(_)
                | SchemaType::NullSchema(_)
            ))
        }
        _ => false,
    }
}

fn literal_default(schema: &SchemaType) -> Option<String> {
    let base = schema_base(schema);
    base.default.as_ref().map(|v| match v {
        PrimitiveValue::Null => "None".into(),
        PrimitiveValue::Bool(true) => "True".into(),
        PrimitiveValue::Bool(false) => "False".into(),
        PrimitiveValue::Integer(i) => i.to_string(),
        PrimitiveValue::Float(f) => f.to_string(),
        PrimitiveValue::String(s) => format!("\"{s}\""),
    })
}

fn literal_title(schema: &SchemaType) -> Option<String> {
    schema_base(schema).title.clone()
}

fn literal_description(schema: &SchemaType) -> Option<String> {
    schema_base(schema).description.clone()
}

fn schema_base(schema: &SchemaType) -> &bo4e_schemas::models::json_schema::TypeBase {
    match schema {
        SchemaType::StringSchema(s) => &s.base,
        SchemaType::IntegerSchema(s) => &s.base,
        SchemaType::NumberSchema(s) => &s.base,
        SchemaType::BooleanSchema(s) => &s.base,
        SchemaType::DecimalSchema(s) => &s.base,
        SchemaType::NullSchema(s) => &s.base,
        SchemaType::AnySchema(s) => &s.base,
        SchemaType::Array(s) => &s.base,
        SchemaType::AnyOf(s) => &s.base,
        SchemaType::AllOf(s) => &s.base,
        SchemaType::ConstantSchema(s) => &s.base,
        SchemaType::ReferenceSchema(s) => &s.base,
        SchemaType::Object(s) => &s.base,
        SchemaType::StrEnum(s) => &s.base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::{Schema, Schemas};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn fixture_schemas() -> Schemas {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bo4e_sql_min");
        bo4e_schemas::io::schemas::read_schemas(&path)
            .expect("read bo4e_sql_min")
            .schemas
    }

    fn enum_schema(name: &str, members: &[&str]) -> Schemas {
        let mut s = Schemas::new("v202401.0.0".parse().unwrap());
        let body = format!(
            r#"{{"title":"{name}","type":"string","enum":[{}]}}"#,
            members.iter().map(|m| format!("\"{m}\"")).collect::<Vec<_>>().join(",")
        );
        let mut sch = Schema::new(vec!["enum".into(), name.into()], None).unwrap();
        sch.load_schema(body);
        s.add_schema(Rc::new(RefCell::new(sch))).unwrap();
        s
    }

    #[test]
    fn enum_schema_produces_enum_table_plan() {
        let schemas = enum_schema("Typ", &["ANGEBOT", "VERTRAG"]);
        let plan = build_plan(&schemas);
        let key = vec!["enum".to_string(), "Typ".to_string()];
        let table = plan.tables.get(&key).expect("enum table present");
        assert!(table.is_enum);
        assert_eq!(table.class_name, "Typ");
        assert_eq!(table.enum_members, vec!["ANGEBOT".to_string(), "VERTRAG".to_string()]);
        assert!(table.sql_fields.is_empty());
    }

    #[test]
    fn object_table_synthesises_primary_key_id() {
        let plan = build_plan(&fixture_schemas());
        let angebot = plan.tables.get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table present");
        match &angebot.sql_fields[0] {
            SqlField::Scalar { name, type_, default, .. } => {
                assert_eq!(name, "id");
                assert_eq!(type_, "uuid_pkg.UUID");
                assert_eq!(default.as_deref(), Some("Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias=\"_id\", title=\" Id\")"));
            }
            other => panic!("expected Scalar id field, got {:?}", other),
        }
    }

    #[test]
    fn nullable_scalar_field_emits_none_default() {
        let plan = build_plan(&fixture_schemas());
        let angebot = plan.tables.get(&vec!["bo".to_string(), "Angebot".to_string()]).unwrap();
        let nummer = angebot.sql_fields.iter().find_map(|f| match f {
            SqlField::Scalar { name, type_, nullable, default, .. } if name == "angebotsnummer" => {
                Some((type_.clone(), *nullable, default.clone()))
            }
            _ => None,
        }).expect("angebotsnummer field present");
        assert_eq!(nummer.0, "str | None");
        assert!(nummer.1);
        assert_eq!(nummer.2.as_deref(), Some("None"));
    }
}
