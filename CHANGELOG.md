# Changelog

All notable changes to this project are documented here. This file is generated
from conventional commits by [git-cliff](https://github.com/orhun/git-cliff).

## 0.1.0 - 2026-05-11

### Added

- **cli**: Verbose bump detection + dirty-version stamp + matrix chain order
- **codegen**: Roll pydantic v2 fixes into sql_model + raise on unclassifiable property
- **codegen**: Pydantic v2 generator improvements + import smoke test
- **cli**: Styled --help via clap Styles
- **cli/io/github**: Per-API-call and per-schema verbose lines
- **cli/generate**: Squish spinner during code generation
- **cli/diff**: Squish spinner during compare + matrix build
- **cli/io/cleanse**: Grenade spinner + verbose log lines
- **cli/io/github**: Show earth spinner during GitHub API calls
- **cli/console**: Add spinner factories (earth/squish/grenade)
- **codegen/sql_model**: Wire generate_sql_model end-to-end
- **codegen/sql_model**: Render package-level files
- **codegen/sql_model**: Render_table emits SQLModel class body
- **codegen/sql_model**: Vendor templates + register in env
- **codegen/sql_model**: Build_plan classifies refs/arrays/enums/Any
- **codegen/sql_model**: Build_plan handles enums + scalar fields
- **codegen**: Scaffold sql_model module with SqlPlan types
- **cli**: Wire bo4e generate subcommand for python-pydantic-v2
- **codegen**: Implement python-pydantic-v2 generator end-to-end
- **codegen**: Wire MiniJinja loader (embedded + --templates-dir override)
- **codegen**: Add import collector with deterministic block ordering
- **codegen**: Map JSON Schema to Python pydantic-v2 type strings
- **codegen**: Add naming utilities (module_file_name, to_snake_case)
- **codegen**: Add bo4e-codegen crate skeleton with feature flags and stub generate()
- **cli**: Wire Repo subcommand + end-to-end smoke test
- **cli/repo**: Add Repo command + Versions handler with hand-rolled table
- **io/github**: Add release_exists with 404 → Ok(false), 403 → guidance
- **io/git**: Implement get_last_n_tags via tags_merged + filter_tags
- **io/git**: Add get_ref with HEAD fallback; expose is_* helpers
- **io/git**: Implement tags_merged (real-git fixture test)
- **repo**: Filter_tags skip rules (RCs, technical bumps, first)
- **repo**: Filter_tags stop rules (n cap and threshold)
- **repo**: Scaffold filter module with FilterOptions
- **models**: Add RefKind enum for git reference classification
- **console**: Add cwarn!/cerror! macros, route cprint* to stdout
- **cli/diff**: Wire diff schemas/matrix/version-bump subcommand group
- **diff/version**: Check_version_bump with VersionBumpKind
- **diff/matrix**: Linear-chain validation + compatibility matrix generation
- **diff/diff**: Schema comparison via mutable Change collector
- **diff/filters**: Is_change_critical + has_critical
- **io/matrix**: CSV and JSON writers for compatibility matrix
- **io/changes**: Read/write diff JSON files
- **models/schema_meta**: Add module_difference and module_intersection iterators
- **cli**: Add --quiet flag, resolve verbose/quiet into Level
- **models/version**: Cross-type PartialEq/PartialOrd between Version and DirtyVersion
- **models/version**: Expose bumped/is_release_candidate/is_dirty + add DirtyVersion::version()

### Changed

- **cli/io/github**: Gate progress bar via CONSOLE, drop enable_output bool
- **cli/console/spinner**: Make quiet branch testable, rename inner fn
- **codegen/sql_model**: Split renderer out of mod.rs
- **codegen/python**: Share module_paths + subdir-init helpers
- **codegen/python**: Share literal_default + schema_base helpers
- **codegen/sql_model**: Drop allow(dead_code), precompute enum set
- **codegen**: Rename pydantic_v2 module/templates to pydantic
- **codegen**: Rename python-pydantic-v2 feature to python-pydantic
- **codegen**: Drop unused python-pydantic-v1 flavour
- **workspace**: Extract bo4e-schemas crate from bo4e-cli
- **workspace**: Move bo4e-cli into crates/ as workspace member
- **io/schemas**: Route 'skipping unreadable entry' through cwarn!
- **io/git**: Use parse_reference instead of get_ref
- Migrate 'Warning:' info-prefixed prints to cwarn! (stderr)
- **console**: Split print into print_info (stdout) + print_warn/print_error (stderr)
- **diff/matrix**: Drop wasted clones in determine_compatibility
- **edit**: Migrate cprint! call sites to cprint_normal!
- **console**: Cprint!(level, …) + cprint_{quiet,normal,verbose}! wrappers
- **console**: Replace verbose flag with Level-based emission
- **models/matrix**: Split CompatibilitySymbol/Text + Compatibility wrapper, IndexMap for ordering
- **visitable**: Drop nightly Try trait, use ControlFlow<B> directly
- **utils**: Replace iterator traversal with closure-based Visitor pattern

### Documentation

- **readme**: Some changes
- **readme**: Drop implementation-language details
- **readme**: Align with the Rust implementation
- **readme**: Trim install section to GitHub-Releases-only channels
- **readme**: Rewrite install section + add uninstall
- **cli/io/github**: Note why schema_downloads block is scoped
- Add cli-ux-and-robustness implementation plan
- Add cli-ux-and-robustness design spec
- README mentions for python-sql-model install + example
- **codegen/sql_model**: Note ManyLinks template divergence from upstream
- Fix stale 'all three' phrasing after dropping pydantic-v1
- Add sql-model generator implementation plan
- **sql-model spec**: Drop stale ConstantSchema TODO note
- Add sql-model generator design spec
- **generate**: Restore #supported-languages anchor and concrete example
- **generate**: Document Rust generate command and feature-gated install
- **generate**: Add implementation plan for Plan 1 (workspace + python-pydantic-v2)
- **generate**: Add design spec for Rust port of generate command
- Implementation plan for bo4e repo versions command port
- Design spec for bo4e repo versions command port
- Implementation plan for diff command Rust port
- **plans**: Add convenience wrappers for cprint levels
- **plans**: Redesign console as logging-level system
- **plans**: Add diff command design
- **plans**: Add edit command design doc
- **readme**: 📄Create documentation for CLI (#124)

### Fixed

- **highlighter**: Match full dirty version including commit + workdir date
- **highlighter**: Style absolute + relative paths on Windows and Unix
- **schemas**: Parse dirty version with .d<date> but no +g<commit>
- **github**: Actionable rate-limit errors + verbose token-discovery diagnostics
- **schemas**: Default optional ObjectSchema fields (resolves v202501.0.0 parse failure)
- **cli/console/spinner**: Clear rendered line on drop via RAII wrapper
- **codegen/sql_model**: Resolve relationship target module from plan, not hardcoded com
- **codegen/sql_model**: Escape id-field title and comment AnyOf skip path
- **codegen**: Guard empty sibling module, assert depth>=1, unify fmt helper
- **codegen**: Emit typing.Any import for Any/Object arms; clippy + test cleanup
- **codegen**: Exercise env loader in stub generate to silence dead_code
- **io/git**: Resolve commit refs to concrete SHA in get_ref
- **io/git**: Serialize cwd-mutating tests; tighten tags_merged parsing
- **models/version**: Normalize CRLF to LF and restore is_dirty doc

## 0.0.3 - 2025-07-12

### Fixed

- 🩹Fix version bump detection (#96)

## 0.0.2 - 2025-07-11

### Added

- Check for `BO4E-Schemas` releases in `bo4e repo versions` (#92)
- Adapt `Version` model to match the versions created by hatch in `BO4E-python` (#95)

### Fixed

- **diff**: Fix diff-detection for "dirty" versions (#91)
- **readme**: Fix status badges in README (#90)

## 0.0.1 - 2025-07-07

### Added

- **repo**: ✨Implement command `bo4e repo versions` (#88)
- **diff**: ✨Implement command `bo4e diff version-bump` (#86)
- **diff**: ✨Implement `bo4e diff matrix` command (#84)
- **generate**: ✨Implement code generation for Python (#27)
- **edit**: Implement command `bo4e edit` (#10)

### Changed

- **diff**: Preparation - Copied diff-matrix code from BO4E-python + Implement `diff schemas` command (#33)
- **schemas**: ♻️Use property instead of get method (#17)
- **generator**: 🚚Copy code from `BO4E-Python-Generator` (#16)

### Fixed

- **docs**: 🩹Fix links of test badges in readme (#85)
- **generator-python**: 🩹Fix python code generator for newer BO4E versions (#80)


