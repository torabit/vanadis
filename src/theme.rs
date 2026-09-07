//! Theme files: parsing, validation and reference resolution.

use std::fmt;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::Document;

use crate::token::{Tokens, is_segment};

mod problem;
mod resolve;
mod walk;

pub use problem::Problem;
use problem::listed;
use walk::{Definition, RawValue, meta_path, meta_problems};

/// A theme's identifier: its filename minus `.toml`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeId(String);

impl ThemeId {
    /// Parses `text` as an identifier, returning `None` when it is not a single segment.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        is_segment(text).then(|| Self(text.to_owned()))
    }

    /// The identifier as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ThemeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which background a theme is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Variant {
    /// Written for a dark background.
    Dark,
    /// Written for a light background.
    Light,
}

impl Variant {
    /// Parses `text` as a variant, returning `None` when it is neither `dark` nor `light`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }

    /// The variant as a theme file spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A theme, loaded and fully resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    id: ThemeId,
    name: String,
    variant: Variant,
    tokens: Tokens,
}

/// Loading a theme failed.
#[derive(Debug, Error)]
pub enum ThemeError {
    /// The file is not valid TOML.
    #[error("{}: {source}", .path.display())]
    Syntax {
        /// The file the parse failed in.
        path: PathBuf,
        /// The parse error.
        source: toml_edit::TomlError,
    },
    /// The filename is not a valid identifier.
    #[error("{}: `{id}` is not a theme identifier: lowercase, digits, internal hyphens", .path.display())]
    Id {
        /// The file the identifier came from.
        path: PathBuf,
        /// The filename, minus `.toml`.
        id: String,
    },
    /// The file could not be read.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The file the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// The file parses as TOML but is not a valid theme.
    #[error("{}: invalid theme\n{}", .path.display(), listed(.problems))]
    Invalid {
        /// The file the problems were found in.
        path: PathBuf,
        /// Every problem, in source order.
        problems: Vec<Problem>,
    },
}

impl Theme {
    /// Reads the theme stored at `path`.
    ///
    /// # Errors
    ///
    /// Returns the read error, or every problem the file has, in one pass.
    pub fn load(path: &Path) -> Result<Self, ThemeError> {
        let source = std::fs::read_to_string(path).map_err(|source| ThemeError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(path, &source)
    }

    /// Parses `source` as the theme stored at `path`.
    ///
    /// # Errors
    ///
    /// Returns every problem found, in one pass.
    pub fn parse(path: &Path, source: &str) -> Result<Self, ThemeError> {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let Some(id) = ThemeId::parse(&stem) else {
            return Err(ThemeError::Id {
                path: path.to_owned(),
                id: stem.into_owned(),
            });
        };
        let document = Document::parse(source).map_err(|source| ThemeError::Syntax {
            path: path.to_owned(),
            source,
        })?;

        let walked = walk::walk(source, document.as_table());
        let mut definitions = walked.definitions;
        let mut problems = walked.problems;
        problems.extend(meta_problems(&definitions, &walked.namespaces));

        // `meta.id` is the filename, so it is readable as a token but never written in the
        // file. It joins the definitions after `[meta]` is checked, so writing it is the
        // unknown-key error it should be.
        definitions.insert(
            meta_path("id"),
            Definition {
                value: RawValue::Literal(id.0.clone()),
                line: 0,
            },
        );

        problems.extend(resolve::problems(&definitions, &walked.namespaces));
        problems.sort_by_key(Problem::line);

        if !problems.is_empty() {
            return Err(ThemeError::Invalid {
                path: path.to_owned(),
                problems,
            });
        }

        let tokens = resolve::resolve(&definitions);
        let line = definitions
            .get(&meta_path("variant"))
            .map_or(0, |definition| definition.line);
        match metadata(&tokens, line) {
            Ok((name, variant)) => Ok(Self {
                id,
                name,
                variant,
                tokens,
            }),
            Err(problem) => Err(ThemeError::Invalid {
                path: path.to_owned(),
                problems: vec![problem],
            }),
        }
    }

    /// The theme's identifier.
    #[must_use]
    pub fn id(&self) -> &ThemeId {
        &self.id
    }

    /// The theme's display name, `meta.name`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The background the theme is written for, `meta.variant`.
    #[must_use]
    pub fn variant(&self) -> Variant {
        self.variant
    }

    /// Every token the theme defines, resolved.
    #[must_use]
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }
}

/// `meta.name` and `meta.variant`, read back out of the resolved tokens.
///
/// `walk::meta_problems` has already established that both keys are written and that
/// `variant` reads `dark` or `light`, so nothing here is reachable from a theme file that
/// got this far. It is reported rather than panicked on. `line` is the line `variant` is
/// written on.
fn metadata(tokens: &Tokens, line: usize) -> Result<(String, Variant), Problem> {
    match (
        tokens.get(&meta_path("name")),
        tokens.get(&meta_path("variant")),
    ) {
        (Some(name), Some(value)) => Variant::parse(value)
            .map(|variant| (name.to_owned(), variant))
            .ok_or_else(|| Problem::MetaVariant {
                line,
                value: value.to_owned(),
            }),
        (None, _) => Err(Problem::MetaMissing {
            key: "name".to_owned(),
        }),
        (_, None) => Err(Problem::MetaMissing {
            key: "variant".to_owned(),
        }),
    }
}

/// `value` as red, green and blue, or `None` when it is not a hex literal.
///
/// `docs/theme-format.md` decides that a hex literal is `#` and six hex digits. Three-digit
/// shorthand and eight-digit `#rrggbbaa` are rejected rather than expanded or truncated,
/// neither of which the file that holds them asked for. Either case is read: a theme stores
/// lowercase, and an upstream scheme is under no such rule.
pub(crate) fn rgb(value: &str) -> Option<[u8; 3]> {
    let digits = value.strip_prefix('#')?.as_bytes();
    let [r0, r1, g0, g1, b0, b1] = <[u8; 6]>::try_from(digits).ok()?;
    let channel = |high: u8, low: u8| -> Option<u8> { Some(nibble(high)? * 16 + nibble(low)?) };
    Some([channel(r0, r1)?, channel(g0, g1)?, channel(b0, b1)?])
}

/// What one hex digit is worth, in either case.
///
/// Decoded here rather than through `u8::from_str_radix`, which also accepts a leading `+`
/// and would let `#+f+f+f` through as a colour.
fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;
    use crate::token::TokenPath;

    const META: &str = "[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"light\"\n";

    fn parse(source: &str) -> Result<Theme, ThemeError> {
        Theme::parse(Path::new("themes/papercolor-light.toml"), source)
    }

    fn path(text: &str) -> TokenPath {
        TokenPath::parse(text).unwrap()
    }

    fn problems(source: &str) -> Vec<Problem> {
        match parse(source) {
            Err(ThemeError::Invalid { problems, .. }) => problems,
            other => panic!("expected problems, got {other:?}"),
        }
    }

    fn value(source: &str, path: &str) -> String {
        let theme = parse(source).unwrap();
        theme
            .tokens()
            .get(&TokenPath::parse(path).unwrap())
            .unwrap_or_else(|| panic!("{path} is not defined"))
            .to_owned()
    }

    #[test]
    fn resolves_a_literal() {
        let source = format!("{META}[colors]\npaper = \"#eeeeee\"\n");
        assert_eq!(value(&source, "colors.paper"), "#eeeeee");
    }

    #[test]
    fn resolves_a_reference() {
        let source =
            format!("{META}[colors]\npaper = \"#eeeeee\"\n[role]\nbg = \"{{{{colors.paper}}}}\"\n");
        assert_eq!(value(&source, "role.bg"), "#eeeeee");
    }

    #[test]
    fn resolves_a_chain() {
        let source = format!(
            "{META}[colors]\ngray = \"#878787\"\n[role]\ncomment = \"{{{{colors.gray}}}}\"\ninactive = \"{{{{role.comment}}}}\"\n"
        );
        assert_eq!(value(&source, "role.inactive"), "#878787");
    }

    #[test]
    fn stores_hex_in_lowercase() {
        let source = format!("{META}[colors]\npaper = \"#EEEEEE\"\n");
        assert_eq!(value(&source, "colors.paper"), "#eeeeee");
    }

    #[test]
    fn takes_the_id_from_the_filename() {
        let source = META.to_owned();
        assert_eq!(parse(&source).unwrap().id().as_str(), "papercolor-light");
    }

    #[test]
    fn exposes_the_id_as_a_token() {
        let source = META.to_owned();
        assert_eq!(value(&source, "meta.id"), "papercolor-light");
    }

    #[test]
    fn exposes_metadata_as_tokens() {
        let source = META.to_owned();
        assert_eq!(value(&source, "meta.name"), "Paper");
    }

    #[test]
    fn exposes_the_format_as_a_token() {
        let source = META.to_owned();
        assert_eq!(value(&source, "meta.format"), "1");
    }

    #[test]
    fn keeps_text_values_opaque() {
        let source = format!("{META}[text]\ndelta-syntax-theme = \"gruvbox-light\"\n");
        assert_eq!(value(&source, "text.delta-syntax-theme"), "gruvbox-light");
    }

    #[test]
    fn resolves_a_nested_namespace() {
        let source = format!("{META}[role.git]\nadded = \"#008700\"\n");
        assert_eq!(value(&source, "role.git.added"), "#008700");
    }

    #[test]
    fn reports_an_undefined_reference() {
        let source = format!("{META}[role]\nbg = \"{{{{colors.papper}}}}\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Undefined {
                path: path("role.bg"),
                line: 6,
                target: path("colors.papper"),
            }]
        );
    }

    #[test]
    fn reports_every_undefined_reference_in_one_pass() {
        let source =
            format!("{META}[role]\nbg = \"{{{{colors.papper}}}}\"\nfg = \"{{{{colors.inc}}}}\"\n");
        assert_eq!(problems(&source).len(), 2);
    }

    #[test]
    fn reports_a_reference_to_a_namespace() {
        let source =
            format!("{META}[colors]\npaper = \"#eeeeee\"\n[role]\nbg = \"{{{{colors}}}}\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Namespace {
                path: path("role.bg"),
                line: 8,
                target: path("colors"),
            }]
        );
    }

    #[test]
    fn reports_a_direct_cycle() {
        let source = format!("{META}[role]\nbg = \"{{{{role.bg}}}}\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Cycle {
                members: vec![path("role.bg")],
                line: 6,
            }]
        );
    }

    #[test]
    fn reports_an_indirect_cycle() {
        let source = format!("{META}[role]\nbg = \"{{{{role.fg}}}}\"\nfg = \"{{{{role.bg}}}}\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Cycle {
                members: vec![path("role.bg"), path("role.fg")],
                line: 6,
            }]
        );
    }

    #[test]
    fn reports_a_cycle_once_rather_than_once_per_member() {
        let source = format!(
            "{META}[role]\na = \"{{{{role.b}}}}\"\nb = \"{{{{role.c}}}}\"\nc = \"{{{{role.a}}}}\"\n"
        );
        assert_eq!(problems(&source).len(), 1);
    }

    #[test]
    fn reports_both_of_two_separate_cycles() {
        let source = format!(
            "{META}[role]\na = \"{{{{role.b}}}}\"\nb = \"{{{{role.a}}}}\"\nc = \"{{{{role.d}}}}\"\nd = \"{{{{role.c}}}}\"\n"
        );
        assert_eq!(problems(&source).len(), 2);
    }

    #[test]
    fn names_the_cycle_in_the_message() {
        let source = format!("{META}[role]\nbg = \"{{{{role.fg}}}}\"\nfg = \"{{{{role.bg}}}}\"\n");
        assert!(
            problems(&source)[0]
                .to_string()
                .contains("role.bg -> role.fg -> role.bg"),
            "{}",
            problems(&source)[0]
        );
    }

    #[test]
    fn rejects_a_key_that_is_not_a_segment() {
        let source = format!("{META}[role]\nadded_bg = \"#d7ffd7\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Key {
                key: "added_bg".to_owned(),
                line: 6,
            }]
        );
    }

    #[test]
    fn rejects_a_value_that_is_not_a_string() {
        let source = format!("{META}[colors]\npaper = 0xeeeeee\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Type {
                path: path("colors.paper"),
                line: 6,
                found: "integer",
            }]
        );
    }

    #[test]
    fn rejects_malformed_hex() {
        let source = format!("{META}[colors]\npaper = \"#eee\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Hex {
                path: path("colors.paper"),
                line: 6,
                value: "#eee".to_owned(),
            }]
        );
    }

    #[test]
    fn rejects_a_reference_that_is_not_the_whole_value() {
        let source = format!("{META}[role]\nbg = \"rgb({{{{colors.ink}}}})\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Reference {
                path: path("role.bg"),
                line: 6,
                value: "rgb({{colors.ink}})".to_owned(),
            }]
        );
    }

    #[test]
    fn rejects_an_ansi_key_outside_the_sixteen_slots() {
        let source = format!("{META}[ansi]\n16 = \"#eeeeee\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::AnsiSlot {
                key: "16".to_owned(),
                line: 6,
            }]
        );
    }

    #[test]
    fn rejects_an_ansi_key_with_a_leading_zero() {
        let source = format!("{META}[ansi]\n01 = \"#eeeeee\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::AnsiSlot {
                key: "01".to_owned(),
                line: 6,
            }]
        );
    }

    #[test]
    fn rejects_a_sub_table_under_ansi() {
        let source = format!("{META}[ansi]\n1.0 = \"#eeeeee\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::AnsiSlot {
                key: "1".to_owned(),
                line: 6,
            }]
        );
    }

    #[test]
    fn accepts_the_sixteen_ansi_slots() {
        let slots = (0..16).fold(String::new(), |mut slots, slot| {
            let _ = writeln!(slots, "{slot} = \"#eeeeee\"");
            slots
        });
        let source = format!("{META}[ansi]\n{slots}");
        assert_eq!(value(&source, "ansi.15"), "#eeeeee");
    }

    #[test]
    fn reports_a_missing_name() {
        let source = "[meta]\nformat = 1\nvariant = \"light\"\n";
        assert_eq!(
            problems(source),
            vec![Problem::MetaMissing {
                key: "name".to_owned(),
            }]
        );
    }

    #[test]
    fn reports_every_missing_metadata_key() {
        assert_eq!(problems("[colors]\npaper = \"#eeeeee\"\n").len(), 3);
    }

    #[test]
    fn rejects_an_unknown_metadata_key() {
        let source = format!("{META}varient = \"dark\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::MetaUnknown {
                key: "varient".to_owned(),
                line: 5,
            }]
        );
    }

    #[test]
    fn rejects_a_variant_that_is_neither_dark_nor_light() {
        let source = "[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"pale\"\n";
        assert_eq!(
            problems(source),
            vec![Problem::MetaVariant {
                line: 4,
                value: "pale".to_owned(),
            }]
        );
    }

    #[test]
    fn rejects_a_format_version_it_does_not_know() {
        let source = "[meta]\nformat = 2\nname = \"Paper\"\nvariant = \"light\"\n";
        assert_eq!(
            problems(source),
            vec![Problem::MetaFormat {
                line: 2,
                found: "2".to_owned(),
            }]
        );
    }

    #[test]
    fn rejects_a_format_that_is_not_a_number() {
        let source = "[meta]\nformat = \"1\"\nname = \"Paper\"\nvariant = \"light\"\n";
        assert_eq!(
            problems(source),
            vec![Problem::MetaFormat {
                line: 2,
                found: "string".to_owned(),
            }]
        );
    }

    #[test]
    fn accepts_an_optional_author() {
        let source = format!("{META}author = \"torabit\"\n");
        assert_eq!(value(&source, "meta.author"), "torabit");
    }

    #[test]
    fn reports_every_problem_in_one_pass() {
        let source = format!(
            "{META}[colors]\npaper = \"#eee\"\n[role]\nadded_bg = \"#d7ffd7\"\nbg = \"{{{{colors.papper}}}}\"\n"
        );
        assert_eq!(problems(&source).len(), 3);
    }

    #[test]
    fn reports_problems_in_source_order() {
        let source = format!("{META}[colors]\npaper = \"#eee\"\nink = \"#44\"\n");
        let lines: Vec<usize> = problems(&source).iter().map(Problem::line).collect();
        assert_eq!(lines, vec![6, 7]);
    }

    #[test]
    fn rejects_a_filename_that_is_not_a_segment() {
        let error = Theme::parse(Path::new("PaperColor Light.toml"), META).unwrap_err();
        assert!(
            matches!(error, ThemeError::Id { ref id, .. } if id == "PaperColor Light"),
            "{error:?}"
        );
    }

    #[test]
    fn reports_a_cycle_at_the_line_it_starts_on() {
        let source =
            format!("{META}[role]\nfg = \"{{{{role.comment}}}}\"\ncomment = \"{{{{role.fg}}}}\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Cycle {
                members: vec![path("role.fg"), path("role.comment")],
                line: 6,
            }]
        );
    }

    #[test]
    fn reads_the_variant() {
        assert_eq!(parse(META).unwrap().variant(), Variant::Light);
    }

    #[test]
    fn reads_the_display_name() {
        assert_eq!(parse(META).unwrap().name(), "Paper");
    }

    #[test]
    fn writes_a_variant_the_way_a_theme_file_spells_it() {
        assert_eq!(Variant::Dark.to_string(), "dark");
    }

    #[test]
    fn reads_the_three_channels_of_a_hex_literal() {
        assert_eq!(rgb("#1d2021"), Some([0x1d, 0x20, 0x21]));
    }

    #[test]
    fn reads_a_hex_literal_written_in_either_case() {
        assert_eq!(rgb("#EEEEEE"), rgb("#eeeeee"));
    }

    #[test]
    fn rejects_a_literal_without_a_hash() {
        assert_eq!(rgb("1d2021"), None);
    }

    #[test]
    fn rejects_three_digit_shorthand() {
        assert_eq!(rgb("#eee"), None);
    }

    #[test]
    fn rejects_eight_digits() {
        assert_eq!(rgb("#eeeeeeff"), None);
    }

    #[test]
    fn rejects_a_signed_digit_that_from_str_radix_would_take() {
        assert_eq!(rgb("#+f+f+f"), None);
    }
}
