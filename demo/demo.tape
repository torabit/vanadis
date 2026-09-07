# The README's hero GIF. Run from the repository root:
#
#   cargo build --release
#   VHS_NO_SANDBOX=true PATH="$PWD/target/release:$PATH" vhs demo/demo.tape
#
# Everything before `Show` builds the herdr session and is not recorded. What is recorded
# is two `vanadis cycle` runs: light to dark and back, so the loop has no seam.
#
# The whole layout is built against a headless server and attached to at the very end.
# Typing the setup into the attached TUI instead loses it: herdr takes a while to become
# interactive, and keys sent before then go nowhere, which leaves one full-screen pane and
# sends the recorded `vanadis cycle` into nvim's buffer.

Output media/demo.gif

Require vanadis
Require herdr
Require nvim
Require btop
Require starship

Set Shell "bash"

# Not `JetBrainsMono Nerd Font` unless one is installed. fontconfig answers a name it does
# not have with whatever it does have, and here that is a Japanese font whose glyphs are
# full width, which doubles every cell and halves the grid without any error at all.
Set FontFamily "JetBrains Mono"
Set FontSize 14
Set Width 1700
Set Height 950
Set Padding 0
Set Framerate 24
Set TypingSpeed 70ms

# The size is not a matter of taste. btop refuses to draw below 80x24 and prints
# "Terminal size too small" instead. 1500 wide left it 77x26 and it drew nothing; the cell
# is about 9.3px, not the 8.4 the font size suggests, and herdr's sidebar takes 26 columns
# before the split gets any. 1700 leaves btop about 90 columns.

# The terminal vhs draws is not a target, so its own palette never changes. herdr paints
# over all of it, but the seed is papercolor-light so nothing flashes before the attach.
Set Theme { "name": "papercolor-light", "background": "#eeeeee", "foreground": "#444444", "cursor": "#d70087", "selection": "#d7d7af", "black": "#eeeeee", "red": "#af0000", "green": "#008700", "yellow": "#5f8700", "blue": "#0087af", "magenta": "#878787", "cyan": "#005f87", "white": "#444444", "brightBlack": "#bcbcbc", "brightRed": "#d70000", "brightGreen": "#d70087", "brightYellow": "#8700af", "brightBlue": "#d75f00", "brightMagenta": "#d75f00", "brightCyan": "#005faf", "brightWhite": "#005f87" }

Hide

# Nothing outside demo/ is read or written. HERDR_CONFIG_PATH is what keeps the recording
# off the herdr the recorder is already running.
Type "export COLORTERM=truecolor" Enter
Type "export VANADIS_CONFIG=$PWD/demo/vanadis" Enter
Type "export XDG_CONFIG_HOME=$PWD/demo/home" Enter
Type "export STARSHIP_CONFIG=$PWD/demo/home/starship.toml" Enter
Type "export HERDR_CONFIG_PATH=$PWD/demo/home/herdr/config.toml" Enter

# nvim's plugins come from XDG_DATA_HOME and not from XDG_CONFIG_HOME, so without this the
# recorder's own plugins load into the demo's nvim. vanadis keeps the theme it applied last
# under XDG_STATE_HOME, so without this a recording overwrites the recorder's.
Type "export XDG_DATA_HOME=$PWD/demo/home/share" Enter
Type "export XDG_STATE_HOME=$PWD/demo/home/state" Enter

# A session that already exists keeps its panes, and every new run splits three more onto
# the end. The btop target names a pane by id, so the run has to start from nothing. A
# socket left behind by a killed recording stops the next nvim from listening on it.
Type "herdr --session vanadis-demo server stop >/dev/null 2>&1; herdr session delete vanadis-demo >/dev/null 2>&1; rm -f $PWD/demo/home/nvim.sock; true" Enter
Sleep 2s

# The starting state. Nothing is running yet, so every reload fails and apply exits
# non-zero; the files it would have reloaded are written, which is all this needs.
Type "vanadis apply papercolor-light >/dev/null 2>&1; true" Enter
Sleep 1s

Type "(setsid herdr --session vanadis-demo server >/dev/null 2>&1 </dev/null &)" Enter
Sleep 5s

# A headless server starts with no workspace at all, so the root pane has to be asked for.
Type "herdr --session vanadis-demo workspace create --cwd $PWD --label vanadis >/dev/null" Enter
Sleep 1s

# Left pane the editor, right column the TUI over the shell.
Type "herdr --session vanadis-demo pane split w1:p1 --direction right --ratio 0.42 --cwd $PWD --no-focus >/dev/null" Enter
Sleep 1s
Type "herdr --session vanadis-demo pane split w1:p2 --direction down --ratio 0.55 --cwd $PWD --no-focus >/dev/null" Enter
Sleep 2s

Type "herdr --session vanadis-demo pane run w1:p2 btop" Enter
Sleep 2s
Type "herdr --session vanadis-demo pane run w1:p3 bash --rcfile demo/home/bashrc" Enter
Sleep 2s
Type "herdr --session vanadis-demo pane run w1:p3 clear" Enter
Sleep 1s
Type "herdr --session vanadis-demo pane run w1:p1 nvim --listen $PWD/demo/home/nvim.sock demo/vanadis/config.toml" Enter
Sleep 3s
Type "herdr --session vanadis-demo pane focus --pane w1:p2 --direction down >/dev/null" Enter
Sleep 1s

Type "clear; herdr session attach vanadis-demo" Enter
Sleep 7s

Show

# Light. Let it be read before anything moves.
Sleep 2s

Type "vanadis cycle" Sleep 500ms Enter
Sleep 5s

Type "vanadis cycle" Sleep 500ms Enter
Sleep 5s

Hide
Type "herdr --session vanadis-demo server stop" Enter
Sleep 3s
Type "herdr session delete vanadis-demo" Enter
