<h1 align="center">vanadis</h1>

<p align="center">
  <a href="https://github.com/torabit/vanadis/actions"
    ><img
      src="https://img.shields.io/github/actions/workflow/status/torabit/vanadis/ci.yml?branch=main&label=ci&style=flat-square"
      alt="CI status"
  /></a>
  <a href="https://crates.io/crates/vanadis"
    ><img
      src="https://img.shields.io/crates/v/vanadis?style=flat-square"
      alt="crates.io version"
  /></a>
  <a href="https://github.com/torabit/vanadis/blob/main/LICENSE-MIT"
    ><img
      src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=flat-square"
      alt="MIT OR Apache-2.0"
  /></a>
</p>

<p align="center">
  <a href="#-installation">Installation</a>
  ·
  <a href="docs/config.md">Configuration</a>
  ·
  <a href="docs/theme-format.md">Theme format</a>
  ·
  <a href="skills/vanadis/SKILL.md">Agent skill</a>
</p>

Colour lives in every tool's own config file. Changing one shade means editing ten of them,
and they drift apart; switching between light and dark means editing them all again.

**vanadis renders every one of those files from a single palette, and reloads the tools that
can reload.** You write the templates and you choose the token names. There is no spec to
conform to and no template repository to maintain.

- **Yours:** your templates, your vocabulary. `role.accent` or `colors.mauve` — vanadis does
  not have a list of names to squeeze into.
- **Checked:** `vanadis check` re-renders every target and names the ones that no longer match.
- **All or nothing:** every target renders before anything is written. A failure at any one of
  them writes no files at all.
- **Adoptive:** `vanadis init` turns a config file you already have into a template and a
  theme, so nothing has to be rewritten by hand.
- **Stocked:** `vanadis import` converts base16, base24 and tinted8 schemes out of the
  [tinted-theming collection](https://github.com/tinted-theming/schemes), 538 of them, offline
  after one fetch.
- **Scriptable:** `vanadis get role.bg` prints one colour and nothing else, for a tool that
  would rather ask than read a file.

<a name="-installation"></a>

## 🚀 Installation

### Step 1. Install vanadis

| Repository      | Instructions                     |
| --------------- | -------------------------------- |
| **[crates.io]** | `cargo install vanadis --locked` |

Or from this repository, without waiting for a release:

```sh
cargo install --git https://github.com/torabit/vanadis --locked
```

Pre-built binaries and a Homebrew tap are not published yet.

### Step 2. Get a theme

```sh
vanadis remote update           # cache the tinted-theming collection, once
vanadis search nord             # then search it offline
vanadis import nord             # convert one into ~/.config/vanadis/themes/nord.toml
```

Or write your own: [`docs/examples/papercolor-light.toml`](docs/examples/papercolor-light.toml)
is a complete theme, and [`docs/theme-format.md`](docs/theme-format.md) is the format.

### Step 3. Adopt a config you already have

```sh
vanadis init ~/.config/hunk/config.toml
```

`init` reads the file, finds the colours in it, asks which token each one is, and writes three
things: a template beside your config, the colours into a theme, and a target entry into
`config.toml`. The file it read is never modified — it becomes the target's output.

### Step 4. Apply

```sh
vanadis apply nord --diff       # what would change
vanadis apply nord              # write it, and run each target's reload
```

## 🎨 What it looks like

`~/.config/vanadis/config.toml` — one entry per file vanadis writes:

```toml
[auto]
light = "papercolor-light"
dark = "papercolor-dark"

[[targets]]
name = "hunk"
template = "templates/hunk/config.toml.in"
output = "~/.config/hunk/config.toml"

# bat matches a theme by the name inside the file, so the output is named for vanadis rather
# than for the theme it currently holds.
[[targets]]
name = "bat"
template = "templates/bat/theme.tmTheme.in"
output = "~/.config/bat/themes/vanadis.tmTheme"
reload = ["bat", "cache", "--build"]

# An editor plugin colours far more than one palette carries, so nvim stays on gruvbox while
# everything else follows the theme being applied. Light and dark still flip together.
[[targets]]
name = "nvim"
template = "templates/neovim/palette.lua.in"
output = "~/.config/nvim/lua/palette.lua"
themes = { light = "gruvbox-light", dark = "gruvbox-dark" }
```

A template is the tool's own config with the colours replaced. Nothing else:

```toml
[themes.vanadis]
background   = "{{role.bg}}"
panel        = "{{role.hover-bg}}"
border       = "{{role.border}}"
accent       = "{{role.accent}}"
text         = "{{role.fg}}"
muted        = "{{role.comment}}"
selectedHunk = "{{role.selection-bg}}"
```

`vanadis apply papercolor-light` writes:

```toml
[themes.vanadis]
background   = "#eeeeee"
panel        = "#e4e4e4"
border       = "#bcbcbc"
accent       = "#d70087"
text         = "#444444"
muted        = "#878787"
selectedHunk = "#d7d7af"
```

`{{token}}` substitution is the whole template language. No conditionals, no loops, no filters,
no includes. A template that needs logic is a template that has moved a decision out of the
theme, where it can be read.

## 📋 Commands

| command                            | what it does                                                                    |
| ---------------------------------- | ------------------------------------------------------------------------------- |
| `vanadis apply <theme>`            | renders every target, writes them, runs each `reload`                             |
| `vanadis apply --variant dark`     | the theme `[auto]` names for dark, so a shell hook needs no theme names           |
| `vanadis apply <theme> --diff`     | the change line by line, writing nothing                                          |
| `vanadis cycle`                    | the theme after the one in use, from `[cycle]`                                    |
| `vanadis check`                    | do the generated files still match their templates?                               |
| `vanadis list` / `vanadis current` | the themes in `themes/`, and the one applied last                                 |
| `vanadis get role.bg`              | one resolved colour, for a prompt or a script                                     |
| `vanadis init <file>`              | turn a config you already have into a template, a theme and a target              |
| `vanadis remote update`            | cache the tinted-theming scheme collection                                        |
| `vanadis search <query>`           | find a scheme in the cache, offline                                               |
| `vanadis import <scheme>`          | convert one into a theme                                                          |

## 🤖 The agent skill

`init` removes the mechanical half of adoption. The half it cannot remove is judgement: where a
tool keeps its config, which lines carry colour, whether a colour is `role.accent` or
`role.accent-alt`, what belongs in `reload`, and whether a failing `check` means the template
is wrong or the theme is.

[`skills/vanadis/SKILL.md`](skills/vanadis/SKILL.md) carries that judgement. An agent reads it
and does the work; a person reads the same file and does the work by hand. It ships as a plugin
under both the [Agent Plugins](https://agent-plugins.org/specification) manifest and Claude
Code's, over the one `skills/` tree.

## 🧭 What this is not

**A theme distribution system.** If you want to browse hundreds of ready-made themes for tools
other people already support, use [tinty](https://github.com/tinted-theming/tinty). That is what
it is for and it does it well.

vanadis is for the case tinty does not cover: you have written your own configs, for tools that
may have no template repository at all, in a palette that may not fit base16's sixteen slots.

|                                                     | renders templates          | vocabulary                | needs          |
| --------------------------------------------------- | -------------------------- | ------------------------- | -------------- |
| [tinty](https://github.com/tinted-theming/tinty)     | no, copies pre-built files | base16 / base24 / tinted8 | template repos |
| [flavours](https://github.com/Misterio77/flavours)   | yes                        | base16's 16 slots         | —              |
| [Stylix](https://github.com/danth/stylix)            | yes                        | base16                    | Nix            |
| vanadis                                              | yes                        | arbitrary                 | —              |

The niche is the intersection: arbitrary templates **and** an arbitrary token vocabulary. It is
a narrow one, and the three above are the better choice whenever they fit.

One thing follows from it that is worth saying out loud. Template repository ecosystems are
structurally behind: every new tool — ghostty, zellij, yazi, atuin, helix — has no template
repository for months or years after it ships. With vanadis you write ten lines of template and
it works the day the tool does.

## 📚 Documentation

Every decision in vanadis is written down with the alternatives it rejected.

- [`docs/config.md`](docs/config.md) — `config.toml`, the state file, and every path vanadis uses
- [`docs/theme-format.md`](docs/theme-format.md) — the theme file, token paths, references
- [`docs/core-vocabulary.md`](docs/core-vocabulary.md) — the tokens every theme must define
- [`docs/schemes.md`](docs/schemes.md) — the scheme cache, `search`, and what `import` writes
- [`docs/init.md`](docs/init.md) — what `init` asks and why

## 📝 Licence

MIT OR Apache-2.0.

[crates.io]: https://crates.io/crates/vanadis
