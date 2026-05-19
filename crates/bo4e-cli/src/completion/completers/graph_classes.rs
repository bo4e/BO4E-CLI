use clap_complete::CompletionCandidate;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Scan `std::env::args_os()` for `-i <path>` or `--input <path>` and return
/// the value if found. Used by the shell-completion hook where no parsed
/// `ArgMatches` is available.
pub fn find_input_from_env_args() -> Option<PathBuf> {
    let mut args = std::env::args_os();
    while let Some(arg) = args.next() {
        let s = arg.to_string_lossy();
        if s == "-i" || s == "--input" {
            return args.next().map(PathBuf::from);
        }
        // Also handle `--input=<path>` form.
        if let Some(val) = s.strip_prefix("--input=") {
            return Some(PathBuf::from(val));
        }
    }
    None
}

/// Entry point for `ArgValueCompleter` on graph args that need the `-i` path.
pub fn complete(prefix: &OsStr) -> Vec<CompletionCandidate> {
    let input_path = find_input_from_env_args();
    complete_with_input(prefix, input_path.as_deref())
}

/// Pure function: given the prefix and an optional `-i` path, return the
/// candidates. This is the testable shape.
pub fn complete_with_input(
    prefix: &OsStr,
    input_path: Option<&Path>,
) -> Vec<CompletionCandidate> {
    let Some(path) = input_path else {
        return Vec::new();
    };
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(graph): Result<serde_json::Value, _> = serde_json::from_slice(&bytes) else {
        return Vec::new();
    };
    let mut candidates: Vec<String> = Vec::new();
    if let Some(nodes) = graph.get("nodes").and_then(|n| n.as_array()) {
        for n in nodes {
            // Bare class name plus dotted-module form.
            if let Some(name) = n.get("id").and_then(|s| s.as_str()) {
                candidates.push(name.to_string());
                if let Some(pkg) = n.get("package").and_then(|s| s.as_str()) {
                    candidates.push(format!("{pkg}.{name}"));
                }
            }
        }
    }
    let prefix = prefix.to_string_lossy().to_string();
    candidates
        .into_iter()
        .filter(|c| c.starts_with(&prefix))
        .map(CompletionCandidate::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn returns_empty_when_input_absent() {
        let r = complete_with_input(&OsString::from(""), None);
        assert!(r.is_empty());
    }

    #[test]
    fn returns_empty_when_file_unreadable() {
        let r = complete_with_input(&OsString::from(""), Some(Path::new("/nonexistent")));
        assert!(r.is_empty());
    }

    #[test]
    fn returns_classes_from_valid_graph() {
        let mut f = NamedTempFile::new().unwrap();
        let graph = serde_json::json!({
            "nodes": [
                { "id": "Angebot", "package": "bo" },
                { "id": "Vertrag", "package": "bo" },
            ]
        });
        f.write_all(serde_json::to_string(&graph).unwrap().as_bytes()).unwrap();
        let r = complete_with_input(&OsString::from(""), Some(f.path()));
        let names: Vec<_> = r.iter().map(|c| c.get_value().to_string_lossy().to_string()).collect();
        assert!(names.contains(&"Angebot".to_string()));
        assert!(names.contains(&"bo.Angebot".to_string()));
        assert!(names.contains(&"Vertrag".to_string()));
    }

    #[test]
    fn malformed_json_returns_empty() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"not json").unwrap();
        let r = complete_with_input(&OsString::from(""), Some(f.path()));
        assert!(r.is_empty());
    }

    #[test]
    fn prefix_filter_applies() {
        let mut f = NamedTempFile::new().unwrap();
        let graph = serde_json::json!({
            "nodes": [
                { "id": "Angebot", "package": "bo" },
                { "id": "Vertrag", "package": "bo" },
            ]
        });
        f.write_all(serde_json::to_string(&graph).unwrap().as_bytes()).unwrap();
        let r = complete_with_input(&OsString::from("Ver"), Some(f.path()));
        let names: Vec<_> = r.iter().map(|c| c.get_value().to_string_lossy().to_string()).collect();
        assert!(names.contains(&"Vertrag".to_string()));
        assert!(!names.contains(&"Angebot".to_string()));
    }
}
