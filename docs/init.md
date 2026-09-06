# init

This document decides how `vanadis init` turns a config file somebody already has into a
template, a theme and a `[[targets]]` entry. It builds on
[docs/config.md](config.md), which decides where those files live, and
[docs/theme-format.md](theme-format.md), which decides what a theme value may hold.

Hand-writing a template for every tool already configured is the largest cost of adopting
vanadis, and most of that cost is mechanical: find the colour literals, replace each with a
token, write the values into a theme. `init` does the mechanical half. Deciding what a
colour means is the half a person keeps.

```
$ vanadis init ~/.config/nvim/lua/palette.lua
found 10 hex values

  #eeeeee  1 occurrence   token name? > bg
  #444444  1 occurrence   token name? > fg
  #af0000  1 occurrence   token name? > error
  #bcbcbc  1 occurrence   token name? > border
  ...

wrote:
  ~/.config/vanadis/themes/papercolor-light.toml
  ~/.config/vanadis/templates/nvim/palette.lua.in
  ~/.config/vanadis/config.toml   (added 1 target)
```

## The command

```
vanadis init <FILE> [--theme <ID>] [--name <NAME>] [--variant <dark|light>]
```

`FILE` is one config file that already exists. Every flag is optional and is asked for
instead when it is left out.

**One file per run.** Taking several paths would have to interleave their dialogues or run
them in sequence, and running it again says the second thing without deciding anything.

**`FILE` is never modified.** It stays where it is and becomes the target's `output`, so the
next `vanadis apply` writes over it with what the template renders. That is the whole point
of the byte-identical guarantee [below](#verifying-before-writing): the file the user has is
the file they keep.

## The dialogue

One prompt per distinct value, in the order the values first appear, with the number of
occurrences shown.

```
  #878787  4 occurrences   token name? [role.comment] >
```

| answer | meaning |
| --- | --- |
| a bare name | `role.<name>` |
| a name with a `.` | that token path, as written |
| several names | the value means more than one thing; ask per occurrence |
| empty | take the default, or skip the value when there is no default |
| `-` | skip the value even when there is a default |

A skipped value keeps its literal in the template, which is what a hex that is not a colour
needs. `herdr/host-colors.py` in the fixtures carries `#444444`, `#ffffff` and `#000000`
inside prose describing a terminal bug; tokenising those would let a theme switch rewrite the
explanation.

The default comes from the theme being written into: if some token already resolves to this
value, it is offered. A `role.*` token wins over any other, and ties break in path order.
That is what makes the eleventh file mostly the return key.

### Why the prompt is per value, and per occurrence only when asked for

Per value alone is too coarse. In `docs/examples/papercolor-light.toml`, seventeen `role`
tokens sit on fifteen distinct values: `comment` and `inactive` share `#878787`, and `warn`
and `accent-warm` share `#d75f00`. Collapsing by value merges each pair, and the merge is
silent — the template reads `{{role.comment}}` in the place `inactive` belongs, and the
mistake only surfaces on a theme where the two differ.

Per occurrence alone is too fine. The eleven fixtures hold 41 hex occurrences over 13 distinct
values for btop, 32 over 14 for rio, and 31 over 13 for bat. Asking per occurrence turns a
13-question dialogue into a 41-question one to catch a distinction that most of those
occurrences do not have.

So the prompt is per value, and answering with more than one name opens the per-occurrence
pass for that value only.

```
  #878787  4 occurrences   token name? > comment inactive

    line 22  accentMuted = "#878787"       # inactive
      which? [comment/inactive] > inactive
    line 24  muted = "#878787"             # comment
      which? [comment/inactive] > comment
```

Those two lines are `tests/fixtures/expected/hunk/config.toml`, and the trailing comments
are the file's own.

The user pays the fine-grained cost exactly where they said the value carries more than one
meaning. On a palette shaped like the reference one that is two prompts out of fifteen.

## Colour notations

**`#rrggbb`, lowercase, and nothing else is substituted.**

[docs/theme-format.md](theme-format.md#values) stores a colour as lowercase six-digit hex.
Any other notation would be normalised on the way into the theme and come back out changed,
which breaks the guarantee [below](#verifying-before-writing) that an apply reproduces the
original byte for byte. `#EEEEEE` renders as `#eeeeee`, `#eee` renders as `#eeeeee`, and
`rgb(238, 238, 238)` cannot be produced at all: the template language is substitution with no
filters, so there is nowhere to put the conversion.

Every colour notation in the corpus is already lowercase `#rrggbb`. Across the eleven files
in `tests/fixtures/expected/` there is not one uppercase literal, three-digit literal,
`rgb()` call, or colour name used as a value. The restriction costs the corpus nothing, and
the report below is what keeps it from costing a file outside it silently.

### What is reported

Everything recognised as a colour and not substituted is printed with its line and its text.

| reported | recognised by |
| --- | --- |
| `#RRGGBB` with an uppercase digit | shape |
| `#rgb`, `#rrggbbaa` | shape |
| `0xRRGGBB` | shape |
| `rgb()`, `rgba()`, `hsl()`, `hsla()` | shape |
| `\033[…m`, `\e[…m`, `\x1b[…m`, `38;5;N` | shape |
| `color123`, `colour123` | shape |
| a quoted string of ANSI colour names and style words | contents |

The last row is the only one that is not decidable from shape, and it is deliberately narrow:
the whole quoted value must be made of the sixteen ANSI colour names and the style words
`bold`, `dim`, `italic`, `underline`, `reverse`, `bright`, `light`, `default` and `none`, and
at least one of the words must be a colour name. It catches a starship `style = "bold red"`.
Nothing in this corpus trips it, which is the same measurement as the row above: the eleven
files use no colour name as a value.

The two halves of the rule each do work. Requiring the whole string keeps `style = "visual"`
out, which is one of starship's own `[palettes]` names and not a colour word at all.
Requiring a colour name keeps lazygit's `- reverse` and `- bold` out, which say nothing about
colour.

**A full CSS colour-name list is rejected.** All 148 names, matched anywhere, would report
`black = "#eeeeee"` in rio's config and `# green` in hunk's, because in this corpus colour
names appear as keys and in comments, never as values. A report that is mostly wrong is one
the user stops reading, which costs more than the names it would have caught.

## What it writes

```
$VANADIS_CONFIG/themes/<id>.toml
$VANADIS_CONFIG/templates/<name>/<basename>.in
$VANADIS_CONFIG/config.toml
```

The config directory and `themes/` are created when they do not exist. `init` is the command
somebody runs before there is anything to run it against.

### The template beside the other templates, not beside the output

`templates/<name>/<basename>.in` under the config directory, which is the layout
`docs/examples/config.toml` already writes.

Writing `~/.config/nvim/lua/palette.lua.in` next to the original was the original sketch and
is rejected for the reason [docs/config.md](config.md#rejected-alternatives) gives for not
deriving `output` from `template`: btop enumerates its themes directory, so a `.theme.in`
left there appears in its theme list. Keeping templates in one place is also what makes them
version controlled as a unit, which is the thing a user of this tool most wants to do with
them.

### The theme

Named tokens are written as literal hex in the namespace the answer named.

```toml
[meta]
format = 1
name = "PaperColor Light"
variant = "light"

[role]
bg = "#eeeeee"
fg = "#444444"
error = "#af0000"
```

**No `[colors]` layer.** The reference palette points `[role]` at appearance names in
`[colors]`, and that is the better-factored theme. Producing it would mean asking for an
appearance name as well as a role for every value, which doubles a dialogue whose length is
the thing being optimised. Refactoring a flat theme into two layers afterwards is an edit to
one file that changes no rendered byte.

`meta.name` is derived from the identifier: hyphens become spaces and each word is
capitalised, so `papercolor-light` becomes `Papercolor Light`. The upstream spelling is
`PaperColor`, and no derivation recovers it —
[docs/theme-format.md](theme-format.md#metadata) says as much when it explains why the
identifier and the display name are two fields. It is not asked for because nothing resolves
a theme by it and editing one line is cheaper than one more prompt on every new theme.

`meta.variant` is not derived. [docs/theme-format.md](theme-format.md#metadata) makes it a
statement of intent rather than something computed from a background colour, so `init` asks
when it is writing a new theme and never asks again.

A second run against an existing theme adds keys to it and leaves everything else, including
comments and key order, as written.

### The core it did not fill

One config file does not carry seventeen roles and sixteen ANSI slots, so a theme `init`
wrote is normally incomplete. It prints what is still missing when it finishes.

```
core tokens still undefined (11):
  role.keyword role.string role.visual role.linenr role.accent
  role.accent-alt role.accent-warm role.selection-bg role.hover-bg
  ansi.0 … ansi.15
```

**The skeleton is not written into the theme file as commented-out lines.** That was the
earlier reading of [docs/core-vocabulary.md](core-vocabulary.md#a-missing-core-token-is-not-a-load-error),
and it is rejected here. A commented list is a second copy of a list vanadis already compiles
in, so a later run has to find and delete the right comment line, and a user who edits or
deletes the block leaves `init` guessing what it means. `check` is where an incomplete theme
is reported and stays reported; the printed list is the same answer offered at the moment it
is useful, and costs nothing to keep in sync.

### The config entry

A `[[targets]]` table is appended, with `name`, `template` and `output`. `reload` is not
guessed: `docs/config.md` counts two of eleven targets that have one at all, and a wrong
command is worse than no command.

Formatting elsewhere in `config.toml` is preserved, so an entry can be added to a file
somebody has been editing by hand.

## Verifying before writing

**`init` renders the template it built against the theme it built and compares the result to
the original file, before writing anything.** A single differing byte and it reports the
mismatch and writes nothing.

`check` is core to this tool because vanadis generates output. `init` generates output too,
and it generates it in the one situation where the correct answer is known exactly: the
original file is on disk. So the acceptance condition — that an apply straight after an init
reproduces the file — is an invariant `init` enforces on itself rather than a property a test
hopes for.

It is also what makes brace escaping non-optional. A config for a tool with its own template
syntax carries sequences the renderer would consume: rio writes `{{columns}}`, which is a
valid token path and so an undefined-token error, and `{{{{` and `}}}}` already mean literal
braces.

**Every `{{` and `}}` is doubled, not only the ones the renderer would act on.** A single
brace is left alone, which is what rio's `{workspace}` and `{-}` need.

Escaping only the markers the renderer acts on is not enough, and a property test is what
said so. Given `{{#eeeeee`, that rule leaves the `{{` alone, because on its own the renderer
copies it, and then writes `{{role.bg}}` against it. The renderer reads the four braces at
the seam as one escape and the file comes back wrong. Doubling every pair leaves no
unescaped `{{` in the template for a substitution to run into, and it is the shorter rule.

It costs `{{ columns }}` being written `{{{{ columns }}}}` where it would have survived
untouched. That is the right way round.
[docs/theme-format.md](theme-format.md#escaping) introduces the escape for rio precisely
because `{{columns}}` and `{{ columns }}` are the same thing to rio and different things to
vanadis, and a template that says which one it means does not depend on the spacing.

Without the escaping the verification fails, which is the point: the check finds the case
rather than the user finding it later.

## Failure

Any of these reports and writes nothing at all:

- `FILE` cannot be read, or is not UTF-8
- `FILE` is already some target's `output`
- the template path already exists
- the theme exists and does not load
- the verification above does not match

All three files are staged beside their destinations before any of them replaces the file it
is going to, which is the order [docs/config.md](config.md#order) has an apply write in. A
failure while staging leaves nothing behind and nothing replaced.

## Rejected alternatives

**Guessing token names from the surrounding key.** `muted = "#878787"` and
`lineNumberFg = "#b2b2b2"` are strong hints, and a table from common config key names to core
roles would answer many prompts before they are asked. It is rejected because a wrong guess
accepted by a return key is worse than no guess: the dialogue's whole value is that a person
decided what each colour means, and a plausible default is exactly what stops them deciding.
The one default `init` does offer is not a guess — it is a value this theme has already been
told the name of.

**Deriving `--variant` from the luminance of the value named `bg`.** The herdr shim in the
fixtures computes sRGB luma in six lines, so the arithmetic is not the objection.
[docs/theme-format.md](theme-format.md#metadata) settles it: the variant is a statement of
intent, and pairing themes in `[auto]` should not depend on a background colour someone
changes for an unrelated reason.

**Writing the target's `reload`.** See [above](#the-config-entry).

**A `--dry-run`.** `apply` has one because it overwrites files that exist. `init` writes
three files that do not exist yet and refuses to overwrite any of them, so the run is already
reversible by deleting what it names.
