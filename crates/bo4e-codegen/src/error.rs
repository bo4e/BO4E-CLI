use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("template render error: {0}")]
    TemplateRender(#[from] minijinja::Error),

    #[error("template not found: {name}")]
    TemplateNotFound { name: String },

    #[error("schema lookup miss: {0}")]
    SchemaLookup(String),

    #[error("schema model error: {0}")]
    Schema(String),

    #[error(
        "cannot classify property `{class}.{property}`: schema shape is unsupported by the SQL plan"
    )]
    UnclassifiableProperty { class: String, property: String },

    #[error("schema {schema_name} property `{property}`: unsupported shape ({shape})")]
    UnsupportedSchemaShape {
        schema_name: String,
        property: String,
        shape: String,
    },

    /// One of the schema-consistency invariants enforced by
    /// [`crate::validate::all_schemas`] is violated. Examples: a name in
    /// `required` not declared in `properties`; a property in `required`
    /// that also carries a `default`; a `default` whose primitive kind
    /// doesn't match the schema type; a `$ref` default referencing a
    /// non-existent enum variant.
    #[error("inconsistent schema {schema}::{property}: {reason}")]
    InconsistentSchema {
        schema: String,
        property: String,
        reason: String,
    },

    /// A caller-supplied option (e.g. `RustCrateOptions::crate_name`) is
    /// malformed and cannot be rendered safely.
    #[error("invalid {what}: {reason}")]
    InvalidOption { what: String, reason: String },
}
