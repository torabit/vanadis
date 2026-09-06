# Core vocabulary

This document decides which tokens every theme must define. It builds on
[docs/theme-format.md](theme-format.md), which decides the file format and defers this
question here.

The core is the set of token paths a template may rely on without binding itself to one
theme. A template that reads only core tokens renders against every theme. A template that
reads anything else is theme-specific, which is a legitimate thing to be, and `check` is what
reports the mismatch before an apply hits it.

## What belongs in the core

A token is core when it satisfies both conditions.

1. **A golden template reads it, and the template must keep working when the theme changes.**
   The core follows from what templates need. It is not derived from any upstream scheme's
   slot count, and it is not a list of names that seemed likely.
2. **Every supported upstream scheme can fill it by naming a colour it already carries.** A
   converter never invents a colour. If filling a token would mean computing one, the token
   is not core.

The two conditions do different work. The first sets the floor: leave a needed token out and
the templates that read it break on a theme switch, which is the failure the core exists to
prevent. The second sets the ceiling: put a token in that base16 cannot supply and every
converted theme is incomplete the moment it is written, so the guarantee is empty.

### The core

`[role]`, seventeen tokens:

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
| `linenr` | line numbers and gutter text |
| `accent` | the primary accent, for the element a theme wants looked at first |
| `accent-alt` | a second accent, for a distinct element next to the first |
| `accent-warm` | a warm accent, used where the other two would read as cold |
| `inactive` | unfocused panes, disabled entries |
| `border` | pane borders, box drawing, separators |
| `selection-bg` | background of a selection |
| `hover-bg` | background of the row under the cursor, one step from `bg` |

`[ansi]`, all sixteen slots, `0` through `15`.

`[meta]`: `format`, `name` and `variant`, which
[docs/theme-format.md](theme-format.md#metadata) already requires. `meta.id` is core and is
never written in the file: it is the filename. `meta.author` is optional and therefore not
core.

Nothing else. `[text]` is not core, because a converter leaves it empty. `[diff]` is not
core, for the reason below.

### Why seventeen and not eight

The obvious starting point is the eight names any syntax-highlighting vocabulary has: `bg`,
`fg`, `comment`, `keyword`, `string`, `error`, `ok`, `warn`. Counting what the golden
templates actually reference settles it against that. The eleven templates name every one of
the seventeen `role` tokens in `tests/fixtures/palette.json` and no others. There is no
unused role token to drop.

The nine tokens the starting point omits are what makes the core worth having. Under an
eight-token core exactly one template — `herdr/host-colors.py.in`, which reads `role.bg` and
`role.fg` and nothing else — survives a theme switch. Under the seventeen, six do. The other
five are held back by `colors.*` and `diff.*`, which no size of `role` would fix.

The seventeen come from one person's dotfiles, which is a fair objection to their generality
and not one this document can answer from evidence it has. What it can say is that the cost of
a core token is one line in a theme file and one row in a converter's mapping table, and that
`check` makes an unfilled one visible rather than silent. Seventeen is affordable at that
price.

### Why `[diff]` is not core

`lazygit/theme.yml.in`, the template for a git UI, reads `diff.added-bg`, `diff.added-emph`,
`diff.removed-bg` and `diff.removed-emph`. It is the one template held back from the core by
something other than a hue name, so the case is worth stating.

No upstream scheme carries a diff background. base16 and base24 are foreground slots plus a
handful of background steps, none of them a tint of green or red. tinted8 has
`syntax.markup.inserted` and `deleted`, and both are foreground colours. A converter asked to
fill `diff.added-bg` would have to derive a tint from `role.ok`, which is condition 2's
failure case exactly, and
[docs/theme-format.md](theme-format.md#left-open) puts derived colour outside this format.

So `[diff]` is an extra, `lazygit/theme.yml.in` is theme-specific, and it is pinned to its
theme the same way `bat/PaperColor-Light.tmTheme.in` is pinned by `{{colors.purple}}`:
`check` reports it, and a per-target theme override is what keeps it rendering.

The diff *foregrounds* need no new token. `role.ok` and `role.error` already carry added and
removed, and the templates already use them that way.

### Why all sixteen ANSI slots

All sixteen are required, which is the answer
[docs/theme-format.md](theme-format.md#layers) states without arguing for.

`rio/config.toml.in`, a terminal emulator's config, is the only template that reads `[ansi]`
and it reads every slot. A
partial `[ansi]` would render a terminal config with a hole in it, and a terminal keeps
whatever colour it had for a slot the config does not set, so the hole is invisible until
some program prints in that colour. base16 fills all sixteen from ten distinct slots, so
requiring them costs a converter nothing.

## What the core guarantees, and where it is checked

Six of the eleven golden templates read only core tokens: `btop/papercolor-light.theme.in`,
`herdr/config.env.in`, `herdr/host-colors.py.in`, `hunk/config.toml.in`,
`rio/config.toml.in` and `zsh/palette.zsh.in`. Those six render against any theme that loads.
That sentence is the guarantee, and it is the whole return on the core.

The remaining five are theme-bound: four through `colors.*` (`bat`, `herdr/config.toml`,
`neovim`, `starship`) and one through `diff.*` (`lazygit`).

### A missing core token is not a load error

A theme that omits `role.linenr` is not malformed. Every token it does define resolves, and
the failures [docs/theme-format.md](theme-format.md#errors) makes fatal — bad hex, a
reference cycle, a key that is not a segment, a non-string value — are all cases where a
value cannot be produced at all. Incompleteness is not one of them.

Treating it as one would also be disproportionate. Themes are discovered by enumerating a
directory, so a hard load error on incompleteness would mean one unfinished file makes
`vanadis list` fail, and with it every command that has to enumerate themes. A broken theme
should cost the user that theme, not the tool.

**`check` is the only place the core is enforced.** It reports every missing core token by
name and exits non-zero. That is what `check` is for, and it is the moment the answer is
useful: before an apply, not after one has half-written a config.

It asks it of the themes a run resolves to, and not of every file in `themes/`.
[docs/config.md](config.md#checking) decides that and says why: a theme no target is on is not
part of whether this machine is consistent, and failing a run over one would be the same
disproportion this section rejects for the loader. Naming a theme is how it is asked of one
that is not applied.

**`apply` fails only when a template it is rendering actually references a token the theme
does not define**, which is the renderer's undefined-token error and needs nothing added
here. It fails atomically: every target renders before anything is written, so a failure
leaves no file touched. A theme missing `role.linenr` applies cleanly to a target whose
templates never mention it.

`list`, `current` and `get` never fail because a theme is incomplete.

`init` names the core tokens it could not fill when it finishes, so a theme built up one
config file at a time says what it is still short of without waiting for a `check`.
[docs/init.md](init.md#the-core-it-did-not-fill) decides why the list is printed rather than
written into the theme as commented-out lines.

### Converters must fill the entire core

The rule above is about hand-written themes. A converter is held to more: it must emit every
core token or the conversion is a bug. An imported theme that fails `check` on the day it is
written would make importing pointless.

The mapping below is the proof that the requirement is met for base16, which is the tightest
of the three formats.

## base16 onto the core

Slot meanings are from the base16 styling specification, `home/styling.md` v0.4.2.

| base16 | its stated meaning | core token |
| --- | --- | --- |
| base00 | Default Background | `bg` |
| base01 | Lighter Background (status bars, line number, folding marks) | `hover-bg` |
| base02 | Selection Background | `selection-bg`, `border` |
| base03 | Comments, Invisibles, Line Highlighting | `comment`, `inactive` |
| base04 | Dark Foreground (status bars) | `linenr` |
| base05 | Default Foreground, Caret, Delimiters, Operators | `fg` |
| base06 | Light Foreground (not often used) | — |
| base07 | Light Background (not often used) | — |
| base08 | Variables, XML Tags, Markup Lists, Diff Deleted | `error` |
| base09 | Integers, Boolean, Constants, Markup Link Url | `accent-warm` |
| base0A | Classes, Markup Bold, Search Text Background | `warn` |
| base0B | Strings, Inherited Class, Markup Code, Diff Inserted | `string`, `ok` |
| base0C | Support, Regular Expressions, Escape Characters, Markup Quotes | `accent-alt` |
| base0D | Functions, Methods, Attribute IDs, Headings | `accent`, `visual` |
| base0E | Keywords, Storage, Selector, Markup Italic, Diff Changed | `keyword` |
| base0F | Deprecated, Opening/Closing Embedded Language Tags | — |

`linenr` takes base04 rather than base01, whose stated meaning names line numbers. base01 is
a background and base04 is a foreground; `role.linenr` colours the digits, not the column
behind them. The template for hunk, a diff viewer, reads it as `lineNumberFg`, which is the
same reading.

All seventeen `role` tokens are filled. `[ansi]` is filled by the table in
[docs/theme-format.md](theme-format.md#a-base16-scheme-in-this-format), which draws on ten
slots and leaves none of the sixteen empty. `[meta]` comes across from the scheme's own
`name` and `variant`, with `format = 1` added.

Thirteen distinct slots carry seventeen role tokens, so four pairs collide:

| slot | tokens | why the collision is tolerable |
| --- | --- | --- |
| base02 | `selection-bg`, `border` | base16 has one surface step above the selection background and no separate border colour |
| base03 | `comment`, `inactive` | dimmed text and comment text are the same idea in a scheme with one grey |
| base0B | `string`, `ok` | base16's own conflation: the slot is documented as both Strings and Diff Inserted |
| base0D | `accent`, `visual` | base16 has no notion of a UI accent, so both land on the colour schemes conventionally use for emphasis |

The collisions are arithmetic, not a mapping error. Thirteen of base16's sixteen slots carry
a role token and seventeen tokens cannot be injective into thirteen.

The other three go unused by `[role]`: base06, base07 and base0F. base07 is not idle overall,
as `ansi.15` takes it. base06 and base0F are read by nothing, which is the same conclusion
[docs/theme-format.md](theme-format.md#a-base16-scheme-in-this-format) reached from the
export direction: neither has a plausible role name, and neither appears in the terminal's
sixteen slots, so a shell is unaffected either way.

base24 is wider than base16 and inherits this mapping through the slots it shares with it.
Where it names something base16 does not, its converter decides whether to use it.

tinted8 shares no slot name with base16, so it inherits nothing from the table above and
derives its own mapping from its own specification. That mapping is tabulated in the module
documentation of `src/scheme/convert/tinted8.rs`, next to the code it decides.

Neither can be blocked by a core token base16 already fills.

## Extras

Everything outside the core. A theme adds a namespace and templates read it. There is no
registry, no declaration, and no reserved-name list beyond the four
[docs/theme-format.md](theme-format.md#layers) already reserves.

In `docs/examples/papercolor-light.toml` the extras are the nineteen appearance names in
`[colors]`, the four tints in `[diff]`, and the two strings in `[text]`.

Reading an extra is what makes a template theme-specific. That is a property of the template,
reported by `check` and managed by a per-target theme override, and never an error in the
theme.

## Versioning

The core list is compiled into vanadis. It is not read from a file and a theme cannot extend
it.

**Adding a core token is a breaking change** and bumps `meta.format`, because every existing
theme becomes incomplete the moment the token is added and starts failing `check` without
having changed. **Removing one is not**, because a theme that still defines it is merely a
theme with one more extra.

## Rejected alternatives

**Renaming `linenr` and `visual`.** Both are Vim's vocabulary rather than neutral names, and
`linenr` is read by exactly one template. Renaming them means editing
`tests/fixtures/templates/` and `tests/fixtures/expected/`, and
[docs/theme-format.md](theme-format.md#rejected-alternatives) settles that case already: the
golden templates are the API. A cosmetic rename is not worth touching golden data for, and
the names are at least the ones a Vim or Neovim config author already knows.

**A core of base16's sixteen slots.** This is base16. The tool's stated niche is rendering
arbitrary templates from a vocabulary the theme author chooses, and adopting a fixed
sixteen-slot core would put the fixed list back in the one place it was removed from. The
rejection rests on the niche rather than on any measurement, so it stands as long as the
niche does: a vanadis that only ever rendered base16 schemes would have no reason to refuse
base16's vocabulary.

**Promoting `colors.purple`, `colors.brown` and `colors.slate` to `role`.** They are the only
reason four templates are theme-bound, so promoting them would buy real portability. It is
rejected because they have no role: `mauve = "{{colors.purple}}"` in the herdr template is
filling a slot in catppuccin's palette by hue, and a `role.purple` would be an appearance
name in the semantic layer, which is the distinction
[docs/theme-format.md](theme-format.md#layers) draws between `[colors]` and everything else.
Whether a converter emits appearance-name aliases into `[colors]` — `purple` pointing at
`base0E` — is the converter's own decision, and would fix those four templates without
touching the core.

**Requiring a theme to declare its extras.** A `[extras]` list, or a per-namespace opt-in.
Nothing would read it. `check` finds an undefined token by resolving the template against the
theme, which needs no declaration, and a declaration that disagreed with the file would be a
third thing to keep in sync.

**Deriving the core from the templates at runtime.** vanadis could compute "core" as the
intersection of tokens all installed templates use, which needs no hardcoded list. It is
rejected because the core would then change when a user adds a template, and the guarantee a
theme author is working against would depend on somebody else's config directory.
