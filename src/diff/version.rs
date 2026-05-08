use crate::cprint_verbose;
use crate::models::changes::Changes;
use crate::models::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionBumpKind {
    Technical,
    Functional,
    Major,
}

pub fn check_version_bump(
    changes: &Changes,
    major_bump_allowed: bool,
) -> Result<VersionBumpKind, String> {
    let v_old: Version = changes.old_version().try_into().map_err(|e: String| {
        format!("Old version of diff is dirty and cannot serve as a baseline: {e}")
    })?;
    let v_new: Version = changes.new_version().try_into().map_err(|e: String| {
        format!("New version of diff is dirty and cannot be validated: {e}")
    })?;

    cprint_verbose!("Checking bump from {} to {}", v_old, v_new);

    if v_new <= v_old {
        return Err("The new version must be newer than the old version.".into());
    }

    if v_new.bumped_major(&v_old) {
        if !major_bump_allowed {
            return Err("Major version bump detected. Major bump is not allowed.".into());
        }
        return Ok(VersionBumpKind::Major);
    }

    let functional = !changes.changes.is_empty();
    let is_functional_bump = v_new.bumped_functional(&v_old);

    if functional && !is_functional_bump {
        return Err("Technical bump detected but functional changes found.".into());
    }
    if !functional && is_functional_bump {
        return Err("Functional bump detected but no functional changes found.".into());
    }

    Ok(if functional {
        VersionBumpKind::Functional
    } else {
        VersionBumpKind::Technical
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};
    use crate::models::changes::{Change, ChangeType};
    use crate::models::schema_meta::Schemas;
    use crate::models::version::DirtyVersion;

    fn ensure_console() {
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    fn changes(old: &str, new: &str, items: Vec<Change>) -> Changes {
        ensure_console();
        let v_old: DirtyVersion = old.parse().unwrap();
        let v_new: DirtyVersion = new.parse().unwrap();
        Changes {
            old_schemas: Schemas::new(v_old),
            new_schemas: Schemas::new(v_new),
            changes: items,
        }
    }

    fn ch(t: ChangeType) -> Change {
        Change {
            r#type: t,
            old: None,
            new: None,
            old_trace: String::new(),
            new_trace: String::new(),
        }
    }

    #[test]
    fn test_errors_when_old_version_is_dirty() {
        let c = changes("v202401.0.1+gabc", "v202401.0.2", vec![]);
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("dirty"));
    }

    #[test]
    fn test_errors_when_new_not_newer_than_old() {
        let c = changes("v202401.0.2", "v202401.0.1", vec![]);
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("newer"));
    }

    #[test]
    fn test_major_bump_disallowed_returns_err() {
        let c = changes("v202401.0.1", "v202402.0.0", vec![]);
        let err = check_version_bump(&c, false).unwrap_err();
        assert!(err.to_lowercase().contains("major"));
    }

    #[test]
    fn test_major_bump_allowed_returns_major() {
        let c = changes("v202401.0.1", "v202402.0.0", vec![]);
        assert_eq!(
            check_version_bump(&c, true).unwrap(),
            VersionBumpKind::Major
        );
    }

    #[test]
    fn test_functional_bump_with_no_changes_errors() {
        let c = changes("v202401.0.1", "v202401.1.0", vec![]);
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("functional"));
    }

    #[test]
    fn test_technical_bump_with_changes_errors() {
        let c = changes(
            "v202401.0.1",
            "v202401.0.2",
            vec![ch(ChangeType::FieldAdded)],
        );
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("technical"));
    }

    #[test]
    fn test_valid_technical_bump() {
        let c = changes("v202401.0.1", "v202401.0.2", vec![]);
        assert_eq!(
            check_version_bump(&c, true).unwrap(),
            VersionBumpKind::Technical
        );
    }

    #[test]
    fn test_valid_functional_bump() {
        let c = changes(
            "v202401.0.1",
            "v202401.1.0",
            vec![ch(ChangeType::FieldAdded)],
        );
        assert_eq!(
            check_version_bump(&c, true).unwrap(),
            VersionBumpKind::Functional
        );
    }
}
