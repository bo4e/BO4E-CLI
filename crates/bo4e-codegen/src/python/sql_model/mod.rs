//! python-sql-model generator orchestration.
//!
//! Two-phase: a pure pre-pass walks `Schemas` and produces an immutable
//! [`plan::SqlPlan`]; a render pass consumes the plan and writes Python files
//! via vendored MiniJinja templates.

pub(crate) mod plan;

// Re-export the entry point so lib.rs can call into us via `python::sql_model::generate_sql_model`.
// Filled in by Task 11.
#[allow(unused_imports)]
pub(crate) use plan::SqlPlan;
