# Getting a change to reach the tool

Rendering a file is not the same as the tool showing the new colours. Most tools cannot be
reloaded at all, and `reload` in `config.toml` does not pretend otherwise: it is optional, and
absent means nothing runs.

```toml
reload = ["herdr", "server", "reload-config"]
```

An array of arguments, executed directly. **No shell.** What runs does not depend on which
shell is installed or on how the value quotes, and `--dry-run` can print the command without
reimplementing word splitting. A pipeline is reachable by writing a script and naming the
script.

A reload that exits non-zero is reported and does not stop the remaining targets. The files are
already written by then, and a failed reload does not make them wrong.

There is no `pre` counterpart and no hook system.

## How a change reaches each tool

| tool | what it is | how the change takes effect | goes in `reload` |
| --- | --- | --- | --- |
| bat | pager | `bat cache --build`, **mandatory** — without it bat keeps serving the cached theme | `["bat", "cache", "--build"]` |
| herdr | terminal multiplexer | `herdr server reload-config` | `["herdr", "server", "reload-config"]` |
| starship | shell prompt | rereads its config on the next prompt | nothing to run |
| nvim | editor | restart, or `:luafile` the generated file | not a command vanadis can run |
| btop | system monitor | restart | not a command |
| hunk | diff viewer | restart | not a command |
| lazygit | git UI | restart | not a command |
| zsh with fzf | shell and fuzzy finder | the shell sources the output again — fzf reads its colours from the environment | **no**: mark the target `shell = "zsh"` and use [the prompt hook](#a-target-a-shell-sources) |
| rio | terminal emulator | rereads its config; note it runs on the machine the terminal is on, which over SSH is not the machine vanadis runs on | nothing to run |

Two of these nine have a command vanadis can usefully run. That ratio is the normal case, not a
gap in the format.

`bat` is the one to remember. It is the only entry where skipping the reload leaves the tool
showing the old theme with no error anywhere — the file on disk is correct and `vanadis check`
is clean.

## A target a shell sources

`reload` cannot reach a tool configured through the environment. The shell holding the stale
colours is vanadis's parent, and no process replaces its parent's image, so `exec zsh` is
unreachable however it is written. `reload = ["exec", "zsh"]` fails before that: `exec` is a
shell builtin and `reload` takes no shell.

Mark the target with the shell that sources its output:

```toml
[[targets]]
name = "zsh"
template = "templates/zsh/palette.zsh.in"
output = "~/.config/zsh/palette.zsh"
shell = "zsh"
```

`zsh`, `fish` and `bash`. The value names the shell whose syntax the output is written in, so a
zsh target and a fish target sit side by side, each rendered from its own template.

Then the user adds one line to their rc file:

```zsh
eval "$(vanadis hook zsh)"
```

```fish
vanadis hook fish | source
```

That registers a prompt hook which sources the output again whenever the file changes, so an
`apply` or a `cycle` run in another terminal reaches this shell at its next prompt. The paths
are written into the snippet, so the line has to be re-run after an `output` moves or a target
is added.

`shell` does not replace `reload`. Both may be present: `reload` runs a command after the
write, and `shell` reaches a shell vanadis cannot run a command in.

**What belongs in such a template.** Exports and variable assignments. The file is sourced in
every new shell and again on every change, so a template that starts a program or writes a file
does that each time. A template that renders invalid shell syntax breaks every shell started
after the next apply.

## Working out a tool that is not in the table

Ask in this order, and stop at the first yes.

1. **Does it watch its config file?** Then nothing goes in `reload`. Say so in a comment beside
   the target so the next person does not go looking.
2. **Does it have a reload subcommand, or does it document a signal?** `<tool> --help` and
   `<tool> reload --help` are where this shows up. A subcommand is what `reload` wants. A signal
   needs a pid, which argv cannot compute — write a script and name the script.
3. **Does it cache or compile the theme?** bat is the case: the file it reads is not the file it
   serves. Look for a `cache`, `build`, `compile` or `index` subcommand. This is the class most
   often missed, because everything looks right and the colours do not change.
4. **Otherwise it reads at startup.** Leave `reload` out. The user restarts the program.

**Do not guess.** A `reload` that names a command the machine does not have fails on every
apply, and one that names the wrong command can do something the user did not ask for. Absent
`reload` costs a restart; a wrong one costs trust in the whole apply.

If the answer is genuinely unknown, leave it out, note the question in a comment on the target,
and let the user fill it in the first time they notice the colours did not move.
