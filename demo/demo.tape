# The README's hero GIF. Run from the repository root:
#
#   vanadis apply papercolor-light  # not needed; the setup below does it
#   vhs demo/demo.tape
#
# Everything before `Show` builds the tmux session and is not recorded. What is recorded
# is two `vanadis cycle` runs: light to dark and back, so the loop has no seam.

Output media/demo.gif

Require vanadis
Require tmux
Require nvim
Require btop
Require starship

Set Shell "bash"
Set FontFamily "JetBrainsMono Nerd Font"
Set FontSize 15
Set Width 1400
Set Height 800
Set Padding 0
Set Framerate 24
Set TypingSpeed 70ms

# The terminal vhs draws is not a target, so its own palette never changes. tmux paints
# over all of it, but the seed is papercolor-light so nothing flashes before the attach.
Set Theme { "name": "papercolor-light", "background": "#eeeeee", "foreground": "#444444", "cursor": "#d70087", "selection": "#d7d7af", "black": "#eeeeee", "red": "#af0000", "green": "#008700", "yellow": "#5f8700", "blue": "#0087af", "magenta": "#878787", "cyan": "#005f87", "white": "#444444", "brightBlack": "#bcbcbc", "brightRed": "#d70000", "brightGreen": "#d70087", "brightYellow": "#8700af", "brightBlue": "#d75f00", "brightMagenta": "#d75f00", "brightCyan": "#005faf", "brightWhite": "#005f87" }

Hide

# Nothing outside demo/ is read or written.
Type "export COLORTERM=truecolor" Enter
Type "export VANADIS_CONFIG=$PWD/demo/vanadis" Enter
Type "export XDG_CONFIG_HOME=$PWD/demo/home" Enter
Type "export STARSHIP_CONFIG=$PWD/demo/home/starship.toml" Enter

# The starting state. tmux is not running yet, so its reload fails and is reported; the
# file it would have sourced is written, which is what the new session reads.
Type "vanadis apply papercolor-light >/dev/null 2>&1" Enter
Sleep 1s

Type "tmux kill-server >/dev/null 2>&1; true" Enter
Type "tmux -f demo/home/tmux/vanadis.conf new-session -d -s vanadis-demo" Enter
Sleep 1s

# Left pane the editor, right column the TUI over the shell.
Type "tmux split-window -h -l 42% -t vanadis-demo:0.0" Enter
Type "tmux split-window -v -l 45% -t vanadis-demo:0.1" Enter
Type "tmux send-keys -t vanadis-demo:0.0 'nvim demo/vanadis/config.toml' Enter" Enter
Type "tmux respawn-pane -k -t vanadis-demo:0.1 btop" Enter
Type "tmux respawn-pane -k -t vanadis-demo:0.2 bash --rcfile demo/home/bashrc" Enter
Type "tmux select-pane -t vanadis-demo:0.2" Enter
Sleep 2s

Type "clear; tmux attach -t vanadis-demo" Enter
Sleep 4s

Show

# Light. Let it be read before anything moves.
Sleep 2s

Type "vanadis cycle" Sleep 500ms Enter
Sleep 4s

Type "vanadis cycle" Sleep 500ms Enter
Sleep 4s

Hide
Type "tmux kill-server" Enter
