mod env;
mod error;
pub mod imports;
pub mod layout;
pub mod naming;
mod output_type;
pub mod refs;

#[cfg(any(feature = "python-pydantic", feature = "python-sql-model",))]
pub mod python;

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
    #[cfg_attr(
        not(any(feature = "python-pydantic", feature = "python-sql-model",)),
        allow(unused_variables)
    )]
    schemas: &Schemas,
    output_type: OutputType,
    output_dir: &Path,
    options: &Options,
) -> Result<Vec<std::path::PathBuf>, Error> {
    #[allow(unreachable_patterns)]
    match output_type {
        #[cfg(feature = "python-pydantic")]
        OutputType::PythonPydantic => python::pydantic::generate(schemas, output_dir, options),
        #[cfg(feature = "python-sql-model")]
        OutputType::PythonSqlModel => python::sql_model::generate(schemas, output_dir, options),
        _ => unreachable!("OutputType variant not handled"),
    }
}

pub(crate) fn clear_dir_if_exists(dir: &Path) -> Result<(), Error> {
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
    } else {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
