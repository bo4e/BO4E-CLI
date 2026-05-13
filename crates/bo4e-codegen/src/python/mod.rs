use crate::error::Error;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) mod imports;
pub(crate) mod types;

#[cfg(feature = "python-pydantic")]
pub mod pydantic;

#[cfg(feature = "python-sql-model")]
pub mod sql_model;

/// Render the BO4E module-level docstring used by every generator's root `__init__.py`.
/// One source of truth so pydantic, sql-model, and any future Python output stay in sync.
pub(crate) fn root_init_module_docstring(version: &str) -> String {
    format!(
        "\"\"\"\nBO4E {version} - Generated Python implementation of the BO4E standard\n\n\
         BO4E is a standard for the exchange of business objects in the energy industry.\n\
         All our software used to generate this BO4E-implementation is open-source and \
         released under the Apache-2.0 license.\n\n\
         The BO4E version can be queried using `bo4e.__version__`.\n\"\"\"\n"
    )
}

/// Python keywords + common builtins whose names a model attribute must not shadow.
/// Used by [`python_attr_name`] when stripping a leading underscore exposes a clash
/// (e.g. JSON `_id` → would-be Python `id`, which shadows the `id()` builtin).
pub(crate) const PYTHON_RESERVED: &[&str] = &[
    // keywords
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield", // builtin shadows we care about
    "id", "type", "list", "dict", "set", "tuple", "str", "int", "float", "bool", "bytes", "object",
    "input", "print", "open", "range", "iter", "next", "len", "min", "max", "sum", "any", "all",
    "map", "filter",
];

/// Translate a snake-case JSON property name into a valid Pydantic model attribute.
///
/// Pydantic v2 forbids leading-underscore field names. BO4E uses `_id`, `_typ`,
/// `_version` for discriminator/identity slots; we strip the leading `_` and append
/// a trailing `_` if that exposes a Python keyword/builtin clash.
///
/// The caller is responsible for emitting an explicit `Field(alias=...)` whenever
/// the returned name differs from the original JSON name.
pub(crate) fn python_attr_name(snake: &str) -> String {
    let stripped = snake.strip_prefix('_').unwrap_or(snake);
    if PYTHON_RESERVED.contains(&stripped) {
        format!("{stripped}_")
    } else {
        stripped.to_string()
    }
}

/// Write an empty `__init__.py` to each first-level subdirectory listed in `subdirs`,
/// skipping any that already exist. Pushes resulting paths onto `written`.
pub(crate) fn write_empty_subdir_inits(
    output_dir: &Path,
    subdirs: &BTreeSet<String>,
    written: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    for sub in subdirs {
        let p = output_dir.join(sub).join("__init__.py");
        if !p.exists() {
            std::fs::write(&p, "")?;
            written.push(p);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_init_module_docstring_interpolates_version() {
        let s = root_init_module_docstring("v202501.0.0");
        assert!(s.starts_with("\"\"\"\nBO4E v202501.0.0 - Generated"));
        assert!(s.contains("`bo4e.__version__`"));
        assert!(s.ends_with("\"\"\"\n"));
    }

    #[test]
    fn python_attr_name_strips_underscore_prefix() {
        assert_eq!(python_attr_name("_typ"), "typ");
        assert_eq!(python_attr_name("_version"), "version");
    }

    #[test]
    fn python_attr_name_appends_underscore_on_builtin_clash() {
        assert_eq!(python_attr_name("_id"), "id_");
        assert_eq!(python_attr_name("_type"), "type_");
        assert_eq!(python_attr_name("_class"), "class_");
    }

    #[test]
    fn python_attr_name_unchanged_when_no_underscore_prefix() {
        assert_eq!(python_attr_name("angebotsdatum"), "angebotsdatum");
        assert_eq!(python_attr_name("anfragereferenz"), "anfragereferenz");
    }
}
