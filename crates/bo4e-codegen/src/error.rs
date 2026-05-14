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

    /// The schema violates the required/default invariant: a property must be
    /// in `required` *iff* it has no schema-declared default. See
    /// [`crate::validate::object_invariants`].
    #[error("inconsistent schema {schema}::{property}: {reason}")]
    InconsistentSchema {
        schema: String,
        property: String,
        reason: String,
    },
}
