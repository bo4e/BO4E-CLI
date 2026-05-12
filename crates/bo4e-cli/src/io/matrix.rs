use crate::models::matrix::CompatibilityMatrix;
use std::path::Path;

pub fn write_compatibility_matrix_csv(
    output: &Path,
    matrix: &CompatibilityMatrix,
    versions: &[String],
) -> Result<(), String> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
    if versions.len() < 2 {
        return Err("Need at least two versions to write a CSV matrix.".to_string());
    }
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b',')
        .terminator(csv::Terminator::Any(b'\n'))
        .escape(b'/')
        .from_path(output)
        .map_err(|e| format!("Failed to open {} for writing: {}", output.display(), e))?;

    // Header: ("", "v0 ↦ v1", "↦ v2", "↦ v3", …)
    let mut header: Vec<String> = Vec::with_capacity(versions.len());
    header.push(String::new());
    header.push(format!("{} \u{21A6} {}", versions[0], versions[1]));
    for v in &versions[2..] {
        header.push(format!("\u{21A6} {}", v));
    }
    wtr.write_record(&header)
        .map_err(|e| format!("CSV header write failed: {}", e))?;

    for (module_name, entries) in &matrix.root {
        let mut row: Vec<String> = Vec::with_capacity(entries.len() + 1);
        row.push(module_name.clone());
        for e in entries {
            row.push(e.compatibility.to_string());
        }
        wtr.write_record(&row)
            .map_err(|e| format!("CSV row write failed: {}", e))?;
    }

    wtr.flush().map_err(|e| format!("CSV flush failed: {}", e))
}

pub fn write_compatibility_matrix_json(
    output: &Path,
    matrix: &CompatibilityMatrix,
) -> Result<(), String> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
    let text = serde_json::to_string_pretty(matrix)
        .map_err(|e| format!("Failed to serialize matrix: {}", e))?;
    std::fs::write(output, text).map_err(|e| format!("Failed to write {}: {}", output.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::matrix::{
        Compatibility, CompatibilityMatrix, CompatibilityMatrixEntry, CompatibilitySymbol,
        CompatibilityText,
    };
    use bo4e_schemas::models::version::DirtyVersion;
    use indexmap::IndexMap;

    fn dv(s: &str) -> DirtyVersion {
        s.parse().unwrap()
    }

    fn entry(prev: &str, next: &str, c: Compatibility) -> CompatibilityMatrixEntry {
        CompatibilityMatrixEntry {
            previous_version: dv(prev),
            next_version: dv(next),
            compatibility: c,
        }
    }

    #[test]
    fn test_csv_header_uses_arrow_between_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.csv");

        let mut root = IndexMap::new();
        root.insert(
            "bo.Angebot".to_string(),
            vec![
                entry(
                    "v202401.0.1",
                    "v202401.0.2",
                    Compatibility::Text(CompatibilityText::ChangeNone),
                ),
                entry(
                    "v202401.0.2",
                    "v202401.1.0",
                    Compatibility::Text(CompatibilityText::ChangeNonCritical),
                ),
            ],
        );
        let m = CompatibilityMatrix { root };
        let versions = vec![
            "v202401.0.1".to_string(),
            "v202401.0.2".to_string(),
            "v202401.1.0".to_string(),
        ];

        write_compatibility_matrix_csv(&path, &m, &versions).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lines = text.lines();
        let header = lines.next().unwrap();
        assert_eq!(
            header,
            ",v202401.0.1 \u{21A6} v202401.0.2,\u{21A6} v202401.1.0"
        );
        let row = lines.next().unwrap();
        assert_eq!(row, "bo.Angebot,none,non-critical");
    }

    #[test]
    fn test_csv_emoji_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.csv");
        let mut root = IndexMap::new();
        root.insert(
            "bo.Angebot".to_string(),
            vec![entry(
                "v202401.0.1",
                "v202401.0.2",
                Compatibility::Symbol(CompatibilitySymbol::ChangeCritical),
            )],
        );
        let m = CompatibilityMatrix { root };
        let versions = vec!["v202401.0.1".to_string(), "v202401.0.2".to_string()];

        write_compatibility_matrix_csv(&path, &m, &versions).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\u{1F534}"),
            "expected red-circle emoji in csv"
        );
    }

    #[test]
    fn test_json_roundtrip_preserves_module_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.json");
        let mut root = IndexMap::new();
        root.insert(
            "bo.Angebot".to_string(),
            vec![entry(
                "v202401.0.1",
                "v202401.0.2",
                Compatibility::Text(CompatibilityText::ChangeNone),
            )],
        );
        root.insert(
            "com.Adresse".to_string(),
            vec![entry(
                "v202401.0.1",
                "v202401.0.2",
                Compatibility::Text(CompatibilityText::ChangeNone),
            )],
        );
        let m = CompatibilityMatrix { root };

        write_compatibility_matrix_json(&path, &m).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let pos_bo = text.find("bo.Angebot").unwrap();
        let pos_com = text.find("com.Adresse").unwrap();
        assert!(pos_bo < pos_com);
    }
}
