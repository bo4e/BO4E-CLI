use bo4e_schemas::models::version::Version;

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
    let mut out: Vec<Version> = Vec::new();
    let mut last_yielded: Option<Version> = None;
    for (i, v) in candidates.iter().enumerate() {
        if opts.n > 0 && out.len() as u32 >= opts.n {
            break;
        }
        if opts.n == 0 && *v == opts.threshold {
            break;
        }
        if opts.exclude_candidates && v.is_release_candidate() {
            continue;
        }
        if opts.exclude_technical_bumps
            && let Some(prev) = &last_yielded
            && prev.bumped_technical(v)
        {
            continue;
        }
        if i == 0 && opts.skip_first {
            continue;
        }
        if !is_release(v)? {
            continue;
        }
        out.push(v.clone());
        last_yielded = out.last().cloned();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::version::Version;

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

    #[test]
    fn test_n_positive_stops_after_n_yields() {
        let cands = vec![
            v("v202401.5.0"),
            v("v202401.4.0"),
            v("v202401.3.0"),
            v("v202401.2.0"),
        ];
        let out = filter_tags(&cands, &opts(2), |_| Ok(true)).unwrap();
        assert_eq!(out, vec![v("v202401.5.0"), v("v202401.4.0")]);
    }

    #[test]
    fn test_n_zero_stops_at_threshold() {
        let cands = vec![
            v("v202401.2.0"),
            v("v202401.1.0"),
            v("v202401.0.0"),
            v("v202400.9.0"),
        ];
        let out = filter_tags(&cands, &opts(0), |_| Ok(true)).unwrap();
        assert_eq!(out, vec![v("v202401.2.0"), v("v202401.1.0")]);
    }

    #[test]
    fn test_n_zero_no_threshold_returns_all() {
        let cands = vec![v("v202401.2.0"), v("v202401.1.0")];
        let out = filter_tags(&cands, &opts(0), |_| Ok(true)).unwrap();
        assert_eq!(out, vec![v("v202401.2.0"), v("v202401.1.0")]);
    }

    #[test]
    fn test_exclude_candidates_drops_rcs_only() {
        let cands = vec![v("v202401.3.0"), v("v202401.2.0-rc1"), v("v202401.2.0")];
        let mut o = opts(0);
        o.exclude_candidates = true;
        let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
        assert_eq!(out, vec![v("v202401.3.0"), v("v202401.2.0")]);
    }

    #[test]
    fn test_exclude_technical_bumps_keeps_newest_per_group() {
        // Three technical bumps under v202401.2.x, plus a different functional group above.
        let cands = vec![
            v("v202401.3.0"),
            v("v202401.2.5"),
            v("v202401.2.4"),
            v("v202401.2.3"),
            v("v202401.1.0"),
        ];
        let mut o = opts(0);
        o.exclude_technical_bumps = true;
        let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
        // Newest of each functional group: v202401.3.0, v202401.2.5, v202401.1.0
        assert_eq!(
            out,
            vec![v("v202401.3.0"), v("v202401.2.5"), v("v202401.1.0")]
        );
    }

    #[test]
    fn test_skip_first_drops_index_zero() {
        let cands = vec![v("v202401.3.0"), v("v202401.2.0"), v("v202401.1.0")];
        let mut o = opts(0);
        o.skip_first = true;
        let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
        assert_eq!(out, vec![v("v202401.2.0"), v("v202401.1.0")]);
    }

    #[test]
    fn test_skip_first_is_by_input_index_not_post_filter() {
        // Index 0 is an RC. With both flags on, it gets dropped by skip_first OR
        // by exclude_candidates — but we still only drop one element.
        let cands = vec![v("v202401.3.0-rc1"), v("v202401.3.0"), v("v202401.2.0")];
        let mut o = opts(0);
        o.skip_first = true;
        o.exclude_candidates = true;
        let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
        assert_eq!(out, vec![v("v202401.3.0"), v("v202401.2.0")]);
    }

    #[test]
    fn test_is_release_false_skips_version() {
        let cands = vec![v("v202401.3.0"), v("v202401.2.0"), v("v202401.1.0")];
        let out = filter_tags(&cands, &opts(0), |x| Ok(*x != v("v202401.2.0"))).unwrap();
        assert_eq!(out, vec![v("v202401.3.0"), v("v202401.1.0")]);
    }

    #[test]
    fn test_is_release_err_aborts() {
        let cands = vec![v("v202401.3.0"), v("v202401.2.0")];
        let result = filter_tags(&cands, &opts(0), |_| Err("network".to_string()));
        assert_eq!(result, Err("network".to_string()));
    }

    #[test]
    fn test_combination_n_with_skip_rules() {
        // n=3, exclude_candidates, exclude_technical_bumps, skip_first
        // Input (descending):  rc, 3.5, 3.0, 2.0, 1.5, 1.0
        // skip_first: drops rc (index 0)
        // exclude_candidates: would also drop rc (no double-count)
        // exclude_technical_bumps: from each functional group, keep newest:
        //   3.x → 3.5, 2.x → 2.0, 1.x → 1.5
        // n=3 cap: stop after 3 yields → 3.5, 2.0, 1.5
        let cands = vec![
            v("v202401.4.0-rc1"),
            v("v202401.3.5"),
            v("v202401.3.0"),
            v("v202401.2.0"),
            v("v202401.1.5"),
            v("v202401.1.0"),
        ];
        let mut o = opts(3);
        o.exclude_candidates = true;
        o.exclude_technical_bumps = true;
        o.skip_first = true;
        let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
        assert_eq!(
            out,
            vec![v("v202401.3.5"), v("v202401.2.0"), v("v202401.1.5")]
        );
    }
}
