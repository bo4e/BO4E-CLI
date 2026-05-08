use crate::models::version::Version;

pub struct FilterOptions {
    /// Number of versions to return. `0` means "all since `threshold`".
    pub n: u32,
    /// Drop release-candidate versions.
    pub exclude_candidates: bool,
    /// In each functional group, keep only the newest technical version.
    pub exclude_technical_bumps: bool,
    /// Drop the first input element (set when the user-supplied ref is itself a tag).
    pub skip_first: bool,
    /// Stop iteration when this version is reached. Used only when `n == 0`.
    pub threshold: Version,
}

/// Pure filter over an already-sorted (descending) list of candidate versions.
///
/// `is_release` is invoked for each candidate that survives all other rules.
/// Returning `Ok(false)` skips the candidate; returning `Err` aborts the whole
/// filter and propagates the error to the caller.
pub fn filter_tags(
    candidates: &[Version],
    opts: &FilterOptions,
    mut is_release: impl FnMut(&Version) -> Result<bool, String>,
) -> Result<Vec<Version>, String> {
    let _ = (candidates, opts, &mut is_release);
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::version::Version;

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    fn opts(n: u32) -> FilterOptions {
        FilterOptions {
            n,
            exclude_candidates: false,
            exclude_technical_bumps: false,
            skip_first: false,
            threshold: v("v202401.0.0"),
        }
    }

    #[test]
    fn test_empty_input_returns_empty() {
        let out = filter_tags(&[], &opts(0), |_| Ok(true)).unwrap();
        assert!(out.is_empty());
    }
}
