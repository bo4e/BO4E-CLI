use clap::ValueEnum;

/// Which output type to generate. Variants are gated by Cargo features —
/// a feature compiled out removes its variant entirely so the CLI's clap
/// parser only accepts compiled-in values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum OutputType {
    #[cfg(feature = "python-pydantic-v2")]
    #[value(name = "python-pydantic-v2")]
    PythonPydanticV2,
    #[cfg(feature = "python-sql-model")]
    #[value(name = "python-sql-model")]
    PythonSqlModel,
}

impl OutputType {
    pub fn as_str(&self) -> &'static str {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(feature = "python-pydantic-v2")]
            Self::PythonPydanticV2 => "python-pydantic-v2",
            #[cfg(feature = "python-sql-model")]
            Self::PythonSqlModel => "python-sql-model",
            // Safety: when no features are compiled in, OutputType has no variants and this
            // branch is unreachable. When at least one feature is enabled, the arms above
            // cover every variant exhaustively.
            _ => unreachable!("OutputType variant not handled"),
        }
    }
}
