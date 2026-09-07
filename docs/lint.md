# lint

This document decides how vanadis catches a literal a template left behind: a string that has
to follow the theme, is not a colour, and is invisible to every check vanadis has today.
[docs/config.md](config.md) decides `config.toml`, the paths, and `check`, which compares an
output against what its template renders. That is a claim about a file on disk. This is a claim
about the template itself, which is why it is not a `check` finding.

## The problem

A template carries strings that are not colours and still have to change with the theme:
`--color=light` in a pager's arguments, another tool's theme name, a display name written into
a status line. Substituting the colours and leaving those behind produces a config that
contradicts itself and passes everything vanadis can currently ask of it. Every colour in it is
a well-formed colour, and the output still matches what the template renders, so `check` is
clean.

A `catppuccin-latte` found by reading a herdr template is the case this exists for. Finding it
was a person's job, done once, by eye.

## The mechanism

Two themes of opposing `meta.variant`, and one comparison:

> A line that renders the same against a light theme and against a dark one does not follow the
> background. If it also names a background, it is a literal that should have.

That is the whole rule. A line that genuinely varies differs between the two renders and is
never reported, so this needs no word list tuned against false positives and no knowledge of
the file format it is reading. `--color={{meta.variant}}` renders `light` against one theme and
`dark` against the other, which is exactly what disqualifies it.

**Each line is rendered on its own**, not cut out of a rendered file. A token path cannot
contain a newline ([docs/theme-format.md](theme-format.md#keys-and-token-paths) restricts every
segment), so a `{{` opened on one line and closed on the next is not a substitution and never
was, and splitting the source into lines changes nothing about what renders. Rendering the file
whole and indexing back into the source would be wrong instead: a `[text]` value is opaque and
may carry a newline of its own, which moves every line under it.

The line number reported is the template's, because the template is where the fix goes.

## What is named

A line is reported when its two renders are equal and it contains, as a whole word and without
regard to case, any of:

- `light` or `dark`,
- either theme's identifier,
- either theme's `meta.name`.

**Whole word** means delimited by something that is not a letter or a digit, at either end or
against the ends of the line. `highlight_style` is not a `light`, and a rule that reported it
would be trained away within a day. `dark_theme = "{{...}}"` does contain the word and is not
reported anyway, because the token makes the two renders differ.

The two identifiers are in the list because a template that hard-codes the name of one of the
themes being swapped between is the same mistake as one that hard-codes `light`.

## The command

```
vanadis lint [<light> <dark>] [--only <name>]
```

**Not `check`.** `check` asks whether the machine still holds what vanadis generated. This asks
whether a template is honest. Folding it in would mean a `check` run reporting two kinds of
thing that need different fixes, and one of them fires on a template that was never applied.

**`lint` and not `sweep`.** The prose that handed this job to a person calls it a sweep, and
continuing to call it that would be nice. `lint` wins because a person reading `--help` knows
what a lint is without being told, and because it is the right home if a second static claim
about a template is ever worth making. This is its first rule and not its only possible one.

Findings are printed one per line, in target order, the shape `check` prints:

```
herdr: /home/ada/.config/vanadis/templates/herdr/config.toml.in:12: theme = "catppuccin-latte"
```

Target, template, line, and the line itself. The template is printed resolved, the way `check`
prints an output, because a path relative to the config directory is not one an editor opens. A
run with nothing to say prints how many targets it swept.

## The two themes

**Two positional themes, defaulting to `[auto]`.** `vanadis lint` with no themes uses the pair
`[auto]` names, which is the pair the machine actually swaps between. `[auto]` is optional, and
a config without one names two themes on the command line instead. A config with neither is an
error and not a silent skip.

**They must not share a `meta.variant`.** This is what makes the rule sound rather than a
preference. Two light themes render `{{meta.variant}}` as `light` in both passes, which makes
the one construction this exists to bless look exactly like the literal it replaces. Naming two
themes of the same background is refused.

**A target pinned to its own `themes` is rendered through it**, the way an apply resolves a
pinned target: the light theme selects that target's light theme, and the dark one its dark. A
target whose table names one theme for both backgrounds has no pair to compare and is named as
skipped. Reporting it as clean would claim a sweep that could not happen.

**A pair that shares values weakens the rule.** Two themes that define the same `role.accent`
make every line reading only that token look static. The pair to lint with is a pair that
actually differs, which is what `[auto]` holds by construction.

## Exit status

Non-zero when there is anything to report, the way `check` exits non-zero on a finding. That is
what lets it sit beside `check` in CI rather than being a thing somebody remembers to run.

A target that does not render against one of the two themes is reported and counts as a
finding. It cannot be swept, and a clean exit would say it was.

**There is no suppression marker.** The template language is `{{token}}` substitution over an
arbitrary file format, so there is no comment syntax to hide one in: a marker that a TOML
template ignores is a syntax error in a `.theme` file. If a tool's own schema ever collides for
real, by naming a section `[colors.dark]` that has nothing to do with the theme, the answer is
a key in that target's `[[targets]]` entry, where the rest of a target's exceptions already
live. Nothing is added until one turns up.

## What it cannot see

The sweep this replaces was a person reading a whole template, and it does not cover everything
that person did.

**A literal that follows the theme rather than the background.** A delta or hunk style name, a
`text.*` value, another tool's theme name that is not one of the two being compared: none of
these are variant-dependent, so two renders of opposing backgrounds are silent about them. The
table in the agent skill that lists them stays, because it is how a template author writes the
fix, and this document does not claim to have replaced it.

**A third theme's name.** `theme = "gruvbox-dark"` in a template linted against `paper-light`
and `ink-dark` contains the word `dark`, so that one is caught by the background rule. A
`theme = "nord"` is not caught by anything until `nord` is one of the two.

**Anything that is not a registered target.** A template nothing points at is not rendered by
an apply and is not read here either.

## Rejected alternatives

**A `check` finding.** Covered above: different subject, different fix, and it would fire on
templates no apply has touched.

**Scanning the template source for the words, without rendering.** A line carrying a token is
skipped and a line without one is reported, which agrees with the mechanism above almost
everywhere and is cheaper. It disagrees where a token resolves to the same value in both
themes, and there it is wrong in the direction that matters: it stays quiet. Rendering twice is
the version that cannot be fooled by a line that only looks variable.

**Every identifier in `themes/` as the word list.** It would catch a hard-coded
`catppuccin-latte` without `catppuccin-latte` being one of the two themes. It also makes the
finding set depend on files the sweep is not about: importing an unrelated theme would change
what a template is guilty of, and a comment naming another theme becomes a finding nobody
asked for. The two identifiers named on the command line are the two the outputs actually swap
between.

**Requiring `[auto]`.** `[auto]` is optional, and a config without one still has templates
worth sweeping. Requiring it would make an optional table load-bearing for a command that only
needs two names.

**Failing an apply.** A literal is not a reason to refuse to write a file the user asked for.
`apply` writes; `check` and `lint` are the commands that judge.

**A word list the user can extend.** `light` and `dark` name the two backgrounds vanadis has,
and a theme's own name is already in the list. Anything beyond that is a per-theme string,
which the mechanism cannot see however long the list gets.

## Acceptance

- A template with `--color=light` is reported when a dark theme is one of the two.
- A template with `--color={{meta.variant}}` is not.
