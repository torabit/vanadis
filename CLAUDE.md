# vanadis

## What it is

Builds CLI tool configs from one palette. You write templates; it renders them and reloads
the affected tools.

A build tool for dotfiles, not a theme distribution system.

- `tinty` copies pre-built theme files out of template repositories. It does not render.
- `flavours` renders, but only base16's 16 slots.
- `Stylix` renders system-wide, and requires Nix.
- `vanadis` renders arbitrary templates from an arbitrary token vocabulary. That
  intersection is the whole niche.

Two things follow and neither is negotiable:

- `check` is core. vanadis generates output, so it verifies output.
- The template language is `{{token}}` substitution and nothing else. No conditionals,
  loops, filters or includes.

## What ships

`skills/`, `plugin.json` and `.claude-plugin/` are the plugin. Everything else exists to
build vanadis and is never shipped.

The shipped skill covers a user's `config.toml` and templates. It carries nothing about this
codebase.

## Fixtures

`tests/fixtures/expected/` is golden data. A diff means the renderer is wrong. Never
regenerate it to make a test pass.

`tests/fixtures/palette.json` must round-trip through whatever format the data model settles
on, without loss.

## Rules

Machine-checkable rules are gates, not documents. Run the CI gates locally before pushing.
Once a lint covers a rule, delete it from `.claude/rules/`.

No dependency ahead of the work that needs it.

## Working an issue

1. `design` issues resolve to a document under `docs/` before code is written.
2. `Depends on #N` is binding.
3. Branch as `torabit/<type>/<slug>`.
4. Open a PR referencing the issue.

## Commits, issues and pull requests

Written in English.

Conventional Commits: `type(scope): description`.

- Types: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `style`, `perf`, `ci`.
- Subject imperative, lowercase, no trailing period, at most 72 characters.
- Body explains why, when that is not self-evident.
