# Writing templates

## The two output shapes

Which one a tool gets decides whether the user can still edit their config by hand. Pick it
before writing anything.

### Colour-only output, included by a hand-written config

The generated file holds nothing but colours. The hand-written config reads it and stays
hand-written.

| tool | generated | how the hand-written side reads it |
| --- | --- | --- |
| nvim | `lua/palette.lua` | `require("palette")` from `init.lua` |
| zsh | `.config/zsh/palette.zsh` | `source` from `.zshrc` |
| lazygit | `.config/lazygit/theme.yml` | `LG_CONFIG_FILE` merges it with `config.yml` |

**Prefer this whenever the tool has any include mechanism.** The accident it prevents is the
common one: the user edits their config, forgets it is generated, and the next apply eats the
edit.

### Whole-file output

The entire file is generated. Editing it directly loses the edit on the next apply.

Right when the file is nearly all colour anyway — a btop theme, a bat tmTheme, a hunk config —
or when the tool has no include mechanism at all, as with herdr and starship.

Put a line near the top of the template saying the file is generated and naming the `.in` to
edit instead. The template writes it; coloris knows nothing about output formats and adds no
header of its own. Where the line goes depends on the format: a tmTheme has an XML declaration
and a DOCTYPE first, so the notice is the third line.

**starship has no include and still gets an indirection layer.** Its `[palettes]` table lets
the substitutions sit in one place while every module says `style = "accent"` by name. The
whole file is still generated, but the colour is confined to one table.

## Escaping

`{{{{` renders as a literal `{{`, and `}}}}` as a literal `}}`. Single braces are always
literal, so a tool's own `{workspace}` or `{-}` passes through untouched.

**Double every `{{` and `}}` in the file you copied, not only the ones that look like a token
path.** The rule that only escapes what the renderer would act on is wrong, and cheaply so:
given `{{#eeeeee`, it leaves the `{{` alone, then writes `{{role.bg}}` in place of the hex, and
the renderer reads the four braces at the seam as one escape. Doubling every pair leaves no
unescaped `{{` for a substitution to run into, and it is the shorter rule.

The cost is that `{{ columns }}` gets written `{{{{ columns }}}}` where it would have survived
untouched. That is the right way round. rio matches its own placeholders with
`\{\{(.*?)\}\}` and then trims and lowercases, so `{{ columns }}` and `{{columns}}` are one
thing to rio and two things to coloris; a template that escapes says which it means regardless
of the spacing.

## Colour notations

Only lowercase `#rrggbb` is substitutable. Everything else has to stay as it is, because a theme
stores lowercase six-digit hex and there is nowhere to put a conversion — the template language
has no filters.

Notations to leave alone, and to tell the user about:

- `#RRGGBB` with an uppercase digit, `#rgb`, `#rrggbbaa`, `0xRRGGBB`
- `rgb()`, `rgba()`, `hsl()`, `hsla()`
- escape sequences: `\033[…m`, `\e[…m`, `\x1b[…m`, `38;5;N`
- `color123`, `colour123`
- a value made of ANSI colour names and style words — starship's `style = "bold red"`

A colour name is only a colour when the whole value is made of colour names and style words and
at least one is a colour name. `black = "#eeeeee"` is a key, `# green` is a comment, and
lazygit's `- reverse` and `- bold` say nothing about colour. Reporting those would be noise.

Indexed colour has no answer at all. Upstream PaperColor stores `['#eeeeee', '255']` — a hex and
a cterm index together — and a theme token holds a hex with no way to derive the index. A Vim
template written against that cannot be generated.

## Naming the output

**Never put the theme's name in the output path.** Apply gruvbox over `papercolor-light.theme`
and it is still called that. btop lists whatever it finds in its themes directory, so the stale
name is visible in the tool's own UI. Write `coloris.theme`, `coloris.tmTheme`.

**bat has a second layer the config cannot reach.** It selects a theme by the `name` inside the
tmTheme, not by the filename. A template writing `{{meta.name}}` there moves the name bat has to
be configured with every time the theme changes. Either write a fixed name in the template and
configure bat once, or write `{{meta.name}}` and accept that bat's own config has to follow.
Decide before writing the template; it is not a thing to discover later.

## Outputs that are scripts

An output keeps the mode it already has, and a newly created output takes the template's mode.
A generated file with a shebang that a tool executes — herdr runs `host-colors.py` — breaks if it
is written back without the executable bit. `chmod +x` the template.

There is no `mode` key in `config.toml`. The filesystem already records the answer, and a second
place to state it is a second place for it to disagree.

## Order and atomicity

Every target renders before anything is written. A failure at any target — an undefined token, a
missing theme, an unreadable template — writes nothing at all. A half-applied set of configs is
not a state coloris can produce, so a failed apply never needs unpicking.

Writes happen in the order the targets appear in `config.toml`; reloads run after every write,
in the same order.
