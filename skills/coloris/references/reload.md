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
| nvim | editor | restart, or `:luafile` the generated file | not a command coloris can run |
| btop | system monitor | restart | not a command |
| hunk | diff viewer | restart | not a command |
| lazygit | git UI | restart | not a command |
| zsh with fzf | shell and fuzzy finder | `exec zsh` — fzf reads its colours from the environment | **no**: it replaces the user's shell, and coloris is a child process |
| rio | terminal emulator | rereads its config; note it runs on the machine the terminal is on, which over SSH is not the machine coloris runs on | nothing to run |

Two of these nine have a command coloris can usefully run. That ratio is the normal case, not a
gap in the format.

`bat` is the one to remember. It is the only entry where skipping the reload leaves the tool
showing the old theme with no error anywhere — the file on disk is correct and `coloris check`
is clean.

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
