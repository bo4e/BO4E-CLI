//! SQL-model build plan: an immutable description of all tables, fields, and junctions
//! produced by walking a [`bo4e_schemas::Schemas`].
//!
//! `build_plan` is pure — it has no side effects and writes no files. The renderer in
//! [`super`] consumes the plan and produces source.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::Error;
use crate::naming::to_snake_case;
use crate::python::types::{literal_default, map_pydantic, schema_base};
use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::{ObjectSchema, SchemaRootType, SchemaType};

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
        #[allow(dead_code)]
        nullable: bool,
        /// Already-quoted Python default expression, or `None` for required.
        default: Option<String>,
        #[allow(dead_code)]
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>_id: UUID = Field(default=None, foreign_key="adresse.id")`.
    /// Sibling of a `Relationship` entry that follows immediately in `sql_fields`.
    ForeignKey {
        /// The FK column name, already `_id`-suffixed (e.g. `"adresse_id"`).
        name: String,
        #[allow(dead_code)]
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
        #[allow(dead_code)]
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))`.
    ScalarArray {
        name: String,
        py_inner: String,
        sa_inner: &'static str,
        nullable: bool,
        #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub(crate) owner_table: String,
    pub(crate) owner_id_field: String,
    pub(crate) target_class: String,
    #[allow(dead_code)]
    pub(crate) target_table: String,
    pub(crate) target_id_field: String,
    /// The source field on the owner (diagnostic only — appears in the junction class docstring).
    pub(crate) source_field: String,
}

/// Build the immutable plan from a parsed `Schemas`. Pure — no I/O, no template rendering.
///
/// Returns an error if any object property cannot be mapped to one of the
/// supported [`SqlField`] variants — silent drops would otherwise leave gaps in
/// the generated tables.
pub(crate) fn build_plan(schemas: &Schemas) -> Result<SqlPlan, Error> {
    // Precompute enum class names to avoid O(n²) classification
    let enum_names: BTreeSet<String> = schemas
        .iter()
        .filter_map(|schema_rc| {
            let mut s = schema_rc.borrow_mut();
            let name = s.name().to_string();
            match s.schema() {
                Ok(SchemaRootType::StrEnum(_)) => Some(name),
                _ => None,
            }
        })
        .collect();

    let mut tables: BTreeMap<Vec<String>, TablePlan> = BTreeMap::new();
    let mut junction_buf: Vec<JunctionTable> = Vec::new();

    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();
        let parsed = schema.schema().expect("schema parsed").clone();
        drop(schema);

        match &parsed {
            SchemaRootType::StrEnum(e) => {
                tables.insert(
                    module.clone(),
                    TablePlan {
                        module: module.clone(),
                        class_name: class_name.clone(),
                        is_enum: true,
                        description: e.str_enum.base.description.clone(),
                        enum_members: e.str_enum.enum_values.clone(),
                        sql_fields: Vec::new(),
                    },
                );
            }
            SchemaRootType::Object(o) => {
                // Same gate as `for_each_schema_file` — sql_model doesn't go
                // through that helper (it builds an SqlPlan up-front), so
                // enforce the strict required/default invariant here too.
                crate::validate::object_invariants(&class_name, &o.object)?;
                let id_field = synth_id_field(&o.object);
                let mut fields = vec![id_field];
                let mut local_junctions: Vec<JunctionTable> = Vec::new();
                for (prop_name, prop_schema) in o.object.properties.iter() {
                    if prop_name == "_id" {
                        continue;
                    }
                    if is_simple_scalar(prop_schema) {
                        if let Some(field) = simple_scalar_field(prop_name, prop_schema) {
                            fields.push(field);
                        }
                        continue;
                    }
                    classify_property(
                        &class_name,
                        prop_name,
                        prop_schema,
                        &enum_names,
                        &mut fields,
                        &mut local_junctions,
                    )?;
                }
                tables.insert(
                    module.clone(),
                    TablePlan {
                        module: module.clone(),
                        class_name: class_name.clone(),
                        is_enum: false,
                        description: o.object.base.description.clone(),
                        enum_members: Vec::new(),
                        sql_fields: fields,
                    },
                );
                junction_buf.extend(local_junctions);
            }
        }
    }

    Ok(SqlPlan {
        tables,
        junctions: junction_buf,
    })
}

fn synth_id_field(obj: &ObjectSchema) -> SqlField {
    let title = obj
        .properties
        .get("_id")
        .and_then(literal_title)
        .unwrap_or_else(|| "Primary key ID-Field".to_string());
    let escaped_title = title.replace('\\', "\\\\").replace('"', "\\\"");
    // The renderer rewrites `_id` → `id_` and injects `alias="_id"` automatically
    // via the shared `python_attr_name` / `inject_alias` path; keep the leading
    // underscore here so that single code path stays the source of truth.
    let default = format!(
        "Field(default_factory=uuid_pkg.uuid4, primary_key=True, title=\"{escaped_title}\")"
    );
    SqlField::Scalar {
        name: "_id".to_string(),
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
        | SchemaType::ConstantSchema(_)
        // BO4E discriminator fields like `_typ` are emitted as
        // `{"const":"X","type":"string","enum":["X"]}`. Serde's untagged dispatch
        // matches StrEnumSchema first (both `type` and `enum` are present),
        // so we treat single-value StrEnums as plain string scalars here —
        // mirroring the pydantic generator, which renders them as `str`.
        | SchemaType::StrEnum(_) => true,
        // AnyOf with a non-scalar variant (reference, array, Any, …) is not a simple
        // scalar — it falls through to `None` in `simple_scalar_field`, where the
        // caller handles the structured cases (relationships, junctions) separately.
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

fn literal_title(schema: &SchemaType) -> Option<String> {
    schema_base(schema).title.clone()
}

fn literal_description(schema: &SchemaType) -> Option<String> {
    schema_base(schema).description.clone()
}

/// Classify a non-simple-scalar JSON-Schema property and push the resulting
/// `SqlField`(s) (and any `JunctionTable`) onto the buffers.
///
/// Returns [`Error::UnclassifiableProperty`] when the schema does not match any
/// supported shape — silent drops are not allowed because they leave the
/// generated table missing fields the user expects to see.
fn classify_property(
    owner_class: &str,
    prop_name: &str,
    schema: &SchemaType,
    enum_names: &BTreeSet<String>,
    fields: &mut Vec<SqlField>,
    junctions: &mut Vec<JunctionTable>,
) -> Result<(), Error> {
    let snake = to_snake_case(prop_name);
    let docstring = literal_description(schema);

    if let SchemaType::ReferenceSchema(r) = schema
        && let Some(target) = ref_target_class(&r.r#ref)
    {
        if enum_names.contains(&target) {
            fields.push(SqlField::EnumColumn {
                name: snake,
                enum_class: target,
                is_list: false,
                nullable: false,
                default: None,
                title: literal_title(schema),
                docstring,
            });
        } else {
            push_one_to_one(owner_class, &snake, &target, false, fields);
        }
        return Ok(());
    }

    if let SchemaType::Array(a) = schema {
        match &*a.items {
            SchemaType::ReferenceSchema(r) if ref_target_class(&r.r#ref).is_some() => {
                let target = ref_target_class(&r.r#ref).unwrap();
                if enum_names.contains(&target) {
                    fields.push(SqlField::EnumColumn {
                        name: snake,
                        enum_class: target,
                        is_list: true,
                        nullable: false,
                        default: None,
                        title: literal_title(schema),
                        docstring,
                    });
                } else {
                    push_many_to_many(
                        owner_class,
                        &snake,
                        &target,
                        false,
                        prop_name,
                        fields,
                        junctions,
                    );
                }
                return Ok(());
            }
            SchemaType::AnySchema(_) => {
                fields.push(SqlField::AnyColumn {
                    name: snake,
                    is_array: true,
                    nullable: false,
                    docstring,
                });
                return Ok(());
            }
            inner
                if matches!(
                    inner,
                    SchemaType::StringSchema(_)
                        | SchemaType::IntegerSchema(_)
                        | SchemaType::NumberSchema(_)
                        | SchemaType::BooleanSchema(_)
                        | SchemaType::DecimalSchema(_)
                ) =>
            {
                let (py_inner, sa_inner) = scalar_array_inners(inner);
                fields.push(SqlField::ScalarArray {
                    name: snake,
                    py_inner,
                    sa_inner,
                    nullable: false,
                    title: literal_title(schema),
                    docstring,
                });
                return Ok(());
            }
            _ => {}
        }
    }

    if let SchemaType::AnySchema(_) = schema {
        fields.push(SqlField::AnyColumn {
            name: snake,
            is_array: false,
            nullable: true,
            docstring,
        });
        return Ok(());
    }

    if let SchemaType::AnyOf(a) = schema {
        let nulls = a
            .any_of
            .iter()
            .filter(|t| matches!(t, SchemaType::NullSchema(_)))
            .count();
        if nulls == 1 && a.any_of.len() == 2 {
            let inner = a
                .any_of
                .iter()
                .find(|t| !matches!(t, SchemaType::NullSchema(_)))
                .unwrap();
            return classify_optional(
                owner_class,
                prop_name,
                &snake,
                inner,
                schema,
                enum_names,
                fields,
                junctions,
            );
        }
    }

    Err(Error::UnclassifiableProperty {
        class: owner_class.to_string(),
        property: prop_name.to_string(),
    })
}

#[allow(clippy::too_many_arguments)]
fn classify_optional(
    owner_class: &str,
    prop_name: &str,
    snake: &str,
    inner: &SchemaType,
    full_schema: &SchemaType,
    enum_names: &BTreeSet<String>,
    fields: &mut Vec<SqlField>,
    junctions: &mut Vec<JunctionTable>,
) -> Result<(), Error> {
    let docstring = literal_description(full_schema);
    let title = literal_title(full_schema);

    match inner {
        SchemaType::ReferenceSchema(r) => {
            let target =
                ref_target_class(&r.r#ref).ok_or_else(|| Error::UnclassifiableProperty {
                    class: owner_class.to_string(),
                    property: prop_name.to_string(),
                })?;
            if enum_names.contains(&target) {
                let default = literal_default(full_schema).and_then(|d| {
                    if d == "None" {
                        None
                    } else {
                        let trimmed = d.trim_matches('"').to_string();
                        Some(format!("{target}.{trimmed}"))
                    }
                });
                fields.push(SqlField::EnumColumn {
                    name: snake.to_string(),
                    enum_class: target,
                    is_list: false,
                    nullable: true,
                    default,
                    title,
                    docstring,
                });
            } else {
                push_one_to_one(owner_class, snake, &target, true, fields);
            }
            Ok(())
        }
        SchemaType::Array(a) => match &*a.items {
            SchemaType::ReferenceSchema(r) if ref_target_class(&r.r#ref).is_some() => {
                let target = ref_target_class(&r.r#ref).unwrap();
                if enum_names.contains(&target) {
                    fields.push(SqlField::EnumColumn {
                        name: snake.to_string(),
                        enum_class: target,
                        is_list: true,
                        nullable: true,
                        default: None,
                        title,
                        docstring,
                    });
                } else {
                    push_many_to_many(
                        owner_class,
                        snake,
                        &target,
                        true,
                        prop_name,
                        fields,
                        junctions,
                    );
                }
                Ok(())
            }
            SchemaType::AnySchema(_) => {
                fields.push(SqlField::AnyColumn {
                    name: snake.to_string(),
                    is_array: true,
                    nullable: true,
                    docstring,
                });
                Ok(())
            }
            other
                if matches!(
                    other,
                    SchemaType::StringSchema(_)
                        | SchemaType::IntegerSchema(_)
                        | SchemaType::NumberSchema(_)
                        | SchemaType::BooleanSchema(_)
                        | SchemaType::DecimalSchema(_)
                ) =>
            {
                let (py_inner, sa_inner) = scalar_array_inners(other);
                fields.push(SqlField::ScalarArray {
                    name: snake.to_string(),
                    py_inner,
                    sa_inner,
                    nullable: true,
                    title,
                    docstring,
                });
                Ok(())
            }
            _ => Err(Error::UnclassifiableProperty {
                class: owner_class.to_string(),
                property: prop_name.to_string(),
            }),
        },
        SchemaType::AnySchema(_) => {
            fields.push(SqlField::AnyColumn {
                name: snake.to_string(),
                is_array: false,
                nullable: true,
                docstring,
            });
            Ok(())
        }
        _ => Err(Error::UnclassifiableProperty {
            class: owner_class.to_string(),
            property: prop_name.to_string(),
        }),
    }
}

fn push_one_to_one(
    owner_class: &str,
    snake: &str,
    target_class: &str,
    nullable: bool,
    fields: &mut Vec<SqlField>,
) {
    let fk_name = format!("{snake}_id");
    let target_table = target_class.to_ascii_lowercase();
    fields.push(SqlField::ForeignKey {
        name: fk_name.clone(),
        target_class: target_class.to_string(),
        target_table,
        nullable,
        ondelete: if nullable {
            Some("SET NULL".to_string())
        } else {
            None
        },
        docstring: Some(format!(
            "The id to implement the relationship (field {snake} references {target_class})."
        )),
    });
    fields.push(SqlField::Relationship {
        name: snake.to_string(),
        target_class: target_class.to_string(),
        owner_class: owner_class.to_string(),
        fk_field_name: fk_name,
        nullable,
        docstring: None,
    });
}

fn push_many_to_many(
    owner_class: &str,
    snake: &str,
    target_class: &str,
    nullable: bool,
    source_field: &str,
    fields: &mut Vec<SqlField>,
    junctions: &mut Vec<JunctionTable>,
) {
    let pascal_field = pascal_case(snake);
    let link_class = format!("{owner_class}{pascal_field}Link");
    fields.push(SqlField::ManyRelationship {
        name: snake.to_string(),
        target_class: target_class.to_string(),
        link_class: link_class.clone(),
        nullable,
        docstring: None,
    });
    let owner_table = owner_class.to_ascii_lowercase();
    let target_table = target_class.to_ascii_lowercase();
    junctions.push(JunctionTable {
        class_name: link_class,
        owner_class: owner_class.to_string(),
        owner_table: owner_table.clone(),
        owner_id_field: format!("{owner_table}_id"),
        target_class: target_class.to_string(),
        target_table: target_table.clone(),
        target_id_field: format!("{target_table}_id"),
        source_field: source_field.to_string(),
    });
}

fn scalar_array_inners(inner: &SchemaType) -> (String, &'static str) {
    match inner {
        SchemaType::StringSchema(_) => ("str".into(), "String"),
        SchemaType::IntegerSchema(_) => ("int".into(), "Integer"),
        SchemaType::NumberSchema(_) => ("float".into(), "Float"),
        SchemaType::BooleanSchema(_) => ("bool".into(), "Boolean"),
        SchemaType::DecimalSchema(_) => ("Decimal".into(), "Numeric"),
        _ => unreachable!("scalar_array_inners called with non-scalar inner"),
    }
}

fn ref_target_class(ref_str: &str) -> Option<String> {
    let path = ref_str.split('#').next().unwrap_or(ref_str);
    let last = path.rsplit('/').next()?;
    last.strip_suffix(".json").map(|s| s.to_string())
}

fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
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
            members
                .iter()
                .map(|m| format!("\"{m}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut sch = Schema::new(vec!["enum".into(), name.into()], None).unwrap();
        sch.load_schema(body);
        s.add_schema(Rc::new(RefCell::new(sch))).unwrap();
        s
    }

    #[test]
    fn enum_schema_produces_enum_table_plan() {
        let schemas = enum_schema("Typ", &["ANGEBOT", "VERTRAG"]);
        let plan = build_plan(&schemas).expect("build_plan");
        let key = vec!["enum".to_string(), "Typ".to_string()];
        let table = plan.tables.get(&key).expect("enum table present");
        assert!(table.is_enum);
        assert_eq!(table.class_name, "Typ");
        assert_eq!(
            table.enum_members,
            vec!["ANGEBOT".to_string(), "VERTRAG".to_string()]
        );
        assert!(table.sql_fields.is_empty());
    }

    #[test]
    fn object_table_synthesises_primary_key_id() {
        let plan = build_plan(&fixture_schemas()).expect("build_plan");
        let angebot = plan
            .tables
            .get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table present");
        match &angebot.sql_fields[0] {
            SqlField::Scalar {
                name,
                type_,
                default,
                ..
            } => {
                assert_eq!(name, "_id");
                assert_eq!(type_, "uuid_pkg.UUID");
                assert_eq!(
                    default.as_deref(),
                    Some("Field(default_factory=uuid_pkg.uuid4, primary_key=True, title=\" Id\")")
                );
            }
            other => panic!("expected Scalar id field, got {:?}", other),
        }
    }

    #[test]
    fn nullable_scalar_field_emits_none_default() {
        let plan = build_plan(&fixture_schemas()).expect("build_plan");
        let angebot = plan
            .tables
            .get(&vec!["bo".to_string(), "Angebot".to_string()])
            .unwrap();
        let nummer = angebot
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::Scalar {
                    name,
                    type_,
                    nullable,
                    default,
                    ..
                } if name == "angebotsnummer" => Some((type_.clone(), *nullable, default.clone())),
                _ => None,
            })
            .expect("angebotsnummer field present");
        assert_eq!(nummer.0, "str | None");
        assert!(nummer.1);
        assert_eq!(nummer.2.as_deref(), Some("None"));
    }

    fn angebot_table(plan: &SqlPlan) -> &TablePlan {
        plan.tables
            .get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table present")
    }

    #[test]
    fn one_to_one_reference_emits_fk_then_relationship() {
        let plan = build_plan(&fixture_schemas()).expect("build_plan");
        let angebot = angebot_table(&plan);

        let fk_idx = angebot
            .sql_fields
            .iter()
            .position(|f| {
                matches!(f,
                    SqlField::ForeignKey { name, .. } if name == "adresse_id"
                )
            })
            .expect("adresse_id FK present");
        let rel_idx = angebot
            .sql_fields
            .iter()
            .position(|f| {
                matches!(f,
                    SqlField::Relationship { name, .. } if name == "adresse"
                )
            })
            .expect("adresse Relationship present");

        assert_eq!(rel_idx, fk_idx + 1, "Relationship must follow FK directly");

        match &angebot.sql_fields[fk_idx] {
            SqlField::ForeignKey {
                target_class,
                target_table,
                nullable,
                ondelete,
                ..
            } => {
                assert_eq!(target_class, "Adresse");
                assert_eq!(target_table, "adresse");
                assert!(*nullable);
                assert_eq!(ondelete.as_deref(), Some("SET NULL"));
            }
            _ => unreachable!(),
        }
        match &angebot.sql_fields[rel_idx] {
            SqlField::Relationship {
                target_class,
                owner_class,
                fk_field_name,
                nullable,
                ..
            } => {
                assert_eq!(target_class, "Adresse");
                assert_eq!(owner_class, "Angebot");
                assert_eq!(fk_field_name, "adresse_id");
                assert!(*nullable);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn many_reference_emits_many_relationship_and_junction() {
        let plan = build_plan(&fixture_schemas()).expect("build_plan");
        let angebot = angebot_table(&plan);

        let many = angebot
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::ManyRelationship {
                    name,
                    target_class,
                    link_class,
                    nullable,
                    ..
                } if name == "adressen" => {
                    Some((target_class.clone(), link_class.clone(), *nullable))
                }
                _ => None,
            })
            .expect("adressen ManyRelationship present");
        assert_eq!(many.0, "Adresse");
        assert_eq!(many.1, "AngebotAdressenLink");
        // After the strict-schema fixture update, `adressen` is now
        // `anyOf:[array, null]` with `default: null`, so it lands as a
        // nullable ManyRelationship.
        assert!(
            many.2,
            "list[Reference] inside anyOf:[T, null] should be nullable"
        );

        let junction = plan
            .junctions
            .iter()
            .find(|j| j.class_name == "AngebotAdressenLink")
            .expect("AngebotAdressenLink junction present");
        assert_eq!(junction.owner_class, "Angebot");
        assert_eq!(junction.owner_table, "angebot");
        assert_eq!(junction.owner_id_field, "angebot_id");
        assert_eq!(junction.target_class, "Adresse");
        assert_eq!(junction.target_table, "adresse");
        assert_eq!(junction.target_id_field, "adresse_id");
        assert_eq!(junction.source_field, "adressen");
    }

    #[test]
    fn enum_reference_with_default_emits_enum_column() {
        let plan = build_plan(&fixture_schemas()).expect("build_plan");
        let angebot = angebot_table(&plan);
        let typ = angebot
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::EnumColumn {
                    name,
                    enum_class,
                    is_list,
                    nullable,
                    default,
                    ..
                } if name == "_typ" => {
                    Some((enum_class.clone(), *is_list, *nullable, default.clone()))
                }
                _ => None,
            })
            .expect("_typ EnumColumn present");
        assert_eq!(typ.0, "Typ");
        assert!(!typ.1);
        assert!(typ.2);
        assert_eq!(typ.3.as_deref(), Some("Typ.ANGEBOT"));
    }

    #[test]
    fn scalar_array_of_decimal_emits_scalar_array() {
        let plan = build_plan(&fixture_schemas()).expect("build_plan");
        let angebot = angebot_table(&plan);
        let werte = angebot
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::ScalarArray {
                    name,
                    py_inner,
                    sa_inner,
                    nullable,
                    ..
                } if name == "werte" => Some((py_inner.clone(), *sa_inner, *nullable)),
                _ => None,
            })
            .expect("werte ScalarArray present");
        assert_eq!(werte.0, "Decimal");
        assert_eq!(werte.1, "Numeric");
        assert!(!werte.2);
    }

    #[test]
    fn any_field_emits_any_column() {
        let plan = build_plan(&fixture_schemas()).expect("build_plan");
        let angebot = angebot_table(&plan);
        let extras = angebot
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::AnyColumn {
                    name,
                    is_array,
                    nullable,
                    ..
                } if name == "extras" => Some((*is_array, *nullable)),
                _ => None,
            })
            .expect("extras AnyColumn present");
        assert!(!extras.0);
        assert!(extras.1);

        let anhaenge = angebot
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::AnyColumn {
                    name,
                    is_array,
                    nullable,
                    ..
                } if name == "anhaenge" => Some((*is_array, *nullable)),
                _ => None,
            })
            .expect("anhaenge AnyColumn present");
        assert!(anhaenge.0);
        assert!(!anhaenge.1);
    }

    fn schemas_with_object(name: &str, body: &str) -> Schemas {
        let mut s = Schemas::new("v202501.0.0".parse().unwrap());
        let mut sch = Schema::new(vec!["bo".into(), name.into()], None).unwrap();
        sch.load_schema(body.to_string());
        s.add_schema(Rc::new(RefCell::new(sch))).unwrap();
        s
    }

    #[test]
    fn inline_strenum_const_typ_field_emits_string_scalar() {
        // Mirrors the real BO4E `_typ` shape: {const, type:string, enum:[X]}.
        // Serde untagged dispatches this to StrEnumSchema before ConstantSchema,
        // so the SQL plan must accept StrEnum as a simple scalar or the field
        // is silently dropped.
        let body = r#"{
            "type":"object",
            "properties":{
                "_typ":{"const":"ANGEBOT","default":"ANGEBOT","enum":["ANGEBOT"],"type":"string","title":" Typ"}
            }
        }"#;
        let plan = build_plan(&schemas_with_object("Angebot", body)).expect("build_plan");
        let table = plan
            .tables
            .get(&vec!["bo".to_string(), "Angebot".to_string()])
            .unwrap();
        let typ = table
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::Scalar {
                    name,
                    type_,
                    default,
                    ..
                } if name == "_typ" => Some((type_.clone(), default.clone())),
                _ => None,
            })
            .expect("_typ Scalar present");
        assert_eq!(typ.0, "str");
        assert_eq!(typ.1.as_deref(), Some("\"ANGEBOT\""));
    }

    #[test]
    fn untyped_property_emits_nullable_any_column() {
        // ZusatzAttribut.wert: `{default:null, title:"Wert"}` has no type.
        // Without a `$ref` the property dispatches to `AnySchema`, which we
        // emit as a nullable AnyColumn.
        let body = r#"{
            "type":"object",
            "properties":{
                "wert":{"default":null,"title":"Wert"}
            }
        }"#;
        let plan = build_plan(&schemas_with_object("ZusatzAttribut", body)).expect("build_plan");
        let table = plan
            .tables
            .get(&vec!["bo".to_string(), "ZusatzAttribut".to_string()])
            .unwrap();
        let wert = table
            .sql_fields
            .iter()
            .find_map(|f| match f {
                SqlField::AnyColumn {
                    name,
                    is_array,
                    nullable,
                    ..
                } if name == "wert" => Some((*is_array, *nullable)),
                _ => None,
            })
            .expect("wert AnyColumn present");
        assert!(!wert.0, "wert is not an array");
        assert!(wert.1, "wert is nullable (default null)");
    }

    #[test]
    fn unclassifiable_property_returns_error() {
        // `allOf` is outside the supported shape catalogue for SQL columns.
        // The plan must surface this as `Error::UnclassifiableProperty` so the
        // user sees what went wrong instead of getting a silently incomplete table.
        //
        // `weird` is in `required` so the strict-schema invariant validator
        // (required ⇔ no default) lets it through — failure must come from
        // the plan builder's shape classification, not the validator.
        let body = r#"{
            "type":"object",
            "required":["weird"],
            "properties":{
                "weird":{"allOf":[{"type":"string"},{"type":"integer"}]}
            }
        }"#;
        let err = build_plan(&schemas_with_object("Weird", body)).unwrap_err();
        match err {
            Error::UnclassifiableProperty { class, property } => {
                assert_eq!(class, "Weird");
                assert_eq!(property, "weird");
            }
            other => panic!("expected UnclassifiableProperty, got {other:?}"),
        }
    }
}
