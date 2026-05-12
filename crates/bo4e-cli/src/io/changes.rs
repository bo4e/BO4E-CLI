use crate::models::changes::Changes;
use std::path::{Path, PathBuf};

pub fn read_changes_from_diff_files(paths: &[PathBuf]) -> Result<Vec<Changes>, String> {
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        if !p.exists() {
            return Err(format!("Diff file does not exist: {}", p.display()));
        }
        let text = std::fs::read_to_string(p)
            .map_err(|e| format!("Failed to read {}: {}", p.display(), e))?;
        let c: Changes = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse {} as Changes: {}", p.display(), e))?;
        out.push(c);
    }
    Ok(out)
}

pub fn write_changes(changes: &Changes, file_path: &Path) -> Result<(), String> {
    if let Some(parent) = file_path.parent()
        && !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
    let text = serde_json::to_string_pretty(changes)
        .map_err(|e| format!("Failed to serialize Changes: {}", e))?;
    std::fs::write(file_path, text)
        .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::Changes;
    use bo4e_schemas::models::schema_meta::Schemas;
    use bo4e_schemas::models::version::DirtyVersion;

    fn empty_changes(old: &str, new: &str) -> Changes {
        let v_old: DirtyVersion = old.parse().unwrap();
        let v_new: DirtyVersion = new.parse().unwrap();
        Changes {
            old_schemas: Schemas::new(v_old),
            new_schemas: Schemas::new(v_new),
            changes: vec![],
        }
    }

    #[test]
    fn test_roundtrip_write_then_read_preserves_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diff.json");
        let original = empty_changes("v202401.0.1", "v202401.0.2");

        write_changes(&original, &path).unwrap();
        let read_back = read_changes_from_diff_files(&[path]).unwrap();
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].old_version().to_string(), "v202401.0.1");
        assert_eq!(read_back[0].new_version().to_string(), "v202401.0.2");
    }

    #[test]
    fn test_read_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let err = read_changes_from_diff_files(&[path]).unwrap_err();
        assert!(err.contains("nope.json") || err.to_lowercase().contains("not"));
    }

    #[test]
    fn test_write_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/sub/diff.json");
        let c = empty_changes("v202401.0.1", "v202401.0.2");
        write_changes(&c, &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_pretty_indent_is_two_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.json");
        let c = empty_changes("v202401.0.1", "v202401.0.2");
        write_changes(&c, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // serde_json::to_string_pretty uses two-space indent: lines must contain "  \"".
        assert!(
            text.contains("  \""),
            "expected two-space indent in pretty output"
        );
    }
}
