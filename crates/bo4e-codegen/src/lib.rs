mod env;
mod error;
pub mod naming;
mod output_type;

#[cfg(any(
    feature = "python-pydantic-v1",
    feature = "python-pydantic-v2",
    feature = "python-sql-model",
))]
mod python;

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
        not(any(
            feature = "python-pydantic-v1",
            feature = "python-pydantic-v2",
            feature = "python-sql-model",
        )),
        allow(unused_variables)
    )]
    schemas: &Schemas,
    output_type: OutputType,
    output_dir: &Path,
    options: &Options,
) -> Result<(), Error> {
    if options.clear_output {
        clear_dir_if_exists(output_dir)?;
    } else {
        std::fs::create_dir_all(output_dir)?;
    }

    #[allow(unused_variables)]
    let env = env::make_environment(options.templates_dir)?;

    #[allow(unreachable_patterns)]
    match output_type {
        #[cfg(feature = "python-pydantic-v2")]
        OutputType::PythonPydanticV2 => {
            python::pydantic_v2::generate_pydantic_v2(schemas, output_dir, &env)?;
            Ok(())
        }
        #[cfg(feature = "python-pydantic-v1")]
        OutputType::PythonPydanticV1 => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
        #[cfg(feature = "python-sql-model")]
        OutputType::PythonSqlModel => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
        // When all python features are compiled out, OutputType has no variants and
        // this match has no arms; the wildcard keeps the code well-formed.
        _ => unreachable!("OutputType variant not handled"),
    }
}

fn clear_dir_if_exists(dir: &Path) -> Result<(), Error> {
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
