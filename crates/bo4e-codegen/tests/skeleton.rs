#[cfg(feature = "python-pydantic")]
#[test]
fn generate_pydantic_writes_at_least_one_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut schemas = bo4e_schemas::Schemas::new("v202401.0.0".parse().unwrap());

    let mut s = bo4e_schemas::Schema::new(vec!["enum".into(), "Typ".into()], None).unwrap();
    s.load_schema(r#"{"type":"string","title":"Typ","enum":["A","B"]}"#.into());
    schemas
        .add_schema(std::rc::Rc::new(std::cell::RefCell::new(s)))
        .unwrap();

    bo4e_codegen::python::pydantic::generate(
        &schemas,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("generate");

    let typ_py = tmp.path().join("enum/typ.py");
    assert!(typ_py.exists(), "expected {:?} to exist", typ_py);
    let body = std::fs::read_to_string(&typ_py).unwrap();
    assert!(body.contains("class Typ"));
}
