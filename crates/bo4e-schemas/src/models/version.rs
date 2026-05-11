use chrono::prelude::*;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

lazy_static! {
    static ref REGEX_VERSION: Regex = Regex::new(
        "^v(?P<major>\\d{6})\\.\
        (?P<functional>\\d+)\\.\
        (?P<technical>\\d+)\
        (?:-rc(?P<candidate>\\d*))?\
        $",
    )
    .unwrap();
    static ref REGEX_DIRTY_VERSION: Regex = Regex::new(
        "^v(?P<major>\\d{6})\\.\
        (?P<functional>\\d+)\\.\
        (?P<technical>\\d+)\
        (?:-rc(?P<candidate>\\d*))?\
        (?:\\+g(?P<commit_part>\\w+)\
        (?:\\.d\
        (?P<dirty_workdir_date_year>\\d{4})\
        (?P<dirty_workdir_date_month>\\d{2})\
        (?P<dirty_workdir_date_day>\\d{2})\
        )?)?$",
    )
    .unwrap();
}

/// A version of the BO4E-Schemas.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Version {
    major: u32,
    functional: u32,
    technical: u32,
    candidate: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DirtyVersion {
    #[serde(flatten)]
    version: Version,
    /// The commit hash or at least a starting substring of it.
    /// When retrieving the version from a commit which has no tag on it, the version will have
    /// the commit hash after the last version tag in the history.
    commit_part: Option<String>,
    /// E.g. if you have local changes in your working directory of the BO4E-python repo,
    /// hatch will create version information similar to ``v202401.0.1.dev0+g12984hdac.d20240101``.
    /// The last part after the ``.d`` is the date of the last change in the working directory
    /// - I believe.
    dirty_worktree_date: Option<NaiveDate>,
}

fn _get_basic_version_from_match(captures: &regex::Captures) -> Version {
    // Note: unwrap is safe here because the regex ensures that these groups are present
    // and valid unsigned integers.
    let major = captures.name("major").unwrap().as_str().parse().unwrap();
    let functional = captures
        .name("functional")
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let technical = captures
        .name("technical")
        .unwrap()
        .as_str()
        .parse()
        .unwrap();

    let candidate = captures
        .name("candidate")
        .map(|m| m.as_str().parse().unwrap());

    Version {
        major,
        functional,
        technical,
        candidate,
    }
}

impl FromStr for Version {
    type Err = String;

    /// Parse a version string into a Version object e.g. 'v202401.0.1-rc8+g12asdf34.d20240101'
    /// or 'v202401.0.1'. Returns an error if the version string is invalid.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let captures = REGEX_VERSION
            .captures(s)
            .ok_or_else(|| format!("Invalid version format: {}", s))?;

        Ok(_get_basic_version_from_match(&captures))
    }
}

impl FromStr for DirtyVersion {
    type Err = String;

    /// Parse a version string into a Version object e.g. 'v202401.0.1-rc8+g12asdf34.d20240101'
    /// or 'v202401.0.1'. Returns an error if the version string is invalid.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let captures = REGEX_DIRTY_VERSION
            .captures(s)
            .ok_or_else(|| format!("Invalid version format: {}", s))?;

        let version = _get_basic_version_from_match(&captures);

        let commit_part = captures.name("commit_part").map(|m| m.as_str().to_string());

        let dirty_worktree_date = if let Some(year) = captures.name("dirty_workdir_date_year") {
            let month = captures.name("dirty_workdir_date_month").unwrap();
            let day = captures.name("dirty_workdir_date_day").unwrap();

            NaiveDate::from_ymd_opt(
                year.as_str().parse().unwrap(),
                month.as_str().parse().unwrap(),
                day.as_str().parse().unwrap(),
            )
        } else {
            None
        };

        Ok(DirtyVersion {
            version,
            commit_part,
            dirty_worktree_date,
        })
    }
}

impl TryFrom<&DirtyVersion> for Version {
    type Error = String;

    /// Convert a DirtyVersion to a Version, ignoring the commit_part and dirty_worktree_date.
    fn try_from(dirty_version: &DirtyVersion) -> Result<Self, Self::Error> {
        if dirty_version.is_dirty() {
            return Err("Cannot convert DirtyVersion to Version: it is dirty.".to_string());
        }
        Ok(Version {
            ..dirty_version.version
        })
    }
}

impl TryFrom<DirtyVersion> for Version {
    type Error = String;

    /// Convert a DirtyVersion to a Version, ignoring the commit_part and dirty_worktree_date.
    fn try_from(dirty_version: DirtyVersion) -> Result<Self, Self::Error> {
        if dirty_version.is_dirty() {
            return Err("Cannot convert DirtyVersion to Version: it is dirty.".to_string());
        }
        Ok(dirty_version.version)
    }
}

impl From<&Version> for DirtyVersion {
    /// Convert a Version to a DirtyVersion, setting commit_part and dirty_worktree_date to None.
    fn from(version: &Version) -> Self {
        DirtyVersion {
            version: Version {
                major: version.major,
                functional: version.functional,
                technical: version.technical,
                candidate: version.candidate,
            },
            commit_part: None,
            dirty_worktree_date: None,
        }
    }
}

impl From<Version> for DirtyVersion {
    /// Convert a Version to a DirtyVersion, setting commit_part and dirty_worktree_date to None.
    fn from(version: Version) -> Self {
        DirtyVersion {
            version,
            commit_part: None,
            dirty_worktree_date: None,
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.functional, self.technical)?;
        if let Some(candidate) = self.candidate {
            write!(f, "-rc{}", candidate)?;
        }
        Ok(())
    }
}

impl Display for DirtyVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Run fmt method of Version first
        self.version.fmt(f)?;
        if let Some(commit_part) = &self.commit_part {
            write!(f, "+g{}", commit_part)?;
        }
        if let Some(date) = self.dirty_worktree_date {
            write!(f, ".d{}", date.format("%Y%m%d"))?;
        }
        Ok(())
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.functional == other.functional
            && self.technical == other.technical
            && self.candidate == other.candidate
    }
}

impl Eq for Version {}

impl PartialOrd for Version {
    /// Compare two versions.
    /// If a < b this means that version a is older than version b.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    /// Compare two versions.
    /// If a < b this means that version a is older than version b.
    fn cmp(&self, other: &Self) -> Ordering {
        let mut cmp = self.major.cmp(&other.major);
        if cmp != Ordering::Equal {
            return cmp;
        }
        cmp = self.functional.cmp(&other.functional);
        if cmp != Ordering::Equal {
            return cmp;
        }
        cmp = self.technical.cmp(&other.technical);
        if cmp != Ordering::Equal {
            return cmp;
        }
        if self.is_release_candidate() && other.is_release_candidate() {
            self.candidate.cmp(&other.candidate)
        } else if self.is_release_candidate() {
            Ordering::Less
        } else if other.is_release_candidate() {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

impl Version {
    /// Check if the version is a release candidate.
    pub fn is_release_candidate(&self) -> bool {
        self.candidate.is_some()
    }

    /// Return True if this version is a major bump from the other version.
    pub fn bumped_major(&self, other: &Self) -> bool {
        self.major > other.major
    }
    /// Return True if this version is a functional bump from the other version.
    pub fn bumped_functional(&self, other: &Self) -> bool {
        self.major == other.major && self.functional > other.functional
    }
    /// Return True if this version is a technical bump from the other version.
    pub fn bumped_technical(&self, other: &Self) -> bool {
        self.major == other.major
            && self.functional == other.functional
            && self.technical > other.technical
    }
    /// Return True if this version is a candidate bump from the other version.
    /// Returns an error if one of the versions is not a candidate version.
    pub fn bumped_candidate(&self, other: &Self) -> Result<bool, String> {
        Ok(self.major == other.major
            && self.functional == other.functional
            && self.technical == other.technical
            && self.candidate.ok_or_else(
                || "Cannot compare candidate versions if one of them is not a candidate.",
            )? > other.candidate.ok_or_else(
                || "Cannot compare candidate versions if one of them is not a candidate.",
            )?)
    }
}

impl DirtyVersion {
    /// Borrow the semantic version, discarding dirt metadata.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Check if the version is on a commit without a tag or if it corresponds to a dirty working
    /// directory.
    pub fn is_dirty(&self) -> bool {
        self.commit_part.is_some() || self.dirty_worktree_date.is_some()
    }

    /// Get the dirty workdir date, if any.
    pub fn dirty_worktree_date(&self) -> Option<NaiveDate> {
        self.dirty_worktree_date
    }

    /// Set (or replace) the `.d<YYYYMMDD>` dirty-workdir suffix so the version round-trips
    /// as `<semver>[.+g<commit>].d<date>`. Used by `bo4e edit` to brand the output schemas
    /// as locally edited.
    pub fn set_dirty_worktree_date(&mut self, date: NaiveDate) {
        self.dirty_worktree_date = Some(date);
    }
}

impl PartialEq<DirtyVersion> for Version {
    /// `Version == DirtyVersion` iff same semver and the dirty side is clean.
    fn eq(&self, other: &DirtyVersion) -> bool {
        *self == other.version && !other.is_dirty()
    }
}

impl PartialOrd<DirtyVersion> for Version {
    /// At equal semver, a dirty `DirtyVersion` is strictly newer than a clean `Version`.
    fn partial_cmp(&self, other: &DirtyVersion) -> Option<Ordering> {
        match self.cmp(&other.version) {
            Ordering::Equal if other.is_dirty() => Some(Ordering::Less),
            ord => Some(ord),
        }
    }
}

impl PartialEq<Version> for DirtyVersion {
    fn eq(&self, o: &Version) -> bool {
        o == self
    }
}

impl PartialOrd<Version> for DirtyVersion {
    fn partial_cmp(&self, o: &Version) -> Option<Ordering> {
        o.partial_cmp(self).map(Ordering::reverse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_version_accessor_returns_inner_version() {
        let dv: DirtyVersion = "v202401.0.1+gabc.d20260101".parse().unwrap();
        let v = dv.version();
        assert_eq!(v.to_string(), "v202401.0.1");
    }

    #[test]
    fn test_bumped_helpers_are_callable_publicly() {
        let a: Version = "v202401.0.1".parse().unwrap();
        let b: Version = "v202401.0.2".parse().unwrap();
        let c: Version = "v202401.1.0".parse().unwrap();
        let d: Version = "v202402.0.0".parse().unwrap();
        assert!(b.bumped_technical(&a));
        assert!(c.bumped_functional(&a));
        assert!(d.bumped_major(&a));
        assert!(!a.is_release_candidate());
    }

    #[test]
    fn test_dirty_version_is_dirty_public() {
        let clean: DirtyVersion = "v202401.0.1".parse().unwrap();
        let with_commit: DirtyVersion = "v202401.0.1+gabcdef".parse().unwrap();
        assert!(!clean.is_dirty());
        assert!(with_commit.is_dirty());
    }

    #[test]
    fn test_version_eq_clean_dirty_at_same_semver() {
        let v: Version = "v202401.0.1".parse().unwrap();
        let dv_clean: DirtyVersion = "v202401.0.1".parse().unwrap();
        let dv_dirty: DirtyVersion = "v202401.0.1+gabc".parse().unwrap();
        assert!(v == dv_clean);
        assert!(!(v == dv_dirty)); // dirty is strictly newer at same semver
    }

    #[test]
    fn test_version_lt_dirty_at_same_semver() {
        let v: Version = "v202401.0.1".parse().unwrap();
        let dv_dirty: DirtyVersion = "v202401.0.1+gabc".parse().unwrap();
        assert!(v < dv_dirty);
        assert!(dv_dirty > v);
    }

    #[test]
    fn test_version_cmp_dirty_when_semver_differs() {
        let v_old: Version = "v202401.0.1".parse().unwrap();
        let dv_new: DirtyVersion = "v202401.0.2".parse().unwrap();
        assert!(v_old < dv_new);
        let v_new: Version = "v202401.0.2".parse().unwrap();
        let dv_old: DirtyVersion = "v202401.0.1+gabc".parse().unwrap();
        assert!(v_new > dv_old);
    }

    #[test]
    fn test_dirty_eq_clean_symmetric() {
        let v: Version = "v202401.0.1".parse().unwrap();
        let dv: DirtyVersion = "v202401.0.1".parse().unwrap();
        assert!(dv == v);
    }
}
