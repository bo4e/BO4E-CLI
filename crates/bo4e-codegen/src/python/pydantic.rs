//! pydantic generator orchestration.
//!
//! Walks a [`Schemas`] collection and writes one Python module per schema, plus a
//! root `__init__.py`, `__version__.py`, and one empty `__init__.py` per subpackage
//! directory.
//!
//! ## Approach: build the vendored template context faithfully
//!
//! The vendored Jinja2 templates under `templates/python/pydantic/` are byte-identical
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
//!   `None` the include is skipped at render time. (pydantic outputs don't need `Config`.)
//! - **Imports are prepended to the rendered body.** The vendored `BaseModel.jinja2` only
//!   emits an import section for the SQL flavour; for the plain pydantic path we own
//!   the import block and stitch it on before writing the file.

use crate::error::Error;
use crate::layout::module_file_name;
use crate::naming::{sanitize_member_name, to_snake_case};
use crate::python::imports::ImportBlock;
use crate::python::types::{Import, enum_ref_target, literal_default, map_pydantic, schema_base};
use crate::python::{python_attr_name, root_init_module_docstring};
use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::SchemaRootType;
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

pub fn generate(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
) -> Result<crate::GenerateOutput, Error> {
    if opts.clear_output {
        crate::clear_dir_if_exists(output_dir)?;
    } else {
        std::fs::create_dir_all(output_dir)?;
    }
    let env = crate::env::make_environment(opts.templates_dir)?;

    let mut written: Vec<PathBuf> = Vec::new();
    let version_str = schemas.version.to_string();

    // ── Per-schema files ───────────────────────────────────────────────────────
    written.extend(crate::for_each_schema_file(
        schemas,
        output_dir,
        "py",
        |ctx| match &ctx.parsed {
            SchemaRootType::StrEnum(e) => {
                render_enum(&env, &ctx.class_name, &e.str_enum.enum_values)
            }
            SchemaRootType::Object(o) => render_object(&env, &ctx.class_name, &o.object, ctx.depth),
        },
    )?);

    // ── __version__.py at the root ─────────────────────────────────────────────
    let version_path = output_dir.join("__version__.py");
    std::fs::write(
        &version_path,
        format!("__version__: str = \"{version_str}\"\n"),
    )?;
    written.push(version_path);

    // ── Root __init__.py with re-exports ───────────────────────────────────────
    let init_tpl = env.get_template("python/pydantic/__init__.jinja2")?;
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
    std::fs::write(
        &init_path,
        format!("{}\n{init_body}", root_init_module_docstring(&version_str)),
    )?;
    written.push(init_path);

    // ── Empty __init__.py at every nested subdirectory ─────────────────────────
    // Build the module tree so nested depths (e.g. `foo/bar/Baz.json`) get
    // an __init__.py at every intermediate level, not just `foo/`.
    let tree = crate::layout::ModuleTree::from_schemas(schemas);
    crate::python::write_empty_subdir_inits_recursive(output_dir, &tree, &mut written)?;

    Ok(crate::GenerateOutput {
        written,
        diagnostics: vec![],
    })
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
            name: sanitize_member_name(v),
            default: format!("\"{v}\""),
            docstring: None,
        })
        .collect();

    let tpl = env.get_template("python/pydantic/Enum.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => class_name,
        base_class => "StrEnum",
        description => None::<String>,
        fields => fields,
    })?;

    Ok(stitch(class_name, &imports, 1, &rendered))
}

fn render_object(
    env: &Environment<'static>,
    class_name: &str,
    obj: &bo4e_schemas::models::json_schema::ObjectSchema,
    depth: usize,
) -> Result<String, Error> {
    let mut imports = ImportBlock::new();
    imports.extend([
        Import::Named {
            module: "pydantic".into(),
            name: "BaseModel".into(),
        },
        Import::Named {
            module: "pydantic".into(),
            name: "ConfigDict".into(),
        },
        Import::Named {
            module: "pydantic.alias_generators".into(),
            name: "to_camel".into(),
        },
    ]);

    let mut fields: Vec<PydanticField> = Vec::new();
    let mut needs_field_import = false;
    let required: BTreeSet<&str> = obj.required.iter().map(|s| s.as_str()).collect();

    for (prop_name, prop_schema) in &obj.properties {
        let mapped = map_pydantic(prop_schema).map_err(|e| Error::UnsupportedSchemaShape {
            schema_name: class_name.to_string(),
            property: prop_name.clone(),
            shape: e.0,
        })?;
        imports.extend(mapped.imports.iter().cloned());

        let is_required = required.contains(prop_name.as_str());

        // Strict matrix: the rendered type follows the schema's nullability
        // *only*. No `| None` auto-widening for optional fields — optionality
        // is expressed by the field's default expression below, not by
        // widening the type beyond what the schema declares.
        let type_str = mapped.rendered.clone();

        let name_snake = to_snake_case(prop_name);
        let python_name = python_attr_name(&name_snake);
        // Pydantic's to_camel alias generator handles snake↔camel automatically.
        // We only need an explicit alias when the rename strips a leading underscore
        // (or otherwise diverges from to_camel's roundtrip).
        let needs_alias = python_name != *prop_name;

        // Strict matrix: the field's default expression is the schema's
        // literal `default` (when present), with enum-`$ref` strings rewritten
        // to `EnumName.MEMBER`. The validator (`crate::validate`) enforces
        // `required ⇔ no default`, so we don't have to invent a `= None`
        // fallback for optional-without-default fields any more.
        let schema_default = literal_default(prop_schema);
        let schema_default = qualify_enum_default(schema_default, prop_schema, &mut imports);
        let default_expr: Option<String> = schema_default;

        let docstring = schema_base(prop_schema)
            .description
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let (field_expr, represented_default, required_flag) = if needs_alias {
            needs_field_import = true;
            let inner = match &default_expr {
                Some(d) => format!("default={d}, alias=\"{prop_name}\""),
                None => format!("..., alias=\"{prop_name}\""),
            };
            (Some(format!("Field({inner})")), String::new(), false)
        } else {
            let rep = default_expr.clone().unwrap_or_default();
            (None, rep, is_required)
        };

        fields.push(PydanticField {
            name: python_name,
            type_hint: type_str,
            field: field_expr,
            annotated: None,
            required: required_flag,
            represented_default,
            strip_default_none: false,
            docstring,
        });
    }

    if needs_field_import {
        imports.extend([Import::Named {
            module: "pydantic".into(),
            name: "Field".into(),
        }]);
    }

    let model_config = "model_config = ConfigDict(alias_generator=to_camel, \
                        populate_by_name=True, use_attribute_docstrings=True)";

    let tpl = env.get_template("python/pydantic/BaseModel.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => class_name,
        base_class => "BaseModel",
        description => obj.base.description.clone(),
        fields => fields,
        methods => Vec::<String>::new(),
        model_config => model_config,
        config => None::<String>,
        SQL => None::<String>,
    })?;

    Ok(stitch(class_name, &imports, depth, &rendered))
}

/// If `default` is a quoted string literal (`"VALUE"`) AND the property's
/// schema is (or `anyOf`-wraps) a `$ref` to an `enum/<Name>` schema,
/// promote the literal to `EnumName.<sanitized_member>` and inject the
/// matching sibling import. Anything else — non-string defaults, raw
/// `None`, quoted strings whose schema isn't an enum ref — passes through
/// untouched. Driven by schema shape only: no field-name special cases,
/// so `bo4e edit` changes flow through.
fn qualify_enum_default(
    default: Option<String>,
    prop_schema: &bo4e_schemas::models::json_schema::SchemaType,
    imports: &mut ImportBlock,
) -> Option<String> {
    let d = default?;
    let Some(value) = d.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return Some(d);
    };

    let Some((enum_name, enum_module)) = enum_ref_target(prop_schema) else {
        return Some(d);
    };

    imports.extend([Import::Sibling {
        module: enum_module,
        name: enum_name.clone(),
    }]);
    let member = sanitize_member_name(value);
    Some(format!("{enum_name}.{member}"))
}

/// Prepend module docstring + rendered import block to a template body.
fn stitch(class_name: &str, imports: &ImportBlock, depth: usize, body: &str) -> String {
    let docstring = format!("\"\"\"Contains class {class_name}.\"\"\"\n");
    let imports_text = imports.render(depth);
    let body_trimmed = body.trim_start_matches('\n');
    if imports_text.is_empty() {
        format!("{docstring}\n{body_trimmed}")
    } else {
        format!("{docstring}\n{imports_text}\n\n{body_trimmed}")
    }
}
