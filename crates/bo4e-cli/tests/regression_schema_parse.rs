//! Regression tests for individual schemas that previously failed to parse.
//!
//! Each fixture under `fixtures/regressions/` represents one historical bug.
//! When a new BO4E release breaks parsing, copy the offending JSON in here
//! with a descriptive filename and add a paired `#[test]`.

use bo4e_schemas::models::json_schema::SchemaRootType;

fn parse(raw: &str) -> SchemaRootType {
    serde_json::from_str(raw).expect("regression: schema must parse")
}

// === ADD ONE TEST PER FIXTURE BELOW. PATTERN: ===
//
// #[test]
// fn parses_<filename_without_extension>() {
//     let raw = include_str!("fixtures/regressions/<filename>.json");
//     parse(raw);
// }

#[test]
fn parses_marktteilnehmer_missing_required() {
    let raw = include_str!("fixtures/regressions/marktteilnehmer_missing_required.json");
    parse(raw);
}

#[test]
fn parses_zusatzattribut_missing_required_and_additional_properties() {
    let raw = include_str!(
        "fixtures/regressions/zusatzattribut_missing_required_and_additional_properties.json"
    );
    parse(raw);
}
