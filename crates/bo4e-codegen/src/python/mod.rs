use std::path::Path;

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

/// Escape an arbitrary string into a Python double-quoted string literal.
///
/// Handles the cases that matter for generated source:
/// - `\` → `\\`, `"` → `\"`, `\n`/`\r`/`\t` → escaped forms.
/// - Control characters (`< 0x20`) → `\xHH`.
/// - Any other Unicode is passed through as a literal `char` (Python
///   source is UTF-8; pydantic / sql_model output files declare no
///   explicit encoding, so the system default is fine).
///
/// Symmetric with Rust's `format!("{s:?}")` Debug pattern used in
/// [`crate::rust::types::literal_default_rust`]; both flavours emit
/// safe string literals from arbitrary JSON-schema default values.
pub(crate) fn python_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

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

/// Stage an empty `__init__.py` for every non-root directory implied by
/// the supplied [`crate::layout::ModuleTree`]. Used to make nested
/// packages importable across arbitrary depth (`foo/bar/Baz.json`
/// produces `foo/__init__.py` and `foo/bar/__init__.py`).
///
/// Returns prepared `(path, body)` pairs — does no IO. The caller
/// merges these into its file buffer and commits via
/// [`crate::write_prepared`].
#[cfg(any(feature = "python-pydantic", feature = "python-sql-model"))]
pub(crate) fn prepare_empty_subdir_inits_recursive(
    output_dir: &Path,
    tree: &crate::layout::ModuleTree,
) -> Vec<crate::PreparedFile> {
    let mut out: Vec<crate::PreparedFile> = Vec::new();
    for (dir_path, _) in tree.iter() {
        if dir_path.is_empty() {
            continue; // root __init__.py is staged separately with re-exports
        }
        let mut p = output_dir.to_path_buf();
        for seg in dir_path {
            p.push(seg);
        }
        out.push((p.join("__init__.py"), String::new()));
    }
    out
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
    fn python_string_literal_escapes_quotes_and_backslashes() {
        assert_eq!(python_string_literal("hi"), "\"hi\"");
        assert_eq!(
            python_string_literal("He said \"X\""),
            "\"He said \\\"X\\\"\""
        );
        assert_eq!(python_string_literal("c:\\path"), "\"c:\\\\path\"");
        assert_eq!(python_string_literal("line1\nline2"), "\"line1\\nline2\"");
        // Control char (BEL = 0x07).
        assert_eq!(python_string_literal("\x07"), "\"\\x07\"");
    }

    #[test]
    fn python_attr_name_unchanged_when_no_underscore_prefix() {
        assert_eq!(python_attr_name("angebotsdatum"), "angebotsdatum");
        assert_eq!(python_attr_name("anfragereferenz"), "anfragereferenz");
    }
}
