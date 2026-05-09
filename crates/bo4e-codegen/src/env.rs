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
                ]
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
        let tpl = env.get_template("python/sql_model/ManyLinks.jinja2").unwrap();
        let out = tpl.render(context! {
            links => vec![context! {
                table_name => "AngebotAdressenLink",
                cls1 => "Angebot",
                cls2 => "Adresse",
                rel_field_name1 => "adressen",
                id_field_name1 => "angebot_id",
                id_field_name2 => "adresse_id",
            }]
        }).unwrap();
        assert!(out.contains("class AngebotAdressenLink(SQLModel, table=True):"), "got: {out}");
        assert!(out.contains("angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"angebot.id\", ondelete=\"CASCADE\")"), "got: {out}");
        assert!(out.contains("adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"adresse.id\", ondelete=\"CASCADE\")"), "got: {out}");
    }

    #[cfg(feature = "python-sql-model")]
    #[test]
    fn embedded_sql_model_init_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env.get_template("python/sql_model/__init__.jinja2").unwrap();
        let out = tpl.render(context! {
            classes => vec![context!{ name => "Angebot", module_path => vec!["bo", "angebot"] }],
            links => vec!["AngebotAdressenLink"],
        }).unwrap();
        assert!(out.contains("from .bo.angebot import Angebot"));
        assert!(out.contains("from .many import AngebotAdressenLink"));
    }
}
