//! pydantic-v2 generator orchestration.
//!
//! Walks a [`Schemas`] collection and writes one Python module per schema, plus a
//! root `__init__.py`, `__version__.py`, and one empty `__init__.py` per subpackage
//! directory.
//!
//! ## Approach (Option A from Task 8 plan)
//!
//! The vendored Jinja2 templates under `templates/python/pydantic_v2/` are byte-identical
//! to the upstream `data-model-code-generator` templates used by the Python implementation
//! of `bo4e generate`. They expect a *rich* per-field context (`name`, `type_hint`, `field`,
//! `annotated`, `required`, `represented_default`, `strip_default_none`, `docstring`) and
//! a top-level class context (`class_name`, `base_class`, `decorators`, `description`,
//! `fields`, `methods`, `config`, `SQL`, `comment`). Rather than fork the templates, this
//! generator builds that exact context shape — keeping re-vendoring a clean `cp`.
//!
//! Two deliberate workarounds:
//!
//! - **`config` is never set.** The vendored `BaseModel.jinja2` `{%- if config %}` branch
//!   `{% include 'Config.jinja2' %}`s a template we don't ship. As long as `config` is
//!   `None` the include is skipped at render time. (pydantic-v2 outputs don't need `Config`.)
//! - **Imports are prepended to the rendered body.** The vendored `BaseModel.jinja2` only
//!   emits an import section for the SQL flavour; for the plain pydantic-v2 path we own
//!   the import block and stitch it on before writing the file.

use crate::error::Error;
use crate::naming::{module_file_name, to_snake_case};
use crate::python::imports::ImportBlock;
use crate::python::types::{Import, map_pydantic_v2};
use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::{PrimitiveValue, SchemaRootType, SchemaType};
use minijinja::{Environment, context};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Per-field context shape that mirrors what the vendored
/// `data-model-code-generator` `BaseModel.jinja2` template expects.
#[derive(Debug, Serialize)]
struct PydanticField {
    name: String,
    type_hint: String,
    /// Pydantic `Field(...)` expression, or `None` for plain assignment.
    field: Option<String>,
    /// Pre-baked `Annotated[...]` slot — unused by this generator.
    annotated: Option<String>,
    required: bool,
    represented_default: String,
    /// When true, a `= None` default is suppressed for required-or-Optional fields.
    strip_default_none: bool,
    docstring: Option<String>,
}

/// Per-enum-member context shape that mirrors `Enum.jinja2`.
#[derive(Debug, Serialize)]
struct EnumMember {
    name: String,
    default: String,
    docstring: Option<String>,
}

pub(crate) fn generate_pydantic_v2(
    schemas: &Schemas,
    output_dir: &Path,
    env: &Environment<'static>,
) -> Result<Vec<PathBuf>, Error> {
    std::fs::create_dir_all(output_dir)?;

    let mut written: Vec<PathBuf> = Vec::new();
    let version_str = schemas.version.to_string();

    // ── Per-schema files ───────────────────────────────────────────────────────
    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();

        let path_segments: Vec<String> = module
            .iter()
            .take(module.len().saturating_sub(1))
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let file_name = format!("{}.py", module_file_name(&module));

        let mut out_dir = output_dir.to_path_buf();
        for seg in &path_segments {
            out_dir.push(seg);
        }
        std::fs::create_dir_all(&out_dir)?;
        let out_path = out_dir.join(&file_name);

        // depth = number of dots in relative imports; matches ImportBlock semantics
        // (root-level module is depth 1, one subdir is depth 2, …).
        let depth = path_segments.len() + 1;

        // Resolve the parsed JSON Schema (clone so we drop the borrow before render).
        let parsed = schema.schema().map_err(Error::Schema)?.clone();
        drop(schema); // release the RefCell borrow before any further work

        let body = match &parsed {
            SchemaRootType::StrEnum(e) => render_enum(env, &class_name, &e.str_enum.enum_values)?,
            SchemaRootType::Object(o) => {
                render_object(env, &class_name, &o.object, depth)?
            }
        };

        std::fs::write(&out_path, body)?;
        written.push(out_path);
    }

    // ── __version__.py at the root ─────────────────────────────────────────────
    let version_path = output_dir.join("__version__.py");
    std::fs::write(
        &version_path,
        format!("__version__: str = \"{version_str}\"\n"),
    )?;
    written.push(version_path);

    // ── Root __init__.py with re-exports ───────────────────────────────────────
    let init_tpl = env.get_template("python/pydantic_v2/__init__.jinja2")?;
    let init_classes: Vec<_> = schemas
        .iter()
        .map(|s| {
            let s = s.borrow();
            let module = s.module();
            let lower: Vec<String> = module
                .iter()
                .take(module.len().saturating_sub(1))
                .map(|m| m.to_ascii_lowercase())
                .chain(std::iter::once(module_file_name(module)))
                .collect();
            context! {
                name => s.name().to_string(),
                module_path => lower,
            }
        })
        .collect();
    let init_body = init_tpl.render(context! { classes => init_classes })?;
    let init_path = output_dir.join("__init__.py");
    std::fs::write(&init_path, init_body)?;
    written.push(init_path);

    // ── Empty __init__.py per first-level subdirectory ─────────────────────────
    let mut subdirs: BTreeSet<String> = BTreeSet::new();
    for schema_rc in schemas {
        let s = schema_rc.borrow();
        let module = s.module();
        if module.len() > 1 {
            subdirs.insert(module[0].to_ascii_lowercase());
        }
    }
    for sub in subdirs {
        let p = output_dir.join(&sub).join("__init__.py");
        if !p.exists() {
            std::fs::write(&p, "")?;
            written.push(p);
        }
    }

    Ok(written)
}

// ── Renderers ────────────────────────────────────────────────────────────────

fn render_enum(
    env: &Environment<'static>,
    class_name: &str,
    members: &[String],
) -> Result<String, Error> {
    let mut imports = ImportBlock::new();
    imports.extend([Import::Named {
        module: "enum".into(),
        name: "StrEnum".into(),
    }]);

    let fields: Vec<EnumMember> = members
        .iter()
        .map(|v| EnumMember {
            name: v.clone(),
            default: format!("\"{v}\""),
            docstring: None,
        })
        .collect();

    let tpl = env.get_template("python/pydantic_v2/Enum.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => class_name,
        base_class => "StrEnum",
        description => None::<String>,
        fields => fields,
    })?;

    Ok(stitch(&imports, 1, &rendered))
}

fn render_object(
    env: &Environment<'static>,
    class_name: &str,
    obj: &bo4e_schemas::models::json_schema::ObjectSchema,
    depth: usize,
) -> Result<String, Error> {
    let mut imports = ImportBlock::new();
    imports.extend([Import::Named {
        module: "pydantic".into(),
        name: "BaseModel".into(),
    }]);

    let mut fields: Vec<PydanticField> = Vec::new();
    let required: BTreeSet<&str> = obj.required.iter().map(|s| s.as_str()).collect();

    for (prop_name, prop_schema) in &obj.properties {
        let mapped = map_pydantic_v2(prop_schema);
        imports.extend(mapped.imports.iter().cloned());

        let is_required = required.contains(prop_name.as_str());

        // pydantic-v2 dialect: optional fields render as `T | None` only when the
        // mapper hasn't already produced that union (e.g. via anyOf with null).
        let type_str = if is_required || mapped.rendered.contains("| None") {
            mapped.rendered.clone()
        } else {
            format!("{} | None", mapped.rendered)
        };

        let name_snake = to_snake_case(prop_name);
        let needs_alias = name_snake != *prop_name;

        // Choose the default expression. The schema may carry a JSON `default`;
        // otherwise optional fields default to None and required fields have no default.
        let schema_default = literal_default(prop_schema);
        let default_expr: Option<String> = if let Some(d) = schema_default {
            Some(d)
        } else if is_required {
            None
        } else {
            Some("None".into())
        };

        // Special-case: `version` on objects defaults to the imported `__version__`.
        let (default_expr, is_version_field) = if prop_name == "version" {
            (Some("__version__".to_string()), true)
        } else {
            (default_expr, false)
        };

        if is_version_field {
            // Sibling import shape: `from ..__version__ import __version__`. The
            // ImportBlock lowercases the *last* segment of `module`; since
            // `__version__` is already lowercase the result is unchanged.
            imports.extend([Import::Sibling {
                module: vec!["__version__".into()],
                name: "__version__".into(),
            }]);
        }

        // Build the rendered field expression.
        let (field_expr, represented_default, strip_default_none) = if needs_alias {
            // Alias requires a Field(...) call regardless of default presence.
            imports.extend([Import::Named {
                module: "pydantic".into(),
                name: "Field".into(),
            }]);
            let inner = match &default_expr {
                Some(d) => format!("default={d}, alias=\"{prop_name}\""),
                None => format!("..., alias=\"{prop_name}\""),
            };
            (Some(format!("Field({inner})")), String::new(), false)
        } else if is_version_field {
            // No alias but we still want the `= __version__` literal default.
            (None, "__version__".to_string(), false)
        } else {
            // Plain `name: type = default` (or no default for required). We keep
            // `strip_default_none=false` so optional fields render as `= None`
            // — matching the Python BO4E generator's parity contract.
            let rep = default_expr.clone().unwrap_or_default();
            (None, rep, false)
        };

        fields.push(PydanticField {
            name: name_snake,
            type_hint: type_str,
            field: field_expr,
            annotated: None,
            required: is_required && !needs_alias && !is_version_field,
            represented_default,
            strip_default_none,
            docstring: None,
        });
    }

    let tpl = env.get_template("python/pydantic_v2/BaseModel.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => class_name,
        base_class => "BaseModel",
        description => obj.base.description.clone(),
        fields => fields,
        methods => Vec::<String>::new(),
        config => None::<String>,
        SQL => None::<String>,
    })?;

    Ok(stitch(&imports, depth, &rendered))
}

/// Prepend a rendered import block (if non-empty) to a template body.
fn stitch(imports: &ImportBlock, depth: usize, body: &str) -> String {
    let imports_text = imports.render(depth);
    let body_trimmed = body.trim_start_matches('\n');
    if imports_text.is_empty() {
        body_trimmed.to_string()
    } else {
        format!("{imports_text}\n\n{body_trimmed}")
    }
}

/// Render a JSON Schema `default` (when present, primitive) as a Python literal expression.
fn literal_default(schema: &SchemaType) -> Option<String> {
    let base = match schema {
        SchemaType::StringSchema(s) => &s.base,
        SchemaType::IntegerSchema(s) => &s.base,
        SchemaType::NumberSchema(s) => &s.base,
        SchemaType::BooleanSchema(s) => &s.base,
        SchemaType::DecimalSchema(s) => &s.base,
        SchemaType::NullSchema(s) => &s.base,
        SchemaType::AnySchema(s) => &s.base,
        SchemaType::Array(s) => &s.base,
        SchemaType::AnyOf(s) => &s.base,
        SchemaType::AllOf(s) => &s.base,
        SchemaType::ConstantSchema(s) => &s.base,
        SchemaType::ReferenceSchema(s) => &s.base,
        SchemaType::Object(s) => &s.base,
        SchemaType::StrEnum(s) => &s.base,
    };
    base.default.as_ref().map(|v| match v {
        PrimitiveValue::Null => "None".into(),
        PrimitiveValue::Bool(b) => if *b { "True".into() } else { "False".into() },
        PrimitiveValue::Integer(i) => i.to_string(),
        PrimitiveValue::Float(f) => f.to_string(),
        PrimitiveValue::String(s) => format!("\"{s}\""),
    })
}
