use crate::error::Error;
use std::path::Path;

pub(crate) fn make_environment(
    templates_dir: Option<&Path>,
) -> Result<minijinja::Environment<'static>, Error> {
    let mut env = minijinja::Environment::new();
    if let Some(dir) = templates_dir {
        env.set_loader(minijinja::path_loader(dir));
    } else {
        load_embedded(&mut env)?;
    }
    // Support Jinja2-style `.items()` / `.dict()` method calls on map values,
    // which the vendored templates use (e.g. `SQL.fields.items()`).
    env.set_unknown_method_callback(|state, value, method, args| {
        use minijinja::value::{ValueKind, from_args};
        use minijinja::{Error as MjError, ErrorKind};
        match (value.kind(), method) {
            (ValueKind::Map, "items" | "dict") => {
                let _: () = from_args(args)?;
                state.apply_filter("items", std::slice::from_ref(value))
            }
            _ => Err(MjError::from(ErrorKind::UnknownMethod)),
        }
    });
    Ok(env)
}

#[allow(unused_variables)]
fn load_embedded(env: &mut minijinja::Environment<'static>) -> Result<(), Error> {
    #[cfg(feature = "python-pydantic")]
    {
        env.add_template(
            "python/pydantic/BaseModel.jinja2",
            include_str!("templates/python/pydantic/BaseModel.jinja2"),
        )?;
        env.add_template(
            "python/pydantic/Enum.jinja2",
            include_str!("templates/python/pydantic/Enum.jinja2"),
        )?;
        env.add_template(
            "python/pydantic/__init__.jinja2",
            include_str!("templates/python/pydantic/__init__.jinja2"),
        )?;
    }

    #[cfg(feature = "python-sql-model")]
    {
        env.add_template(
            "python/sql_model/BaseModel.jinja2",
            include_str!("templates/python/sql_model/BaseModel.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/Config.jinja2",
            include_str!("templates/python/sql_model/Config.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/Enum.jinja2",
            include_str!("templates/python/sql_model/Enum.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/ManyLinks.jinja2",
            include_str!("templates/python/sql_model/ManyLinks.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/__init__.jinja2",
            include_str!("templates/python/sql_model/__init__.jinja2"),
        )?;
    }

    #[cfg(feature = "rust-plain")]
    {
        env.add_template(
            "rust/plain/Struct.jinja2",
            include_str!("templates/rust/plain/Struct.jinja2"),
        )?;
        env.add_template(
            "rust/plain/Enum.jinja2",
            include_str!("templates/rust/plain/Enum.jinja2"),
        )?;
        env.add_template(
            "rust/plain/DefaultImpl.jinja2",
            include_str!("templates/rust/plain/DefaultImpl.jinja2"),
        )?;
        env.add_template(
            "rust/plain/ModRs.jinja2",
            include_str!("templates/rust/plain/ModRs.jinja2"),
        )?;
        env.add_template(
            "rust/plain/RootModRs.jinja2",
            include_str!("templates/rust/plain/RootModRs.jinja2"),
        )?;
    }

    #[cfg(feature = "rust-crate")]
    {
        env.add_template(
            "rust/crate_/CargoToml.jinja2",
            include_str!("templates/rust/crate_/CargoToml.jinja2"),
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    #[cfg(feature = "python-pydantic")]
    #[test]
    fn embedded_pydantic_init_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("python/pydantic/__init__.jinja2")
            .expect("template registered");
        let out = tpl
            .render(context! {
                classes => vec![
                    context!{ name => "Angebot", module_path => vec!["bo", "angebot"] }
                ],
            })
            .unwrap();
        assert!(out.contains("from .bo.angebot import Angebot"));
    }

    #[test]
    fn disk_loader_loads_templates_from_supplied_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("python/pydantic");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("Hello.jinja2"), "Hello, {{ name }}!").unwrap();

        let env = make_environment(Some(dir.path())).unwrap();
        let tpl = env.get_template("python/pydantic/Hello.jinja2").unwrap();
        let out = tpl.render(context! { name => "Welt" }).unwrap();
        assert_eq!(out, "Hello, Welt!");
    }

    #[cfg(feature = "python-sql-model")]
    #[test]
    fn embedded_sql_model_many_links_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("python/sql_model/ManyLinks.jinja2")
            .unwrap();
        let out = tpl
            .render(context! {
                links => vec![context! {
                    table_name => "AngebotAdressenLink",
                    cls1 => "Angebot",
                    cls2 => "Adresse",
                    rel_field_name1 => "adressen",
                    id_field_name1 => "angebot_id",
                    id_field_name2 => "adresse_id",
                }]
            })
            .unwrap();
        assert!(
            out.contains("class AngebotAdressenLink(SQLModel, table=True):"),
            "got: {out}"
        );
        assert!(out.contains("angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"angebot.id\", ondelete=\"CASCADE\")"), "got: {out}");
        assert!(out.contains("adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"adresse.id\", ondelete=\"CASCADE\")"), "got: {out}");
    }

    #[cfg(feature = "python-sql-model")]
    #[test]
    fn embedded_sql_model_init_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("python/sql_model/__init__.jinja2")
            .unwrap();
        let out = tpl.render(context! {
            classes => vec![context!{ name => "Angebot", module_path => vec!["bo", "angebot"] }],
            links => vec!["AngebotAdressenLink"],
            all_names => vec!["Angebot", "AngebotAdressenLink"],
        }).unwrap();
        assert!(out.contains("from .bo.angebot import Angebot"));
        assert!(out.contains("from .many import AngebotAdressenLink"));
        assert!(out.contains("__all__ = ["));
        assert!(out.contains("\"Angebot\","));
    }

    #[cfg(feature = "rust-plain")]
    #[test]
    fn embedded_rust_plain_struct_template_loads() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("rust/plain/Struct.jinja2")
            .expect("template registered");
        let out = tpl
            .render(context! {
                uses => "use serde::{Deserialize, Serialize};",
                extra_enums => Vec::<String>::new(),
                doc => "/// docstring",
                class_name => "Foo",
                fields => vec![context!{
                    name => "id",
                    type_hint => "Option<String>",
                    serde_attrs => "rename = \"_id\", default, skip_serializing_if = \"Option::is_none\"",
                    doc => "/// id docstring"
                }],
                default_impl => "",
            })
            .unwrap();
        assert!(out.contains("pub struct Foo"));
        assert!(out.contains("pub id: Option<String>"));
        assert!(out.contains("rename = \"_id\""));
    }

    #[cfg(feature = "rust-plain")]
    #[test]
    fn embedded_rust_plain_enum_template_loads() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("rust/plain/Enum.jinja2")
            .expect("template registered");

        // Single-variant discriminator shape: `Copy`/`Default` derives + `#[default]`.
        let single = tpl
            .render(context! {
                doc => "/// Angebot discriminator",
                single_variant => true,
                class_name => "AngebotTyp",
                variants => vec![context!{ wire_quoted => "\"ANGEBOT\"", name => "Angebot" }],
            })
            .unwrap();
        assert!(single.contains("pub enum AngebotTyp"));
        assert!(single.contains("Copy"));
        assert!(single.contains("Default"));
        assert!(single.contains("#[default]"));
        assert!(single.contains("#[serde(rename = \"ANGEBOT\")]"));

        // Multi-variant str-enum shape: no `Copy`/`Default` derives, no `#[default]`.
        let multi = tpl
            .render(context! {
                doc => "",
                single_variant => false,
                class_name => "Typ",
                variants => vec![
                    context!{ wire_quoted => "\"A\"", name => "A" },
                    context!{ wire_quoted => "\"B\"", name => "B" },
                ],
            })
            .unwrap();
        assert!(multi.contains("pub enum Typ"));
        assert!(!multi.contains("Copy"));
        assert!(!multi.contains("#[default]"));
    }

    #[cfg(feature = "rust-plain")]
    #[test]
    fn embedded_rust_plain_default_impl_template_loads() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("rust/plain/DefaultImpl.jinja2")
            .expect("template registered");

        // Emitted variant: full `impl Default` block.
        let emitted = tpl
            .render(context! {
                class_name => "Angebot",
                missing => Vec::<&str>::new(),
                fields => vec![context!{ name => "id", expr => "None" }],
            })
            .unwrap();
        assert!(emitted.contains("impl Default for Angebot"));
        assert!(emitted.contains("id: None,"));

        // Skipped variant, single missing field — uses singular grammar.
        let skipped_one = tpl
            .render(context! {
                class_name => "Angebot",
                missing => vec!["bad_field"],
                fields => Vec::<minijinja::Value>::new(),
            })
            .unwrap();
        assert!(skipped_one.contains("Default impl omitted"));
        assert!(skipped_one.contains("field `bad_field` has no"));
        assert!(!skipped_one.contains("impl Default for"));

        // Skipped variant, multiple missing fields — uses plural grammar.
        let skipped_many = tpl
            .render(context! {
                class_name => "Angebot",
                missing => vec!["anhaenge", "werte"],
                fields => Vec::<minijinja::Value>::new(),
            })
            .unwrap();
        assert!(skipped_many.contains("fields `anhaenge`, `werte` have no"));
    }

    #[cfg(feature = "rust-crate")]
    #[test]
    fn embedded_rust_crate_cargo_toml_template_loads() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("rust/crate_/CargoToml.jinja2")
            .expect("template registered");
        let out = tpl
            .render(context! {
                crate_name => "my_bo4e",
                semver => "202401.4.0",
                bo4e_version => "v202401.4.0+gabc1234",
            })
            .unwrap();
        assert!(out.contains("name = \"my_bo4e\""));
        assert!(out.contains("version = \"202401.4.0\""));
        // The bo4e_version goes in the description with its full original shape.
        assert!(out.contains("version v202401.4.0+gabc1234"));
        // Critical dependencies for the generated crate are present.
        assert!(out.contains("serde = "));
        assert!(out.contains("chrono = "));
    }
}
