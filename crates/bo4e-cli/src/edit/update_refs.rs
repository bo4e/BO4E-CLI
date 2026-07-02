use crate::cprint_verbose;
use bo4e_schemas::models::json_schema::ReferenceSchema;
use bo4e_schemas::models::schema_meta::{Schema, Schemas};
use bo4e_schemas::visitable::{Visitable, cntrl_to_result, result_to_cntrl};
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::LazyLock;

static REF_ONLINE_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"^https://raw\.githubusercontent\.com/(?:BO4E|bo4e|Bo4e|Hochfrequenz)/BO4E-Schemas/(?P<version>[^/]+)/src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$",
    )
    .unwrap()
});
static REF_DEFS_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$").unwrap());

fn update_reference(
    reference: &mut ReferenceSchema,
    current_module: &[String],
    namespace: &HashMap<String, Vec<String>>,
    version: &str,
) -> Result<(), String> {
    let reference_module_path: Vec<String>;

    if let Some(caps) = REF_ONLINE_REGEX.captures(&reference.r#ref) {
        let ref_version = caps.name("version").unwrap().as_str();
        if ref_version != version {
            return Err(format!(
                "Version mismatch: '{}' does not match '{}' for reference '{}'",
                ref_version, version, reference.r#ref
            ));
        }
        let sub_path = caps.name("sub_path").map_or("", |m| m.as_str());
        let model = caps.name("model").unwrap().as_str();
        reference_module_path = sub_path
            .split('/')
            .filter(|s| !s.is_empty())
            .chain(std::iter::once(model))
            .map(String::from)
            .collect();
    } else if let Some(caps) = REF_DEFS_REGEX.captures(&reference.r#ref) {
        let model = caps.name("model").unwrap().as_str();
        reference_module_path = namespace
            .get(model)
            .cloned()
            .ok_or_else(|| format!("Could not find schema '{}' in namespace", model))?;
    } else {
        cprint_verbose!(
            "Reference unchanged. Could not parse reference: {}",
            reference.r#ref
        );
        return Ok(());
    }

    // Find the index where reference_module_path diverges from current_module.
    let diverge = reference_module_path
        .iter()
        .zip(current_module.iter())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| reference_module_path.len().min(current_module.len()));

    let relative_ref = if diverge == reference_module_path.len() && diverge == current_module.len()
    {
        // Identical module paths — self-reference.
        "#".to_string()
    } else {
        // How many levels up from current_module to the divergence point.
        // current_module has `current_module.len()` segments; we stop at `diverge`,
        // then need to go up `current_module.len() - diverge - 1` levels
        // (minus 1 because the last segment is the file name, not a directory).
        let up = current_module.len().saturating_sub(diverge + 1);
        let remaining = reference_module_path[diverge..].join("/");
        format!("{}{}.json#", "../".repeat(up), remaining)
    };

    cprint_verbose!("Updated reference {} to: {}", reference.r#ref, relative_ref);
    reference.r#ref = relative_ref;
    Ok(())
}

fn update_references_single(
    schema: &mut Schema,
    namespace: &HashMap<String, Vec<String>>,
    version: &str,
) -> Result<(), String> {
    let module: Vec<String> = schema.module().to_vec();
    let visitable: &mut dyn Visitable = schema.schema_mut()?;
    cntrl_to_result(
        visitable.try_visit_all_mut::<ReferenceSchema, String>(&mut |reference| {
            result_to_cntrl(update_reference(reference, &module, namespace, version))
        }),
    )
}

pub fn update_references_all(schemas: &mut Schemas) -> Result<(), String> {
    let namespace = schemas.modules_by_name();
    let version = schemas.version.to_string();
    for schema in schemas.iter_mut() {
        update_references_single(schema.borrow_mut().deref_mut(), &namespace, &version)?;
    }
    Ok(())
}

/// Resolve a `$ref` string to a canonical module path (e.g. `["bo", "Geschaeftspartner"]`),
/// independent of how the reference is expressed. Handles three forms:
///
/// - Online URL: `https://raw.githubusercontent.com/.../src/bo4e_schemas/bo/Foo.json`
/// - Self-reference: `#` (resolves to `current_module`)
/// - Relative path: `../com/Bar.json#`, `Foo.json#` (resolved against `current_module`'s parent)
///
/// Returns `None` for forms we cannot resolve here (e.g. `#/$defs/Foo` requires a namespace
/// map). Callers should fall back to literal string comparison when this returns `None`.
pub fn canonical_ref(ref_str: &str, current_module: &[String]) -> Option<Vec<String>> {
    if let Some(caps) = REF_ONLINE_REGEX.captures(ref_str) {
        let sub_path = caps.name("sub_path").map_or("", |m| m.as_str());
        let model = caps.name("model").unwrap().as_str();
        return Some(
            sub_path
                .split('/')
                .filter(|s| !s.is_empty())
                .chain(std::iter::once(model))
                .map(String::from)
                .collect(),
        );
    }

    if ref_str == "#" || ref_str.is_empty() {
        return Some(current_module.to_vec());
    }

    // `#/$defs/...` form requires a namespace map — caller falls back to literal compare.
    if ref_str.starts_with('#') {
        return None;
    }

    // Strip JSON pointer fragment, then resolve as a filesystem-style path.
    let path = ref_str.split('#').next().unwrap_or("");
    let mut resolved: Vec<String> = current_module
        .iter()
        .take(current_module.len().saturating_sub(1))
        .cloned()
        .collect();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            resolved.pop()?;
        } else {
            resolved.push(segment.to_string());
        }
    }
    let last = resolved.last_mut()?;
    let stripped = last.strip_suffix(".json")?;
    *last = stripped.to_string();
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};
    use bo4e_schemas::models::json_schema::ReferenceSchema;
    use std::collections::HashMap;

    fn init_console() {
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    fn make_ref(r: &str) -> ReferenceSchema {
        ReferenceSchema {
            base: Default::default(),
            r#ref: r.to_string(),
        }
    }

    fn namespace(entries: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn test_online_ref_same_dir() {
        init_console();
        // Reference from bo/Angebot to bo/Angebot — same module → "#"
        let mut r = make_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0/\
             src/bo4e_schemas/bo/Angebot.json",
        );
        let module = vec!["bo".to_string(), "Angebot".to_string()];
        let ns = namespace(&[("Angebot", &["bo", "Angebot"])]);
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        assert_eq!(r.r#ref, "#");
    }

    #[test]
    fn test_online_ref_cross_dir() {
        init_console();
        // Reference from com/Adresse to bo/Angebot — one level up, one level down
        let mut r = make_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0/\
             src/bo4e_schemas/bo/Angebot.json",
        );
        let module = vec!["com".to_string(), "Adresse".to_string()];
        let ns = namespace(&[("Angebot", &["bo", "Angebot"])]);
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        assert_eq!(r.r#ref, "../bo/Angebot.json#");
    }

    #[test]
    fn test_defs_ref_rewritten() {
        init_console();
        let mut r = make_ref("#/$defs/Angebot");
        let module = vec!["com".to_string(), "Adresse".to_string()];
        let ns = namespace(&[("Angebot", &["bo", "Angebot"])]);
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        assert_eq!(r.r#ref, "../bo/Angebot.json#");
    }

    #[test]
    fn test_unknown_ref_unchanged() {
        init_console();
        let mut r = make_ref("../already/relative.json#");
        let module = vec!["bo".to_string(), "Foo".to_string()];
        let ns = HashMap::new();
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        assert_eq!(r.r#ref, "../already/relative.json#");
    }

    #[test]
    fn test_canonical_ref_online_url() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let canon = canonical_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202501.0.0/\
             src/bo4e_schemas/bo/Geschaeftspartner.json",
            &m,
        );
        assert_eq!(
            canon,
            Some(vec!["bo".to_string(), "Geschaeftspartner".to_string()])
        );
    }

    #[test]
    fn test_canonical_ref_self_ref() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        assert_eq!(canonical_ref("#", &m), Some(m.clone()));
        assert_eq!(canonical_ref("", &m), Some(m));
    }

    #[test]
    fn test_canonical_ref_relative_same_dir() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let canon = canonical_ref("Geschaeftspartner.json#", &m);
        assert_eq!(
            canon,
            Some(vec!["bo".to_string(), "Geschaeftspartner".to_string()])
        );
    }

    #[test]
    fn test_canonical_ref_relative_cross_dir() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let canon = canonical_ref("../com/Adresse.json#", &m);
        assert_eq!(canon, Some(vec!["com".to_string(), "Adresse".to_string()]));
    }

    #[test]
    fn test_canonical_ref_defs_form_unresolvable() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        assert_eq!(canonical_ref("#/$defs/Foo", &m), None);
    }

    #[test]
    fn test_canonical_ref_online_and_relative_match() {
        // The two forms a $ref might take for the same target should canonicalize identically.
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let online = canonical_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202501.0.0/\
             src/bo4e_schemas/bo/Geschaeftspartner.json",
            &m,
        );
        let relative = canonical_ref("Geschaeftspartner.json#", &m);
        assert_eq!(online, relative);
    }

    #[test]
    fn test_version_mismatch_is_error() {
        init_console();
        let mut r = make_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.0.0/\
             src/bo4e_schemas/bo/Angebot.json",
        );
        let module = vec!["bo".to_string(), "Foo".to_string()];
        let ns = HashMap::new();
        let result = update_reference(&mut r, &module, &ns, "v202401.1.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Version mismatch"));
    }
}
