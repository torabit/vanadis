# Recording the README's GIFs

Two recordings, both driven by [vhs](https://github.com/charmbracelet/vhs). Everything they
read and write lives under `demo/`, so a recording touches nothing in `~/.config`.

| tape | output | what it shows |
| --- | --- | --- |
| `demo.tape` | `media/demo.gif` | one `vanadis cycle`, and a terminal, an editor and a TUI all change |
| `init.tape` | `media/init.gif` | `vanadis init` turning a ghostty config into a template, a theme and a target |

## What has to be installed

`vhs`, and the four programs the hero GIF puts on screen.

```sh
brew install vhs tmux neovim btop starship
```

A Nerd Font is required too. The tapes name `JetBrainsMono Nerd Font`; change
`Set FontFamily` if a different one is installed.

## Recording

From the repository root, and only from there. The tapes use relative paths, and the reload
commands in `demo/vanadis/config.toml` do too.

```sh
cargo build --release
PATH="$PWD/target/release:$PATH" vhs demo/demo.tape
PATH="$PWD/target/release:$PATH" vhs demo/init.tape
```

`demo.tape` kills any tmux server on the machine before it starts. Detach from anything that
matters first.

## How it is wired

```
demo/
  vanadis/          VANADIS_CONFIG. config.toml, four templates, two themes
  home/             XDG_CONFIG_HOME. nvim, btop and the shell read from here
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

The four targets reload differently, and that difference is the point of the hero GIF.

| target | how it follows a theme |
| --- | --- |
| starship | re-reads its config on every prompt, so nothing runs at all |
| tmux | `tmux source-file`, which redraws every pane |
| nvim | `tmux send-keys` sends `:colorscheme vanadis`, which re-executes the generated file |
| btop | `tmux respawn-pane`, because btop reads a theme once, at startup |

tmux is a target and not just a stage. It paints `window-style`, the pane borders and the
status bar, which is what makes the whole frame change rather than the programs inside it.
The terminal vhs draws is not a target and its own palette never moves, so `Set Padding 0`
in `demo.tape` keeps it out of the frame.

## Putting them in the README

After both recordings exist, `media/demo.gif` goes directly below the opening two paragraphs
of `README.md`, above `## 🚀 Installation`:

```html
<p align="center">
  <img src="media/demo.gif" alt="One vanadis cycle, and tmux, Neovim, btop and the prompt all change together" width="900" />
</p>
```

and `media/init.gif` goes under Step 3, below the sentence explaining what `init` writes:

```html
<p align="center">
  <img src="media/init.gif" alt="vanadis init reading a ghostty config and matching each colour to a token the palette already holds" width="820" />
</p>
```

Keep each file under about 3MB. Lowering `Set Framerate` or trimming a `Sleep` is the first
thing to try; `Set Width` and `Set Height` are the second.
