# Recording the README's GIFs

Two recordings, both driven by [vhs](https://github.com/charmbracelet/vhs). Everything they
read and write lives under `demo/`, so a recording touches nothing in `~/.config` and nothing
in the herdr you are already running.

| tape | output | what it shows |
| --- | --- | --- |
| `demo.tape` | `media/demo.gif` | one `vanadis cycle`, and a multiplexer, an editor and a TUI all change |
| `init.tape` | `media/init.gif` | `vanadis init` turning a ghostty config into a template, a theme and a target |

## What has to be installed

`vhs`, and the four programs the hero GIF puts on screen.

```sh
brew install vhs
```

herdr, nvim, btop and starship are the other four. The tapes name `JetBrains Mono`, and
naming a font that is not installed is worth avoiding: fontconfig answers with whatever it
does have rather than an error, and on this machine that is a Japanese font whose glyphs are
full width, which doubles every cell, halves the grid and says nothing.

```sh
fc-match "JetBrains Mono"   # should not answer with something else
```

## Recording

From the repository root, and only from there. The tapes use relative paths, and so do the
reload commands in `demo/vanadis/config.toml`.

```sh
cargo build --release
VHS_NO_SANDBOX=true PATH="$PWD/target/release:$PATH" vhs demo/demo.tape
VHS_NO_SANDBOX=true PATH="$PWD/target/release:$PATH" vhs demo/init.tape
```

`VHS_NO_SANDBOX` is needed on Ubuntu, and the failure without it is not obvious: vhs draws
the terminal in a headless Chromium, AppArmor blocks the unprivileged user namespace that
Chromium's zygote sandbox needs, and Chromium aborts with a stack trace ending in
`content::ZygoteHostImpl::Init()` and the single line `recording failed`.

```sh
sysctl kernel.apparmor_restrict_unprivileged_userns   # 1 on Ubuntu 24.04 and later
```

Turning that sysctl off fixes it machine-wide and is the worse trade. The variable disables
the sandbox for one throwaway Chromium rendering a terminal on the local machine, which loads
nothing from the network.

## How it is wired

```
demo/
  vanadis/          VANADIS_CONFIG. config.toml, four templates, two themes
  home/             XDG_CONFIG_HOME. herdr, nvim, btop and the shell read from here
  bin/reload-btop   the one reload that needs two calls
  adopt/ghostty/    the config init.tape adopts
  .run/             a copy of the two above, written into by init.tape, gitignored
```

Three of the four templates read core tokens only. The herdr one also reads
`text.herdr-base`, a string and not a colour: the name of the built-in theme herdr falls back
to for anything `[theme.custom]` does not set. Getting it wrong is visible, and only just —
a light pane border drawn across an otherwise dark frame.

`herdr config check` is what says which keys `[theme.custom]` accepts. It names every key it
does not know and then ignores it, so a token invented in the template is silent at runtime
and loud only there:

```sh
HERDR_CONFIG_PATH=$PWD/demo/home/herdr/config.toml herdr config check
```

Both themes render against all four targets. Rendering the matrix is the proof, and it is
what to run after editing a template:

```sh
for theme in papercolor-light papercolor-dark; do
  for template in demo/vanadis/templates/*/*.in; do
    VANADIS_CONFIG=$PWD/demo/vanadis vanadis render "$template" --theme "$theme" >/dev/null \
      || echo "$theme $template"
  done
done
```

`vanadis check` answers a different question. It compares what is on disk against what the
theme would render, so naming the theme that is not applied reports drift on all four, which
is the correct answer and not a failure.

The four targets follow a theme differently, and that difference is the point of the hero GIF.

| target | how it follows a theme |
| --- | --- |
| starship | re-reads its config on every prompt, so nothing runs at all |
| herdr | `herdr server reload-config`, and the sidebar, tab bar, borders and pane backgrounds redraw |
| nvim | `nvim --server … --remote-send` over its own RPC socket, which re-executes the colorscheme |
| btop | quit and start again, because btop reads a theme once, at startup |

Two of them go through `demo/bin/` rather than straight into `reload`. btop needs two calls
and `reload` is an argv with no shell. herdr needs one, but it prints its JSON reply on
stdout and vanadis does not capture a reload's output, so the reply would land in the middle
of the frame.

herdr is a target and not just a stage. The terminal vhs draws is not a target and its palette
never moves, so without herdr painting `panel_bg`, `sidebar_bg` and the rest, the frame around
the panes would sit still while the panes flipped. `Set Padding 0` keeps the terminal itself
out of shot.

## Three things that will bite

**Name the session on every herdr call.** `herdr server` and `herdr status` ignore
`XDG_CONFIG_HOME` and resolve to the socket under `~/.config/herdr`, which is the herdr you
are running right now. A bare `herdr server stop` will stop it. `herdr session list` and
`herdr session delete` do honour `XDG_CONFIG_HOME`, so the two disagree; `--session
vanadis-demo` names the socket outright and settles it.

**Delete the session before recording.** A stopped session keeps its panes, so a second run
splits three more onto the end and `w1:p2` is no longer btop. `demo.tape` deletes it first and
again at the end.

**Build the layout before attaching, not after.** `demo.tape` starts a headless server, asks
it for a workspace, makes the splits and starts the three programs, and only then runs `herdr
session attach`. Typing the same commands into an attached TUI does not work: herdr takes
several seconds to become interactive and keys sent before then go nowhere, so the splits are
lost, one full-screen pane is left, and the recorded `vanadis cycle` is typed into nvim's
buffer instead of a shell. Nothing about that failure is visible while recording, because the
setup is inside `Hide`.

**btop needs 80x24 in its own pane.** Below that it prints "Terminal size too small" and draws
nothing. The cell is about 9.3px wide at font size 14, not the 8.4 the font size suggests, and
herdr's sidebar takes 26 columns before either split gets any: `Set Width 1500` left btop 77
columns and it drew a black rectangle. 1700 leaves it about 90. Shrinking the recording
without looking at that pane is how the TUI ends up blank.

**Start every pane through `demo/bin/demo-env`.** Exporting the demo's variables before the
server starts is not enough. herdr gives each pane the recorder's own `$SHELL`, which reads
the recorder's rc files, and a line as ordinary as

```sh
export XDG_CONFIG_HOME="$HOME/.config"
```

in `~/.zshrc` puts the recorder's config directory back for everything that pane launches.
What that looks like is nvim opening the recorder's plugin manager and btop drawing in the
recorder's theme, in a recording that is supposed to be showing off a different one.

Three variables matter and only one of them is obvious. nvim loads plugins from
`XDG_DATA_HOME`, not `XDG_CONFIG_HOME`. vanadis keeps the theme it applied last under
`XDG_STATE_HOME`, so a recording without it overwrites what the recorder had applied.

## Putting them in the README

After both recordings exist, `media/demo.gif` goes directly below the opening two paragraphs
of `README.md`, above `## 🚀 Installation`:

```html
<p align="center">
  <img src="media/demo.gif" alt="One vanadis cycle, and herdr, Neovim, btop and the prompt all change together" width="900" />
</p>
```

and `media/init.gif` goes under Step 3, below the sentence explaining what `init` writes:

```html
<p align="center">
  <img src="media/init.gif" alt="vanadis init reading a ghostty config and matching each colour to a token the palette already holds" width="820" />
</p>
```

Keep each file under about 3MB. Lowering `Set Framerate` or trimming a `Sleep` is the first
thing to try; the width and height are the last, for the reason above.
