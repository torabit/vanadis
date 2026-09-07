//! Colour written into the output that is already there.
//!
//! A swatch is one escape sequence and there is no state to keep, so the only decision is
//! whether to write it. That decision is made once, in `main`, and carried as a [`Paint`]:
//! a command that was not handed one does not paint, which is what keeps every command
//! answering the question the same way.
//!
//! `get` is handed none on purpose. Its stdout is the answer, so `$(vanadis get role.bg)`
//! is a colour and nothing else.

use crate::theme::rgb;

/// Whether escape sequences are written.
#[derive(Debug, Clone, Copy)]
pub struct Paint {
    on: bool,
}

impl Paint {
    /// Decides whether a swatch would be seen.
    ///
    /// Painting needs all four to agree: a terminal on the other end, `NO_COLOR` unset or
    /// empty, a `TERM` that is not `dumb`, and a `COLORTERM` saying twenty-four bit colour.
    ///
    /// The last is the one that is not a general convention. A palette is `#rrggbb`, and a
    /// 256-colour approximation would show a colour the theme does not hold, which for a
    /// command whose whole job is deciding what a colour means is worse than showing none.
    /// So the swatch appears where it is exact and the hex stands alone everywhere else.
    #[must_use]
    pub fn wanted(
        tty: bool,
        no_color: Option<&str>,
        term: Option<&str>,
        colorterm: Option<&str>,
    ) -> Self {
        Self {
            on: tty
                && no_color.is_none_or(str::is_empty)
                && term != Some("dumb")
                && matches!(colorterm, Some("truecolor" | "24bit")),
        }
    }

    /// A block of `value` and one space after it.
    ///
    /// Empty when painting is off, and empty when `value` is not a hex literal, so a caller
    /// writes it in front of what it describes and needs no branch of its own.
    #[must_use]
    pub fn swatch(&self, value: &str) -> String {
        match self.block(value) {
            Some(block) => format!("{block} "),
            None => String::new(),
        }
    }

    /// One block per entry, run together, and two spaces after them.
    ///
    /// An entry that is absent or is not a hex literal takes the width of a block in blanks,
    /// so a row of an incomplete theme stays aligned with the rows around it.
    #[must_use]
    pub fn strip(&self, values: &[Option<&str>]) -> String {
        if !self.on {
            return String::new();
        }
        let mut strip = String::new();
        for value in values {
            match value.and_then(|value| self.block(value)) {
                Some(block) => strip.push_str(&block),
                None => strip.push_str(BLANK),
            }
        }
        strip.push_str("  ");
        strip
    }

    /// `value` as a block of background colour, or `None` when nothing is written.
    fn block(self, value: &str) -> Option<String> {
        let [red, green, blue] = self.on.then(|| rgb(value)).flatten()?;
        Some(format!("\x1b[48;2;{red};{green};{blue}m{BLANK}\x1b[0m"))
    }
}

/// A block, unpainted. Two columns wide, which is what every block is.
const BLANK: &str = "  ";

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Paint {
        Paint::wanted(true, None, Some("xterm-256color"), Some("truecolor"))
    }

    fn off() -> Paint {
        Paint::wanted(false, None, Some("xterm-256color"), Some("truecolor"))
    }

    #[test]
    fn paints_on_a_terminal_that_says_truecolor() {
        assert!(on().on);
    }

    #[test]
    fn paints_on_a_terminal_that_says_24bit() {
        assert!(Paint::wanted(true, None, Some("xterm-256color"), Some("24bit")).on);
    }

    #[test]
    fn does_not_paint_off_a_terminal() {
        assert!(!off().on);
    }

    #[test]
    fn does_not_paint_when_no_color_is_set() {
        assert!(!Paint::wanted(true, Some("1"), Some("xterm-256color"), Some("truecolor")).on);
    }

    #[test]
    fn paints_when_no_color_is_set_to_nothing() {
        assert!(Paint::wanted(true, Some(""), Some("xterm-256color"), Some("truecolor")).on);
    }

    #[test]
    fn does_not_paint_on_a_dumb_terminal() {
        assert!(!Paint::wanted(true, None, Some("dumb"), Some("truecolor")).on);
    }

    #[test]
    fn does_not_paint_when_the_terminal_does_not_say_truecolor() {
        assert!(!Paint::wanted(true, None, Some("xterm-256color"), None).on);
        assert!(!Paint::wanted(true, None, Some("xterm-256color"), Some("8bit")).on);
    }

    #[test]
    fn writes_the_three_channels_of_the_value() {
        assert_eq!(on().swatch("#1d2021"), "\x1b[48;2;29;32;33m  \x1b[0m ");
    }

    #[test]
    fn writes_nothing_for_a_value_that_is_not_a_hex_literal() {
        assert_eq!(on().swatch("bold red"), "");
    }

    #[test]
    fn writes_nothing_at_all_when_painting_is_off() {
        assert_eq!(off().swatch("#1d2021"), "");
        assert_eq!(off().strip(&[Some("#1d2021")]), "");
    }

    #[test]
    fn runs_the_blocks_of_a_strip_together() {
        assert_eq!(
            on().strip(&[Some("#000000"), Some("#ffffff")]),
            "\x1b[48;2;0;0;0m  \x1b[0m\x1b[48;2;255;255;255m  \x1b[0m  "
        );
    }

    #[test]
    fn pads_a_slot_the_theme_does_not_define() {
        assert_eq!(
            on().strip(&[None, Some("#ffffff")]),
            "  \x1b[48;2;255;255;255m  \x1b[0m  "
        );
    }

    #[test]
    fn pads_a_slot_whose_value_is_not_a_hex_literal() {
        assert_eq!(on().strip(&[Some("bold red")]), "    ");
    }
}
