# The GIF for adoption. Run from the repository root:
#
#   vhs demo/init.tape
#
# It works on a copy under demo/.run, so a recording never touches demo/vanadis. The file
# it adopts is a ghostty config, which is the case the README argues for: a tool with no
# template repository anywhere.

Output media/init.gif

Require vanadis

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 17
Set Width 1100
Set Height 640
Set Padding 16
Set Framerate 24
Set TypingSpeed 80ms

Set Theme { "name": "papercolor-light", "background": "#eeeeee", "foreground": "#444444", "cursor": "#d70087", "selection": "#d7d7af", "black": "#eeeeee", "red": "#af0000", "green": "#008700", "yellow": "#5f8700", "blue": "#0087af", "magenta": "#878787", "cyan": "#005f87", "white": "#444444", "brightBlack": "#bcbcbc", "brightRed": "#d70000", "brightGreen": "#d70087", "brightYellow": "#8700af", "brightBlue": "#d75f00", "brightMagenta": "#d75f00", "brightCyan": "#005faf", "brightWhite": "#005f87" }

Hide

# The swatch beside each hex is only drawn when the terminal says it can show the exact
# colour. vhs does not set COLORTERM, so without this the column is blank.
Type "export COLORTERM=truecolor" Enter

# A throwaway HOME, so init writes its theme, template and target entry somewhere the
# repository does not track, and so the paths on screen are the ones a real run prints.
# Pointing VANADIS_CONFIG at the copy would work too and would put `demo/.run` in every
# path in the summary, which reads as scaffolding rather than as somebody's config.
Type "rm -rf demo/.run && mkdir -p demo/.run/.config" Enter
Type "cp -r demo/vanadis demo/.run/.config/vanadis" Enter
Type "cp -r demo/adopt/ghostty demo/.run/.config/ghostty" Enter
Type "export HOME=$PWD/demo/.run" Enter

# With HOME redirected, the default paths are the demo's, so neither variable is wanted.
Type "unset XDG_CONFIG_HOME VANADIS_CONFIG XDG_STATE_HOME" Enter
Type "clear" Enter
Sleep 1s

Show

Sleep 1s
Type "vanadis init ~/.config/ghostty/config" Sleep 500ms Enter
Sleep 2s

# Two themes sit in themes/, so there is nothing to offer. This is the only answer typed.
Type "papercolor-light" Sleep 300ms Enter
Sleep 1s

# The suggested name comes from the path, and for a file called `config` that is the
# directory holding it. It is already right, so this one is the return key too.
Enter
Sleep 1500ms

# Every one of the six is already in the palette, so every answer is the return key.
Enter
Sleep 900ms
Enter
Sleep 900ms
Enter
Sleep 900ms
Enter
Sleep 900ms
Enter
Sleep 900ms
Enter

Sleep 4s
