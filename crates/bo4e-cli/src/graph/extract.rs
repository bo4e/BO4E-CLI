use bo4e_schemas::models::json_schema::{AnyOfSchema, SchemaType, StringSchemaFormat};
use petgraph::Graph;

use crate::models::graph::Cardinality;

pub type PetGraph = Graph<Vec<String>, EdgeData>;

#[derive(Debug, Clone)]
pub struct EdgeData {
    pub through_field: String,
    pub cardinality: Cardinality,
}

/// Pretty-print a SchemaType for display in diagrams. Display-only, not generator-grade.
pub fn type_repr(s: &SchemaType) -> String {
    match s {
        SchemaType::ReferenceSchema(r) => ref_class_name(&r.r#ref),
        SchemaType::Object(_) => "object".into(),
        SchemaType::StringSchema(ss) => match ss.format {
            Some(StringSchemaFormat::DateTime) => "datetime".into(),
            Some(StringSchemaFormat::Date) => "date".into(),
            Some(StringSchemaFormat::Time) => "time".into(),
            Some(StringSchemaFormat::Uuid) => "UUID".into(),
            _ => "str".into(),
        },
        SchemaType::ConstantSchema(_) => "str".into(),
        SchemaType::NumberSchema(_) => "float".into(),
        SchemaType::DecimalSchema(_) => "Decimal".into(),
        SchemaType::IntegerSchema(_) => "int".into(),
        SchemaType::BooleanSchema(_) => "bool".into(),
        SchemaType::NullSchema(_) => "None".into(),
        SchemaType::AnySchema(_) => "Any".into(),
        SchemaType::Array(a) => format!("list[{}]", type_repr(&a.items)),
        SchemaType::AnyOf(any_of) => any_of_repr(any_of),
        SchemaType::AllOf(all_of) => {
            if let Some(only) = all_of.all_of.first() {
                type_repr(only)
            } else {
                "Any".into()
            }
        }
        SchemaType::StrEnum(_) => "str".into(),
    }
}

fn any_of_repr(a: &AnyOfSchema) -> String {
    let mut non_null: Vec<&SchemaType> = Vec::new();
    let mut has_null = false;
    for branch in &a.any_of {
        if matches!(branch, SchemaType::NullSchema(_)) {
            has_null = true;
        } else {
            non_null.push(branch);
        }
    }
    match (has_null, non_null.as_slice()) {
        (true, [t]) => format!("Optional[{}]", type_repr(t)),
        (false, [t]) => type_repr(t),
        _ => "Any".into(),
    }
}

/// Extract the bare class name from a `$ref` like `../com/Adresse.json#` or `#/$defs/Adresse`.
pub fn ref_class_name(ref_str: &str) -> String {
    let (before_hash, after_hash) = match ref_str.split_once('#') {
        Some((b, a)) => (b, a),
        None => (ref_str, ""),
    };
    let segment = if !before_hash.is_empty() {
        before_hash
    } else {
        after_hash
    };
    let last = segment.rsplit('/').next().unwrap_or(segment);
    last.strip_suffix(".json").unwrap_or(last).to_string()
}

pub fn extract(
    _: &bo4e_schemas::models::schema_meta::Schemas,
) -> Result<crate::models::graph::GraphIR, String> {
    Err("not implemented yet".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::json_schema::{
        ArraySchema, BooleanSchema, DecimalSchema, IntegerSchema, LiteralTypeArray, NullSchema,
        ReferenceSchema, StringSchema, TypeBase,
    };

    fn s_string() -> SchemaType {
        SchemaType::StringSchema(StringSchema::default())
    }
    fn s_datetime() -> SchemaType {
        SchemaType::StringSchema(StringSchema {
            format: Some(StringSchemaFormat::DateTime),
            ..Default::default()
        })
    }
    fn s_decimal() -> SchemaType {
        SchemaType::DecimalSchema(DecimalSchema::default())
    }
    fn s_integer() -> SchemaType {
        SchemaType::IntegerSchema(IntegerSchema::default())
    }
    fn s_boolean() -> SchemaType {
        SchemaType::BooleanSchema(BooleanSchema::default())
    }
    fn s_null() -> SchemaType {
        SchemaType::NullSchema(NullSchema::default())
    }
    fn s_ref(ref_: &str) -> SchemaType {
        SchemaType::ReferenceSchema(ReferenceSchema {
            r#ref: ref_.to_string(),
            ..Default::default()
        })
    }

    #[test]
    fn primitives_render_as_expected() {
        assert_eq!(type_repr(&s_string()), "str");
        assert_eq!(type_repr(&s_datetime()), "datetime");
        assert_eq!(type_repr(&s_decimal()), "Decimal");
        assert_eq!(type_repr(&s_integer()), "int");
        assert_eq!(type_repr(&s_boolean()), "bool");
    }

    #[test]
    fn references_render_as_class_name() {
        assert_eq!(type_repr(&s_ref("../com/Adresse.json")), "Adresse");
        assert_eq!(type_repr(&s_ref("../enum/Typ.json#")), "Typ");
        assert_eq!(type_repr(&s_ref("#/$defs/Foo")), "Foo");
    }

    #[test]
    fn array_renders_with_list_prefix() {
        let a = ArraySchema {
            base: TypeBase::default(),
            r#type: LiteralTypeArray::Array,
            items: Box::new(s_ref("../com/Adresse.json")),
        };
        assert_eq!(type_repr(&SchemaType::Array(a)), "list[Adresse]");
    }

    #[test]
    fn anyof_t_plus_null_renders_optional() {
        let any = SchemaType::AnyOf(AnyOfSchema {
            base: Default::default(),
            any_of: vec![s_ref("../com/Adresse.json"), s_null()],
        });
        assert_eq!(type_repr(&any), "Optional[Adresse]");
    }
}
