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
    #[cfg(feature = "python-pydantic-v2")]
    {
        env.add_template(
            "python/pydantic_v2/BaseModel.jinja2",
            include_str!("templates/python/pydantic_v2/BaseModel.jinja2"),
        )?;
        env.add_template(
            "python/pydantic_v2/Enum.jinja2",
            include_str!("templates/python/pydantic_v2/Enum.jinja2"),
        )?;
        env.add_template(
            "python/pydantic_v2/__init__.jinja2",
            include_str!("templates/python/pydantic_v2/__init__.jinja2"),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    #[cfg(feature = "python-pydantic-v2")]
    #[test]
    fn embedded_pydantic_v2_init_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env
            .get_template("python/pydantic_v2/__init__.jinja2")
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
        let sub = dir.path().join("python/pydantic_v2");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("Hello.jinja2"), "Hello, {{ name }}!").unwrap();

        let env = make_environment(Some(dir.path())).unwrap();
        let tpl = env.get_template("python/pydantic_v2/Hello.jinja2").unwrap();
        let out = tpl.render(context! { name => "Welt" }).unwrap();
        assert_eq!(out, "Hello, Welt!");
    }
}
