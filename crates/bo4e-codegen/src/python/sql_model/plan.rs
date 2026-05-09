//! SQL-model build plan: an immutable description of all tables, fields, and junctions
//! produced by walking a [`bo4e_schemas::Schemas`].
//!
//! `build_plan` is pure — it has no side effects and writes no files. The renderer in
//! [`super`] consumes the plan and produces source.

#![allow(dead_code)] // Filled in across Tasks 6, 7. Wired up in Task 11.

use std::collections::BTreeMap;

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
pub(crate) fn build_plan(_schemas: &bo4e_schemas::Schemas) -> SqlPlan {
    SqlPlan {
        tables: BTreeMap::new(),
        junctions: Vec::new(),
    }
}
