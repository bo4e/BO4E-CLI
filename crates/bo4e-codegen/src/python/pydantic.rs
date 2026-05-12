//! pydantic generator orchestration.
//!
//! Walks a [`Schemas`] collection and writes one Python module per schema, plus a
//! root `__init__.py`, `__version__.py`, and one empty `__init__.py` per subpackage
//! directory.
//!
//! ## Approach (Option A from Task 8 plan)
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
use crate::naming::{module_file_name, sanitize_member_name, to_snake_case};
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

pub(crate) fn generate_pydantic(
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

        let (out_dir, file_name, depth) = crate::python::module_paths(output_dir, &module);
        std::fs::create_dir_all(&out_dir)?;
        let out_path = out_dir.join(&file_name);

        // Resolve the parsed JSON Schema (clone so we drop the borrow before render).
        let parsed = schema.schema().map_err(Error::Schema)?.clone();
        drop(schema); // release the RefCell borrow before any further work

        let body = match &parsed {
            SchemaRootType::StrEnum(e) => render_enum(env, &class_name, &e.str_enum.enum_values)?,
            SchemaRootType::Object(o) => {
                render_object(env, &class_name, &module, &o.object, depth)?
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

    // ── Empty __init__.py per first-level subdirectory ─────────────────────────
    let modules: Vec<Vec<String>> = schemas
        .iter()
        .map(|s| s.borrow().module().to_vec())
        .collect();
    let subdirs = crate::python::first_level_subdirs(modules.iter().map(|m| m.as_slice()));
    crate::python::write_empty_subdir_inits(output_dir, &subdirs, &mut written)?;

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
    parent_module: &[String],
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
        let mut mapped = map_pydantic(prop_schema);
        // BO4E `_typ` discriminators carry exactly one allowed value (either as
        // `const: "X"` or as a one-entry `enum: ["X"]` — both shapes appear in the
        // schemas). Tighten the type to `Literal[BoTyp.<MEMBER>]` (or
        // `Literal[ComTyp.<MEMBER>]`) so the type system reflects the single-value
        // constraint instead of the loose `str` fallback from `map_pydantic`.
        if prop_name == "_typ" {
            use bo4e_schemas::models::json_schema::SchemaType;
            let const_value: Option<&str> = match prop_schema {
                SchemaType::ConstantSchema(c) => Some(c.constant.as_str()),
                SchemaType::StrEnum(s) if s.enum_values.len() == 1 => {
                    Some(s.enum_values[0].as_str())
                }
                _ => None,
            };
            if let Some(value) = const_value {
                let typing_enum = match parent_module.first().map(|s| s.as_str()) {
                    Some("bo") => Some(("BoTyp", vec!["enum".to_string(), "BoTyp".to_string()])),
                    Some("com") => Some(("ComTyp", vec!["enum".to_string(), "ComTyp".to_string()])),
                    _ => None,
                };
                if let Some((enum_name, enum_module)) = typing_enum {
                    let member = sanitize_member_name(value);
                    mapped.rendered = format!("Literal[{enum_name}.{member}]");
                    mapped.imports.clear();
                    mapped.imports.insert(Import::Named {
                        module: "typing".into(),
                        name: "Literal".into(),
                    });
                    mapped.imports.insert(Import::Sibling {
                        module: enum_module,
                        name: enum_name.into(),
                    });
                }
            }
        }
        imports.extend(mapped.imports.iter().cloned());

        let is_required = required.contains(prop_name.as_str());

        // pydantic dialect: optional fields render as `T | None` only when the
        // mapper hasn't already produced that union (e.g. via anyOf with null).
        // `Any` already covers None, so don't widen it.
        let type_str =
            if is_required || mapped.rendered == "Any" || mapped.rendered.contains("| None") {
                mapped.rendered.clone()
            } else {
                format!("{} | None", mapped.rendered)
            };

        let name_snake = to_snake_case(prop_name);
        let python_name = python_attr_name(&name_snake);
        // Pydantic's to_camel alias generator handles snake↔camel automatically.
        // We only need an explicit alias when the rename strips a leading underscore
        // (or otherwise diverges from to_camel's roundtrip).
        let needs_alias = python_name != *prop_name;

        // Choose the default expression. The schema may carry a JSON `default`;
        // otherwise optional fields default to None and required fields have no default.
        // String defaults that originate from an enum are qualified as `EnumName.MEMBER`
        // instead of bare string literals — see qualify_enum_default.
        let schema_default = literal_default(prop_schema);
        let schema_default = qualify_enum_default(
            schema_default,
            prop_schema,
            prop_name,
            parent_module,
            &mut imports,
        );
        let default_expr: Option<String> = if let Some(d) = schema_default {
            Some(d)
        } else if is_required {
            None
        } else {
            Some("None".into())
        };

        // Special-case: `_version` carries the BO4E version of the schema; default it
        // to the live module-level `__version__` constant so generated objects round-trip.
        let is_version_field = prop_name == "_version";
        let default_expr = if is_version_field {
            Some("__version__".to_string())
        } else {
            default_expr
        };

        if is_version_field {
            imports.extend([Import::Sibling {
                module: vec!["__version__".into()],
                name: "__version__".into(),
            }]);
        }

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
            (None, rep, is_required && !is_version_field)
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

/// If `default` is a quoted string literal (`"VALUE"`) AND the property's type is an
/// enum, promote the literal to `EnumName.<sanitized_member>` and add the matching
/// import to `imports`. Otherwise return `default` unchanged.
///
/// Two enum-detection paths:
/// 1. The schema directly references (or `anyOf:[$ref, null]`-wraps) an `enum/<Name>`
///    schema — e.g. `Adresse.landescode` (default `"DE"` → `Landescode.DE`),
///    `Bilanzierung._typ` (default `"BILANZIERUNG"` → `BoTyp.BILANZIERUNG`).
/// 2. The field is named `_typ` and the parent schema lives in `bo/` or `com/` —
///    even when the schema is an inline `const`/`enum` string with no `$ref`,
///    BO4E convention says the value belongs to `BoTyp`/`ComTyp` respectively, so
///    we promote `default="ANGEBOT"` to `default=BoTyp.ANGEBOT` and import `BoTyp`.
fn qualify_enum_default(
    default: Option<String>,
    prop_schema: &bo4e_schemas::models::json_schema::SchemaType,
    prop_name: &str,
    parent_module: &[String],
    imports: &mut ImportBlock,
) -> Option<String> {
    let d = default?;
    // Only quoted string literals can be enum members; pass through anything else
    // (`None`, `True`, integers, …) untouched.
    let value = d.strip_prefix('"').and_then(|s| s.strip_suffix('"'))?;

    let (enum_name, enum_module) = enum_ref_target(prop_schema).or_else(|| {
        // Fallback: special-case `_typ` for BO/COM modules where the schema is an
        // inline const string with no enum $ref (most BO models follow this shape).
        if prop_name != "_typ" {
            return None;
        }
        match parent_module.first().map(|s| s.as_str()) {
            Some("bo") => Some(("BoTyp".to_string(), vec!["enum".into(), "BoTyp".into()])),
            Some("com") => Some(("ComTyp".to_string(), vec!["enum".into(), "ComTyp".into()])),
            _ => None,
        }
    })?;

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
