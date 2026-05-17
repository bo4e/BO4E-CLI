use crate::models::graph::{Cardinality, Field};
use std::path::Path;

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

/// Render a link template per spec §9. `cwd` is `std::env::current_dir()` at
/// run time; `output_dir` is the parent of `-o` (or `-o` itself if it's a dir).
pub fn render_link(
    template: Option<&str>,
    pkg: &str,
    module: &str,
    class: &str,
    version: &str,
    cwd: &Path,
    output_dir: &Path,
) -> Option<String> {
    let tpl = template?;
    if tpl.is_empty() || tpl.eq_ignore_ascii_case("none") {
        return None;
    }
    Some(substitute(
        tpl, pkg, module, class, version, cwd, output_dir,
    ))
}

fn substitute(
    template: &str,
    pkg: &str,
    module: &str,
    class: &str,
    version: &str,
    cwd: &Path,
    output_dir: &Path,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            let mut closed = false;
            for (_, nc) in chars.by_ref() {
                if nc == '}' {
                    closed = true;
                    break;
                }
                name.push(nc);
            }
            if !closed {
                out.push('{');
                out.push_str(&name);
                continue;
            }
            match resolve_placeholder(&name, pkg, module, class, version, cwd, output_dir) {
                Some(v) => out.push_str(&v),
                None => {
                    out.push('{');
                    out.push_str(&name);
                    out.push('}');
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn resolve_placeholder(
    name: &str,
    pkg: &str,
    module: &str,
    class: &str,
    version: &str,
    cwd: &Path,
    output_dir: &Path,
) -> Option<String> {
    match name {
        "pkg" => Some(pkg.to_string()),
        "module" => Some(module.to_string()),
        "class" => Some(class.to_string()),
        "version" => Some(version.to_string()),
        n if n == "cwd" || n.starts_with("cwd.") => apply_path_accessor(cwd, accessor_of(n, "cwd")),
        n if n == "output_dir" || n.starts_with("output_dir.") => {
            apply_path_accessor(output_dir, accessor_of(n, "output_dir"))
        }
        _ => None,
    }
}

fn accessor_of<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = name.strip_prefix(prefix)?;
    if rest.is_empty() {
        Some("abs")
    } else {
        rest.strip_prefix('.')
    }
}

fn apply_path_accessor(p: &Path, accessor: Option<&str>) -> Option<String> {
    let accessor = accessor?;
    match accessor {
        "abs" => Some(p.display().to_string()),
        "uri" => url::Url::from_file_path(p).ok().map(|u| u.to_string()),
        "rel" => Some(strip_root(p)),
        "posix" => Some(p.display().to_string().replace('\\', "/")),
        "name" => Some(
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        _ => None,
    }
}

fn strip_root(p: &Path) -> String {
    let s = p.display().to_string();
    #[cfg(unix)]
    {
        s.strip_prefix('/').unwrap_or(&s).to_string()
    }
    #[cfg(windows)]
    {
        if let Some(after_drive) = s.get(2..) {
            after_drive.trim_start_matches('\\').to_string()
        } else {
            s
        }
    }
}

/// True iff `field` is a ref-field that already has an outgoing edge — used by
/// emitters to skip the in-class-box rendering.
pub fn skip_inline_for_reference(field: &Field) -> bool {
    field.is_reference
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cwd() -> PathBuf {
        PathBuf::from("/home/user/proj")
    }
    fn outd() -> PathBuf {
        PathBuf::from("/home/user/proj/out")
    }

    #[test]
    fn node_placeholders_substitute() {
        let r = render_link(
            Some("{pkg}/{class}#{module}"),
            "bo",
            "bo.Angebot",
            "Angebot",
            "v202501.0.0",
            &cwd(),
            &outd(),
        );
        assert_eq!(r.unwrap(), "bo/Angebot#bo.Angebot");
    }

    #[test]
    fn cwd_uri_yields_file_url() {
        let r = render_link(
            Some("{cwd.uri}/api/{pkg}.html"),
            "bo",
            "bo.Angebot",
            "Angebot",
            "v",
            &cwd(),
            &outd(),
        );
        let s = r.unwrap();
        assert!(s.starts_with("file:///"), "got: {s}");
        assert!(s.ends_with("/api/bo.html"));
    }

    #[test]
    fn cwd_name_is_last_segment() {
        let r = render_link(
            Some("https://x/{cwd.name}/api"),
            "",
            "",
            "",
            "",
            &cwd(),
            &outd(),
        );
        assert_eq!(r.unwrap(), "https://x/proj/api");
    }

    #[cfg(unix)]
    #[test]
    fn cwd_rel_strips_leading_slash_unix() {
        let r = render_link(Some("{cwd.rel}"), "", "", "", "", &cwd(), &outd());
        assert_eq!(r.unwrap(), "home/user/proj");
    }

    #[test]
    fn empty_template_returns_none() {
        let r = render_link(Some(""), "bo", "bo.A", "A", "v", &cwd(), &outd());
        assert!(r.is_none());
        let r = render_link(Some("none"), "bo", "bo.A", "A", "v", &cwd(), &outd());
        assert!(r.is_none());
        let r = render_link(None, "bo", "bo.A", "A", "v", &cwd(), &outd());
        assert!(r.is_none());
    }

    #[test]
    fn unknown_placeholder_stays_literal() {
        let r = render_link(
            Some("https://x/{unknown}/{pkg}"),
            "bo",
            "",
            "",
            "",
            &cwd(),
            &outd(),
        );
        assert_eq!(r.unwrap(), "https://x/{unknown}/bo");
    }

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
