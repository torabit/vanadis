//! Finding colours in a config file somebody already wrote.
//!
//! `docs/init.md` decides that lowercase `#rrggbb` is the only notation substituted, and
//! decides which of the rest are reported rather than passed over in silence.

use std::fmt;
use std::ops::Range;

/// The colour words a quoted value may be made of, and the style words that may sit beside
/// them. `docs/init.md` decides why the list is this short.
const NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];
const STYLES: [&str; 10] = [
    "bold",
    "dim",
    "italic",
    "underline",
    "reverse",
    "blink",
    "bright",
    "light",
    "default",
    "none",
];

/// One place a colour appears in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    line: usize,
    start: usize,
    end: usize,
}

impl Occurrence {
    /// The 1-based line it is written on.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// The whole line it is written on, with surrounding whitespace trimmed.
    #[must_use]
    pub fn context<'a>(&self, source: &'a str) -> &'a str {
        let start = source[..self.start]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        let end = source[self.end..]
            .find('\n')
            .map_or(source.len(), |newline| self.end + newline);
        source[start..end].trim()
    }

    /// The bytes it covers.
    pub(crate) fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

/// One colour value, and every place the source writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colour {
    value: String,
    occurrences: Vec<Occurrence>,
}

impl Colour {
    /// The value, as a theme would store it: lowercase `#rrggbb`.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Every occurrence, in source order.
    #[must_use]
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }
}

/// A colour notation that cannot be substituted, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unhandled {
    line: usize,
    text: String,
    why: &'static str,
}

impl Unhandled {
    /// The 1-based line it is written on.
    #[must_use]
    pub fn line(&self) -> usize {
        self.line
    }

    /// The notation, as the source writes it.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// What kind of notation it is.
    #[must_use]
    pub fn why(&self) -> &'static str {
        self.why
    }
}

impl fmt::Display for Unhandled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {} ({})", self.line, self.text, self.why)
    }
}

/// What one pass over a file found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    colours: Vec<Colour>,
    unhandled: Vec<Unhandled>,
}

impl Scan {
    /// Every distinct value, in the order the source first writes each one.
    #[must_use]
    pub fn colours(&self) -> &[Colour] {
        &self.colours
    }

    /// Every notation that cannot be substituted, in source order.
    #[must_use]
    pub fn unhandled(&self) -> &[Unhandled] {
        &self.unhandled
    }
}

/// Reads `source` once, collecting the colours it holds.
///
/// Values are grouped, so a value written four times is one entry with four occurrences.
/// `docs/init.md` decides that the dialogue is per value for that reason.
#[must_use]
pub fn scan(source: &str) -> Scan {
    let mut scan = Scan::default();
    let mut line = 1;
    let mut at = 0;

    while at < source.len() {
        if source.as_bytes()[at] == b'\n' {
            line += 1;
            at += 1;
            continue;
        }
        match found(source, at, line) {
            Some(Found::Colour { value, occurrence }) => {
                at = occurrence.end;
                match scan.colours.iter_mut().find(|colour| colour.value == value) {
                    Some(colour) => colour.occurrences.push(occurrence),
                    None => scan.colours.push(Colour {
                        value,
                        occurrences: vec![occurrence],
                    }),
                }
            }
            Some(Found::Unhandled { unhandled, end }) => {
                at = end;
                scan.unhandled.push(unhandled);
            }
            None => {
                at += 1;
                while at < source.len() && !source.is_char_boundary(at) {
                    at += 1;
                }
            }
        }
    }
    scan
}

/// What a match at one position turned out to be.
enum Found {
    Colour {
        value: String,
        occurrence: Occurrence,
    },
    Unhandled {
        unhandled: Unhandled,
        end: usize,
    },
}

/// Whatever starts at `at`, or `None` when nothing does.
fn found(source: &str, at: usize, line: usize) -> Option<Found> {
    let rest = &source[at..];
    let unhandled = |end: usize, why| {
        Some(Found::Unhandled {
            unhandled: Unhandled {
                line,
                text: source[at..end].to_owned(),
                why,
            },
            end,
        })
    };

    match rest.as_bytes().first()? {
        b'#' => match hex(rest) {
            Some((6, end)) if rest[1..end].bytes().all(|byte| !byte.is_ascii_uppercase()) => {
                Some(Found::Colour {
                    value: rest[..end].to_owned(),
                    occurrence: Occurrence {
                        line,
                        start: at,
                        end: at + end,
                    },
                })
            }
            Some((6, end)) => unhandled(at + end, "uppercase hex"),
            Some((3, end)) => unhandled(at + end, "three-digit hex"),
            Some((8, end)) => unhandled(at + end, "eight-digit hex"),
            _ => None,
        },
        b'0' if opens(source, at) => {
            let literal = rest
                .strip_prefix("0x")
                .or_else(|| rest.strip_prefix("0X"))?;
            match hex(&rest[1..]) {
                Some((6 | 8, _)) => unhandled(at + 2 + digits(literal), "0x literal"),
                _ => None,
            }
        }
        b'r' | b'R' | b'h' | b'H' if opens(source, at) => {
            let open = ["rgb(", "rgba(", "hsl(", "hsla("]
                .iter()
                .find(|prefix| opening(rest, prefix))?;
            let inner = &rest[open.len()..];
            let close = inner
                .find(')')
                .filter(|end| !inner[..*end].contains('\n'))?;
            unhandled(at + open.len() + close + 1, "function notation")
        }
        b'c' | b'C' if opens(source, at) => {
            let word = ["colour", "color"]
                .iter()
                .find(|prefix| opening(rest, prefix))?;
            let end = word.len() + index(&rest[word.len()..])?;
            closes(source, at + end).then_some(())?;
            unhandled(at + end, "indexed colour")
        }
        b'\\' | b'\x1b' => {
            let end = at + sgr(rest)?;
            unhandled(end, "ANSI colour")
        }
        b'3' | b'4' if opens(source, at) => {
            let end = at + selector(rest)?;
            unhandled(end, "ANSI colour")
        }
        b'"' | b'\'' => {
            let end = at + 1 + quoted(&rest[1..], rest.as_bytes()[0])?;
            unhandled(end, "colour name")
        }
        _ => None,
    }
}

/// The `#` literal at the front of `rest`: how many hex digits it has, and where it ends.
///
/// A run followed by a letter or an underscore is part of a word rather than a literal, so
/// it is not one.
fn hex(rest: &str) -> Option<(usize, usize)> {
    let count = digits(&rest[1..]);
    closes(rest, 1 + count).then_some((count, 1 + count))
}

/// The leading run of hex digits in `rest`.
fn digits(rest: &str) -> usize {
    rest.bytes().take_while(u8::is_ascii_hexdigit).count()
}

/// The `123` of `color123`, when it is a slot number.
fn index(rest: &str) -> Option<usize> {
    let count = rest.bytes().take_while(u8::is_ascii_digit).count();
    (1..=3).contains(&count).then_some(())?;
    rest[..count]
        .parse::<u16>()
        .ok()
        .filter(|slot| *slot < 256)?;
    Some(count)
}

/// Whether `rest` starts with `prefix`, ignoring case.
///
/// Compared as bytes. `prefix` is ASCII and `rest` need not be, so slicing `rest` to the
/// prefix's length would land inside a character.
fn opening(rest: &str, prefix: &str) -> bool {
    rest.len() >= prefix.len()
        && rest.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// Whether the byte before `at` lets a notation start there.
fn opens(source: &str, at: usize) -> bool {
    at == 0 || !word(source.as_bytes()[at - 1])
}

/// Whether the byte at `at` lets a notation end there.
fn closes(source: &str, at: usize) -> bool {
    source.as_bytes().get(at).is_none_or(|byte| !word(*byte))
}

/// Whether `byte` is one a name can be spelled with.
fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// An escape sequence at the front of `rest`, when its parameters select a colour.
fn sgr(rest: &str) -> Option<usize> {
    let open = ["\\033[", "\\e[", "\\x1b[", "\\x1B[", "\u{1b}["]
        .iter()
        .find(|prefix| rest.starts_with(**prefix))?;
    let inner = &rest[open.len()..];
    let count = inner
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b';')
        .count();
    colouring(&inner[..count]).then_some(())?;
    let end = open.len() + count;
    Some(end + usize::from(inner[count..].starts_with('m')))
}

/// A bare `38;5;238` at the front of `rest`, which is what a colour selector looks like
/// outside an escape sequence.
fn selector(rest: &str) -> Option<usize> {
    ["38;5;", "48;5;", "38;2;", "48;2;"]
        .iter()
        .find(|prefix| rest.starts_with(**prefix))?;
    let count = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b';')
        .count();
    Some(count)
}

/// Whether SGR parameters set a colour, rather than resetting or setting an attribute.
fn colouring(parameters: &str) -> bool {
    let mut fields = parameters.split(';');
    while let Some(field) = fields.next() {
        let Ok(parameter) = field.parse::<u16>() else {
            continue;
        };
        match parameter {
            30..=37 | 39..=47 | 49 | 90..=97 | 100..=107 => return true,
            38 | 48 => {
                if matches!(fields.next(), Some("5" | "2")) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// A quoted string at the front of `rest`, when it names a colour and nothing else.
///
/// `quote` closes it. A string that runs to the end of the line is not one.
fn quoted(rest: &str, quote: u8) -> Option<usize> {
    let end = rest
        .bytes()
        .position(|byte| byte == quote)
        .filter(|end| !rest[..*end].contains('\n'))?;
    let mut named = false;
    for word in rest[..end]
        .split([' ', '\t', '-'])
        .filter(|word| !word.is_empty())
    {
        let lower = word.to_ascii_lowercase();
        if NAMES.contains(&lower.as_str()) {
            named = true;
        } else if !STYLES.contains(&lower.as_str()) {
            return None;
        }
    }
    named.then_some(end + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(source: &str) -> Vec<String> {
        scan(source)
            .colours
            .into_iter()
            .map(|colour| colour.value)
            .collect()
    }

    fn counts(source: &str) -> Vec<(String, usize)> {
        scan(source)
            .colours
            .into_iter()
            .map(|colour| (colour.value, colour.occurrences.len()))
            .collect()
    }

    fn reported(source: &str) -> Vec<String> {
        scan(source)
            .unhandled
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn why(source: &str) -> Vec<&'static str> {
        scan(source).unhandled.iter().map(Unhandled::why).collect()
    }

    #[test]
    fn finds_a_lowercase_six_digit_literal() {
        assert_eq!(values("bg = \"#eeeeee\""), vec!["#eeeeee"]);
    }

    #[test]
    fn groups_a_value_written_more_than_once() {
        assert_eq!(
            counts("a = \"#eeeeee\"\nb = \"#444444\"\nc = \"#eeeeee\""),
            vec![("#eeeeee".to_owned(), 2), ("#444444".to_owned(), 1)]
        );
    }

    #[test]
    fn keeps_values_in_the_order_they_first_appear() {
        assert_eq!(
            values("#444444 #eeeeee #444444"),
            vec!["#444444", "#eeeeee"]
        );
    }

    #[test]
    fn records_the_line_each_occurrence_is_on() {
        let scan = scan("a\n\nbg = \"#eeeeee\"");
        assert_eq!(scan.colours[0].occurrences[0].line(), 3);
    }

    #[test]
    fn records_the_line_after_a_multibyte_line() {
        let scan = scan("# 配色\nbg = \"#eeeeee\"");
        assert_eq!(scan.colours[0].occurrences[0].line(), 2);
    }

    #[test]
    fn finds_a_literal_after_multibyte_text_on_the_same_line() {
        assert_eq!(values("端末の色 (#444444) で暗く出る"), vec!["#444444"]);
    }

    #[test]
    fn shows_the_line_an_occurrence_sits_on() {
        let source = "x\nmuted = \"#878787\"  # comment\ny";
        let scan = scan(source);
        assert_eq!(
            scan.colours[0].occurrences[0].context(source),
            "muted = \"#878787\"  # comment"
        );
    }

    #[test]
    fn covers_exactly_the_literal() {
        let source = "bg = \"#eeeeee\"";
        let scan = scan(source);
        let range = scan.colours[0].occurrences[0].range();
        assert_eq!(&source[range], "#eeeeee");
    }

    #[test]
    fn reports_an_uppercase_literal_rather_than_substituting_it() {
        assert_eq!(values("#EEEEEE"), Vec::<String>::new());
        assert_eq!(why("#EEEEEE"), vec!["uppercase hex"]);
    }

    #[test]
    fn reports_a_literal_with_one_uppercase_digit() {
        assert_eq!(why("#eeeeeE"), vec!["uppercase hex"]);
    }

    #[test]
    fn reports_three_digit_shorthand() {
        assert_eq!(why("#eee"), vec!["three-digit hex"]);
    }

    #[test]
    fn reports_eight_digit_hex() {
        assert_eq!(why("#9d550fb0"), vec!["eight-digit hex"]);
    }

    #[test]
    fn reports_an_0x_literal() {
        assert_eq!(why("bg = 0xeeeeee"), vec!["0x literal"]);
    }

    #[test]
    fn reports_an_rgb_call() {
        assert_eq!(
            reported("a = rgb(238, 238, 238)"),
            vec!["line 1: rgb(238, 238, 238) (function notation)"]
        );
    }

    #[test]
    fn reports_rgba_hsl_and_hsla() {
        assert_eq!(why("rgba(1,2,3,4) hsl(1,2,3) hsla(1,2,3,4)").len(), 3);
    }

    #[test]
    fn reports_an_escape_sequence_that_selects_a_colour() {
        assert_eq!(
            reported("\\033[38;5;238m"),
            vec!["line 1: \\033[38;5;238m (ANSI colour)"]
        );
    }

    #[test]
    fn reports_the_short_escape_spelling() {
        assert_eq!(why("\\e[31m"), vec!["ANSI colour"]);
    }

    #[test]
    fn reports_a_literal_escape_byte() {
        assert_eq!(why("\u{1b}[38;2;1;2;3m"), vec!["ANSI colour"]);
    }

    #[test]
    fn passes_over_an_escape_sequence_that_sets_no_colour() {
        assert!(why("\\033[0m\\033[1m").is_empty());
    }

    #[test]
    fn reports_a_bare_colour_selector() {
        assert_eq!(why("di=38;5;33:"), vec!["ANSI colour"]);
    }

    #[test]
    fn reports_an_indexed_colour() {
        assert_eq!(why("fg=colour238"), vec!["indexed colour"]);
    }

    #[test]
    fn reports_the_american_spelling_of_an_indexed_colour() {
        assert_eq!(why("color0"), vec!["indexed colour"]);
    }

    #[test]
    fn passes_over_a_slot_number_above_the_palette() {
        assert!(why("color256").is_empty());
    }

    #[test]
    fn passes_over_the_word_colors() {
        assert!(why("colors.paper").is_empty());
    }

    #[test]
    fn reports_a_quoted_colour_name() {
        assert_eq!(
            reported("style = \"bold red\""),
            vec!["line 1: \"bold red\" (colour name)"]
        );
    }

    #[test]
    fn reports_a_hyphenated_colour_name() {
        assert_eq!(why("\"bright-blue\""), vec!["colour name"]);
    }

    #[test]
    fn passes_over_a_quoted_style_that_names_no_colour() {
        assert!(why("- \"reverse\"").is_empty());
    }

    #[test]
    fn passes_over_a_quoted_name_the_target_defined_itself() {
        assert!(why("style = \"visual\"").is_empty());
    }

    #[test]
    fn passes_over_a_colour_name_used_as_a_key() {
        assert!(why("black = \"#eeeeee\"").is_empty());
    }

    #[test]
    fn passes_over_a_colour_name_in_a_comment() {
        assert!(why("removedSignColor = \"#af0000\"  # red").is_empty());
    }

    #[test]
    fn passes_over_a_shebang() {
        assert!(why("#!/usr/bin/env python3").is_empty());
    }

    #[test]
    fn passes_over_a_word_that_starts_with_hex_digits() {
        assert!(why("#deadbeefery").is_empty());
    }

    #[test]
    fn finds_nothing_in_a_file_with_no_colour() {
        assert_eq!(scan("name = \"herdr\"\n"), Scan::default());
    }
}
