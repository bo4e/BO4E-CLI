use bo4e_schemas::models::json_schema::{
    AnyOfSchema, SchemaRootType, SchemaType, StringSchemaFormat,
};
use bo4e_schemas::models::schema_meta::Schemas;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;

use crate::console::console::CONSOLE;
use crate::models::graph::{Cardinality, Edge, Field, GraphIR, Node};

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
        SchemaType::ConstantSchema(c) => format!(r#"Literal["{}"]"#, c.constant),
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
        SchemaType::StrEnum(e) => {
            // Inline enum (no separate class). Render as Python `Literal[...]`
            // so single-variant discriminators show their value and multi-variant
            // inline enums show their full set. `$ref` to a class-backed enum
            // is handled above via `ReferenceSchema` (renders as the class name).
            let joined = e
                .enum_values
                .iter()
                .map(|v| format!("\"{v}\""))
                .collect::<Vec<_>>()
                .join(",");
            format!("Literal[{joined}]")
        }
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
        (true, [t]) | (false, [t]) => type_repr(t),
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

pub fn extract(schemas: &Schemas) -> Result<GraphIR, String> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let known_modules: std::collections::HashSet<Vec<String>> =
        schemas.modules().into_iter().cloned().collect();

    for s_rc in schemas.iter() {
        let module = s_rc.borrow().module().to_vec();
        // `Schema::schema(&mut self) -> Result<&SchemaRootType, String>` per
        // crates/bo4e-schemas/src/models/schema_meta.rs. If the schema isn't
        // loaded for any reason, skip with no error.
        let owned_root: SchemaRootType = {
            let mut s_mut = s_rc.borrow_mut();
            match s_mut.schema() {
                Ok(r) => r.clone(),
                Err(_) => continue,
            }
        };

        let object = match &owned_root {
            SchemaRootType::Object(o) => &o.object,
            _ => {
                // StrEnum / Constant root schemas contribute a node with no fields.
                nodes.push(Node {
                    module,
                    fields: vec![],
                });
                continue;
            }
        };

        let mut fields: Vec<Field> = Vec::new();
        for (prop_name, prop_schema) in &object.properties {
            let required = object.required.iter().any(|r| r == prop_name);
            let (cardinality, target_module_opt) =
                property_card_and_target(prop_schema, required, &known_modules);
            let is_reference = target_module_opt.is_some();
            fields.push(Field {
                name: prop_name.clone(),
                type_repr: type_repr(prop_schema),
                cardinality: cardinality.clone(),
                is_reference,
            });
            if let Some(target_module) = target_module_opt {
                edges.push(Edge {
                    from: module.clone(),
                    to: target_module,
                    through_field: prop_name.clone(),
                    cardinality,
                });
            } else if let Some(unreachable_target) = unreachable_ref_target(prop_schema) {
                warnings.push(format!(
                    "{}.{}: $ref target {:?} is not in the Schemas collection — edge dropped",
                    module.join("."),
                    prop_name,
                    unreachable_target,
                ));
            }
        }

        nodes.push(Node { module, fields });
    }

    if let Some(console) = CONSOLE.get() {
        for w in &warnings {
            console.print_warn(w);
        }
    }

    Ok(GraphIR {
        version: schemas.version.clone(),
        nodes,
        edges,
    })
}

pub fn to_petgraph(g: &GraphIR) -> PetGraph {
    let mut pg: PetGraph = PetGraph::new();
    let mut idx: HashMap<Vec<String>, NodeIndex> = HashMap::new();
    for n in &g.nodes {
        let nx = pg.add_node(n.module.clone());
        idx.insert(n.module.clone(), nx);
    }
    for e in &g.edges {
        let (Some(&a), Some(&b)) = (idx.get(&e.from), idx.get(&e.to)) else {
            continue;
        };
        pg.add_edge(
            a,
            b,
            EdgeData {
                through_field: e.through_field.clone(),
                cardinality: e.cardinality.clone(),
            },
        );
    }
    pg
}

/// Reassemble a `GraphIR` from a `PetGraph` by re-attaching field metadata
/// from the original GraphIR via module-path lookup. Used by filter/cluster
/// passes to reconstruct a full IR for emit.
pub fn from_petgraph_with_fields(pg: &PetGraph, original: &GraphIR) -> GraphIR {
    let field_map: HashMap<&Vec<String>, &Vec<Field>> = original
        .nodes
        .iter()
        .map(|n| (&n.module, &n.fields))
        .collect();
    let mut nodes: Vec<Node> = Vec::new();
    for nx in pg.node_indices() {
        let module = pg[nx].clone();
        let fields = field_map
            .get(&module)
            .map(|f| (*f).clone())
            .unwrap_or_default();
        nodes.push(Node { module, fields });
    }
    let mut edges: Vec<Edge> = Vec::new();
    for ex in pg.edge_indices() {
        let (a, b) = pg.edge_endpoints(ex).unwrap();
        let data = &pg[ex];
        edges.push(Edge {
            from: pg[a].clone(),
            to: pg[b].clone(),
            through_field: data.through_field.clone(),
            cardinality: data.cardinality.clone(),
        });
    }
    GraphIR {
        version: original.version.clone(),
        nodes,
        edges,
    }
}

/// Returns (cardinality, Some(target_module) if ref-in-scope else None).
fn property_card_and_target(
    s: &SchemaType,
    required: bool,
    known: &std::collections::HashSet<Vec<String>>,
) -> (Cardinality, Option<Vec<String>>) {
    let min = if required {
        "1".to_string()
    } else {
        "0".to_string()
    };
    let max_one = "1".to_string();
    let max_star = "*".to_string();

    match s {
        SchemaType::Array(a) => {
            let (_, target) = property_card_and_target(&a.items, true, known);
            (Cardinality { min, max: max_star }, target)
        }
        SchemaType::AnyOf(any_of) => {
            let non_null: Vec<&SchemaType> = any_of
                .any_of
                .iter()
                .filter(|b| !matches!(b, SchemaType::NullSchema(_)))
                .collect();
            let (inner_card, target) = if let Some(t) = non_null.first() {
                property_card_and_target(t, true, known)
            } else {
                (
                    Cardinality {
                        min: "0".into(),
                        max: max_one.clone(),
                    },
                    None,
                )
            };
            (
                Cardinality {
                    min: "0".into(),
                    max: inner_card.max,
                },
                target,
            )
        }
        SchemaType::AllOf(all_of) => {
            if let Some(only) = all_of.all_of.first() {
                property_card_and_target(only, required, known)
            } else {
                (Cardinality { min, max: max_one }, None)
            }
        }
        SchemaType::ReferenceSchema(r) => {
            let target = resolve_ref_to_module(&r.r#ref, known);
            (Cardinality { min, max: max_one }, target)
        }
        _ => (Cardinality { min, max: max_one }, None),
    }
}

/// Resolve a `$ref` like `../com/Adresse.json` to the `Vec<String>` module path
/// used in `Schemas`. Returns `Some(module)` only if it's in the known set.
fn resolve_ref_to_module(
    ref_str: &str,
    known: &std::collections::HashSet<Vec<String>>,
) -> Option<Vec<String>> {
    let main = ref_str.split('#').next().unwrap_or(ref_str);
    let stripped = main.strip_suffix(".json").unwrap_or(main);
    let parts: Vec<&str> = stripped
        .split('/')
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .collect();
    if parts.is_empty() {
        return None;
    }
    let candidate: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
    if known.contains(&candidate) {
        Some(candidate)
    } else {
        // Fallback: match by class name only (last segment).
        let name = parts.last().unwrap();
        known
            .iter()
            .find(|m| m.last().map(|s| s.as_str()) == Some(*name))
            .cloned()
    }
}

fn unreachable_ref_target(s: &SchemaType) -> Option<String> {
    match s {
        SchemaType::ReferenceSchema(r) => Some(r.r#ref.clone()),
        SchemaType::Array(a) => unreachable_ref_target(&a.items),
        SchemaType::AnyOf(a) => a
            .any_of
            .iter()
            .filter(|b| !matches!(b, SchemaType::NullSchema(_)))
            .find_map(unreachable_ref_target),
        SchemaType::AllOf(a) => a.all_of.first().and_then(unreachable_ref_target),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::json_schema::{
        AnyOfSchema, ArraySchema, BooleanSchema, ConstantSchema, DecimalSchema, IntegerSchema,
        LiteralTypeArray, LiteralTypeObject, LiteralTypeString, NullSchema, ObjectSchema,
        ReferenceSchema, SchemaRootObject, SchemaRootTypeBase, StrEnumSchema, StringSchema,
        TypeBase,
    };
    use bo4e_schemas::models::schema_meta::{Schema, Schemas};
    use bo4e_schemas::models::version::DirtyVersion;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

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
    fn anyof_t_plus_null_drops_null_branch() {
        // Nullability is encoded in cardinality (min == 0), so `type_repr`
        // never emits an `Optional[...]` wrapping.
        let any = SchemaType::AnyOf(AnyOfSchema {
            base: Default::default(),
            any_of: vec![s_ref("../com/Adresse.json"), s_null()],
        });
        assert_eq!(type_repr(&any), "Adresse");
    }

    #[test]
    fn inline_single_variant_str_enum_renders_as_literal() {
        // Real BO4E discriminator shape, e.g. Angebot._typ:
        // `{"type":"string","default":"ANGEBOT","enum":["ANGEBOT"]}`.
        let e = SchemaType::StrEnum(StrEnumSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            enum_values: vec!["ANGEBOT".into()],
        });
        assert_eq!(type_repr(&e), r#"Literal["ANGEBOT"]"#);
    }

    #[test]
    fn inline_multi_variant_str_enum_renders_as_literal_with_all_values() {
        let e = SchemaType::StrEnum(StrEnumSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            enum_values: vec!["A".into(), "B".into(), "C".into()],
        });
        assert_eq!(type_repr(&e), r#"Literal["A","B","C"]"#);
    }

    #[test]
    fn constant_schema_renders_as_literal() {
        let c = SchemaType::ConstantSchema(ConstantSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: None,
            constant: "FIXED".into(),
        });
        assert_eq!(type_repr(&c), r#"Literal["FIXED"]"#);
    }

    fn make_root(props: BTreeMap<String, SchemaType>, required: Vec<String>) -> SchemaRootType {
        SchemaRootType::Object(SchemaRootObject {
            base: SchemaRootTypeBase::default(),
            object: ObjectSchema {
                base: TypeBase::default(),
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: props,
                required,
            },
        })
    }

    fn make_schema(module: &[&str], root: SchemaRootType) -> Rc<RefCell<Schema>> {
        let m: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        Rc::new(RefCell::new(Schema::new(m, Some(root)).unwrap()))
    }

    fn collect_schemas(modules: Vec<Rc<RefCell<Schema>>>) -> Schemas {
        let v: DirtyVersion = "v202501.0.0".parse().unwrap();
        let mut s = Schemas::new(v);
        for sc in modules {
            s.add_schema(sc).unwrap();
        }
        s
    }

    #[test]
    fn cardinality_required_non_array_is_1_to_1() {
        let mut props = BTreeMap::new();
        props.insert("betrag".to_string(), s_decimal());
        let sc = make_schema(&["com", "Betrag"], make_root(props, vec!["betrag".into()]));
        let schemas = collect_schemas(vec![sc]);
        let g = extract(&schemas).unwrap();
        let node = g
            .nodes
            .iter()
            .find(|n| n.module == vec!["com".to_string(), "Betrag".to_string()])
            .unwrap();
        let f = &node.fields[0];
        assert_eq!(f.name, "betrag");
        assert_eq!(f.cardinality.min, "1");
        assert_eq!(f.cardinality.max, "1");
        assert!(!f.is_reference);
    }

    #[test]
    fn cardinality_optional_anyof_with_null_resolves_target() {
        let mut props = BTreeMap::new();
        let opt = SchemaType::AnyOf(AnyOfSchema {
            base: Default::default(),
            any_of: vec![s_ref("../com/Adresse.json"), s_null()],
        });
        props.insert("adresse".to_string(), opt);
        let sc1 = make_schema(&["bo", "Angebot"], make_root(props, vec![]));
        let target = make_schema(&["com", "Adresse"], make_root(BTreeMap::new(), vec![]));
        let schemas = collect_schemas(vec![sc1, target]);
        let g = extract(&schemas).unwrap();
        let node = g
            .nodes
            .iter()
            .find(|n| n.module == vec!["bo".to_string(), "Angebot".to_string()])
            .unwrap();
        let f = &node.fields[0];
        assert_eq!(f.cardinality.min, "0");
        assert_eq!(f.cardinality.max, "1");
        assert!(
            f.is_reference,
            "anyOf[Adresse,null] points to a target in scope"
        );
        assert_eq!(g.edges.len(), 1);
        assert_eq!(
            g.edges[0].to,
            vec!["com".to_string(), "Adresse".to_string()]
        );
        assert_eq!(g.edges[0].through_field, "adresse");
    }

    #[test]
    fn cardinality_required_array_is_1_to_star() {
        let mut props = BTreeMap::new();
        let arr = SchemaType::Array(ArraySchema {
            base: Default::default(),
            r#type: LiteralTypeArray::Array,
            items: Box::new(s_ref("../com/Variante.json")),
        });
        props.insert("varianten".to_string(), arr);
        let sc1 = make_schema(
            &["bo", "Angebot"],
            make_root(props, vec!["varianten".into()]),
        );
        let target = make_schema(&["com", "Variante"], make_root(BTreeMap::new(), vec![]));
        let schemas = collect_schemas(vec![sc1, target]);
        let g = extract(&schemas).unwrap();
        let node = g
            .nodes
            .iter()
            .find(|n| n.module == vec!["bo".to_string(), "Angebot".to_string()])
            .unwrap();
        let f = &node.fields[0];
        assert_eq!(f.cardinality.min, "1");
        assert_eq!(f.cardinality.max, "*");
        assert!(f.is_reference);
    }

    #[test]
    fn out_of_scope_ref_yields_no_edge_and_marks_non_reference() {
        let mut props = BTreeMap::new();
        props.insert("foo".to_string(), s_ref("../external/External.json"));
        let sc1 = make_schema(&["bo", "Angebot"], make_root(props, vec![]));
        let schemas = collect_schemas(vec![sc1]);
        let g = extract(&schemas).unwrap();
        let node = g
            .nodes
            .iter()
            .find(|n| n.module == vec!["bo".to_string(), "Angebot".to_string()])
            .unwrap();
        let f = &node.fields[0];
        assert!(
            !f.is_reference,
            "external ref must not be marked is_reference"
        );
        assert_eq!(g.edges.len(), 0);
    }

    #[test]
    fn invariant_field_is_reference_iff_edge_exists() {
        let mut p1 = BTreeMap::new();
        p1.insert("adresse".to_string(), s_ref("../com/Adresse.json"));
        p1.insert("betrag".to_string(), s_decimal());
        let sc1 = make_schema(
            &["bo", "Angebot"],
            make_root(p1, vec!["adresse".into(), "betrag".into()]),
        );
        let target = make_schema(&["com", "Adresse"], make_root(BTreeMap::new(), vec![]));
        let schemas = collect_schemas(vec![sc1, target]);
        let g = extract(&schemas).unwrap();
        for node in &g.nodes {
            for field in &node.fields {
                let has_edge = g
                    .edges
                    .iter()
                    .any(|e| e.from == node.module && e.through_field == field.name);
                assert_eq!(
                    field.is_reference, has_edge,
                    "invariant violated for {:?}.{}",
                    node.module, field.name
                );
            }
        }
    }

    #[test]
    fn petgraph_roundtrip_with_fields_preserves_full_ir() {
        let mut p1 = BTreeMap::new();
        p1.insert("adresse".to_string(), s_ref("../com/Adresse.json"));
        p1.insert("betrag".to_string(), s_decimal());
        let sc1 = make_schema(
            &["bo", "Angebot"],
            make_root(p1, vec!["adresse".into(), "betrag".into()]),
        );
        let target = make_schema(&["com", "Adresse"], make_root(BTreeMap::new(), vec![]));
        let schemas = collect_schemas(vec![sc1, target]);
        let g = extract(&schemas).unwrap();
        let pg = super::to_petgraph(&g);
        let g2 = super::from_petgraph_with_fields(&pg, &g);

        // Order can differ; sort both sides for comparison.
        let mut a = g.clone();
        let mut b = g2;
        a.nodes.sort_by(|x, y| x.module.cmp(&y.module));
        a.edges.sort_by(|x, y| {
            (x.from.clone(), x.to.clone(), x.through_field.clone()).cmp(&(
                y.from.clone(),
                y.to.clone(),
                y.through_field.clone(),
            ))
        });
        b.nodes.sort_by(|x, y| x.module.cmp(&y.module));
        b.edges.sort_by(|x, y| {
            (x.from.clone(), x.to.clone(), x.through_field.clone()).cmp(&(
                y.from.clone(),
                y.to.clone(),
                y.through_field.clone(),
            ))
        });
        assert_eq!(a, b);
    }
}
