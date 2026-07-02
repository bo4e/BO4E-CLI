use crate::models::graph::{Cardinality, Field};

pub fn format_cardinality(c: &Cardinality) -> String {
    if c.min == c.max {
        c.min.clone()
    } else {
        format!("{}..{}", c.min, c.max)
    }
}

pub fn dotted(path: &[String]) -> String {
    path.join(".")
}

/// True iff `field` is a ref-field that already has an outgoing edge — used by
/// emitters to skip the in-class-box rendering.
pub fn skip_inline_for_reference(field: &Field) -> bool {
    field.is_reference
}

/// Canonical BO4E palette. Same colours used by `emit_plantuml::emit` for
/// namespace blocks and by `emit_dot::emit` for HTML-label backgrounds, so
/// the overview and per-class diagrams visually agree on package identity.
///
/// Each entry has three shades — the main `header` colour, a `lighter` tone
/// (~50 % mix with white) used for field/enum-detail rows so they read as
/// secondary, and a `darker` tone (~×0.6) used for the node's border so
/// edges visually anchor at the node outline.
pub const COLOUR_BO: &str = "#B6D7A8";
pub const COLOUR_COM: &str = "#E0A86C";
pub const COLOUR_ENUM: &str = "#d1c358";
/// Fallback header colour for class nodes outside a known package — currently
/// only `ZusatzAttribut`, which sits at the schema root. A muted light grey
/// keeps it visually neutral against the green/orange/yellow palette.
pub const COLOUR_DEFAULT: &str = "#D9D9D9";

pub fn pkg_color(pkg: &str) -> &'static str {
    match pkg {
        "bo" => COLOUR_BO,
        "com" => COLOUR_COM,
        "enum" => COLOUR_ENUM,
        _ => COLOUR_DEFAULT,
    }
}

pub fn pkg_color_lighter(pkg: &str) -> &'static str {
    match pkg {
        "bo" => "#DBEBD4",
        "com" => "#EFD3B5",
        "enum" => "#E8E1AB",
        _ => "#EAEAEA",
    }
}

pub fn pkg_color_darker(pkg: &str) -> &'static str {
    match pkg {
        "bo" => "#6D8164",
        "com" => "#866440",
        "enum" => "#7D7534",
        _ => "#8C8C8C",
    }
}

/// Escape a string for inclusion in a Graphviz HTML-like label
/// (`<<TABLE>…</TABLE>>`). The set of characters that need entity-encoding is
/// the same as plain XML.
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_cardinality_compacts_equal_bounds() {
        assert_eq!(
            format_cardinality(&Cardinality {
                min: "1".into(),
                max: "1".into()
            }),
            "1"
        );
        assert_eq!(
            format_cardinality(&Cardinality {
                min: "0".into(),
                max: "*".into()
            }),
            "0..*"
        );
    }
}
