---
name: vanadis
description: Use when adding a CLI tool to vanadis, writing or repairing a vanadis template or theme, moving a dotfiles repository onto one palette, or working out why `vanadis check` exits non-zero. Covers picking a token for a colour, what belongs in `reload`, which files must never be edited by hand, and the colour notations vanadis refuses.
license: MIT OR Apache-2.0
---

# vanadis

vanadis renders CLI tool configs from one palette. The user writes templates; vanadis
substitutes tokens from a theme, writes each result to the file its tool reads, and runs the
reload command for the tools that have one.

Running it is easy. Writing `config.toml` and the templates is the part that takes judgement,
and that is what this skill carries.

## Two rules that never bend

**A generated file is never edited by hand.** It is overwritten by the next apply, and
`vanadis check` reports it as drift until then. Edit the template.

**The template language is `{{token}}` substitution and nothing else.** No conditionals, no
loops, no filters, no includes, no colour arithmetic. If a template seems to need one of
those, the answer is a different token, or a second template, or a script the tool runs.

## Layout

```
~/.config/vanadis/          # $XDG_CONFIG_HOME/vanadis; $VANADIS_CONFIG replaces the whole directory
├── config.toml             # [auto], [cycle] and the [[targets]]
├── themes/
│   ├── papercolor-light.toml
│   └── papercolor-dark.toml
└── templates/
    └── <target>/<basename>.in
```

The filename minus `.toml` is the theme's identifier. Themes are found by scanning `themes/`
and are never listed in the config. Templates live under the config directory, not beside the
files they render to — btop enumerates its themes directory, so a `.theme.in` left there turns
up in btop's theme list.

The theme applied last is recorded under `$XDG_STATE_HOME/vanadis/state.toml`. It is
deliberately outside the config directory: the config is what gets version controlled and
copied between machines, and which theme this machine is showing is the one thing that must
not travel with it.

## Commands

| command | what it does |
| --- | --- |
| `vanadis apply <theme>` | renders every target, writes them, runs each `reload` |
| `vanadis apply --variant dark` | the theme `[auto]` names for dark |
| `vanadis apply <theme> --only <name>` | writes only these targets and records them, so one theme name stops describing the machine |
| `vanadis apply <theme> --diff` | shows the change line by line, writes nothing |
| `vanadis cycle` | applies the theme after the one in use, taken from `[cycle] themes` |
| `vanadis check` | checks against the applied theme, exits non-zero on any finding |
| `vanadis check <theme>` | checks against that theme instead — the CI form, since a fresh checkout has no state file |
| `vanadis check <theme> --only <name>` | one target, which is how a new template is verified |
| `vanadis list` | the themes in `themes/` |
| `vanadis current` | the theme applied last, then one line per target `--only` moved off it; prints nothing on stdout and exits non-zero when nothing has been applied |
| `vanadis get role.bg` | one resolved value and nothing else, so `$(vanadis get role.bg)` is a colour |
| `vanadis get <token> --theme <id>` | reads that theme instead of the applied one, and is the only way to query before any apply |
| `vanadis get --json` | the whole resolved theme, flat, keyed by token path |
| `vanadis render <template> --theme <id>` | one template against one named theme, to stdout. Reads no target and writes no file — this is how a theme is written out as a base16 scheme or any other format |
| `vanadis render --target <name>` | what an apply would write for that target, to stdout, writing no file. The theme resolves the way `check` resolves it, so `render --target x > <its output>` leaves `check --only x` clean |
| `vanadis hook <zsh\|fish\|bash>` | the snippet a shell evaluates to follow an apply, for a target marked `shell` — `eval "$(vanadis hook zsh)"`, or `vanadis hook fish \| source` |
| `vanadis init <file>` | interactive. Hand it to the user — see below. |

`apply` overwrites files the user wrote. Show `vanadis apply <theme> --diff` and get their
go-ahead before running the real thing.

`apply --only` refuses to write before a whole apply has happened — `--only needs a theme
applied to every target first` — because there would be no theme for the targets it does not
name to be on. Adding `--dry-run` or `--diff` records nothing, so one target's diff is readable
there. `check --only` has no such restriction as long as a theme is named, which is what makes
it usable on a target added minutes ago.

## `init` is the user's to run, not yours

`vanadis init` asks for a token name per distinct colour value in the file. Do not drive it
from a script of answers. Prompts appear in the order values first occur, answering with two
names opens a second per-occurrence pass for that value alone, and a misaligned script fails
silently: the file still renders back byte for byte, so `check` stays clean while the colours
sit under the wrong names.

Either tell the user to run it themselves:

```
vanadis init ~/.config/hunk/config.toml
```

or do the same work by hand with the steps below. They are what `init` automates, and they end
in the same verification.

## The core vocabulary

Thirty-three tokens. A template that reads only these plus `meta.*` renders against every theme
that defines the core, which is what a converter from an upstream scheme must emit and what a
hand-written theme is aiming at. That portability is the whole return on the core.

An incomplete theme is not an error and nothing refuses to load it. `vanadis check` is the one
place the core is enforced: it names every core token the themes a run resolves to leave
undefined, and exits non-zero. `vanadis init` prints the same list when it finishes.

`[role]`, seventeen:

| token | use |
| --- | --- |
| `bg` | default background |
| `fg` | default foreground |
| `comment` | comments, and any text deliberately de-emphasised |
| `keyword` | language keywords |
| `string` | string literals |
| `error` | errors, failures, removed lines |
| `ok` | success, added lines |
| `warn` | warnings, modified lines |
| `visual` | a highlight drawn over text: search matches, hints, selected entries |
| `linenr` | line numbers and gutter text — the digits, not the column behind them |
| `accent` | the primary accent, for the element a theme wants looked at first |
| `accent-alt` | a second accent, for a distinct element next to the first |
| `accent-warm` | a warm accent, for where the other two would read as cold |
| `inactive` | unfocused panes, disabled entries |
| `border` | pane borders, box drawing, separators |
| `selection-bg` | background of a selection |
| `hover-bg` | background of the row under the cursor, one step from `bg` |

`[ansi]`, all sixteen slots, keyed `0` through `15`. All sixteen are required: a terminal keeps
whatever colour it had for a slot the config does not set, so a hole is invisible until
something prints in that colour.

`[meta]` — `format`, `name`, `variant` — is required for a theme to load at all, so it is not
counted in the thirty-three. `meta.id` is readable as a token and is never written in the file:
it is the filename.

### Picking a token for a colour

Ask what the colour is *for*, not what it looks like.

- **Use a core name when one fits the job.** It is what buys the template a theme switch.
- **`accent` vs `accent-alt`:** `accent` is the thing the eye should land on first.
  `accent-alt` exists so a second element beside it stays distinguishable. If the two elements
  never appear together, they probably both want `accent`.
- **`accent-warm`** is the escape hatch for a spot where the other two read as cold. Do not
  reach for it as a third accent by default.
- **One hex, two jobs, two tokens.** Sharing a value in this theme does not mean sharing it in
  the next one. Give the comment colour and the disabled-entry colour their own tokens even
  when they are the same grey today. Collapsing them is silent: the template reads
  `{{role.comment}}` where `inactive` belongs, and nothing surfaces until a theme where they
  differ.
- **Never invent an appearance name in `[role]`.** `role.purple` is a colour, not a job.
  Appearance names live in `[colors]`.
- **Reaching outside the core is legitimate and has a price.** A template reading `colors.*`,
  `diff.*` or `text.*` is bound to the theme that defines them. That is a fine thing to be — it
  is how a lazygit theme gets diff background tints no upstream scheme carries. Pin the target
  with a per-target `themes` override so a theme switch does not break it.

## Colour is not the only thing that flips

A config carries values that are not colours and still have to follow the theme: the word
`light` in a flag, another tool's own theme name, a display name. Leave them and a dark palette
gets rendered into a config that still asks fzf and delta for light mode.

`check` cannot catch this. Everything it inspects is a valid colour, so that config passes
clean. Sweeping for these is yours to do, once, while writing the template:

| in the file | becomes |
| --- | --- |
| `--color=light` | `--color={{meta.variant}}` |
| `--light`, `--dark` | `--{{meta.variant}}` |
| `theme = "papercolor-light"` | `{{meta.id}}` |
| `label = "PaperColor Light"` | `{{meta.name}}` |
| `--syntax-theme=gruvbox-light` | `{{text.delta-syntax-theme}}`, defined per theme |
| `base = "github-light-default"` | `{{text.hunk-base}}`, defined per theme |

`[meta]` and `[text]` are the only two namespaces holding strings that are not parsed as
colours. Everything else in a theme must be a hex literal or a whole-string reference, which is
what lets `check` say a colour value is well-formed at all.

## Add a tool as a target

1. **Find the config file the tool actually reads.** If you are not certain of the path, ask.
   Do not guess and do not create one.
2. **Decide the output shape.** If the tool can include or source another file, generate a
   colours-only file and leave the hand-written config to read it — then the hand-written file
   is never at risk of being overwritten. If it cannot, the whole file becomes the template and
   the file is generated from then on. See [references/templates.md](references/templates.md).
3. **Copy, never move.** `cp` the file to `templates/<name>/<basename>.in` under the config
   directory — `${VANADIS_CONFIG:-~/.config/vanadis}`, and `$VANADIS_CONFIG` is usually unset.
   The original stays exactly where it is and becomes the target's `output`.
4. **Double every `{{` and `}}` already in the copy.** `{{{{` renders as a literal `{{`. Do
   this for every pair, not only the ones that look like a token path — a stray `{{` in front of
   a substitution is read as an escape at the seam and the file comes back wrong. Single braces
   are left alone; a tool's own `{workspace}` passes through untouched.
5. **Replace each colour with a token.** Only lowercase `#rrggbb` is substitutable. A hex that
   is not a colour stays literal — prose describing a terminal bug that quotes `#444444` must
   not be rewritten by a theme switch.
6. **Add the values to the theme** under the names you chose, as lowercase `#rrggbb` literals
   or as `{{colors.x}}` references to colours it already carries.
7. **Append the `[[targets]]` entry.** `name`, `template`, `output`. Leave `reload` out unless
   you know the command, and add `shell` when the output is a file a shell sources rather than
   a config a tool reads — see [references/reload.md](references/reload.md).
8. **Verify.** This is the step that makes the work checkable:

   ```
   vanadis check <theme> --only <name>
   ```

   The output file is still the user's original, so a clean check means the template renders
   back to it byte for byte and nothing was dropped. Any finding means the template is wrong.
   **Fix the template. Never touch the original file to make the check pass** — that is
   editing the answer key.
9. **Then show `vanadis apply <theme> --diff`** and let the user decide before you run the
   apply.

**`output` must not be a symlink.** vanadis renames the rendered file over the output, which
replaces a link with a regular file and leaves the dotfiles repository behind it untouched. If
the user manages dotfiles with Stow, chezmoi or `ln -s`, the generated config is a build
artifact: it is not symlinked and not committed, and only `config.toml`, `templates/` and
`themes/` go in the repository. `docs/config.md` records this under Output.

A parent directory can be a link too, and that one is invisible: `~/.config/bat` linked into a
dotfiles repository makes `~/.config/bat/themes/x.tmTheme` land inside the repository while
reading like an ordinary path. `vanadis apply <theme> --dry-run` prints
`<name>: <output> resolves to <file>` for every target that happens to, so run it before the
first apply of a target you just wrote.

### `[[targets]]` keys

| key | required | value |
| --- | --- | --- |
| `name` | yes | the target's identifier: lowercase, digits, internal hyphens, unique in the file |
| `template` | yes | path to the template |
| `output` | yes | path to write |
| `reload` | no | argv to run after writing, as an array — no shell |
| `shell` | no | `zsh`, `fish` or `bash`: the shell whose `vanadis hook` sources this output — see [references/reload.md](references/reload.md) |
| `themes` | no | `{ light = "...", dark = "..." }`, this target's own themes |

`name` identifies the target, not the tool. One program with three config files is three
targets, so `herdr`, `herdr-thumbs` and `herdr-host-colors` are three names for one program.

A leading `~` expands to the home directory. A relative path resolves against the config
directory. `$VAR` is **not** expanded.

**Never encode the theme's name in `output`.** Apply gruvbox over a file called
`papercolor-light.theme` and it is still called that, and btop lists whatever it finds in its
themes directory, so the wrong name shows up in the tool's own UI. Name outputs for vanadis:
`vanadis.theme`, `vanadis.tmTheme`.

## Move a dotfiles repository onto one palette

Same loop, one file at a time, plus two things that only matter in bulk.

**Reuse token names.** Re-read the theme before each new file and answer from the names already
in it. Inventing `role.dim` in file six when file two called the same job `role.inactive` gives
the palette two names for one thing, and no tool catches it.

**Start with the file that has the most colours.** The theme is mostly built by the end of it,
and every later file is matching against names that already exist.

**Finish by proving no hex is left in a hand-written file.** The whole design rests on it.
Templates and generated outputs are supposed to carry hex, so exclude them and look at the rest:

```bash
grep -E '^ *output *=' "${VANADIS_CONFIG:-$HOME/.config/vanadis}/config.toml" | cut -d'"' -f2
grep -rnIE '#[0-9a-fA-F]{6}' ~/dotfiles --exclude-dir=.git
```

Every hit from the second command that is not a path from the first is a colour vanadis does
not control. Hex inside a comment counts: change the palette and the comment is the only thing
still asserting the old value.

## Diagnose a failing check

`vanadis check` prints one finding per line and exits non-zero. Five shapes:

| line | finding | what it means |
| --- | --- | --- |
| `<theme>: N core tokens undefined` | incomplete | a theme the run is on is short of the core |
| `<name>: <output> does not match <template>` | drift | the file no longer holds what the template renders |
| `<name>: <output> does not exist` | missing | never applied, or deleted |
| `<name>: <output>: <io error>` | unreadable | permissions, or a broken symlink. A filesystem problem, not a vanadis one |
| `<name>: <reason>` | unrenderable | an apply would fail here |

### drift

Two causes, and `vanadis apply <theme> --diff` tells them apart by showing the change.

- **Someone edited the generated file.** Move the edit into the template, then apply. Keeping
  it in the output means losing it on the next apply.
- **The template or the theme changed and no apply has run since.** Apply.

Do not resolve drift by editing the theme. Drift is a statement about the output.

### unrenderable

The reason is one of:

- `themes/ holds no theme named <id>` — the config names a theme that is not in `themes/`. Check
  `vanadis list`; a theme file that will not load is reported to stderr as a warning and skipped.
- `target <name> names no theme for <variant>` — the target's `themes` table has no entry for the
  variant being applied.
- an unreadable template — a path in `config.toml` that is wrong or a file that is gone.
- undefined tokens, which print as:

  ```
  lazygit: /home/you/.config/vanadis/templates/lazygit/theme.yml.in: undefined tokens
    line 8: role.inactve
    line 26: colors.purple
  ```

Work through them by asking whether the token path is one the theme should have:

- **A typo** (`role.acent`) — fix the template.
- **A core token the theme is short of** — add it to the theme.
- **A theme-private token** (`colors.*`, `diff.*`, `text.*`) that this theme does not carry —
  the template is bound to the theme it was written for. Either define the token in every theme
  being switched between, or pin the target with `themes = { light = "...", dark = "..." }`.

`vanadis get <token> --theme <other-theme>` answers whether some other theme defines it, and
gives you the value to copy.

### incomplete

Printed once per theme, before the target findings it explains:

```
papercolor-dark: 3 core tokens undefined
  role.linenr role.accent-warm ansi.7
```

The theme is short of the core. Add the tokens; see [Fill in what a theme is
missing](#fill-in-what-a-theme-is-missing).

It is asked of the themes the run resolves to — the applied theme, and whatever a target with
its own `themes` table names — so a half-written theme sitting in `themes/` does not fail a
check of a machine that is not on it. `vanadis check <that-theme>` is how you ask about one
before switching to it.

An incomplete theme is not otherwise an error. It loads, it lists, and it applies cleanly to
any target whose templates never read what it is short of.

## Fill in what a theme is missing

After a theme swap, `check` reports undefined tokens for the tokens the new theme does not carry.

1. `vanadis get <token> --theme <the-theme-that-has-it>` for the value and to confirm the path.
2. Add the token to the new theme.
3. **Point at a colour the theme already carries** — `{{colors.base0e}}` — rather than pasting
   the old theme's hex. A hex lifted from another palette is not part of this one and will read
   as a smudge next to everything around it.
4. Re-run `vanadis check <theme>`.

If the missing token is theme-private and only one target needs it, pinning that target with a
`themes` override is the honest fix. Not every template is meant to survive every theme.

## Theme file rules

A value is a lowercase `#rrggbb` hex literal, or exactly one `{{reference}}` filling the whole
string. `rgb({{colors.ink}})` is an error. `#EEEEEE`, `#eee` and `#rrggbbaa` are all rejected.

Every key must be a single segment: lowercase ASCII, digits, internal hyphens. `added_bg` and
`Paper` are errors, because they would store a value no template can name.

References may chain — `inactive = "{{role.comment}}"` says "inactive text is the comment
colour", which is better than repeating the grey. Cycles are an error and every cycle is
reported.

`meta.variant` is `dark` or `light` and is stated, never derived from the background's
luminance. It is a statement of intent, and `[auto]` pairs themes by it.

`[colors]` holds colour named by appearance — `paper`, `ink`, `crimson` — and is the layer
everything else points at. `[ansi]`, `[role]` and any namespace the author adds hold the jobs.
`meta`, `colors`, `ansi` and `text` are the only reserved top-level names.

## Common mistakes

| mistake | what it costs |
| --- | --- |
| Editing a generated file | Overwritten by the next apply. `check` reports drift until then. |
| Naming the output for the theme | The name is wrong after any switch, and tools that enumerate a themes directory show it. |
| Leaving `{{` unescaped in a template | A tool with its own `{{...}}` placeholders writes things like rio's `{{columns}}`, which is a valid token path, undefined, and a hard error. |
| Guessing a `reload` command | A wrong command is worse than none. Absent means nothing runs and the user restarts the tool. |
| A shell pipeline in `reload` | `reload` is argv, executed directly. Write a script and name the script. |
| Tokenising a hex that is not a colour | A theme switch rewrites prose that was quoting a value, and the explanation becomes a lie. |
| Losing an executable bit | A new output takes the template's mode, so `chmod +x` the template when the output is a script. |
| Moving the original file instead of copying it | The original is the `output`. Move it and the byte-identical check has nothing to compare against. |
| Fixing a failing check by editing the output | That is editing the answer key. Fix the template. |

## References

- [references/templates.md](references/templates.md) — the two output shapes, escaping, and the
  per-tool traps: bat selecting a theme by the name inside the file, starship's `[palettes]`
  indirection, outputs that are scripts.
- [references/reload.md](references/reload.md) — how a change reaches each tool, which ones have
  a command vanadis can run, and how to work out a tool that is not in the table.
