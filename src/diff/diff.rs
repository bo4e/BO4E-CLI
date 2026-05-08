use crate::diff::filters::has_critical;
use crate::models::changes::{Change, ChangeType, ChangeValue, Changes};
use crate::models::json_schema::{
    AllOfSchema, AnyOfSchema, ArraySchema, ObjectSchema, ReferenceSchema, SchemaRootType,
    SchemaType, StrEnumSchema, StringSchema, TypeBase,
};
use crate::models::schema_meta::Schemas;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref REGEX_VERSION_IN_DESC: Regex =
        Regex::new(r"v\d{6}\.\d+\.\d+(?:-rc\d*)?").unwrap();
}

const VERSION_DESC_PLACEHOLDER: &str = "{__gh_version__}";
const VERSION_TITLE_MARKER: &str = " Version";

#[derive(Debug, Clone, Copy)]
enum VariantKind {
    AnyOf,
    AllOf,
}

/// Compare two `Schemas` collections and return the list of changes between them.
pub fn diff_schemas(old: &Schemas, new: &Schemas) -> Changes {
    let mut out: Vec<Change> = Vec::new();
    diff_root_schemas(old, new, &mut out);
    Changes {
        old_schemas: old.clone(),
        new_schemas: new.clone(),
        changes: out,
    }
}

fn diff_root_schemas(old: &Schemas, new: &Schemas, out: &mut Vec<Change>) {
    for s in new.module_difference(old) {
        let module = s.borrow().module().to_vec();
        let trace = format!("/{}", module.join("/"));
        out.push(Change {
            r#type: ChangeType::ClassAdded,
            old: None,
            new: Some(ChangeValue::String(module.join("."))),
            old_trace: String::new(),
            new_trace: trace,
        });
    }

    for s in old.module_difference(new) {
        let module = s.borrow().module().to_vec();
        let trace = format!("/{}", module.join("/"));
        out.push(Change {
            r#type: ChangeType::ClassRemoved,
            old: Some(ChangeValue::String(module.join("."))),
            new: None,
            old_trace: trace,
            new_trace: String::new(),
        });
    }

    for s_old in old.module_intersection(new) {
        let module = s_old.borrow().module().to_vec();
        let s_new = new.get_by_module(&module).expect("intersection guaranteed");
        let trace = format!("/{}", module.join("/"));

        let mut b_old = s_old.borrow_mut();
        let mut b_new = s_new.borrow_mut();
        let root_old = match b_old.schema_mut() {
            Ok(r) => r.clone(),
            Err(_) => continue,
        };
        let root_new = match b_new.schema_mut() {
            Ok(r) => r.clone(),
            Err(_) => continue,
        };
        diff_root_pair(&root_old, &root_new, &trace, &trace, out);
    }
}

fn diff_root_pair(
    old: &SchemaRootType,
    new: &SchemaRootType,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    match (old, new) {
        (SchemaRootType::Object(o), SchemaRootType::Object(n)) => {
            diff_object_schemas(&o.object, &n.object, old_trace, new_trace, out);
        }
        (SchemaRootType::StrEnum(o), SchemaRootType::StrEnum(n)) => {
            diff_enum_schemas(&o.str_enum, &n.str_enum, old_trace, new_trace, out);
        }
        _ => {
            out.push(Change {
                r#type: ChangeType::FieldTypeChanged,
                old: None,
                new: None,
                old_trace: old_trace.to_string(),
                new_trace: new_trace.to_string(),
            });
        }
    }
}

fn diff_type_base(
    old: &TypeBase,
    new: &TypeBase,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    let desc_changed = match (&old.description, &new.description) {
        (Some(o), Some(n)) => {
            let o_norm = REGEX_VERSION_IN_DESC.replace_all(o, VERSION_DESC_PLACEHOLDER);
            let n_norm = REGEX_VERSION_IN_DESC.replace_all(n, VERSION_DESC_PLACEHOLDER);
            o_norm != n_norm
        }
        (None, None) => false,
        _ => true,
    };
    if desc_changed {
        out.push(Change {
            r#type: ChangeType::FieldDescriptionChanged,
            old: old.description.clone().map(ChangeValue::String),
            new: new.description.clone().map(ChangeValue::String),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }

    if old.title != new.title {
        out.push(Change {
            r#type: ChangeType::FieldTitleChanged,
            old: old.title.clone().map(ChangeValue::String),
            new: new.title.clone().map(ChangeValue::String),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }

    let is_version_field = old.title.as_deref() == Some(VERSION_TITLE_MARKER)
        || new.title.as_deref() == Some(VERSION_TITLE_MARKER);
    if !is_version_field && old.default != new.default {
        out.push(Change {
            r#type: ChangeType::FieldDefaultChanged,
            old: None,
            new: None,
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}

fn diff_ref_schemas(
    old: &ReferenceSchema,
    new: &ReferenceSchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    if old.r#ref != new.r#ref {
        out.push(Change {
            r#type: ChangeType::FieldReferenceChanged,
            old: Some(ChangeValue::String(old.r#ref.clone())),
            new: Some(ChangeValue::String(new.r#ref.clone())),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}

fn diff_array_schemas(
    old: &ArraySchema,
    new: &ArraySchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    diff_schema_type(
        &old.items,
        &new.items,
        &format!("{}/items", old_trace),
        &format!("{}/items", new_trace),
        out,
    );
}

fn diff_string_schemas(
    old: &StringSchema,
    new: &StringSchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    if old.format != new.format {
        out.push(Change {
            r#type: ChangeType::FieldStringFormatChanged,
            old: old
                .format
                .as_ref()
                .map(|f| ChangeValue::String(format!("{:?}", f))),
            new: new
                .format
                .as_ref()
                .map(|f| ChangeValue::String(format!("{:?}", f))),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}

fn diff_any_of_schemas(
    old: &AnyOfSchema,
    new: &AnyOfSchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    diff_variant_list(
        &old.any_of,
        &new.any_of,
        old_trace,
        new_trace,
        VariantKind::AnyOf,
        out,
    );
}

fn diff_all_of_schemas(
    old: &AllOfSchema,
    new: &AllOfSchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    diff_variant_list(
        &old.all_of,
        &new.all_of,
        old_trace,
        new_trace,
        VariantKind::AllOf,
        out,
    );
}

fn diff_variant_list(
    old: &[SchemaType],
    new: &[SchemaType],
    old_trace: &str,
    new_trace: &str,
    kind: VariantKind,
    out: &mut Vec<Change>,
) {
    let key = match kind {
        VariantKind::AnyOf => "anyOf",
        VariantKind::AllOf => "allOf",
    };
    let added_t = match kind {
        VariantKind::AnyOf => ChangeType::FieldAnyOfTypeAdded,
        VariantKind::AllOf => ChangeType::FieldAllOfTypeAdded,
    };
    let removed_t = match kind {
        VariantKind::AnyOf => ChangeType::FieldAnyOfTypeRemoved,
        VariantKind::AllOf => ChangeType::FieldAllOfTypeRemoved,
    };

    let mut new_matched = vec![false; new.len()];
    for (oi, ov) in old.iter().enumerate() {
        let ot = format!("{}/{}/{}", old_trace, key, oi);
        let mut paired = false;
        for (ni, nv) in new.iter().enumerate() {
            if new_matched[ni] {
                continue;
            }
            let nt = format!("{}/{}/{}", new_trace, key, ni);
            let mut sub: Vec<Change> = Vec::new();
            diff_schema_type(ov, nv, &ot, &nt, &mut sub);
            if !has_critical(&sub) {
                out.extend(sub);
                new_matched[ni] = true;
                paired = true;
                break;
            }
        }
        if !paired {
            out.push(Change {
                r#type: removed_t.clone(),
                old: None,
                new: None,
                old_trace: ot,
                new_trace: new_trace.to_string(),
            });
        }
    }
    for (ni, matched) in new_matched.iter().enumerate() {
        if !matched {
            let nt = format!("{}/{}/{}", new_trace, key, ni);
            out.push(Change {
                r#type: added_t.clone(),
                old: None,
                new: None,
                old_trace: old_trace.to_string(),
                new_trace: nt,
            });
        }
    }
}

fn diff_schema_differing_types(
    old: &SchemaType,
    new: &SchemaType,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    let cardinality = matches!(
        (old, new),
        (SchemaType::Array(_), SchemaType::Object(_))
            | (SchemaType::Object(_), SchemaType::Array(_))
            | (SchemaType::Array(_), SchemaType::ReferenceSchema(_))
            | (SchemaType::ReferenceSchema(_), SchemaType::Array(_))
    );
    let kind = if cardinality {
        ChangeType::FieldCardinalityChanged
    } else {
        ChangeType::FieldTypeChanged
    };
    out.push(Change {
        r#type: kind,
        old: Some(ChangeValue::Schema(old.clone())),
        new: Some(ChangeValue::Schema(new.clone())),
        old_trace: old_trace.to_string(),
        new_trace: new_trace.to_string(),
    });
}

fn diff_schema_type(
    old: &SchemaType,
    new: &SchemaType,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    use SchemaType::*;
    match (old, new) {
        (Object(o), Object(n)) => diff_object_schemas(o, n, old_trace, new_trace, out),
        (StrEnum(o), StrEnum(n)) => diff_enum_schemas(o, n, old_trace, new_trace, out),
        (Array(o), Array(n)) => diff_array_schemas(o, n, old_trace, new_trace, out),
        (AnyOf(o), AnyOf(n)) => diff_any_of_schemas(o, n, old_trace, new_trace, out),
        (AllOf(o), AllOf(n)) => diff_all_of_schemas(o, n, old_trace, new_trace, out),
        (StringSchema(o), StringSchema(n)) => {
            diff_string_schemas(o, n, old_trace, new_trace, out)
        }
        (ReferenceSchema(o), ReferenceSchema(n)) => {
            diff_ref_schemas(o, n, old_trace, new_trace, out)
        }
        _ if std::mem::discriminant(old) == std::mem::discriminant(new) => {}
        _ => diff_schema_differing_types(old, new, old_trace, new_trace, out),
    }
}

fn diff_object_schemas(
    old: &ObjectSchema,
    new: &ObjectSchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);

    for (name, schema_new) in &new.properties {
        if !old.properties.contains_key(name) {
            let nt = format!("{}/{}", new_trace, name);
            out.push(Change {
                r#type: ChangeType::FieldAdded,
                old: None,
                new: Some(ChangeValue::Schema(schema_new.clone())),
                old_trace: old_trace.to_string(),
                new_trace: nt,
            });
        }
    }
    for (name, schema_old) in &old.properties {
        if !new.properties.contains_key(name) {
            let ot = format!("{}/{}", old_trace, name);
            out.push(Change {
                r#type: ChangeType::FieldRemoved,
                old: Some(ChangeValue::Schema(schema_old.clone())),
                new: None,
                old_trace: ot,
                new_trace: new_trace.to_string(),
            });
        }
    }
    for (name, schema_old) in &old.properties {
        if let Some(schema_new) = new.properties.get(name) {
            diff_schema_type(
                schema_old,
                schema_new,
                &format!("{}/{}", old_trace, name),
                &format!("{}/{}", new_trace, name),
                out,
            );
        }
    }
}

fn diff_enum_schemas(
    old: &StrEnumSchema,
    new: &StrEnumSchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);

    let old_set: std::collections::BTreeSet<&String> = old.enum_values.iter().collect();
    let new_set: std::collections::BTreeSet<&String> = new.enum_values.iter().collect();

    for v in new_set.difference(&old_set) {
        out.push(Change {
            r#type: ChangeType::EnumValueAdded,
            old: None,
            new: Some(ChangeValue::String((*v).clone())),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
    for v in old_set.difference(&new_set) {
        out.push(Change {
            r#type: ChangeType::EnumValueRemoved,
            old: Some(ChangeValue::String((*v).clone())),
            new: None,
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::ChangeType;
    use crate::models::json_schema::{
        LiteralTypeObject, LiteralTypeString, ObjectSchema, SchemaRootObject, SchemaRootStrEnum,
        SchemaRootType, SchemaRootTypeBase, SchemaType, StrEnumSchema, TypeBase,
    };
    use crate::models::schema_meta::{Schema, Schemas};
    use crate::models::version::DirtyVersion;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    fn empty_object_root(title: &str) -> SchemaRootType {
        SchemaRootType::Object(SchemaRootObject {
            base: SchemaRootTypeBase::default(),
            object: ObjectSchema {
                base: TypeBase {
                    description: None,
                    title: Some(title.to_string()),
                    default: None,
                },
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::new(),
                required: vec![],
            },
        })
    }

    fn schema_with(module: &[&str], root: SchemaRootType) -> Rc<RefCell<Schema>> {
        let m: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        Rc::new(RefCell::new(Schema::new(m, Some(root)).unwrap()))
    }

    fn collection(version: &str, items: Vec<Rc<RefCell<Schema>>>) -> Schemas {
        let v: DirtyVersion = version.parse().unwrap();
        let mut s = Schemas::new(v);
        for it in items {
            s.add_schema(it).unwrap();
        }
        s
    }

    #[test]
    fn test_class_added_when_module_only_in_new() {
        let old = collection("v202401.0.1", vec![]);
        let new = collection(
            "v202401.0.2",
            vec![schema_with(&["bo", "Angebot"], empty_object_root("Angebot"))],
        );
        let changes = diff_schemas(&old, &new);
        assert_eq!(changes.changes.len(), 1);
        assert_eq!(changes.changes[0].r#type, ChangeType::ClassAdded);
        assert_eq!(changes.changes[0].new_trace, "/bo/Angebot");
    }

    #[test]
    fn test_class_removed_when_module_only_in_old() {
        let old = collection(
            "v202401.0.1",
            vec![schema_with(&["bo", "Angebot"], empty_object_root("Angebot"))],
        );
        let new = collection("v202401.0.2", vec![]);
        let changes = diff_schemas(&old, &new);
        assert_eq!(changes.changes.len(), 1);
        assert_eq!(changes.changes[0].r#type, ChangeType::ClassRemoved);
        assert_eq!(changes.changes[0].old_trace, "/bo/Angebot");
    }

    fn base(desc: Option<&str>, title: Option<&str>) -> TypeBase {
        TypeBase {
            description: desc.map(String::from),
            title: title.map(String::from),
            default: None,
        }
    }

    #[test]
    fn test_diff_type_base_emits_description_changed() {
        let mut out = vec![];
        diff_type_base(
            &base(Some("alpha"), None),
            &base(Some("beta"), None),
            "/x",
            "/x",
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldDescriptionChanged);
    }

    #[test]
    fn test_diff_type_base_ignores_version_only_description_change() {
        let mut out = vec![];
        diff_type_base(
            &base(Some("Schema for v202401.0.1"), None),
            &base(Some("Schema for v202401.0.2"), None),
            "/x",
            "/x",
            &mut out,
        );
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn test_diff_type_base_emits_title_changed() {
        let mut out = vec![];
        diff_type_base(
            &base(None, Some("A")),
            &base(None, Some("B")),
            "/x",
            "/x",
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldTitleChanged);
    }

    #[test]
    fn test_diff_type_base_skips_version_field_default_change() {
        use crate::models::json_schema::PrimitiveValue;
        let mut a = base(None, Some(" Version"));
        let mut b = base(None, Some(" Version"));
        a.default = Some(PrimitiveValue::String("v202401.0.1".into()));
        b.default = Some(PrimitiveValue::String("v202401.0.2".into()));
        let mut out = vec![];
        diff_type_base(&a, &b, "/x", "/x", &mut out);
        assert_eq!(out.len(), 0);
    }

    fn enum_schema(values: &[&str]) -> StrEnumSchema {
        StrEnumSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            enum_values: values.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_enum_value_added_and_removed() {
        let mut out = vec![];
        diff_enum_schemas(
            &enum_schema(&["A", "B"]),
            &enum_schema(&["B", "C"]),
            "/x",
            "/x",
            &mut out,
        );
        let kinds: Vec<_> = out.iter().map(|c| c.r#type.clone()).collect();
        assert!(kinds.contains(&ChangeType::EnumValueAdded));
        assert!(kinds.contains(&ChangeType::EnumValueRemoved));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn test_ref_change_emits_field_reference_changed() {
        use crate::models::json_schema::ReferenceSchema;
        let r1 = ReferenceSchema {
            base: TypeBase::default(),
            r#ref: "#/A".into(),
        };
        let r2 = ReferenceSchema {
            base: TypeBase::default(),
            r#ref: "#/B".into(),
        };
        let mut out = vec![];
        diff_ref_schemas(&r1, &r2, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldReferenceChanged);
    }

    #[test]
    fn test_string_format_change() {
        use crate::models::json_schema::{StringSchema, StringSchemaFormat};
        let mut a = StringSchema::default();
        let mut b = StringSchema::default();
        a.format = None;
        b.format = Some(StringSchemaFormat::DateTime);
        let mut out = vec![];
        diff_string_schemas(&a, &b, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldStringFormatChanged);
    }

    fn string_schema_t() -> SchemaType {
        use crate::models::json_schema::StringSchema;
        SchemaType::StringSchema(StringSchema::default())
    }

    fn ref_t(r: &str) -> SchemaType {
        use crate::models::json_schema::ReferenceSchema;
        SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase::default(),
            r#ref: r.to_string(),
        })
    }

    #[test]
    fn test_any_of_variant_added_emits_field_any_of_type_added() {
        use crate::models::json_schema::AnyOfSchema;
        let old = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![string_schema_t()],
        };
        let new = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![string_schema_t(), ref_t("#/A")],
        };
        let mut out = vec![];
        diff_any_of_schemas(&old, &new, "/x", "/x", &mut out);
        let added: Vec<_> = out
            .iter()
            .filter(|c| c.r#type == ChangeType::FieldAnyOfTypeAdded)
            .collect();
        assert_eq!(added.len(), 1);
    }

    #[test]
    fn test_any_of_variant_removed() {
        use crate::models::json_schema::AnyOfSchema;
        let old = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![string_schema_t(), ref_t("#/A")],
        };
        let new = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![string_schema_t()],
        };
        let mut out = vec![];
        diff_any_of_schemas(&old, &new, "/x", "/x", &mut out);
        let removed: Vec<_> = out
            .iter()
            .filter(|c| c.r#type == ChangeType::FieldAnyOfTypeRemoved)
            .collect();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_any_of_pairs_with_non_critical_inner_change() {
        use crate::models::json_schema::{AnyOfSchema, StringSchema};
        let mut s_old = StringSchema::default();
        let mut s_new = StringSchema::default();
        s_old.base.description = Some("old".into());
        s_new.base.description = Some("new".into());
        let old = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![SchemaType::StringSchema(s_old)],
        };
        let new = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![SchemaType::StringSchema(s_new)],
        };
        let mut out = vec![];
        diff_any_of_schemas(&old, &new, "/x", "/x", &mut out);
        assert!(out.iter().all(|c| c.r#type != ChangeType::FieldAnyOfTypeAdded));
        assert!(out.iter().all(|c| c.r#type != ChangeType::FieldAnyOfTypeRemoved));
        assert!(out
            .iter()
            .any(|c| c.r#type == ChangeType::FieldDescriptionChanged));
    }

    #[test]
    fn test_field_type_changed_unrelated_types() {
        use crate::models::json_schema::{NumberSchema, StringSchema};
        let old = SchemaType::StringSchema(StringSchema::default());
        let new = SchemaType::NumberSchema(NumberSchema::default());
        let mut out = vec![];
        diff_schema_type(&old, &new, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldTypeChanged);
    }

    #[test]
    fn test_field_cardinality_changed_object_to_array() {
        use crate::models::json_schema::{ArraySchema, LiteralTypeArray, StringSchema};
        let obj = SchemaType::Object(ObjectSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeObject::Object,
            additional_properties: false,
            properties: BTreeMap::new(),
            required: vec![],
        });
        let arr = SchemaType::Array(ArraySchema {
            base: TypeBase::default(),
            r#type: LiteralTypeArray::Array,
            items: Box::new(SchemaType::StringSchema(StringSchema::default())),
        });
        let mut out = vec![];
        diff_schema_type(&obj, &arr, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldCardinalityChanged);
    }

    fn obj(props: &[(&str, SchemaType)]) -> ObjectSchema {
        let mut p = BTreeMap::new();
        for (k, v) in props {
            p.insert(k.to_string(), v.clone());
        }
        ObjectSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeObject::Object,
            additional_properties: false,
            properties: p,
            required: vec![],
        }
    }

    #[test]
    fn test_object_field_added_and_removed() {
        let a = obj(&[("foo", string_schema_t())]);
        let b = obj(&[("bar", string_schema_t())]);
        let mut out = vec![];
        diff_object_schemas(&a, &b, "/x", "/x", &mut out);
        let kinds: Vec<_> = out.iter().map(|c| c.r#type.clone()).collect();
        assert!(kinds.contains(&ChangeType::FieldAdded));
        assert!(kinds.contains(&ChangeType::FieldRemoved));
    }

    #[test]
    fn test_object_field_default_changed_recurses() {
        use crate::models::json_schema::{PrimitiveValue, StringSchema};
        let mut s_old = StringSchema::default();
        let mut s_new = StringSchema::default();
        s_old.base.default = Some(PrimitiveValue::String("a".into()));
        s_new.base.default = Some(PrimitiveValue::String("b".into()));
        let a = obj(&[("foo", SchemaType::StringSchema(s_old))]);
        let b = obj(&[("foo", SchemaType::StringSchema(s_new))]);
        let mut out = vec![];
        diff_object_schemas(&a, &b, "/x", "/x", &mut out);
        assert!(out
            .iter()
            .any(|c| c.r#type == ChangeType::FieldDefaultChanged));
    }
}
