mod env;
mod error;
mod output_type;

pub use error::Error;
pub use output_type::OutputType;

use bo4e_schemas::Schemas;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Options<'a> {
    pub clear_output: bool,
    pub templates_dir: Option<&'a Path>,
}

pub fn generate(
    _schemas: &Schemas,
    output_type: OutputType,
    _output_dir: &Path,
    options: &Options,
) -> Result<(), Error> {
    let _env = env::make_environment(options.templates_dir)?;
    Err(Error::OutputTypeNotCompiledIn(output_type.as_str()))
}
