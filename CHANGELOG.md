# Changelog

All notable changes to this project are documented here. The format is
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Entries below 0.4.0 were written by hand. Everything after is written by release-plz from the
Conventional Commits in the history.

## [0.3.5](https://github.com/torabit/vanadis/compare/v0.3.4...v0.3.5) - 2026-09-07

### Other

- *(demo)* add the vhs tapes and the config they record ([#73](https://github.com/torabit/vanadis/pull/73))

## [0.3.4](https://github.com/torabit/vanadis/compare/v0.3.3...v0.3.4) - 2026-09-07

### Other

- *(init)* decide against a full-screen init

## [0.3.3](https://github.com/torabit/vanadis/compare/v0.3.2...v0.3.3) - 2026-09-07

### Added

- *(paint)* show the colour beside the hex

## [0.3.2](https://github.com/torabit/vanadis/compare/v0.3.1...v0.3.2) - 2026-09-07

### Other

- *(init)* decide the answers as a document keyed by the value

## [0.3.1](https://github.com/torabit/vanadis/compare/v0.3.0...v0.3.1) - 2026-09-07

### Added

- *(apply)* name the file a dry-run's write resolves to
- *(render)* render one target the way an apply would write it
- *(render)* render one named theme to stdout

### Fixed

- *(apply)* let a preview run --only with no state file

### Other

- release from a tag, and decide the tag from the commits
- Merge pull request #54 from torabit/torabit/feat/render

## [0.3.0] - 2026-09-06

The first published release.

### Added

- `apply`, `check`, `list`, `current`, `get` and `init`.
- `cycle`, stepping through the themes `[cycle]` names.
- `remote update`, `search` and `import` over the tinted-theming scheme collection.
- Converters for base16, base24 and tinted8.
- `render`, writing one named theme through one template.
- The agent skill under `skills/`.

[0.3.0]: https://github.com/torabit/vanadis/releases/tag/v0.3.0
