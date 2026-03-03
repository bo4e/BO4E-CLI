use crate::models::json_schema::ReferenceSchema;
use crate::models::schema_meta::{Schema, Schemas};
use crate::utils::visitable::{Visitable, cntrl_to_result, result_to_cntrl};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::ops::{ControlFlow, DerefMut};

lazy_static! {
    pub static ref REF_ONLINE_REGEX: regex::Regex = regex::Regex::new(
        r"^https://raw\.githubusercontent\.com/(?:BO4E|bo4e|Bo4e|Hochfrequenz)/BO4E-Schemas/(?P<version>[^/]+)/src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
    )
    .unwrap();
    pub static ref REF_DEFS_REGEX: regex::Regex =
        regex::Regex::new(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$").unwrap();
}

fn update_reference(
    _reference: &mut ReferenceSchema,
    _current_module: &[String],
    _namespace: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    Ok(()) // TODO: implement reference rewriting logic
}

fn update_references_single(
    schema: &mut Schema,
    namespace: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let module: Vec<String> = schema.module().iter().cloned().collect();
    let visitable: &mut dyn Visitable = schema.schema_mut()?;
    cntrl_to_result(
        visitable.try_visit_all_mut::<ReferenceSchema, String>(&mut |reference| {
            result_to_cntrl(update_reference(reference, &module, namespace))
        }),
    )
}

pub fn update_references_all(schemas: &mut Schemas) -> Result<(), String> {
    let namespace = schemas.modules_by_name();
    for schema in schemas.iter_mut() {
        update_references_single(schema.borrow_mut().deref_mut(), &namespace)?;
    }
    Ok(())
}
