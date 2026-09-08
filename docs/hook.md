# hook

This document decides how a target whose output is read by a shell follows a theme change in a
shell that is already running. [docs/config.md](config.md) decides `config.toml`, the paths,
and `reload`, which runs a command after a write. That command cannot reach the shell that
started vanadis, and this is what covers the targets that need it to.

## The problem

`reload` runs argv directly, in a child process. The shell that has the wrong colours is
vanadis's parent, and no process can replace its parent's image.

```toml
reload = ["exec", "zsh"]
```

That entry fails before reaching the question: `exec` is a shell builtin, and `reload` takes no
shell. Spelling it `["zsh", "-c", "exec zsh"]` runs a shell, replaces that shell with another
one, and exits. The user's shell is untouched either way.

So the whole class of targets whose output is *sourced* rather than *read from a path* has no
way to follow an apply. fzf is the entry [docs/config.md](config.md#reload) names, because it
takes its colours from `FZF_DEFAULT_OPTS` in the environment. `LS_COLORS`, `GREP_COLORS` and a
pager named through a variable are the same shape. The count matters: this is not one tool with
an awkward interface, it is every tool configured through the environment.

The knowledge the fix needs is vanadis's, not the user's. Where the sourced file is, how to
detect a change without forking, and the per-shell syntax for registering a prompt hook are all
things `config.toml` already holds or vanadis already knows. Written by hand into a `.zshrc`,
that knowledge is copied, not referenced, and cannot be corrected when a path moves.

## The mechanism

```
vanadis hook <zsh|fish|bash>
```

Prints a snippet to stdout, to be evaluated by the shell it names:

```zsh
eval "$(vanadis hook zsh)"
```

fish reads it the way fish reads every tool in this shape, because `eval` there would need the
output collected into one argument first:

```fish
vanadis hook fish | source
```

The snippet sources the outputs of the targets that name that shell, once at evaluation, and
registers a prompt hook that sources each of them again when its file has changed. This is the
shape `direnv hook zsh`, `mise activate zsh`, `starship init zsh` and `zoxide init zsh` already
have, and a user who has one of them in an rc file recognises the line without being taught it.

**`hook`, not `init`.** `vanadis init` is the command that turns a config file that already
exists into a template, a theme and a target ([docs/init.md](init.md)). The name is taken, and
taking it twice for two unrelated jobs is worse than differing from starship.

**Every failure prints nothing on stdout.** A config that will not load, a shell name that is
not one of the three, a target whose `output` cannot be resolved: all of them report on stderr
and exit non-zero with stdout empty. `eval` of an empty string is a no-op, so a failure cannot
leave a half-written snippet in the user's shell. That is
[Order](config.md#order)'s guarantee in the shape a command that writes to a shell can have it.

`hook` reads `config.toml` and nothing else. It resolves no theme, reads no state file and does
not scan `themes/`, because the snippet holds paths and no colours. A machine that has applied
nothing yet emits the same snippet as one that has.

## The change signal is the output's own mtime

Not the state file's, for three reasons. It is direct: the file the shell sources is the file
whose change matters. `apply --only nvim` writes the state file without touching a shell
target's output, and watching the state file would re-source for nothing. And the snippet then
needs no knowledge of `$XDG_STATE_HOME` or of the state file format, only of the output path,
which it has to hold anyway.

**A missing output sources nothing and is not an error.** The hook runs on every prompt, so
anything it says, it says forever. An output that has never been applied is the ordinary state
of a machine between a `git clone` and a first `vanadis apply`. The recorded mtime stays empty
until the file appears, at which point it differs and the file is sourced.

**The resolution is one second.** `zstat +mtime` and `path mtime` both report seconds. Two
applies inside the same second as the shell's last observation are seen as one, and the shell
keeps the colours of the first until the next apply moves the mtime again. Reaching for
sub-second stamps to close that is not done: the gap needs two applies in the same second, and
a prompt hook only fires when a person is at the keyboard.

## Saying a target is sourced

A per-target key, beside `reload`:

```toml
[[targets]]
name = "fzf"
template = "templates/fzf/colours.zsh.in"
output = "~/.config/fzf/colours.zsh"
shell = "zsh"
```

| key | required | value |
| --- | --- | --- |
| `shell` | no | `zsh`, `fish` or `bash`: the shell whose `hook` sources this output |

Any other value is rejected when the config loads, the way every other enumerated value in the
file is. A typo is then a config error and not a target that silently never sources.

**A shell name, not a boolean.** The output is a file in that shell's syntax; a `.zshrc`
fragment is not something fish can source. A user with both shells keeps a zsh target and a
fish target side by side, rendered from two templates, and each `hook` sources its own. A
boolean would make that config unsayable and would leave `vanadis hook fish` guessing.

**Per target, not a top-level table.** A `[shell]` table listing target names would be a second
place where a target's name is written, which is what
[docs/config.md](config.md#layout) declined for themes and for the same reason: two places
that can disagree. `shell` sits where `reload` sits, because it answers the same question — how
a write reaches the tool — for the targets `reload` cannot answer it for.

**`shell` and `reload` are independent.** Both may be present. `reload` runs in vanadis's
process tree at apply time; `shell` reaches a shell vanadis is a child of, at that shell's next
prompt. Nothing about one implies the other.

Several targets may name the same shell. They are sourced in the order they appear in
`config.toml`, which is the order [Order](config.md#order) already gives writes and reloads.
Each is compared and sourced on its own, so a change to one does not re-source the others.

## The snippet

Three invariants, in every shell that ships.

**A second `eval` leaves one hook registered.** Re-evaluating the snippet is how a user picks
up a config change ([below](#the-paths-are-baked-in)), so it has to be free. zsh gets it from
`add-zsh-hook`, which deduplicates by function name. fish gets it from redefining the handler
function, which replaces the old one along with its event binding. bash has neither, and guards
the `PROMPT_COMMAND` append by testing for the function name first.

**The hook leaves the exit status it found.** A prompt that shows the last command's status
reads `$?` after the hook has run. A hook that returns the status of its own `stat` call makes
every failed command look successful, or the reverse. The status is saved on entry and returned
on exit.

**The hook touches nothing the user named.** Every function and variable it defines is prefixed
`_vanadis_`. The zsh function runs under `emulate -L zsh`, so a user's `setopt` does not change
what the snippet means.

### Per shell

| shell | mtime | prompt hook | forks per prompt |
| --- | --- | --- | --- |
| zsh | `zstat -A ... +mtime` from `zsh/stat` | `add-zsh-hook precmd` | none |
| fish | `path mtime` | `function ... --on-event fish_prompt` | none |
| bash | `stat` | `PROMPT_COMMAND` | one |

**bash forks once per prompt.** It has no builtin that reads an mtime. Which `stat` to write is
settled when the snippet is generated, not when it runs: vanadis knows the system it is on, so
the emitted snippet carries `stat -c %Y` or `stat -f %m` and no runtime branch. The cost is one
fork per prompt, which is less than the `PROMPT_COMMAND` of anyone who displays a git branch.

The alternative that avoids the fork is rejected [below](#rejected-alternatives). Shipping bash
with a source-at-startup snippet and no following is rejected too: a bash user who runs
`vanadis cycle` in another terminal is exactly the person this document exists for, and half an
answer here would have to be explained everywhere `hook` is mentioned.

**The recorded mtime is one variable per target**, named by the target's position, not an
associative array keyed by path. bash 3.2 is the bash macOS ships and it has no associative
arrays. The paths are baked in, so their positions are too, and nothing is lost.

### When no target names the shell

`vanadis hook fish` on a config with no fish target prints nothing on stdout, one line on
stderr, and exits zero. `eval` reads stdout, so the shell starts with no hook and the user sees
why. Exiting non-zero was considered: it makes `$?` immediately after the `eval` line non-zero
for a shell that started correctly, which is a worse thing to hand a `.zshrc` than a message.

A shell name that is not one of the three is refused by the argument parser, before any file is
read. It is the same treatment `--variant` gives a background that is not `light` or `dark`.

## The paths are baked in

The emitted snippet holds resolved output paths as literals. It does not read `config.toml`,
and it does not run vanadis.

The cost is a snippet that goes stale when a target's `output` moves or a `shell` key is added.
Re-evaluating is the fix, and it is the user's to run — the same answer direnv, mise, starship
and zoxide give for the same staleness. It is a good trade twice over. A config parse per
prompt is a `vanadis` process per prompt, which is what
[docs/config.md](config.md#querying) already refused for `get` when it declined to scan
`themes/` for a command a prompt hook calls. And a baked snippet keeps working in a shell whose
`$PATH` no longer has vanadis on it, which a shell in a container or a rescue environment
routinely does not.

## What is sourced is the user's

The file the hook sources is whatever the user's template rendered. vanadis does not parse it,
does not check that it is valid shell, and does not sandbox it. A template that renders a
syntax error breaks every shell started after the next apply, and the fix is to fix the
template.

That is the same trust `eval "$(vanadis hook zsh)"` already grants, stated out loud because the
indirection hides it: the user evaluates one line, and what runs is a file written by a
different command at a different time.

It also sets what belongs in a shell target's template. Exports and variable assignments follow
the theme. A template that starts a program or writes a file is a template that does it in
every new shell.

## Checking the snippet in CI

The snippet is generated text that is only correct if a shell accepts it. CI evaluates it under
each of the three shells and asserts the behaviour, rather than comparing it against an
expected string: a string assertion establishes that the generator did not change, and says
nothing about whether zsh can parse the result.

What is asserted, per shell: evaluating the snippet sources the output; evaluating it twice
registers one hook; touching the output re-sources it at the next prompt; leaving the output
alone does not; and the exit status of the command before the prompt survives.

This is also what fixes the versions claimed. `path mtime` is not in every fish that is
installed anywhere, and the fish CI runs on is the fish this document claims.

**fish needs a terminal before any of that can be asked.** It emits `fish_prompt` only while
drawing a prompt, and it draws none when its input is a pipe, `-i` or not, so a piped fish
answers every prompt question with the state the evaluation itself left. zsh and bash run
`precmd` and `PROMPT_COMMAND` either way. The fish run is therefore given one, and what comes
back carries the prompt and the echo of what was typed around what the script printed, which is
why the assertion reads its marker out of the text rather than off the start of a line.

## Rejected alternatives

The state file as the signal, a top-level table naming shell targets, and a config read at
prompt time are each decided in the section above that raises them.

**A stamp file per shell, compared with `[[ output -nt stamp ]]`.** bash's `-nt` is a builtin
and `: > "$stamp"` updates a stamp without forking, so this does reach zero forks per prompt.
It needs a stamp per shell process, since a shared one would let the first shell to notice a
change stop every other shell from noticing it. That means a file under a runtime directory
named by `$$`, and an `EXIT` trap to remove it. bash has one `EXIT` trap, and installing ours
silently replaces the user's. A fork per prompt is a smaller thing to spend than that.

**vanadis emitting the exports itself**, as `eval "$(vanadis env zsh)"` printing
`export FZF_DEFAULT_OPTS=...` derived from tokens. It removes the rendered file and with it the
mtime question. It also needs vanadis to know which environment variables which tools read,
which is a list of tools — and vanadis carries no list of tools. The template already expresses
this and expresses more of it: `FZF_DEFAULT_OPTS` is one string a user assembles from many
tokens, in an order only they know.

**One aggregated file that every shell target renders into.** It would let the snippet hold one
path. Nothing in `[[targets]]` would name that file, so `check` could not compare it against
anything, and vanadis would be writing an output it does not otherwise account for.

**A daemon watching outputs with inotify.** It fixes the fork and the one-second resolution
together, and costs a process to start, supervise and stop, plus a decision about what happens
to a shell whose daemon died. A prompt hook has no lifetime beyond the shell it is in.

**`vanadis hook --install`, editing the user's rc file.** The line to add is one line, and a
tool that writes into `.zshrc` has to decide how to find the file, where in it to write, and
what to do when the user has moved the line. Printing the line and letting the user place it is
what every tool in this shape does.

## Acceptance

- `eval "$(vanadis hook zsh)"` in one shell, `vanadis cycle` in another, and the first shell's
  `FZF_DEFAULT_OPTS` holds the new theme's colours at its next prompt, with no `exec zsh`.
- A second `eval` in the same shell leaves one hook registered, not two.
- `vanadis apply --only nvim`, where `nvim` is not a shell target, re-sources nothing.
