use crate::error::Error;
use std::path::Path;

pub(crate) fn make_environment(
    _templates_dir: Option<&Path>,
) -> Result<minijinja::Environment<'static>, Error> {
    Ok(minijinja::Environment::new())
}
