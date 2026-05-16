//! Targeted tests for Copilot's 2026-05-15 review (round 2 on `bfb4842`).
//!
//! Each `cN_*` test corresponds to one of Copilot's claims and either
//! demonstrates the misbehaviour Copilot described (so we have a failing
//! test to drive a fix) or proves the current code already handles the
//! case (so the comment is stale).

#![cfg(feature = "python-pydantic")]

use std::cell::RefCell;
use std::rc::Rc;

use bo4e_codegen::{Options, python::pydantic};
use bo4e_schemas::{Schema, Schemas};

fn schemas_with(schemas_list: Vec<(Vec<String>, &str)>) -> Schemas {
    let version: bo4e_schemas::Version = "v202501.0.0".parse().unwrap();
    let mut s = Schemas::new(version.into());
    for (module, body) in schemas_list {
        let mut sch = Schema::new(module, None).unwrap();
        sch.load_schema(body.to_string());
        s.add_schema(Rc::new(RefCell::new(sch))).unwrap();
    }
    s
}

// ─── C3: identifier-collision detection for `_id`/`id` and `type`/`type_` ────
//
// Copilot's claim: "Distinct JSON keys such as `_id` and `id` (or `type`
// and `type_`) both become the same Rust/Python field identifier, which
// would generate duplicate fields or helper functions. Please add a
// schema-level uniqueness check for the generated identifiers, not just
// the raw property-name shape."
//
// `detect_identifier_collisions` (added in bfb4842) normalises every JSON
// property name to `snake_case(strip_leading_underscore(name))` and
// additionally reserves the `<form>_` keyword-escape slot when the form
// is reserved in either language. Both pairs Copilot named land on the
// same key, so the validator must reject them.

fn assert_collision(schema_body: &str, prop_a: &str, prop_b: &str) {
    let schemas = schemas_with(vec![(vec!["bo".into(), "Collide".into()], schema_body)]);
    let tmp = tempfile::tempdir().unwrap();
    let err = pydantic::generate(
        &schemas,
        tmp.path(),
        &Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect_err("collision must be rejected at validate");
    let msg = format!("{err}");
    assert!(
        msg.contains("normalizes to identifier") && msg.contains("collides with"),
        "expected identifier-collision error for `{prop_a}` vs `{prop_b}`, got:\n{msg}"
    );
}

#[test]
fn c3_underscore_id_collides_with_plain_id() {
    // `_id` → strip → `id` (snake) — reserves `id` and `id_`.
    // `id`  → snake → `id` — collides on the `id` slot.
    let body = r#"{
        "type":"object",
        "required":["_id","id"],
        "properties":{
            "_id":{"type":"string"},
            "id":{"type":"string"}
        }
    }"#;
    assert_collision(body, "_id", "id");
}

#[test]
fn c3_type_collides_with_type_underscore() {
    // `type`  is reserved → forms `["type","type_"]`.
    // `type_` → forms `["type_"]` — collides on `type_`.
    let body = r#"{
        "type":"object",
        "required":["type","type_"],
        "properties":{
            "type":{"type":"string"},
            "type_":{"type":"string"}
        }
    }"#;
    assert_collision(body, "type", "type_");
}

#[test]
fn c3_distinct_snake_forms_do_not_collide() {
    // Sanity guard: two properties whose normalised forms genuinely differ
    // must pass — the collision check must not be over-eager.
    let body = r#"{
        "type":"object",
        "required":["fooBar","baz"],
        "properties":{
            "fooBar":{"type":"string"},
            "baz":{"type":"string"}
        }
    }"#;
    let schemas = schemas_with(vec![(vec!["bo".into(), "Holder".into()], body)]);
    let tmp = tempfile::tempdir().unwrap();
    pydantic::generate(
        &schemas,
        tmp.path(),
        &Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("distinct identifiers must not collide");
}

// ─── C2: pydantic `qualify_enum_default` with quote in member value ──────────
//
// Copilot's claim: "values containing quotes or backslashes are sanitized
// differently from the enum declaration ... the generated default can
// reference a non-existent enum member and make the module fail at import time."
//
// With the fix in bfb4842 the raw schema value drives `sanitize_member_name`,
// so the enum class and the default expression agree even when the value
// contains a quote. Asserts the produced default is exactly the sanitised
// form of `A"B` → `A_B`, and NOT the buggy double-underscore variant.
#[test]
fn c2_enum_default_with_embedded_quote_resolves_to_real_member() {
    let enum_body = r#"{"type":"string","title":"Weird","enum":["A\"B","NORMAL"]}"#;
    let object_body = r#"{
        "type":"object",
        "title":"Holder",
        "properties":{
            "weird":{"default":"A\"B","$ref":"../enum/Weird.json#"}
        }
    }"#;
    let schemas = schemas_with(vec![
        (vec!["enum".into(), "Weird".into()], enum_body),
        (vec!["bo".into(), "Holder".into()], object_body),
    ]);

    let tmp = tempfile::tempdir().unwrap();
    pydantic::generate(
        &schemas,
        tmp.path(),
        &Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("generate");

    let weird_src = std::fs::read_to_string(tmp.path().join("enum/weird.py")).unwrap();
    let holder_src = std::fs::read_to_string(tmp.path().join("bo/holder.py")).unwrap();

    // Enum class uses `A_B` as the member name.
    assert!(
        weird_src.contains("A_B = \"A\\\"B\""),
        "expected `A_B = \"A\\\"B\"` in enum, got:\n{weird_src}"
    );

    // Default expression on Holder.weird must point at `Weird.A_B`, not
    // the double-underscore form the bug would have produced.
    assert!(
        holder_src.contains("Weird.A_B"),
        "default must qualify to Weird.A_B (matching enum member), got:\n{holder_src}"
    );
    assert!(
        !holder_src.contains("Weird.A__B"),
        "default must NOT qualify to Weird.A__B (the pre-fix bug shape), got:\n{holder_src}"
    );
}

// ─── C5: nullable AnyOf with single-member StrEnum classifies as scalar ──────
//
// Copilot's claim: "A nullable single-member enum (`anyOf: [StrEnum, null]`)
// therefore falls through to `classify_optional`, which has no `StrEnum`
// arm and returns `UnclassifiableProperty`."
//
// Since bfb4842, `is_simple_scalar`'s AnyOf arm explicitly lists `StrEnum`,
// so the property reaches `simple_scalar_field` instead. Verified
// end-to-end by generating the SQL plan and asserting the field is
// present (not rejected) and lands as a synthetic-enum scalar.
#[cfg(feature = "python-sql-model")]
#[test]
fn c5_nullable_anyof_single_member_strenum_classifies_as_simple_scalar() {
    let body = r#"{
        "type":"object",
        "title":"Angebot",
        "properties":{
            "_typ":{
                "default":"ANGEBOT",
                "anyOf":[
                    {"type":"string","enum":["ANGEBOT"]},
                    {"type":"null"}
                ]
            }
        }
    }"#;
    let schemas = schemas_with(vec![(vec!["bo".into(), "Angebot".into()], body)]);
    let tmp = tempfile::tempdir().unwrap();
    bo4e_codegen::python::sql_model::generate(
        &schemas,
        tmp.path(),
        &Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("nullable anyOf StrEnum must not be UnclassifiableProperty");

    let src = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();
    assert!(
        src.contains("class AngebotTyp(StrEnum):"),
        "synthetic enum class missing, got:\n{src}"
    );
    assert!(
        src.contains("typ: AngebotTyp | None"),
        "field must use synthetic class with `| None`, got:\n{src}"
    );
}

// ─── C6: SQLModel column annotations do NOT contain `Literal[...]` ───────────
//
// Copilot's claim: "Inline `const` / single-member `StrEnum` fields now
// become `Literal[...]` columns. SQLModel ... can fail during class creation."
//
// Since bfb4842, `narrow_literal_to_synthetic` rewrites `Literal["X"]`
// annotations to the synthetic class name BEFORE the field lands in the
// plan. Asserts the generated source contains zero `Literal[` occurrences
// in the column-annotation area.
#[cfg(feature = "python-sql-model")]
#[test]
fn c6_sql_model_column_annotations_have_no_literal() {
    let body = r#"{
        "type":"object",
        "title":"Angebot",
        "properties":{
            "_typ":{"const":"ANGEBOT","default":"ANGEBOT","enum":["ANGEBOT"],"type":"string"}
        }
    }"#;
    let schemas = schemas_with(vec![(vec!["bo".into(), "Angebot".into()], body)]);
    let tmp = tempfile::tempdir().unwrap();
    bo4e_codegen::python::sql_model::generate(
        &schemas,
        tmp.path(),
        &Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("generate");
    let src = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();
    assert!(
        !src.contains("Literal["),
        "SQLModel output must not carry Literal[...] annotations (SQLModel chokes on them), got:\n{src}"
    );
    assert!(
        src.contains("typ: AngebotTyp = Field(alias=\"_typ\", default=AngebotTyp.ANGEBOT)"),
        "field must reference synthetic class, got:\n{src}"
    );
    // typing.Literal import must also be dropped — otherwise the file
    // imports an unused name and tools that lint imports complain.
    assert!(
        !src.contains("from typing import Literal"),
        "typing.Literal import must be dropped, got:\n{src}"
    );
}

// ─── C9 (suppressed-low-confidence): inline enum/const default-value check ───
//
// Copilot's suppressed claim: "Inline `StrEnum` and `ConstantSchema`
// defaults are only checked to be strings, not checked against the
// declared enum members/const value. A schema like
// `{const: "A", default: "B"}` will pass validation."
//
// `check_inline_enum_const_default` (in `validate.rs`) DOES recurse into
// const/StrEnum schemas and reject mismatched defaults. Tests both
// inline const and inline single-member StrEnum mismatch cases, plus
// a sanity case where the default does match.

fn assert_validation_rejects(body: &str, must_contain: &str) {
    let schemas = schemas_with(vec![(vec!["bo".into(), "Holder".into()], body)]);
    let tmp = tempfile::tempdir().unwrap();
    let err = pydantic::generate(
        &schemas,
        tmp.path(),
        &Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect_err("validator must reject");
    let msg = format!("{err}");
    assert!(
        msg.contains(must_contain),
        "expected error containing `{must_contain}`, got:\n{msg}"
    );
}

#[test]
fn c9_inline_const_default_mismatch_is_rejected() {
    // `{const:"A", default:"B"}` — Copilot's exact example.
    let body = r#"{
        "type":"object",
        "properties":{
            "weird":{"const":"A","default":"B","type":"string"}
        }
    }"#;
    assert_validation_rejects(body, "does not match inline const value");
}

#[test]
fn c9_inline_strenum_default_not_in_members_is_rejected() {
    // {type:"string", enum:["A","B"], default:"C"} — single-member or
    // multi-member StrEnum, the validator must reject defaults outside
    // the declared `enum` list.
    let body = r#"{
        "type":"object",
        "properties":{
            "weird":{"type":"string","enum":["A","B"],"default":"C"}
        }
    }"#;
    assert_validation_rejects(body, "is not a member of inline string enum");
}

#[test]
fn c9_inline_const_matching_default_passes() {
    let body = r#"{
        "type":"object",
        "properties":{
            "weird":{"const":"A","default":"A","type":"string","enum":["A"]}
        }
    }"#;
    let schemas = schemas_with(vec![(vec!["bo".into(), "Holder".into()], body)]);
    let tmp = tempfile::tempdir().unwrap();
    pydantic::generate(
        &schemas,
        tmp.path(),
        &Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("matching default must pass validation");
}
