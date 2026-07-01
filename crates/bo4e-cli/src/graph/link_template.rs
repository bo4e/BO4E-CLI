//! URL-template engine for clickable class nodes (`--link-template`).
//!
//! One `Placeholder` enum is the single source of truth: it defines every
//! placeholder, the accessor [`Family`] it belongs to, and how it resolves
//! against a [`LinkContext`]. The per-family accessor tables
//! ([`CASE_ACCESSORS`] / [`PATH_ACCESSORS`]) are the single source of truth for
//! which `.accessor` suffixes exist and what they do. Both the resolver here
//! and the shell-completion candidate list read from these, so completion and
//! substitution can never drift: add a placeholder → the compiler forces you to
//! wire `name`/`family`/`raw_text`; add an accessor → one table row updates both
//! resolution and completion.

use std::path::Path;

/// Runtime values for expanding a `--link-template` at a single node.
///
/// `module` is the node's dotted path as a slice (e.g. `["com", "Angebotsteil"]`
/// or the root-level `["ZusatzAttribut"]`); every string placeholder is derived
/// from it.
pub struct LinkContext<'a> {
    pub module: &'a [String],
    pub version: &'a str,
    pub cwd: &'a Path,
    pub output_dir: &'a Path,
}

/// The accessor family a placeholder belongs to. Governs which `.accessor`
/// suffixes are valid and how the value is transformed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    /// String placeholders: `.lower` / `.upper`; bare renders verbatim.
    Cased,
    /// Structured strings (the version) where case folding is meaningless: no
    /// accessors, bare only.
    Verbatim,
    /// Filesystem paths: `.abs` (the default) / `.rel` / `.uri` / `.posix` /
    /// `.name`.
    Path,
}

/// Every placeholder. Adding a variant forces `name`/`family`/`raw_text` (and,
/// for paths, `path_value`) to be updated — the matches below are exhaustive.
#[derive(Clone, Copy)]
enum Placeholder {
    Pkg,
    Module,
    Class,
    Namespace,
    Version,
    Cwd,
    OutputDir,
}

impl Placeholder {
    const ALL: &'static [Placeholder] = &[
        Placeholder::Pkg,
        Placeholder::Module,
        Placeholder::Class,
        Placeholder::Namespace,
        Placeholder::Version,
        Placeholder::Cwd,
        Placeholder::OutputDir,
    ];

    fn name(self) -> &'static str {
        match self {
            Placeholder::Pkg => "pkg",
            Placeholder::Module => "module",
            Placeholder::Class => "class",
            Placeholder::Namespace => "namespace",
            Placeholder::Version => "version",
            Placeholder::Cwd => "cwd",
            Placeholder::OutputDir => "output_dir",
        }
    }

    fn family(self) -> Family {
        match self {
            Placeholder::Pkg
            | Placeholder::Module
            | Placeholder::Class
            | Placeholder::Namespace => Family::Cased,
            Placeholder::Version => Family::Verbatim,
            Placeholder::Cwd | Placeholder::OutputDir => Family::Path,
        }
    }

    fn parse(base: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.name() == base)
    }

    /// Value for the `Cased` / `Verbatim` families (the string placeholders).
    /// Paths return `None` here and resolve via [`Self::path_value`].
    fn raw_text(self, ctx: &LinkContext) -> Option<String> {
        let text = match self {
            Placeholder::Pkg => ctx.module.first().cloned().unwrap_or_default(),
            Placeholder::Module => ctx.module.join("."),
            Placeholder::Class => ctx.module.last().cloned().unwrap_or_default(),
            // `bo4e` + the module's parent package (everything but the class),
            // joined with `.`. The join means the root-schema case (empty
            // parent) yields plain `bo4e` with no stray dot.
            Placeholder::Namespace => {
                let mut segs: Vec<&str> = vec!["bo4e"];
                if let Some((_class, parents)) = ctx.module.split_last() {
                    segs.extend(parents.iter().map(String::as_str));
                }
                segs.join(".")
            }
            Placeholder::Version => ctx.version.to_string(),
            Placeholder::Cwd | Placeholder::OutputDir => return None,
        };
        Some(text)
    }

    fn path_value<'a>(self, ctx: &'a LinkContext) -> Option<&'a Path> {
        match self {
            Placeholder::Cwd => Some(ctx.cwd),
            Placeholder::OutputDir => Some(ctx.output_dir),
            _ => None,
        }
    }
}

type CaseFn = fn(&str) -> String;
type PathFn = fn(&Path) -> Option<String>;

/// Case accessors for the `Cased` family. Single source of truth: completion
/// lists these names, resolution applies the matching function.
const CASE_ACCESSORS: &[(&str, CaseFn)] =
    &[("lower", str::to_lowercase), ("upper", str::to_uppercase)];

/// Path accessors for the `Path` family. `abs` is also the bare default.
const PATH_ACCESSORS: &[(&str, PathFn)] = &[
    ("abs", |p| Some(p.display().to_string())),
    ("rel", |p| Some(strip_root(p))),
    ("uri", |p| {
        url::Url::from_file_path(p).ok().map(|u| u.to_string())
    }),
    ("posix", |p| {
        Some(p.display().to_string().replace('\\', "/"))
    }),
    ("name", |p| {
        Some(
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    }),
];

/// Base placeholder names (no braces), for completion when no `.` has been
/// typed yet.
pub fn placeholder_names() -> impl Iterator<Item = &'static str> {
    Placeholder::ALL.iter().map(|p| p.name())
}

/// Accessor names valid after `{<base>.` — empty for an unknown base or the
/// accessor-less `version`. Derived from the same tables the resolver applies,
/// so completion can never offer an accessor that resolution rejects.
pub fn accessor_names_for(base: &str) -> Vec<&'static str> {
    match Placeholder::parse(base).map(Placeholder::family) {
        Some(Family::Cased) => CASE_ACCESSORS.iter().map(|(n, _)| *n).collect(),
        Some(Family::Path) => PATH_ACCESSORS.iter().map(|(n, _)| *n).collect(),
        Some(Family::Verbatim) | None => Vec::new(),
    }
}

/// Render `template` for one node. Returns `None` when the template is absent,
/// empty, or the literal `none` (links explicitly disabled).
pub fn render_link(template: Option<&str>, ctx: &LinkContext) -> Option<String> {
    let tpl = template?;
    if tpl.is_empty() || tpl.eq_ignore_ascii_case("none") {
        return None;
    }
    Some(substitute(tpl, ctx))
}

fn substitute(template: &str, ctx: &LinkContext) -> String {
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
            match resolve(&name, ctx) {
                Some(v) => out.push_str(&v),
                None => {
                    // Unknown placeholder or accessor: leave the token literal.
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

/// Resolve a single `{name}` token (the text between the braces). `None` means
/// "leave literal".
fn resolve(name: &str, ctx: &LinkContext) -> Option<String> {
    let (base, accessor) = match name.split_once('.') {
        Some((b, a)) => (b, Some(a)),
        None => (name, None),
    };
    let ph = Placeholder::parse(base)?;
    match ph.family() {
        Family::Cased => {
            let value = ph.raw_text(ctx)?;
            match accessor {
                None => Some(value),
                Some(a) => CASE_ACCESSORS
                    .iter()
                    .find(|(n, _)| *n == a)
                    .map(|(_, f)| f(&value)),
            }
        }
        Family::Verbatim => match accessor {
            None => ph.raw_text(ctx),
            Some(_) => None, // no accessors on verbatim placeholders
        },
        Family::Path => {
            let path = ph.path_value(ctx)?;
            let a = accessor.unwrap_or("abs");
            PATH_ACCESSORS
                .iter()
                .find(|(n, _)| *n == a)
                .and_then(|(_, f)| f(path))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn m(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }
    fn cwd() -> &'static Path {
        Path::new("/home/user/proj")
    }
    fn outd() -> &'static Path {
        Path::new("/home/user/proj/out")
    }
    fn ctx<'a>(
        module: &'a [String],
        version: &'a str,
        cwd: &'a Path,
        output_dir: &'a Path,
    ) -> LinkContext<'a> {
        LinkContext {
            module,
            version,
            cwd,
            output_dir,
        }
    }

    #[test]
    fn base_placeholders_substitute() {
        let module = m(&["bo", "Angebot"]);
        let r = render_link(
            Some("{pkg}/{class}#{module}"),
            &ctx(&module, "v202501.0.0", cwd(), outd()),
        );
        assert_eq!(r.unwrap(), "bo/Angebot#bo.Angebot");
    }

    #[test]
    fn namespace_prepends_bo4e_and_drops_class() {
        let module = m(&["bo", "Angebot"]);
        let r = render_link(
            Some("api/{namespace}.html#module-{namespace}.{class.lower}"),
            &ctx(&module, "v", cwd(), outd()),
        );
        assert_eq!(r.unwrap(), "api/bo4e.bo.html#module-bo4e.bo.angebot");
    }

    #[test]
    fn namespace_for_root_level_schema_is_bare_bo4e() {
        // Root schemas (e.g. ZusatzAttribut) have a single-segment module; the
        // parent package is empty, so `{namespace}` must be plain `bo4e` (no
        // trailing dot) and the class is documented on `bo4e.html`.
        let module = m(&["ZusatzAttribut"]);
        let r = render_link(
            Some("api/{namespace}.html#module-{namespace}.{class.lower}"),
            &ctx(&module, "v", cwd(), outd()),
        );
        assert_eq!(r.unwrap(), "api/bo4e.html#module-bo4e.zusatzattribut");
    }

    #[test]
    fn cased_accessors_fold() {
        let module = m(&["com", "Angebotsteil"]);
        let c = ctx(&module, "v", cwd(), outd());
        assert_eq!(
            render_link(Some("{class.lower}"), &c).unwrap(),
            "angebotsteil"
        );
        assert_eq!(render_link(Some("{pkg.upper}"), &c).unwrap(), "COM");
        assert_eq!(
            render_link(Some("{module.lower}"), &c).unwrap(),
            "com.angebotsteil"
        );
    }

    #[test]
    fn version_takes_no_accessors() {
        let module = m(&["com", "Angebotsteil"]);
        let c = ctx(&module, "v202601.0.0", cwd(), outd());
        assert_eq!(render_link(Some("{version}"), &c).unwrap(), "v202601.0.0");
        // `.lower` is not a valid accessor for version -> token stays literal.
        assert_eq!(
            render_link(Some("{version.lower}"), &c).unwrap(),
            "{version.lower}"
        );
    }

    #[test]
    fn unknown_placeholder_and_accessor_stay_literal() {
        let module = m(&["bo", "Angebot"]);
        let c = ctx(&module, "v", cwd(), outd());
        assert_eq!(
            render_link(Some("x/{unknown}/{pkg}"), &c).unwrap(),
            "x/{unknown}/bo"
        );
        assert_eq!(
            render_link(Some("{class.bogus}"), &c).unwrap(),
            "{class.bogus}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn path_accessors_work_and_default_to_abs() {
        let module = m(&["bo", "Angebot"]);
        let c = ctx(&module, "v", cwd(), outd());
        assert_eq!(render_link(Some("{cwd}"), &c).unwrap(), "/home/user/proj");
        assert_eq!(
            render_link(Some("{cwd.rel}"), &c).unwrap(),
            "home/user/proj"
        );
        assert_eq!(render_link(Some("{cwd.name}"), &c).unwrap(), "proj");
        assert!(
            render_link(Some("{cwd.uri}"), &c)
                .unwrap()
                .starts_with("file:///")
        );
    }

    #[test]
    fn empty_or_none_template_disables_links() {
        let module = m(&["bo", "A"]);
        let c = ctx(&module, "v", cwd(), outd());
        assert!(render_link(Some(""), &c).is_none());
        assert!(render_link(Some("none"), &c).is_none());
        assert!(render_link(None, &c).is_none());
    }

    #[test]
    fn completion_spec_matches_families() {
        let names: Vec<&str> = placeholder_names().collect();
        assert!(names.contains(&"namespace"));
        assert!(names.contains(&"module"));
        assert_eq!(accessor_names_for("class"), vec!["lower", "upper"]);
        assert_eq!(accessor_names_for("namespace"), vec!["lower", "upper"]);
        assert_eq!(
            accessor_names_for("cwd"),
            vec!["abs", "rel", "uri", "posix", "name"]
        );
        assert!(accessor_names_for("version").is_empty());
        assert!(accessor_names_for("nonexistent").is_empty());
    }
}
