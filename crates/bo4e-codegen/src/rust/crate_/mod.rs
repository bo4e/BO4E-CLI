//! `rust-crate` orchestrator — wraps `rust-plain` output as a self-contained Cargo crate.

use bo4e_schemas::Schemas;
use minijinja::context;
use std::path::Path;

use crate::Error;

/// Validate a Cargo package name. Same rules as the CLI's
/// `--crate-name` parser: starts with ASCII letter or `_`; remaining
/// characters are ASCII alphanumeric / `_` / `-`; length ≤ 64.
///
/// Lifted into the library API so callers building `RustCrateOptions`
/// directly (without going through the CLI) can't generate a malformed
/// `Cargo.toml`.
pub fn validate_crate_name(s: &str) -> Result<(), Error> {
    const MAX_LEN: usize = 64;
    if s.is_empty() {
        return Err(Error::InvalidOption {
            what: "crate_name".into(),
            reason: "must not be empty".into(),
        });
    }
    if s.len() > MAX_LEN {
        return Err(Error::InvalidOption {
            what: "crate_name".into(),
            reason: format!("too long ({} chars); cap is {MAX_LEN}", s.len()),
        });
    }
    let mut chars = s.chars();
    let first = chars.next().expect("non-empty checked above");
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(Error::InvalidOption {
            what: "crate_name".into(),
            reason: format!("`{s}` must start with an ASCII letter or `_`"),
        });
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(Error::InvalidOption {
                what: "crate_name".into(),
                reason: format!(
                    "`{s}` contains invalid character `{c}`; \
                     only ASCII alphanumerics, `_`, and `-` are allowed"
                ),
            });
        }
    }
    Ok(())
}

pub fn generate(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
    crate_opts: &crate::RustCrateOptions,
) -> Result<crate::GenerateOutput, Error> {
    // ── Phase 1: validate + render (no destructive IO) ─────────────────────────
    validate_crate_name(&crate_opts.crate_name)?;

    let src_dir = output_dir.join("src");
    let inner_opts = crate::Options {
        clear_output: false, // commit phase below owns the clear
        templates_dir: opts.templates_dir,
    };
    let (mut files, mut diagnostics) = crate::rust::plain::prepare(schemas, &src_dir, &inner_opts)?;

    // Rename `<src>/mod.rs` → `<src>/lib.rs` in the buffer (no disk IO yet).
    let mod_rs = src_dir.join("mod.rs");
    let lib_rs = src_dir.join("lib.rs");
    crate::rename_in_prepared(&mod_rs, &lib_rs, &mut files);
    diagnostics.push("renamed mod.rs → lib.rs".to_string());

    // Stage Cargo.toml.
    let env = crate::env::make_environment(opts.templates_dir)?;
    let version_str = schemas.version.to_string();
    let semver = to_cargo_semver(&version_str);
    let cargo_tpl = env.get_template("rust/crate_/CargoToml.jinja2")?;
    let cargo_toml = cargo_tpl.render(context! {
        crate_name => &crate_opts.crate_name,
        semver => &semver,
        bo4e_version => &version_str,
    })?;
    files.push((output_dir.join("Cargo.toml"), cargo_toml));
    diagnostics.push(format!(
        "Cargo.toml: name={}, version={}",
        crate_opts.crate_name, semver
    ));

    // ── Phase 2: commit (clear + write) ────────────────────────────────────────
    crate::write_prepared(output_dir, opts.clear_output, files, diagnostics)
}

/// Convert a BO4E version string (as produced by `Version`/`DirtyVersion::Display`) into
/// a SemVer-valid string acceptable as Cargo.toml's `version` field.
///
/// BO4E version forms and their SemVer fate:
/// - `v202401.4.0`                          → `202401.4.0`                  (clean — already valid)
/// - `v202401.4.0-rc1`                      → `202401.4.0-rc1`              (clean rc — valid)
/// - `v202401.4.0+gABCD`                    → `202401.4.0+gABCD`            (dirty commit — valid, uses `+` build metadata)
/// - `v202401.4.0.d20260512`                → `202401.4.0+d20260512`        (worktree-dirty only — `.d…` is NOT valid as a 4th dot-segment; rewrite as `+`-separated build metadata)
/// - `v202401.4.0+gABCD.d20260512`          → `202401.4.0+gABCD.d20260512`  (both — `.d…` is already inside the `+` build-metadata block; valid)
/// - `v202401.4.0-rc1.d20260512`            → `202401.4.0-rc1.d20260512`    (rc + worktree — `.d…` becomes part of the pre-release identifier list, which is valid SemVer)
fn to_cargo_semver(version_str: &str) -> String {
    let s = version_str.strip_prefix('v').unwrap_or(version_str);
    // If there's already a `+` (build metadata starts), any subsequent `.d…` is part of
    // that block. Same for `-` (pre-release): a trailing `.d…` becomes another pre-release
    // identifier, which is fine.
    if s.contains('+') || s.contains('-') {
        return s.to_string();
    }
    // No `-rc` or `+g…` — only the worktree-dirty `.d<date>` suffix can break SemVer.
    if let Some(idx) = s.find(".d")
        && s[idx + 2..].chars().all(|c| c.is_ascii_digit())
    {
        let (head, tail) = s.split_at(idx);
        // Replace the leading `.` of `.d…` with `+` so it becomes valid build metadata.
        return format!("{head}+{}", &tail[1..]);
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    #[test]
    fn cargo_toml_strips_leading_v_for_semver() {
        let env = crate::env::make_environment(None).unwrap();
        let tpl = env.get_template("rust/crate_/CargoToml.jinja2").unwrap();
        let s = tpl
            .render(context! {
                crate_name => "bo4e",
                semver => "202401.4.0",
                bo4e_version => "v202401.4.0",
            })
            .unwrap();
        assert!(s.contains("name = \"bo4e\""));
        assert!(s.contains("version = \"202401.4.0\""));
        assert!(s.contains(
            "description = \"Generated Rust types for the BO4E energy data model, version v202401.4.0\""
        ));
    }

    #[test]
    fn to_cargo_semver_clean_release_unchanged() {
        assert_eq!(to_cargo_semver("v202401.4.0"), "202401.4.0");
    }

    #[test]
    fn to_cargo_semver_release_candidate_unchanged() {
        // pre-release form is already valid SemVer
        assert_eq!(to_cargo_semver("v202401.4.0-rc1"), "202401.4.0-rc1");
    }

    #[test]
    fn to_cargo_semver_commit_dirty_unchanged() {
        // `+g<sha>` is valid SemVer build metadata
        assert_eq!(
            to_cargo_semver("v202401.4.0+gabc1234"),
            "202401.4.0+gabc1234"
        );
    }

    #[test]
    fn to_cargo_semver_worktree_dirty_only_becomes_plus_d() {
        // The buggy case: `.d<date>` after a clean release would be a 4th dot-segment,
        // which is NOT valid SemVer (Cargo rejects this at parse time). Rewrite the
        // separator from `.` to `+` so it becomes valid build metadata.
        assert_eq!(
            to_cargo_semver("v202401.4.0.d20260512"),
            "202401.4.0+d20260512"
        );
    }

    #[test]
    fn to_cargo_semver_commit_and_worktree_dirty_unchanged() {
        // `.d…` already sits inside the `+`-introduced build-metadata block here, so
        // the whole string is a valid SemVer build identifier list (dot-separated).
        assert_eq!(
            to_cargo_semver("v202401.4.0+gabc1234.d20260512"),
            "202401.4.0+gabc1234.d20260512"
        );
    }

    #[test]
    fn to_cargo_semver_rc_and_worktree_dirty_unchanged() {
        // `-rc1.d20260512` is a valid pre-release identifier list (dot-separated).
        assert_eq!(
            to_cargo_semver("v202401.4.0-rc1.d20260512"),
            "202401.4.0-rc1.d20260512"
        );
    }

    #[test]
    fn to_cargo_semver_rc_and_commit_dirty_unchanged() {
        // `-rc1+gabc1234`: pre-release + build metadata. Valid SemVer.
        assert_eq!(
            to_cargo_semver("v202401.4.0-rc1+gabc1234"),
            "202401.4.0-rc1+gabc1234"
        );
    }

    #[test]
    fn to_cargo_semver_rc_commit_and_worktree_dirty_unchanged() {
        // `-rc1+gabc1234.d20260512`: pre-release + build metadata with two identifiers.
        // Valid SemVer.
        assert_eq!(
            to_cargo_semver("v202401.4.0-rc1+gabc1234.d20260512"),
            "202401.4.0-rc1+gabc1234.d20260512"
        );
    }

    /// Round-trip every BO4E-Display version shape through the same `semver` crate
    /// Cargo uses for `[package].version`. This guards us from accepting a string
    /// that only looks valid — if Cargo's parser would reject the output of
    /// `to_cargo_semver`, the generated `Cargo.toml` won't compile.
    #[test]
    fn every_bo4e_version_shape_passes_cargo_semver_parser() {
        use semver::Version;
        for input in [
            // clean
            "v202401.4.0",
            "v202401.4.0-rc1",
            // commit-dirty only
            "v202401.4.0+gabc1234",
            "v202401.4.0-rc1+gabc1234",
            // worktree-dirty only (the buggy case before the fix)
            "v202401.4.0.d20260512",
            "v202401.4.0-rc1.d20260512",
            // both
            "v202401.4.0+gabc1234.d20260512",
            "v202401.4.0-rc1+gabc1234.d20260512",
        ] {
            let semver = to_cargo_semver(input);
            Version::parse(&semver).unwrap_or_else(|e| {
                panic!(
                    "to_cargo_semver({input}) → {semver}, but semver::Version::parse failed: {e}"
                )
            });
        }
    }
}
