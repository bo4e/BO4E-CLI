use crate::models::json_schema::ReferenceSchema;
use crate::models::schema_meta::{Schema, Schemas};
use crate::utils::visitable::Visitable;
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;

// REF_ONLINE_REGEX = re.compile(
//     rf"^https://raw\.githubusercontent\.com/(?:{OWNER.upper()}|{OWNER.lower()}|{OWNER.capitalize()}|Hochfrequenz)/"
//     rf"{REPO}/(?P<version>[^/]+)/"
//     r"src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
// )
// # e.g. https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
// REF_DEFS_REGEX = re.compile(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$")
lazy_static! {
    pub static ref REF_ONLINE_REGEX: regex::Regex = regex::Regex::new(
        r"^https://raw\.githubusercontent\.com/(?:BO4E|bo4e|Bo4e|Hochfrequenz)/BO4E-Schemas/(?P<version>[^/]+)/src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
    )
    .unwrap();
    // e.g. https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
    pub static ref REF_DEFS_REGEX: regex::Regex =
        regex::Regex::new(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$").unwrap();
}

fn update_reference(
    reference: &mut ReferenceSchema,
    current_module: &[String],
    namespace: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
}

fn update_references_single(
    schema: &mut Schema,
    namespace: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let module: Vec<String> = schema.module().iter().cloned().collect();
    let visitable: &mut dyn Visitable = schema.schema_mut()?;
    let results = visitable
        .visit_by_type_mut(&mut |reference: &mut ReferenceSchema| {
            update_reference(reference, &module, namespace)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}

pub fn update_references_all(schemas: &mut Schemas) -> Result<(), String> {
    let namespace = schemas.modules_by_name();
    for schema in schemas.iter_mut() {
        update_references_single(schema.borrow_mut().deref_mut(), &namespace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn test_update_references() {
        let mut map = HashMap::from([("a", 1), ("b", 2)]);

        // Collect the keys first (shallow copy, cheap)
        let keys: Vec<_> = map.keys().cloned().collect();

        for key in keys {
            // We are NOT inside `iter_mut()` anymore → map is free for lookups
            if map.contains_key("a") {
                // Now take the mutable borrow *inside* the loop body
                let value = map.get_mut(&key).unwrap();
                *value += 1;
            }
        }
    }
}
