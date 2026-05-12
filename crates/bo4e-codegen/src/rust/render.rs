//! Pure render helpers consumed by `rust::plain` and `rust::crate_` orchestrators.

use crate::imports::Import;
use crate::naming::{sanitize_member_name, to_pascal_case};
use crate::rust::imports::UseBlock;

/// Render a docstring block as outer `///` lines. Empty input → empty string.
/// Preserves embedded line breaks verbatim — Sphinx RST is not stripped.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_doc_comment(description: Option<&str>, indent: &str) -> String {
    let Some(text) = description.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    text.lines()
        .map(|line| format!("{indent}/// {line}").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render a module-level `//!` docstring. Empty input → empty string.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_module_doc(description: Option<&str>) -> String {
    let Some(text) = description.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    text.lines()
        .map(|line| format!("//! {line}").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the single-variant enum that types a const-valued property.
/// `class_name` = e.g. `"AngebotTyp"`; `wire_value` = the JSON literal, e.g. `"ANGEBOT"`.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_single_variant_enum(
    class_name: &str,
    wire_value: &str,
    docstring: Option<&str>,
) -> String {
    let variant_ident = to_pascal_case(&sanitize_member_name(wire_value));
    let doc = render_doc_comment(docstring, "");
    let mut out = String::new();
    if !doc.is_empty() {
        out.push_str(&doc);
        out.push('\n');
    }
    out.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]\n",
    );
    out.push_str(&format!("pub enum {class_name} {{\n"));
    out.push_str("    #[default]\n");
    out.push_str(&format!("    #[serde(rename = \"{wire_value}\")]\n"));
    out.push_str(&format!("    {variant_ident},\n"));
    out.push_str("}\n");
    out
}

/// Render the plain string-enum form (`enum Typ { Angebot, Ausschreibung, … }`).
#[allow(dead_code)] // consumed by rust::plain orchestrator in Task 22
pub(crate) fn render_str_enum(
    class_name: &str,
    members: &[String],
    docstring: Option<&str>,
) -> String {
    let doc = render_doc_comment(docstring, "");
    let mut out = String::new();
    if !doc.is_empty() {
        out.push_str(&doc);
        out.push('\n');
    }
    out.push_str("#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]\n");
    out.push_str(&format!("pub enum {class_name} {{\n"));
    for m in members {
        let variant = to_pascal_case(&sanitize_member_name(m));
        out.push_str(&format!("    #[serde(rename = \"{m}\")]\n"));
        out.push_str(&format!("    {variant},\n"));
    }
    out.push_str("}\n");
    out
}

/// Render the `use` block for a file at module depth `depth`.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_use_block(imports: impl IntoIterator<Item = Import>, depth: usize) -> String {
    let mut b = UseBlock::new();
    b.extend(imports);
    b.render(depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_comment_single_line() {
        let s = render_doc_comment(Some("Hello world"), "");
        assert_eq!(s, "/// Hello world");
    }

    #[test]
    fn doc_comment_multi_line_preserves_breaks() {
        let s = render_doc_comment(Some("Line one\nLine two"), "");
        assert_eq!(s, "/// Line one\n/// Line two");
    }

    #[test]
    fn doc_comment_empty_returns_empty() {
        assert_eq!(render_doc_comment(None, ""), "");
        assert_eq!(render_doc_comment(Some(""), ""), "");
        assert_eq!(render_doc_comment(Some("   "), ""), "");
    }

    #[test]
    fn doc_comment_indent_applied() {
        let s = render_doc_comment(Some("Hi"), "    ");
        assert_eq!(s, "    /// Hi");
    }

    #[test]
    fn module_doc_renders_bangs() {
        let s = render_module_doc(Some("First\nSecond"));
        assert_eq!(s, "//! First\n//! Second");
    }

    #[test]
    fn single_variant_enum_shape() {
        let s = render_single_variant_enum("AngebotTyp", "ANGEBOT", Some("Angebot discriminator"));
        assert!(s.contains("/// Angebot discriminator"));
        assert!(s.contains("pub enum AngebotTyp"));
        assert!(s.contains("#[default]"));
        assert!(s.contains("#[serde(rename = \"ANGEBOT\")]"));
        assert!(s.contains("Angebot,"));
    }

    #[test]
    fn str_enum_one_variant_per_member() {
        let members = vec!["ANGEBOT".to_string(), "AUSSCHREIBUNG".to_string()];
        let s = render_str_enum("Typ", &members, None);
        assert!(s.contains("pub enum Typ"));
        assert!(s.contains("#[serde(rename = \"ANGEBOT\")]"));
        assert!(s.contains("Angebot,"));
        assert!(s.contains("#[serde(rename = \"AUSSCHREIBUNG\")]"));
        assert!(s.contains("Ausschreibung,"));
    }

    #[test]
    fn str_enum_handles_hyphenated_member() {
        let members = vec!["2-01-7-001".to_string()];
        let s = render_str_enum("Code", &members, None);
        assert!(s.contains("#[serde(rename = \"2-01-7-001\")]"));
        assert!(s.contains("_2_01_7_001,"));
    }

    #[test]
    fn use_block_round_trip() {
        let imports = vec![
            Import::Named {
                module: "serde".into(),
                name: "Serialize".into(),
            },
            Import::Sibling {
                module: vec!["com".into(), "Adresse".into()],
                name: "Adresse".into(),
            },
        ];
        let s = render_use_block(imports, 2);
        assert!(s.contains("use serde::Serialize;"));
        assert!(s.contains("use super::super::com::adresse::Adresse;"));
    }
}
