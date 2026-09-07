# Config format

This document decides where vanadis keeps its files and how a user declares what to render.
It builds on [docs/theme-format.md](theme-format.md), which decides the theme file, and
[docs/core-vocabulary.md](core-vocabulary.md), which decides what a theme must define.

`docs/examples/config.toml` is the reference file: the eleven targets in
`tests/fixtures/templates/`, with `tests/fixtures/MANIFEST.tsv` mapping each template back to
the file it was taken from.

[docs/init.md](init.md) decides how `vanadis init` produces a template, a theme and a
`[[targets]]` entry from a config file that already exists.
[docs/schemes.md](schemes.md) decides the one path this document does not cover: the cache
`vanadis remote update` writes the tinted-theming collection into.
[docs/hook.md](hook.md) decides how a target whose output a shell sources follows an apply,
which is the one case [`reload`](#reload) below cannot reach.

## Layout

```
~/.config/vanadis/
├── config.toml
└── themes/
    ├── papercolor-light.toml
    ├── papercolor-dark.toml
    └── gruvbox-dark.toml
```

The directory is `$XDG_CONFIG_HOME/vanadis`, falling back to `~/.config/vanadis` when
`XDG_CONFIG_HOME` is unset. `$VANADIS_CONFIG` replaces the whole directory, not just the
file, so `themes/` moves with it. That is what lets an integration test point at a fixture
tree, and it keeps the config and the themes it names from drifting to separate places.

Themes are discovered by scanning `themes/` for `*.toml`. They are not listed in the config:
the filename minus `.toml` is the identifier
([docs/theme-format.md](theme-format.md#keys-and-token-paths) already requires it to be a
valid segment), so a list would be a second place to write the same name and a second thing
to keep in sync.

**`themes/` is the only place scanned.** Additional scan directories were considered and
rejected. Two directories holding `gruvbox-dark.toml` need a rule for which one wins, and a
resolution order is what
[docs/theme-format.md](theme-format.md#rejected-alternatives) declined to reintroduce when it
rejected theme inheritance. A theme collection checked out elsewhere is reachable with a
symlink inside `themes/`, which needs no format at all.

Files in `themes/` that do not end in `.toml` are ignored. Subdirectories are ignored, since
a theme nested one level down has no unambiguous identifier.

A `.toml` file that does not load is reported and skipped rather than failing the scan. This
is the same call [docs/core-vocabulary.md](core-vocabulary.md#a-missing-core-token-is-not-a-load-error)
makes for a theme missing a core token, and for the same reason: one unfinished file should
cost the user that theme, not every command that enumerates the directory.

## State

The theme that was applied last is recorded in `$XDG_STATE_HOME/vanadis/state.toml`, falling
back to `~/.local/state/vanadis/state.toml` when `XDG_STATE_HOME` is unset.

```toml
theme = "papercolor-light"
```

`apply` writes it. `current` prints the identifier it holds on the first line, then one line
per diverging target, and prints nothing and exits non-zero when the file does not exist yet,
so a shell hook can tell "not applied" from a theme name without parsing. `list` reads it to
mark the applied theme, and a state file it cannot read costs that mark and nothing else.
`get` reads it to know which theme to query.

The file is written beside its destination and renamed over it, so a state file that exists
is one that was written whole.

`vanadis apply --only nvim` writes the targets it names and leaves every other one alone, so
one theme name stops describing the machine. Those targets are recorded under `[targets]`:

```toml
theme = "papercolor-light"

[targets]
nvim = "nord"
```

The table holds exactly what diverges. A target brought back to the theme every other target
carries stops being recorded, and a whole apply clears the table. An `--only` apply that
writes fails before any whole apply, because there is no theme for the targets it does not
name to be on. `--dry-run` and `--diff` record nothing and so are not held to that: reading
one target's diff is exactly what a config being built up a target at a time needs.

**`$VANADIS_CONFIG` does not move it.** The config directory holds what the user wrote, and
is what gets version controlled or copied between machines. The state file records which
theme this machine is currently showing, which is the one thing that must not travel with it.
The separation also means a test can point `$VANADIS_CONFIG` at a fixture tree without a run
leaving a file behind in it.

**It is TOML rather than the bare identifier.** A file holding `papercolor-light` and nothing
else would be smaller, and `current` would be `cat`. It carries one key today. TOML is what
lets a second one, such as the mode a `--variant` apply resolved, be added later without
changing how the file is read, and `toml_edit` is already a dependency.

## The file

Every name below is the user's. vanadis ships no themes and carries no list of tools, so
`papercolor-light` is a file in `themes/` and `herdr` is whatever the user called that entry.

```toml
[auto]
light = "papercolor-light"
dark = "papercolor-dark"

[cycle]
themes = ["papercolor-light", "nord", "gruvbox-dark", "everforest"]

[[targets]]
name = "herdr"
template = "templates/herdr/config.toml.in"
output = "~/.config/herdr/config.toml"
reload = ["herdr", "server", "reload-config"]

[[targets]]
name = "nvim"
template = "templates/neovim/palette.lua.in"
output = "~/.config/nvim/lua/palette.lua"
themes = { light = "gruvbox-light", dark = "gruvbox-dark" }
```

### `[auto]`

Optional. `light` and `dark` each name a theme, and both are required when the table is
present.

It is the table `vanadis apply --variant dark` resolves through, so a shell hook can flip
the whole set without knowing theme names. Naming a theme directly works with or without it.

`vanadis apply` with neither a theme nor `--variant` fails, whether or not `[auto]` is
present. Re-rendering the theme the state file records was considered for that case and
rejected: an apply overwrites files, and a command that does it with no argument is one
stray return key away from a write the user did not ask for. `vanadis apply $(vanadis current)`
says the same thing and says it out loud.

**`[auto]` is a table, not appearance detection.** Reading the desktop's light/dark setting
was considered and left out: the setting lives on the machine the terminal is on, which over
SSH is not the machine vanadis runs on. Anything that needs to detect it can call
`vanadis apply --variant dark`.

### `[cycle]`

Optional. `themes` is the list `vanadis cycle` steps through, in the order it is written.

```
$ vanadis current
papercolor-light
$ vanadis cycle
applied nord
wrote herdr nvim
```

Everything after the theme is chosen is `apply`: the same targets, the same order, the same
output, the same state file written at the end.

**The position is the applied theme's place in the list.** It is not an index in the state
file. An index would be a second record of where the machine is, and it and `theme` can
disagree: `apply nord` moves one and not the other, and nothing could then say which of the
two is right. Looking the applied theme up costs a scan of a list a person typed by hand.

It follows that the list may not write one theme twice, and that is rejected when the file
loads. Two entries with one name give the lookup two answers, it takes the first, and the
cycle can never step past it. It also follows that a list of fewer than two themes is
rejected: stepping through one theme re-applies it, which `apply` already says in fewer
words.

A theme the list does not name, and a machine that has applied nothing at all, both start the
cycle at the first entry. Neither is an error. `apply` names any theme it likes and is not
required to stay inside the cycle, and a user who has just written the table and run `cycle`
is asking to begin.

`--dry-run` and `--diff` are the same two `apply` carries, and mean the same thing: nothing is
written, so the position does not move and the next run steps to the same theme.

**Whether the themes exist is not checked here.** That matches `[auto]`, which also holds
identifiers and not files. A cycle that steps to a theme `themes/` does not hold fails the way
`apply` naming it fails, which is the message that already exists for it.

**There is no `--back`.** The list wraps, so the way back from an overshoot is round. A
second direction is a flag to add when somebody is cycling a list long enough for that to be
tedious, and it needs no decision recorded before then.

### `[[targets]]`

| key | required | value |
| --- | --- | --- |
| `name` | yes | the target's identifier, a valid segment, unique in the file |
| `template` | yes | path to the template |
| `output` | yes | path to write |
| `reload` | no | argv to run after writing, as an array |
| `themes` | no | `{ light = "...", dark = "..." }`, this target's own themes |

`name` identifies the target, not the tool. Three of the eleven targets in the reference
config belong to herdr, a terminal multiplexer — its config, one plugin's config, and a
script it executes — so `herdr`, `herdr-thumbs` and `herdr-host-colors` are three names for
one program. It is what
`check` prints and what an error message names.

### Paths

A leading `~` expands to the home directory. A relative path resolves against the config
directory, which is what lets the reference config write `templates/herdr/config.toml.in` and
keep every template in one place.

`$VAR` is not expanded. One expansion rule is enough, and a config that expands variables has
to say when: at parse time, at apply time, and whether an unset variable is empty or an
error.

### `reload`

An array of arguments, executed directly. No shell.

```toml
reload = ["herdr", "server", "reload-config"]
```

A shell string was the original sketch and is rejected. Measured against the corpus it buys
nothing: of the eleven targets, two have a reload command at all, and neither
`bat cache --build` nor `herdr server reload-config` needs a shell. Against it, a string
makes what actually runs depend on which shell is installed and how the value quotes, and
`--dry-run` cannot show the command without reimplementing word splitting. A pipeline is
still reachable by writing a script and naming it here.

**Most targets cannot be reloaded, and the format does not pretend otherwise.** Of the eleven:

| target | how a change takes effect | vanadis can run it |
| --- | --- | --- |
| bat, a pager | `bat cache --build`, mandatory | yes |
| herdr, a terminal multiplexer | `herdr server reload-config` | yes |
| starship, a shell prompt | next prompt | nothing to run |
| nvim, btop, hunk, lazygit — an editor, a system monitor, a diff viewer, a git UI | restart the program | not a command |
| zsh with fzf, a shell and a fuzzy finder | the shell sources the output again | no: `exec zsh` replaces the user's shell, and vanadis is a child process |

The last row is the one `reload` cannot be spelled for at all, because the shell to fix is
vanadis's parent. [docs/hook.md](hook.md) decides `vanadis hook`, the prompt hook that lets such
a target follow an apply, and the `shell` key that marks it.

So `reload` is optional and absent means nothing runs. It is not a hook system, and there is
no `pre` counterpart: nothing in the corpus needs work done before a write.

A reload that exits non-zero is reported and does not stop the remaining targets. The files
are already written by then and a reload failure does not make them wrong.

### `themes`

The per-target override, and the reason someone with a favourite editor theme can adopt
vanadis at all. A scheme's editor plugin colours far more than sixteen slots, so its base16
port loses fidelity, and being told to give that up to use the tool is where adoption stops.

Selection works in three steps:

1. The theme to apply is the one named on the command line, or the one `[auto]` gives for the
   requested mode.
2. That theme's `variant` is the mode. It is read, never guessed —
   [docs/theme-format.md](theme-format.md#metadata) makes `variant` a statement of intent
   rather than something derived from a background colour.
3. Each target with `themes` takes `themes[mode]`. Every other target takes the theme from
   step 1.

Light and dark therefore flip together no matter how many targets override, which is the
point. A `themes` table without an entry for the mode being applied is an error, reported
before anything is written.

## Output

**The output path must not encode the theme's name.** Two of the eleven show why. The
generated theme for btop, a system monitor, is `papercolor-light.theme`, and bat's is
`PaperColor-Light.tmTheme`; applying gruvbox to either leaves a file still named for
papercolor, and btop lists whatever it finds in its themes directory. The reference config
writes `vanadis.theme` and `vanadis.tmTheme`.

bat has a second layer that the config cannot reach: it selects a theme by the `name` inside
the tmTheme, so a template writing `{{meta.name}}` moves the name bat has to be configured
with. That is the template author's to solve, and it is worth knowing before writing one.

**An output keeps the mode it already has.** A newly created output takes the template's
mode. One of the eleven, `herdr/host-colors.py`, carries a shebang and is executed by herdr,
so writing it back as a plain non-executable file breaks it. A `mode` key was considered and
rejected: the filesystem already records the answer, and a third place to state it is a third
place for it to disagree.

### An output that is a symlink is replaced

Every write is staged as `<output>.vanadis-new` and renamed over the output, which is what
[Order](#order) needs: a rename is the one operation that cannot leave a half-written file
where a config was. A rename also replaces a symlink with a regular file. So a target whose
`output` is a symlink has the link the first time it is applied, and a regular file every time
after.

```
$ ls -l ~/.config/hunk/config.toml
lrwxrwxrwx  ~/.config/hunk/config.toml -> ~/dotfiles/hunk/config.toml
$ vanadis apply nord
$ ls -l ~/.config/hunk/config.toml
-rw-r--r--  ~/.config/hunk/config.toml
```

The file in the dotfiles repository is not touched, and the next `stow` finds a real file where
its link was.

Writing through the symlink instead was considered and is not done. It keeps the link, and it
also makes an apply write into a path the user did not name: the output would be wherever the
link happens to point, which for a dotfiles repository is a tracked file. Naming the file to be
written is what `output` is for, and a rename cannot both preserve a link and stay atomic.

### Managing vanadis with a symlink farm

This matters because symlink farms are how dotfiles are managed, and vanadis is a build tool
for dotfiles. The arrangement that works treats a generated config as a build artifact, which
is not something to symlink or to commit:

```
dotfiles/vanadis/.config/vanadis/     stowed, tracked
├── config.toml
├── templates/
└── themes/

~/.config/<tool>/...                  written by vanadis, not stowed, not tracked
~/.local/state/vanadis/state.toml     not tracked, and outside the config directory already
```

An `output` names a file vanadis owns. Switching themes then changes nothing the repository can
see, because [State](#state) already keeps the applied theme out of the config directory for
this reason. Importing a theme shows up as one new file under `themes/`, which is a source and
is meant to be committed.

The trade is that a machine without vanadis has no config for those tools until it runs one
apply. Pointing `output` at the real file inside the repository rather than at the link keeps
the symlink working, at the cost of a diff on every theme switch. Both are arrangements the
format allows; neither is one vanadis enforces.

## Order

Every target renders before anything is written. A failure at any target — an undefined
token, a missing theme, an unreadable template — writes nothing at all. This is the same
guarantee [docs/core-vocabulary.md](core-vocabulary.md#a-missing-core-token-is-not-a-load-error)
makes for a theme missing a core token, held at the config level so a half-applied set of
configs is not a state the tool can produce.

Writes happen in the order targets appear. Reloads run after every write, also in order.

## Checking

`vanadis check` renders every target in memory and compares the result against the file on
disk. It reports five things, and any of them exits non-zero:

| finding | what it means |
| --- | --- |
| incomplete | a theme the run resolves to does not define the whole core vocabulary |
| drift | the output no longer holds what its template renders |
| missing | the output has not been written yet, or has been deleted |
| unreadable | the output exists and cannot be read |
| unrenderable | the target does not render, so an apply would fail on it |

Findings go to stdout, one per finding. A clean run prints the number of targets checked.

The first is about a theme rather than a target, so it is reported once however many targets
are on that theme, and before them: a theme short of the core is why some of the target
findings under it exist.

**The core is enforced against the themes the run resolves to, not against everything in
`themes/`.** That is the applied theme, and the theme each target with a `themes` table names
for the mode being applied. `check` asks whether this machine is consistent, and a theme no
target is on is not part of that answer. Auditing the whole directory instead would mean one
unfinished file failing every run, which is the shape
[docs/core-vocabulary.md](core-vocabulary.md#a-missing-core-token-is-not-a-load-error) rejects
for the loader and for the same reason. A theme that is not applied yet is asked the same
question by naming it: `vanadis check <theme>`.

A target that cannot be checked costs that target and nothing else. The remaining targets are
still checked, which is the call
[the theme scan](#layout) already makes for a file that will not load.

`vanadis check` with no argument checks against the state file, so `[targets]` is honoured and
a machine left on a partial apply reports clean. `vanadis check <theme>` and
`vanadis check --variant dark` check against that theme instead. That is what makes it a CI
step: a fresh checkout has no state file, and CI knows which theme the committed outputs were
generated from.

**`check` does not test whether an output is writable.** It reads. Testing a write means
performing one, and the answer is stale by the time an apply runs. An unwritable output is
`apply`'s to report, and it reports it before replacing anything, because every write is
staged beside its destination first.

`vanadis apply <theme> --dry-run` renders the same way and writes nothing, naming the targets
whose output would change and the reload commands that would run. `--diff` adds a unified
diff of each, and implies `--dry-run`.

The list `--dry-run` prints is shorter than the one a real apply prints afterwards.
`--dry-run` answers "what would change". An apply writes every target it rendered, whether or
not the bytes moved.

**A write that resolves elsewhere is named.** A directory on the way to an `output` can be a
link, which the path `config.toml` spells does not show and `ls -l` on the output does not
show either. `~/.config/bat/themes/gruvbox.tmTheme` reads as a file under `~/.config`, and
lands inside a dotfiles repository when `~/.config/bat` is a link into one.
[Managing vanadis with a symlink farm](#managing-vanadis-with-a-symlink-farm) makes that the
ordinary shape rather than an edge case, so `--dry-run` and `--diff` print the file each named
target would actually be written to, whenever it is not the one the config spells:

```
bat: /home/ada/.config/bat/themes/gruvbox.tmTheme resolves to /home/ada/dotfiles/bat/themes/gruvbox.tmTheme
```

Only the directories on the way are resolved. The output's own name is not followed, because
[An output that is a symlink is replaced](#an-output-that-is-a-symlink-is-replaced): the file
that changes is the link itself. An output that resolves to itself prints nothing at all, which
is every target on a machine with no links between the config and the file.

**Refusing the write, or warning outside `--dry-run`, is not done.** The write is not wrong;
the surprise is. Deciding whether the destination sits inside a git work tree is not done
either: under a symlink farm that is the ordinary case, so the warning would fire on every
apply and be trained away. That the path resolved elsewhere is a fact, and it needs no guess
about what the user meant by it.

## Querying

`vanadis get role.bg` prints the value that token resolves to and nothing else, so
`$(vanadis get role.bg)` is a colour. `vanadis get --json` prints the whole resolved theme.
Exactly one of the two is required: a single value rendered as JSON is a quoted string with
nothing to select out of it.

The theme is the one the state file records. `--theme nord` reads that theme instead, and is
the only way to query before anything has been applied. There is no `--variant`, for the
reason [below](#rejected-alternatives).

Failure writes nothing to stdout and exits non-zero. An undefined token, a name that is not a
token path, a theme that will not load and no theme applied all behave that way, so a shell
hook tests the exit status and otherwise uses the value without inspecting it for a plausible
shape.

The JSON is a flat object keyed by token path.

```json
{
  "meta.id": "gruvbox-dark",
  "meta.name": "Gruvbox Dark",
  "meta.variant": "dark",
  "role.bg": "#282828"
}
```

A key is written exactly as a template writes the token, so `{{role.bg}}` and `."role.bg"`
name the same thing. Nesting the object by namespace was considered and rejected: it shortens
the `jq` expression, and it costs the one-to-one correspondence between a key and the token a
template writes.

`get` reads the one theme it was asked for. It does not scan `themes/`. Every other command
scans, and reports the files that will not load, which is right for a command a person runs
and wrong for one a prompt hook runs on every line. A broken theme is `list`'s to report.

## Rendering to stdout

```
vanadis render <TEMPLATE> --theme <ID>
vanadis render --target <NAME> [--theme <ID> | --variant <dark|light>]
```

Renders one thing and writes it to stdout. It writes no file, reloads nothing and records
nothing, either way. What is named settles everything else: a `TEMPLATE` is a path, and a
`--target` is a `[[targets]]` entry.

**The template form exists** because everything else in this document renders the *active*
theme to *every* target.
[docs/theme-format.md](theme-format.md#import-and-export-are-not-symmetric) states that writing
a base16 scheme out of a vanadis theme is a template and not a feature.
Such a template registered as a target would have `gruvbox.yaml` overwritten with whatever
theme is current the next time anything is applied. A per-target `themes` table pins a target
to one theme forever, which is a different thing from naming a theme for one render.

**The target form answers "does this template still produce this file".** That is the question
adopting a config already on disk starts from, and until this existed the only way to ask it
was to point a target's `output` at the file that was already there, run [Checking](#checking),
and then rewrite `output` to the name the target should actually take. It is also the
per-target half of what `--dry-run` and `--diff` answer for a whole apply, and it answers it
without reading a state file that has anything in it yet.

**`--theme` is required with `TEMPLATE`.** Falling back to the applied theme is the one
behaviour that form exists to avoid, so there is no spelling of it. A target has an applied
theme; a file sitting in a directory does not.

**A target resolves its theme the way [Checking](#checking) does**: the one `--theme` names, or
the one `[auto]` holds for `--variant`, or the one the state file records for that target,
which is the applied theme unless a partial apply moved it. Naming none of the three with
nothing applied is an error, and the same error `check` gives. The target's own `themes` table
is then resolved on top, so what is printed is what an apply would write, byte for byte:

```
vanadis render --target btop > ~/.config/btop/themes/current.theme
vanadis check --only btop
```

**stdout, not a `--out` path.** `get` already answers on stdout and nothing else, and a
redirect is what a person writes anyway:

```
vanadis render base16.yaml.in --theme nord > nord.yaml
```

Taking a path would mean deciding whether it may overwrite, whether the write is staged, and
what mode the file takes — three decisions this command can simply not have. Adding `--out`
later is not a breaking change; removing it would be.

**`TEMPLATE` is resolved against the working directory**, not the config directory, which is
what [Paths](#paths) does for a target's `template`. The two rules differ because that form
does not read `config.toml` at all: a command that never opens the config file has no business
resolving paths against the directory it sits in. It also means a template that is not part of
anyone's config — the usual case for an export — is named the way every other program is given
a file. A `--target` renders the template its entry names, so [Paths](#paths) applies to it
unchanged.

The template form loads only the named theme, the way [Querying](#querying) has `get` load one.
Scanning `themes/` would parse every other file and warn about a broken one, which has nothing
to do with that render. The target form scans, because a target may pin itself to a theme
other than the one being rendered, which is a name only the catalogue can resolve.

Both forms render whole before anything is printed, so an undefined token leaves stdout empty
rather than holding the prefix of the file that resolved. That is [Order](#order)'s guarantee
in the shape a command with no output file can have it.

**`<TARGET>` is a flag and not a positional.** The positional is the template path, which
landed first, and one positional cannot be both without deciding what `vanadis render btop`
means when `btop` is also a file in the working directory. A command that reads the config to
find out what its argument meant is the kind of resolution this document rejects everywhere
else.

## Rejected alternatives

**`--variant` on `get`.** `apply` and `check` take it because they act on the machine, and
the machine has a background. A query wants the theme that is on now, which is the default,
or one named outright, which is `--theme`. Reading the value `[auto]` would pick without
picking it answers no question a tool asks.

**No config: scan for `*.in` and write the sibling path.** This is what the reference
JavaScript renderer does, and it needs no config file at all. It works there because it scans
one repository that it owns. vanadis's templates live wherever each tool's config lives, and
scanning `~/.config` for `*.in` would claim files vanadis never wrote, with no way to know
whether the sibling path is safe to overwrite. There is also nowhere to attach `reload` or a
per-target theme.

**Derive `output` from `template` by dropping `.in`.** All eleven fixtures follow this rule,
so it is not a hypothetical saving. It is rejected because it forces every template to sit
next to its output, inside the directory the tool reads — and btop enumerates that directory,
so a `.theme.in` left there appears in its theme list. An explicit `output` also lets every
template live in one place, which is what makes a template directory backed up or version
controlled as a unit.

**Listing themes in the config.** The filename is already the identifier, so a list restates
it and can disagree with the directory.

**A `type` key separating colour-only outputs from whole-file outputs.** The distinction is
real for the person writing templates: an output that is only colours gets included by a
hand-written config, while a whole-file output is destroyed by editing it directly. It is
invisible to vanadis, which renders a template to a path either way, so it belongs in a
template-authoring guide rather than in a schema.

**`enabled = false` per target.** Deleting the entry or commenting it out already says it,
and a disabled entry that still names a template invites the question of whether `check`
should verify it. The premise is that a target is turned off by hand and stays off; a use
that turned targets on and off per machine, or per invocation, would need something the
comment character cannot express.
