
## 1.3.0 - 2026-07-14

### Added

- **diff**: Replace version-title heuristic with regex trace-ignore filters (#214)


## 1.2.5 - 2026-07-02

### Fixed

- **pull**: Bound GitHub requests with connect + request timeouts (#201)

### Performance

- **graph**: Vec-indexed Louvain instead of HashMap<NodeIndex> (#203)
- **pull**: Fetch schemas via one repo tarball instead of ~200 API calls (#199)
- Shrink release binary ~13MB→5.7MB (profile, deps, drop octocrab) (#198)


## 1.2.4 - 2026-07-02

### Added

- **graph**: Add .lower/.upper case accessors to link-template placeholders (#196)


## 1.2.3 - 2026-06-05

### Fixed

- **github**: Accept JWT-style `ghs_…` Actions installation tokens (#178)


## 1.2.2 - 2026-06-04

### Fixed

- **github**: Drop upper length cap on gh*_ token regex (#176)


## 1.2.1 - 2026-05-25

### Fixed

- **diff**: Use ∅ instead of - for NonExistent in matrix emote output (#168)


## 1.2.0 - 2026-05-24

### Added

- **diff**: Detect ConstantSchema.format changes (#157)
- **diff**: Detect ConstantSchema.constant changes (FieldConstantChanged) (#156)
- **diff**: --include-version-changes flag + dirty-version regex fix (#158)

### Fixed

- **diff**: Detect TypeBase changes on leaf SchemaType variants (#155)


## 1.1.1 - 2026-05-20

### Added

- **release**: Publish bo4e-cli to Homebrew via bo4e/homebrew-tap (#153)
- **completion**: Add shell completion with install/uninstall/show + dynamic completers (#152)


## 1.1.0 - 2026-05-18

### Added

- **graph**: Accept bare class name on overview --reachable-from (#150)
- **graph**: Add extract / overview / single subcommands (#148)
- **generate**: Add rust-plain and rust-crate output flavours (#136)

### Changed

- **codegen**: Lift language-neutral helpers into shared modules (#135)

### Documentation

- **readme**: Document bo4e graph extract / overview / single (#149)
- Add AGENTS.md playbook and per-crate STRUCTURE.md files (#134)

## 1.0.0 - 2026-05-11

### Documentation

- **readme**: 📄Create documentation for CLI (#124)

### Features

- Rust port of BO4E-CLI (#123)

## 0.0.3 - 2025-07-12

### What's Changed

- fix: 🩹Fix version bump detection by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/96
- chore: 🔊Add some debugging details for parsing errors of JSON schemas by @lord-haffi
  in https://github.com/bo4e/BO4E-CLI/pull/97

**Full Changelog**: https://github.com/bo4e/BO4E-CLI/compare/v0.0.2...v0.0.3

## 0.0.2 - 2025-07-11

### What's Changed

- fix(readme): Fix status badges in README by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/90
- fix(diff): Fix diff-detection for "dirty" versions by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/91
- chore: 🔊Output if token is retrieved from GH CLI by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/93
- feat: Adapt `Version` model to match the versions created by hatch in `BO4E-python` by @lord-haffi
  in https://github.com/bo4e/BO4E-CLI/pull/95
- test: Add unittest if typer command can be executed as python function by @lord-haffi
  in https://github.com/bo4e/BO4E-CLI/pull/94
- feat: Check for `BO4E-Schemas` releases in `bo4e repo versions` by @lord-haffi
  in https://github.com/bo4e/BO4E-CLI/pull/92

**Full Changelog**: https://github.com/bo4e/BO4E-CLI/compare/v0.0.1...v0.0.2

## 0.0.1 - 2025-07-07

### What's Changed

- Project init by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/2
- Implement CLI structure (without any logic yet) by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/3
- Bump typer from 0.12.4 to 0.12.5 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/5
- Bump mypy from 1.11.1 to 1.11.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/6
- Bump pylint from 3.2.6 to 3.2.7 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/7
- Bump rich from 13.7.1 to 13.8.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/8
- Implement `bo4e pull` to pull JSON-Schemas from GitHub by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/4
- style(console): Improve styling in console + add `--verbose` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/9
- Bump more-itertools from 10.2.0 to 10.5.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/11
- Bump cffi from 1.17.0 to 1.17.1 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/13
- Bump build from 1.2.1 to 1.2.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/12
- Bump cryptography from 43.0.0 to 43.0.1 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/14
- Bump pydantic from 2.8.2 to 2.9.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/15
- feat(edit): Implement command `bo4e edit` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/10
- refactor(generate): 🚚Copy code from `BO4E-Python-Generator` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/16
- build(deps): bump pygithub from 2.3.0 to 2.4.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/19
- build(deps): bump urllib3 from 2.2.2 to 2.2.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/21
- build(deps-dev): bump pytest from 8.3.2 to 8.3.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/25
- refactor(schemas): ♻️Use property instead of get method by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/17
- build(deps): bump rich from 13.8.0 to 13.8.1 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/28
- build(deps): bump dnspython from 2.6.1 to 2.7.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/29
- feat(generate): ✨Implement code generation for Python by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/27
- build(deps): bump idna from 3.8 to 3.10 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/23
- build(deps): ⬆️Bump all dependencies by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/32
- build(deps-dev): bump coverage from 7.6.3 to 7.6.4 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/34
- build(deps): bump markupsafe from 3.0.1 to 3.0.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/35
- build(deps-dev): bump mypy from 1.12.0 to 1.13.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/38
- build(deps): bump cryptography from 43.0.1 to 43.0.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/39
- build(deps): bump sqlalchemy from 2.0.35 to 2.0.36 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/41
- build(deps): bump rich from 13.9.2 to 13.9.4 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/42
- build(deps): bump typer from 0.12.5 to 0.13.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/44
- build(deps): bump pygithub from 2.4.0 to 2.5.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/45
- build(deps): bump pyjwt[crypto] from 2.9.0 to 2.10.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/48
- build(deps): bump deprecated from 1.2.14 to 1.2.15 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/49
- build(deps-dev): bump coverage from 7.6.4 to 7.6.8 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/51
- build(deps-dev): bump pylint-pydantic from 0.3.2 to 0.3.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/54
- build(deps): bump argcomplete from 3.5.1 to 3.5.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/56
- build(deps): bump all dependencies by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/72
- build(deps): bump argcomplete from 3.5.3 to 3.6.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/73
- fix(generator-python): 🩹Fix python code generator for newer BO4E versions by @lord-haffi
  in https://github.com/bo4e/BO4E-CLI/pull/80
- refactor(diff): Preparation - Copied diff-matrix code from BO4E-python + Implement `diff schemas` command by
  @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/33
- chore(deps): ⬆️Bump dependencies by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/83
- feat(diff): ✨Implement `bo4e diff matrix` command by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/84
- fix(docs): 🩹Fix links of test badges in readme by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/85
- feat(diff): ✨Implement command `bo4e diff version-bump` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/86
- feat(repo): ✨Implement command `bo4e repo versions` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/88
- ci(publish): Add publishing workflow for PyPi by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/89

### New Contributors

- @lord-haffi made their first contribution in https://github.com/bo4e/BO4E-CLI/pull/2
- @dependabot made their first contribution in https://github.com/bo4e/BO4E-CLI/pull/5

**Full Changelog**: https://github.com/bo4e/BO4E-CLI/commits/v0.0.1

## 0.0.1-rc3 - 2025-07-07

**Full Changelog**: https://github.com/bo4e/BO4E-CLI/compare/v0.0.1-rc2...v0.0.1-rc3

## 0.0.1-rc2 - 2025-07-07

**Full Changelog**: https://github.com/bo4e/BO4E-CLI/compare/v0.0.1-rc1...v0.0.1-rc2

## 0.0.1-rc1 - 2025-07-07

### What's Changed

- Project init by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/2
- Implement CLI structure (without any logic yet) by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/3
- Bump typer from 0.12.4 to 0.12.5 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/5
- Bump mypy from 1.11.1 to 1.11.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/6
- Bump pylint from 3.2.6 to 3.2.7 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/7
- Bump rich from 13.7.1 to 13.8.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/8
- Implement `bo4e pull` to pull JSON-Schemas from GitHub by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/4
- style(console): Improve styling in console + add `--verbose` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/9
- Bump more-itertools from 10.2.0 to 10.5.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/11
- Bump cffi from 1.17.0 to 1.17.1 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/13
- Bump build from 1.2.1 to 1.2.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/12
- Bump cryptography from 43.0.0 to 43.0.1 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/14
- Bump pydantic from 2.8.2 to 2.9.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/15
- feat(edit): Implement command `bo4e edit` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/10
- refactor(generate): 🚚Copy code from `BO4E-Python-Generator` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/16
- build(deps): bump pygithub from 2.3.0 to 2.4.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/19
- build(deps): bump urllib3 from 2.2.2 to 2.2.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/21
- build(deps-dev): bump pytest from 8.3.2 to 8.3.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/25
- refactor(schemas): ♻️Use property instead of get method by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/17
- build(deps): bump rich from 13.8.0 to 13.8.1 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/28
- build(deps): bump dnspython from 2.6.1 to 2.7.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/29
- feat(generate): ✨Implement code generation for Python by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/27
- build(deps): bump idna from 3.8 to 3.10 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/23
- build(deps): ⬆️Bump all dependencies by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/32
- build(deps-dev): bump coverage from 7.6.3 to 7.6.4 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/34
- build(deps): bump markupsafe from 3.0.1 to 3.0.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/35
- build(deps-dev): bump mypy from 1.12.0 to 1.13.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/38
- build(deps): bump cryptography from 43.0.1 to 43.0.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/39
- build(deps): bump sqlalchemy from 2.0.35 to 2.0.36 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/41
- build(deps): bump rich from 13.9.2 to 13.9.4 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/42
- build(deps): bump typer from 0.12.5 to 0.13.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/44
- build(deps): bump pygithub from 2.4.0 to 2.5.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/45
- build(deps): bump pyjwt[crypto] from 2.9.0 to 2.10.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/48
- build(deps): bump deprecated from 1.2.14 to 1.2.15 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/49
- build(deps-dev): bump coverage from 7.6.4 to 7.6.8 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/51
- build(deps-dev): bump pylint-pydantic from 0.3.2 to 0.3.3 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/54
- build(deps): bump argcomplete from 3.5.1 to 3.5.2 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/56
- build(deps): bump all dependencies by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/72
- build(deps): bump argcomplete from 3.5.3 to 3.6.0 by @dependabot in https://github.com/bo4e/BO4E-CLI/pull/73
- fix(generator-python): 🩹Fix python code generator for newer BO4E versions by @lord-haffi
  in https://github.com/bo4e/BO4E-CLI/pull/80
- refactor(diff): Preparation - Copied diff-matrix code from BO4E-python + Implement `diff schemas` command by
  @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/33
- chore(deps): ⬆️Bump dependencies by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/83
- feat(diff): ✨Implement `bo4e diff matrix` command by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/84
- fix(docs): 🩹Fix links of test badges in readme by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/85
- feat(diff): ✨Implement command `bo4e diff version-bump` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/86
- feat(repo): ✨Implement command `bo4e repo versions` by @lord-haffi in https://github.com/bo4e/BO4E-CLI/pull/88

### New Contributors

- @lord-haffi made their first contribution in https://github.com/bo4e/BO4E-CLI/pull/2
- @dependabot made their first contribution in https://github.com/bo4e/BO4E-CLI/pull/5

**Full Changelog**: https://github.com/bo4e/BO4E-CLI/commits/v0.0.1-rc1
