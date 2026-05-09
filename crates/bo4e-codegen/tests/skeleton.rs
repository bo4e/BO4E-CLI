use std::path::PathBuf;

#[test]
fn generate_with_compiled_out_variant_returns_specific_error() {
    // We intentionally call with no schemas in a temp dir.
    let tmp = tempfile::tempdir().unwrap();
    let schemas = bo4e_schemas::Schemas::new("v202401.0.0".parse().unwrap());

    // The variant we pass MUST be one that is compiled in (otherwise the cfg-gate
    // strips it from the enum). pydantic-v2 is compiled in by default.
    let out = bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: false,
            templates_dir: None,
        },
    );

    // Skeleton stage: every variant returns OutputTypeNotCompiledIn until Task 8 wires v2.
    assert!(matches!(out, Err(bo4e_codegen::Error::OutputTypeNotCompiledIn(_))));
}
