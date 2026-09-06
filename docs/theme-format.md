# Theme data model

This document decides the theme data model: the file a theme is written in, and how its
tokens resolve.

A theme is a TOML file. It holds colour in three layers, plus metadata and a small amount of
non-colour text. Every token resolves before rendering starts, so the renderer never sees a
reference.

## Why not an existing scheme system

The requirement is a vocabulary the theme author chooses. A template needs `diff.added-bg`
because its author's git UI needs a diff background; the next person needs something else. No
fixed list of slots satisfies that, and every existing scheme system is a fixed list of
slots.

That is the whole argument. Two weaker arguments are worth setting aside first, because they
are the obvious ones and they do not hold.

**"base24 cannot express an ANSI slot that disagrees with its name."** It can. In PaperColor,
`ansi.10` is nominally Bright Green and holds pink `#d70087`, `ansi.11` is Bright Yellow and
holds purple `#8700af`. Those land in base24's `base14` and `base13`, and base24's bright
block (`base12`–`base17`) carries `NA` in the Text Editor column of its own styling table. It
has no editor meaning to contradict. base24 added that block precisely to let the terminal's
bright colours part company with syntax roles.

**"base24 pins slot meanings."** It states them, and then permits departure. The base16
styling spec says outright that *scheme designers should pick whichever colors they desire,
e.g. `base0B` (green by default) could be replaced with red.* The apparent conflict in this
palette — `base0B` means both "Strings" and "ANSI green", while `role.string` is olive
`#5f8700` and `ansi.2` is green `#008700` — is real in the sense that one slot cannot hold
two values, but it is not fatal: the upstream base24 PaperColor puts olive in `base0A` and
notes "Strings" against that row instead. Untidy, permitted, and it works.

The counting argument is likewise thin. This palette has 23 distinct hex values; sixteen of
base24's slots are spent on ANSI, and the 9 colours outside the ANSI set do not fit in the 8
that remain. It overflows by one colour, `ash`, used on one template line. Dropping it costs
almost nothing.

What none of them permit is a new name. In tinted8, which otherwise comes closest — it does
separate a `palette` from `ui` and `syntax` roles — the reference implementation declares
every one of those tables `#[serde(deny_unknown_fields)]`. `palette.sand` is a parse error.
The vocabulary is 33 palette entries (eleven colour names across `-bright` and `-dim`), 45
`ui` keys and 105 `syntax` keys, and an author may use them or not use them, but may not add
to them.

tinted8 also cannot state ANSI independently, which is the other half of what this format
needs. Its builder specification does not contain the word ANSI; the terminal's bright slots
*are* the `-bright` palette entries, and `syntax.markup.inserted` defaults to `green-bright`.
Setting the terminal's bright green to pink therefore turns diff-inserted lines pink unless
every dependent role is overridden by hand. There is no separate place to declare the slot.
Nor is there a key for a diff background tint: `syntax.markup.inserted` and `deleted` are
foreground colours.

For completeness, tinted8 is at `0.2.0-beta11`, upstream carries 4 tinted8 schemes against
338 base16 and 196 base24, and its spec and reference implementation disagree about unknown
keys (the spec says warn and ignore, the implementation aborts). That is a reason to wait,
not a reason the design is wrong.

## The file

```toml
[meta]
format = 1
name = "PaperColor Light"
variant = "light"
author = "torabit"

[colors]
paper = "#eeeeee"
olive = "#5f8700"
pink = "#d70087"

[ansi]
0 = "{{colors.paper}}"
3 = "{{colors.olive}}"
10 = "{{colors.pink}}"

[role]
bg = "{{colors.paper}}"
string = "{{colors.olive}}"
accent = "{{colors.pink}}"

[diff]
added-bg = "#d7ffd7"

[text]
delta-syntax-theme = "gruvbox-light"
```

`docs/examples/papercolor-light.toml` is the full file: `tests/fixtures/palette.json`
rewritten in this format. Its 56 colour tokens resolve to the same values as the JSON, and
every token the golden templates reference is present.

## Layers

**`[meta]`** — see [Metadata](#metadata). Addressable as `meta.*`.

**`[colors]`** — colour named by appearance (`paper`, `ink`, `crimson`), never by role. By
convention these are the primitives everything else points at, and a converter fills this
table first. The convention is not enforced: `[colors]` may hold references and other tables
may hold literals, because `[diff]` in the example above holds four tints no primitive
carries, and a rule its own reference file breaks is not a rule.

**`[ansi]`** — the terminal's 16 slots. Keys are the decimal strings `0` through `15`, no
leading zeros, no sub-tables. Any other key is an error, so `ansi.N` always means slot N.
All sixteen must be present; see
[docs/core-vocabulary.md](core-vocabulary.md#why-all-sixteen-ansi-slots). Terminals with
extended slots put them in `[colors]` alongside everything else; `[ansi]` stays exactly
sixteen.

**`[text]`** — string values that are not colours. See [Values](#values).

**Every other table** is a colour namespace, named by the author. `role` and `diff` above are
not privileged; a theme may add `[syntax]` or `[git]` and templates may reference them.
`meta`, `colors`, `ansi` and `text` are the only reserved top-level names.

Namespaces may nest: `[role.git]` gives `role.git.added`. A TOML table is a namespace, a
string is a token.

## Keys and token paths

A **token path** is one or more segments joined by `.`, each segment matching
`[a-z0-9]([a-z0-9-]*[a-z0-9])?`. Lowercase ASCII, digits, internal hyphens.

**Every key in a theme file must be a single valid segment, and a key that is not is an
error.** TOML is more permissive than this — `Paper`, `added_bg`, `"a b"` and `""` are all
legal TOML keys — and without this rule a theme could define a token no template can name.
`[role] added_bg = "#d7ffd7"` would store a value that `{{role.added_bg}}` never reaches,
with no error on either side.

The rule is on keys, not on paths, which also settles two cases TOML does not:

- `[role]` with a quoted dotted key `"git.added"` and `[role.git]` with `added` would flatten
  to the same token path. Quoted dotted keys are not single segments, so the first form is
  rejected and the collision cannot arise.
- `[ansi]` with `1.0 = "#fff"` is a sub-table in TOML, which would make `ansi.1` a namespace
  and quietly remove slot 1. Sub-tables under `[ansi]` are rejected.

**The theme's filename, minus `.toml`, must also be a valid segment.** It is the identifier
`vanadis apply papercolor-light` takes and the one the config file pins in `[auto]`, so it
cannot be `PaperColor Light.toml`.

## Values

Every value is a TOML string. Integers, floats, booleans, datetimes, arrays and arrays of
tables are all errors — `bg = 0xeeeeee` is valid TOML and is not a colour.

In `[colors]`, `[ansi]` and author-defined namespaces, a string is either a **hex literal** or
**exactly one reference** filling the whole string.

```toml
bg      = "#eeeeee"               # literal
fg      = "{{colors.ink}}"        # reference
broken  = "rgb({{colors.ink}})"   # error: not a whole-string reference
```

A hex literal is `#` and six hex digits, accepted in either case and stored lowercase.
Three-digit shorthand is rejected to keep one input form: expanding `#eee` is the author's
job, and a file mixing both spellings gains nothing. Eight-digit `#rrggbbaa` is rejected for
v0.1. Real tmThemes do carry alpha — Dracula's `#9D550FB0`, Solarized's `#93A1A180` — but bat
discards the alpha channel except for two values it repurposes as signals, so no target here
needs it yet.

### Values that are not colours

`[meta]` and `[text]` hold strings that are never parsed as colours. Everything else holds
colour, which is what lets `check` say whether a value is well-formed at all.

This is not a theoretical allowance. Five of the eleven golden templates hard-code a value
that is wrong for any theme but this one, and three of those break outright when the variant
flips:

| template | hard-coded | becomes |
| --- | --- | --- |
| `zsh/palette.zsh.in` | `--color=light` | `--color={{meta.variant}}` |
| `lazygit/theme.yml.in` | `--light` | `--{{meta.variant}}` |
| `lazygit/theme.yml.in` | `--syntax-theme=gruvbox-light` | `{{text.delta-syntax-theme}}` |
| `hunk/config.toml.in` | `base = "github-light-default"` | `{{text.hunk-base}}` |
| `hunk/config.toml.in` | `theme = "papercolor-light"` | `{{meta.id}}` |
| `hunk/config.toml.in` | `label = "PaperColor Light"` | `{{meta.name}}` |
| `bat/PaperColor-Light.tmTheme.in` | `<string>PaperColor Light</string>` | `{{meta.name}}` |

Without `meta` and `text` the tool renders a dark palette into a config that still asks fzf
and delta for light mode, and `check` cannot see it, because everything it inspects is a
valid colour. Confining non-colour values to two reserved namespaces keeps the colour
namespaces verifiable while making theme switching actually work.

`[text]` keys are free-form (subject to the segment rule) and their values are opaque. A
converter leaves `[text]` empty; a template that reads one is theme-specific, the same way a
template that reads `colors.*` is.

## References

The reference syntax is `{{token}}`, the same in theme files and in templates.

`{{` , a token path, `}}`. Anything else between braces is not a token path: `{{ title }}`,
`{{role.*}}` and `{{ansi.*}}` appear in `tests/fixtures/templates/rio/config.toml.in` and
survive verbatim into `tests/fixtures/expected/`. Of the 206 `{{` occurrences across the
golden templates, exactly those three are not token paths. A name that *is* a token path but
undefined (`{{role.acent}}`) is a hard error. Single braces are always literal, so herdr's
`{workspace}` and rio's `{-}` survive untouched.

### Escaping

`{{{{` renders as literal `{{` and `}}}}` as literal `}}`. Neither sequence occurs in the
golden templates, so the escape costs nothing there.

It is needed because rio, a terminal emulator, uses `{{...}}` for its own title placeholders and
matches them with `\{\{(.*?)\}\}`, then trims and lowercases — to rio, `{{ columns }}` and
`{{columns}}` are the same thing. The fixture happens to be written with spaces, which is the
only reason it passes through today. Written the way rio's own documentation writes it,
`{{columns}}` is a valid token path, undefined, and a hard error. `{{{{columns}}}}` says what
the author means.

### Chains

Any token may reference any other, and chains are permitted:

```toml
[role]
comment  = "{{colors.gray}}"
inactive = "{{role.comment}}"
```

Restricting references to point only at `[colors]` was considered and rejected: it forbids the
aliasing above, which is the natural way to say "inactive text is the comment colour" rather
than "inactive text is grey".

### Errors

Loading a theme either produces a flat map of every token path to its resolved value, or
fails. Nothing is skipped, defaulted, or left unresolved. Every failure below names the
offending key and its line, and a load reports all of them in one pass rather than the first,
as `.claude/rules/errors.md` requires.

| failure | reported as |
| --- | --- |
| key is not a valid segment | the key and why |
| value is not a string | the key and the TOML type found |
| malformed hex | the key and the value |
| reference is not the whole value | the key and the value |
| reference to an undefined path | the key, the missing path |
| reference to a namespace rather than a token | the key, the path, and that it is a table |
| `[ansi]` key outside `0`–`15`, or a sub-table | the key |
| `[meta]` unknown key, or missing `name` / `variant`, or `variant` outside `dark` / `light` | the key |
| reference cycle | every cycle, each once |

Reporting each cycle exactly once needs strongly connected components, not a visited set
during a depth-first walk: a plain visited set either stops at the first cycle or reports the
same cycle once per node in it. Tarjan's algorithm over the reference graph gives the right
answer and is the intended implementation.

Producing a line number for each of these rules out `#[derive(Deserialize)]`. `toml::Spanned`
does not survive `#[serde(untagged)]` or `#[serde(flatten)]`, both of which a model with
nested namespaces and open top-level tables would need, and serde does not support `flatten`
together with `deny_unknown_fields`. The parser should read the file with `toml_edit`, whose
items carry spans, and walk it by hand.

## Metadata

| key | required | use |
| --- | --- | --- |
| `format` | yes | format version, `1` for this document |
| `name` | yes | display name, `vanadis list`, `{{meta.name}}` |
| `variant` | yes | `dark` or `light`, nothing else, no default |
| `author` | no | provenance only |

Unknown keys in `[meta]` are an error. A silently ignored `varient = "dark"` would leave the
theme unusable for reasons the file does not show.

`format` is one line now and the only thing that lets v0.2 tell an old theme from a broken
one later.

`meta.id` is readable as a token but is not written in the file: it is the filename, which is
the identifier. **The filename is the identifier, `meta.name` is the display name.** They need
not match, and vanadis never resolves a theme by `meta.name`. Two names look redundant, but
the upstream schemes carry a display name a converter would otherwise discard, and deriving
either from the other loses capitalisation.

`variant` is required rather than derived from the luminance of `role.bg`. Deriving it is easy
— the herdr shim in the fixtures already computes sRGB luma — but it would make a theme's
light/dark pairing in `[auto]` depend on a background colour someone might change for
unrelated reasons. The variant is a statement of intent, so the author states it.

`author` is optional. It is provenance, and nothing in vanadis reads it. Requiring it would
mean `vanadis init` has to invent a value or refuse to finish, for a palette its user wrote
themselves. Converters fill it from the upstream scheme, so a theme that came from somewhere
still says where.

## Rejected alternatives

**YAML as the native format.** The upstream tinted-theming schemes are YAML, which is the
argument for it, but that only matters inside the importer, which converts either way.
Against it: the config file is TOML, so an author learns one format, and YAML coerces types
where TOML does not — an unquoted `no` or `1.0` is a hazard a colour file does not need. The
binary will still link a YAML parser once the importer lands; that is the importer's cost, not
the author's.

**`{token}` in themes, `{{token}}` in templates.** The current JSON fixture uses single
braces. Unifying them makes one rule true everywhere: `{{...}}` is vanadis's, `{...}` is the
target tool's. The two sides still need separate code — a theme value is one whole-string
reference, a template is a scan — so the saving is the shared token-path grammar, not a shared
implementation.

**Bare `colors.paper`, no braces, and `$colors.paper`.** Both are unambiguous today, because a
literal always starts with `#`. Neither survives the arrival of `[text]`, where a value is an
arbitrary string.

**A `[semantic]` parent table.** `[semantic.role]` would make the layering visible in the
file, at the cost of turning every token path into `semantic.role.bg`. The golden templates
say `{{role.bg}}`, and they are the API.

**`ansi` as a 16-element array.** Shorter, and it enforces the count for free, but it makes
ANSI the one namespace resolved by index rather than by path.

**Includes or theme inheritance.** A theme resolves against itself and nothing else.
Cross-file references would reintroduce resolution order and make a theme file non-portable.

**Light and dark in one file.** Upstream is split on this: gruvbox-material and Everforest
carry every variant in one file behind a runtime switch, Tokyo Night ships a file per variant,
Solarized shares sixteen colours and inverts the roles. One file per variant duplicates a
palette across two files, which is a real cost. It is still the right default here, because
`[auto]` in the config file selects a whole theme by name and a per-target override pins one;
both operate on themes, and a file holding two would have to be addressed as
`papercolor#light`, which is a second identifier syntax for one saving.

**The Design Tokens Format Module (DTCG).** The W3C Design Tokens Community Group published a
stable 2025.10 (a Community Group Report, not a W3C Standard) covering much of what this
document decides: groups nest freely, a token carries `$type` and `$value`, and one token
aliases another by path.

Against it as the native format:

- Its colour value is an object, not a string. `paper = "#eeeeee"` becomes
  `"paper": { "$type": "color", "$value": { "colorSpace": "srgb", "components": [0.933, 0.933, 0.933], "hex": "#eeeeee" } }`,
  where `hex` is an optional fallback rather than the value. These files are written by hand
  next to the configs they colour.
- Accepting only the `hex` fallback would implement a fraction of the spec while claiming the
  name.
- DTCG has no notion of the terminal's sixteen slots, so the core vocabulary is still ours to
  decide. Adopting it removes no decision.
- Its alias syntax is `{token}`, which this document reserves as literal text, and it also
  requires JSON Pointer `$ref` support.

**DTCG's model in TOML syntax** — `paper = { value = "#eeeeee" }` — deserves its own line,
because the objection above is to the syntax, not the model. An object per token would let
alpha, colour spaces and non-colour values arrive without breaking existing files, where the
bare-string form makes each of those a breaking change. It is rejected for weight: every one
of the 56 tokens in the reference palette grows a wrapper to buy extensibility for three
things that are all currently out of scope, and `meta.format` handles the version break when
one of them arrives.

## A base16 scheme in this format

The base16 converter reads an upstream YAML:

```yaml
system: "base16"
name: "Gruvbox dark, hard"
author: "Dawid Kurek (dawikur@gmail.com), morhetz (https://github.com/morhetz/gruvbox)"
variant: "dark"
palette:
  base00: "#1d2021"   # ...through base0F
```

`meta` comes straight across, `system` is dropped, `format = 1` is added.

`[colors]` takes the sixteen palette entries. The converter cannot invent appearance names, so
the slot names are what it has — **lowercased**, since `base0A` is not a valid segment:

```toml
[colors]
base00 = "#1d2021"
base08 = "#fb4934"
base0a = "#fabd2f"
```

`[ansi]` follows the ANSI column of the base16 styling specification (`home/styling.md`
v0.4.2), which tinted-shell's `templates/base16.mustache` implements:

| slot | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| | base00 | base08 | base0b | base0a | base0d | base0e | base0c | base05 |

| slot | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| | base03 | base08 | base0b | base0a | base0d | base0e | base0c | base07 |

Slots 9–14 repeat 1–6; base16 has no separate bright set. `base09` and `base0f`, which
tinted-shell exposes as extended slots 16 and 17, stay in `[colors]`.

`[role]` is where the base16 vocabulary is spent. The full mapping is in
[docs/core-vocabulary.md](core-vocabulary.md#base16-onto-the-core), which decides the core;
the eight tokens this document's own examples use land as:

| token | base16 |
| --- | --- |
| `bg` | base00 |
| `fg` | base05 |
| `comment` | base03 |
| `keyword` | base0e |
| `string` | base0b |
| `error` | base08 |
| `ok` | base0b |
| `warn` | base0a |

Every core token is filled. Two slots are used by neither `[ansi]` nor `[role]` and remain in
`[colors]`: `base06` and `base0f`. `string` and `ok` land on the same green, which is
base16's own conflation.

### An imported theme cannot drive every template

Four of the golden templates read `{{colors.brown}}`, `{{colors.purple}}` or
`{{colors.slate}}` directly. A theme converted from base16 has `colors.base09` and
`colors.base0e`; it has no `brown` and no `purple`, and nothing in the core vocabulary
supplies them. Rendering those templates against it fails with undefined tokens.

That is correct behaviour, not a defect, and the rule it illustrates belongs here:
**`colors.*` is a theme's private vocabulary. A template that reads it is bound to that
theme.** A template meant to survive a theme switch uses `role.*`, `ansi.*` and `meta.*`, all
of which the core vocabulary guarantees. `check` is what reports the mismatch, and a
per-target theme override is what pins a template to the theme it was written for.

Whether the converter should additionally emit appearance-name aliases — `purple` pointing at
`base0e`, since base16 does assign hues to slots — is the converter's to decide.

## Import and export are not symmetric

**Import is lossless.** base16, base24 and tinted8 put colours in fixed slots. Reading one
means dropping its entries into `[colors]` under their own names and pointing `[ansi]` and
`[role]` at them. Nothing overflows, because the destination has no fixed size.

**Export can lose colour, and no design fixes that.** A theme carrying more than the target
format holds must drop something on the way out. How much depends on the theme:

| theme | distinct hex |
| --- | --- |
| nord | 16 |
| PaperColor Light, as written in `docs/examples/` | 23 |
| gruvbox, upstream `colors/gruvbox.vim` | 36 |

nord is built as sixteen colours and lands in base16 exactly. gruvbox carries `bright`,
`neutral` and `faded` variants of seven hues plus fifteen background and foreground steps
across both variants, and its base16 scheme upstream is a selection out of those. Importing
that selection and writing it back out is lossless, because the choosing happened upstream.

**Export is not a feature. It is a template.** A base16 scheme is YAML with sixteen colours,
`name`, `author` and `variant` in it, so it is `base16.yaml.in` with sixteen `{{colors...}}`
and three `{{meta...}}` in it. Putting knowledge of output formats into vanadis is what
rendering through templates exists to prevent, and the converter is an importer: its stated
work is reading scheme bytes into this model, and nothing in this document extends it.

What is missing is a way to render one named theme once, rather than rendering the active
theme to every target. The config file's model writes every target on every apply, which
would overwrite `gruvbox.yaml` with whatever theme is active. Export therefore needs something
like `vanadis render <template> --theme <name> --out <path>`, which keeps vanadis ignorant of
formats. That is a separate decision, not this document.

Filling base16 from the core vocabulary alone leaves two slots empty. Ten follow from
`[ansi]`; `base01`, `base02`, `base04` and `base09` carry `role` names in the core
([docs/core-vocabulary.md](core-vocabulary.md#base16-onto-the-core)); `base06` (a foreground
lighter than `role.fg` in a dark scheme, darker in a light one) and `base0f` (brown) carry
none. Neither appears in the terminal's sixteen slots, so a shell reading the export is
unaffected.

## Left open

- Where theme files live and how they are discovered.
- A subcommand for rendering one named theme, which export templates need.
- **Indexed colour.** Upstream PaperColor stores `['#eeeeee', '255']` — hex and cterm index
  together — and a Vim template written against it would need the index. A token holds a hex
  literal and there is no conversion function, so those templates cannot be generated.
- **Transparency.** tokyonight and catppuccin carry `NONE` as a palette value and pass it
  through the same paths as a colour. Here it is neither a hex literal nor a reference, so a
  transparent and an opaque variant need two templates. `[text]` is deliberately not a way in:
  a transparent background is a colour-slot value, and admitting `NONE` there would end the
  guarantee that a colour token holds a colour.
- **Derived colour.** tokyonight writes `bg_visual = blend_bg(blue0, 0.4)` and nightfox has a
  blend/shade library; catppuccin derives its bright ANSI set by scaling LCH lightness. This
  format stores the computed result, so the relationship is lost on import and the derived
  colour does not follow when its source changes. Adding functions would make this a template
  language, which this format rules out, so any answer is a separate design.
