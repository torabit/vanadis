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

herdr, nvim, btop and starship are the other four. A Nerd Font is required too; the tapes
name `JetBrainsMono Nerd Font`, so change `Set FontFamily` if a different one is installed.

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

Every template reads core tokens only, so both themes render against all four targets.
Rendering the matrix is the proof, and it is what to run after editing a template:

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

**btop needs 80x24 in its own pane.** Below that it prints "Terminal size too small" and draws
nothing. `Set Width 1500` and `Set Height 950` at font size 14 is about 178x56 cells, which
leaves btop 88x30 once herdr's sidebar and the two splits have taken theirs. Shrinking the
recording without checking that pane is how the TUI ends up blank.

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
