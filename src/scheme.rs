//! One scheme out of the tinted-theming collection.
//!
//! `docs/schemes.md` decides that the system comes from the directory the file sits in and
//! not from the file's own `system` field, that the identifier is the filename minus its
//! extension, and that tinted8 nests its header under `scheme` while base16 and base24 do
//! not.
//!
//! Every read of a scheme's YAML goes through the `yaml` submodule, both the header this
//! module reads and the palette the converter reads. One layer means one answer to what an
//! absent key is and what a key of the wrong type is.

use std::fmt;
use std::path::{Path, PathBuf};

use saphyr::Yaml;
use thiserror::Error;

use crate::theme::Variant;

pub mod cache;
mod convert;
mod yaml;

pub use convert::{ConvertError, Converted, Family, convert};

/// Which scheme system a file is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System {
    /// base16, sixteen slots.
    Base16,
    /// base24, base16 plus eight ANSI bright slots.
    Base24,
    /// tinted8, eight named colours plus a handful of greys.
    Tinted8,
}

impl System {
    /// Every system, in the order results are printed in.
    pub const ALL: [Self; 3] = [Self::Base16, Self::Base24, Self::Tinted8];

    /// Parses `text` as a system, returning `None` when it names none.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "base16" => Some(Self::Base16),
            "base24" => Some(Self::Base24),
            "tinted8" => Some(Self::Tinted8),
            _ => None,
        }
    }

    /// The system as the collection spells it, which is also its directory name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Base16 => "base16",
            Self::Base24 => "base24",
            Self::Tinted8 => "tinted8",
        }
    }
}

impl fmt::Display for System {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A cached scheme, as much of it as searching needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    system: System,
    id: String,
    name: String,
    author: String,
    variant: Variant,
}

/// A scheme file parses but does not say what it has to.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Problem {
    /// The file is not valid YAML.
    #[error("not valid YAML: {0}")]
    Syntax(String),
    /// The file holds no document.
    #[error("holds no document")]
    Empty,
    /// The file holds a document and it is not a mapping.
    #[error("is not a YAML mapping")]
    Document,
    /// A field the header needs is absent.
    #[error("no `{0}` field")]
    Missing(&'static str),
    /// A field is written and holds the wrong kind of value.
    ///
    /// Separate from [`Problem::Missing`] on purpose. A file writing a mapping under `name`
    /// has a `name` field; reporting it as absent would name the wrong defect.
    #[error("`{key}` is not {expected}")]
    Type {
        /// The field that holds it.
        key: String,
        /// What the field has to hold.
        expected: &'static str,
    },
    /// `variant` holds something that is neither `dark` nor `light`.
    #[error("`variant` is `{0}`, which is neither `dark` nor `light`")]
    Variant(String),
}

/// A scheme file could not be loaded.
#[derive(Debug, Error)]
pub enum SchemeError {
    /// The file could not be read.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The file the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// The filename is not one a scheme can have.
    #[error("{}: not a scheme filename", .path.display())]
    Name {
        /// The file the name came from.
        path: PathBuf,
    },
    /// The file was read but is not a scheme.
    #[error("{}: {source}", .path.display())]
    Invalid {
        /// The file the problem was found in.
        path: PathBuf,
        /// What the file does not say.
        source: Problem,
    },
}

impl Scheme {
    /// Reads the scheme `path` holds, taking its identifier from the filename.
    ///
    /// # Errors
    ///
    /// Returns [`SchemeError::Read`] when the file cannot be read, [`SchemeError::Name`]
    /// when the filename is not usable as an identifier, and [`SchemeError::Invalid`] when
    /// the file is not a scheme.
    pub fn load(system: System, path: &Path) -> Result<Self, SchemeError> {
        let id = path
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .ok_or_else(|| SchemeError::Name {
                path: path.to_owned(),
            })?;
        let source = std::fs::read_to_string(path).map_err(|source| SchemeError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(system, id, &source).map_err(|source| SchemeError::Invalid {
            path: path.to_owned(),
            source,
        })
    }

    /// Reads the header of one scheme file.
    ///
    /// `system` decides where the header sits: tinted8 nests everything but `variant` under
    /// `scheme`, and the other two put it at the top level.
    ///
    /// The document is read through the `yaml` submodule, the same layer the converter
    /// reads a palette through, so an absent field and a field of the wrong type are told
    /// apart here as well.
    ///
    /// Private: [`Scheme::load`] is the only way in from outside, the way `Theme::load` is.
    fn parse(system: System, id: &str, source: &str) -> Result<Self, Problem> {
        let root = yaml::document(source)?;

        let head = match system {
            System::Tinted8 => yaml::nested(&root, "scheme")?,
            System::Base16 | System::Base24 => &root,
        };

        let name = match system {
            System::Tinted8 => tinted8_name(head)?.ok_or(Problem::Missing("scheme.name"))?,
            System::Base16 | System::Base24 => yaml::optional(head, "name")?
                .ok_or(Problem::Missing("name"))?
                .to_owned(),
        };

        let author = yaml::optional(head, "author")?
            .ok_or(Problem::Missing(match system {
                System::Tinted8 => "scheme.author",
                System::Base16 | System::Base24 => "author",
            }))?
            .to_owned();

        let written = yaml::optional(&root, "variant")?.ok_or(Problem::Missing("variant"))?;
        let variant =
            Variant::parse(written).ok_or_else(|| Problem::Variant(written.to_owned()))?;

        Ok(Self {
            system,
            id: id.to_owned(),
            name,
            author,
            variant,
        })
    }

    /// The system the scheme is written for.
    #[must_use]
    pub fn system(&self) -> System {
        self.system
    }

    /// The identifier with its system, as in `base16/nord`.
    ///
    /// A bare identifier is not unique: `nord` names a base16 scheme and a tinted8 one.
    #[must_use]
    pub fn qualified(&self) -> String {
        format!("{}/{}", self.system, self.id)
    }

    /// The display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Who the collection credits the scheme to.
    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Which background the scheme is written for.
    #[must_use]
    pub fn variant(&self) -> Variant {
        self.variant
    }

    /// Whether `query` occurs in any cell a result line prints, compared without case.
    ///
    /// `docs/schemes.md` decides this rule: search matches what search prints, so a result
    /// that looks unrelated carries its own reason on the line.
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        [
            self.qualified(),
            self.variant.as_str().to_owned(),
            self.name.clone(),
            self.author.clone(),
        ]
        .iter()
        .any(|cell| cell.to_lowercase().contains(&query))
    }
}

/// The display name a tinted8 file writes under `scheme`, or `None` when it writes neither
/// spelling.
///
/// `docs/schemes.md` records both: three of the four files spell the name as `family` plus
/// `style` and `nord.yaml` spells it as `name`.
///
/// # Errors
///
/// Returns [`Problem::Type`] when one of the three fields does not hold a string.
fn tinted8_name(head: &Yaml<'_>) -> Result<Option<String>, Problem> {
    if let Some(name) = yaml::optional(head, "name")? {
        return Ok(Some(name.to_owned()));
    }
    let family = yaml::optional(head, "family")?;
    let style = yaml::optional(head, "style")?;
    match (family, style) {
        (Some(family), Some(style)) => Ok(Some(format!("{family} {style}"))),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE16: &str = r##"
system: "base16"
name: "Nord"
author: "arcticicestudio"
variant: "dark"
palette:
  base00: "#2E3440"
"##;

    const TINTED8_FAMILY: &str = r##"
scheme:
  system: "tinted8"
  author: "https://github.com/catppuccin/catppuccin"
  family: "Catppuccin"
  style: "Latte"
variant: "light"
palette:
  black: "#4c4f69"
"##;

    const TINTED8_NAME: &str = r##"
scheme:
  system: "tinted8"
  name: "Nord"
  author: "Tinted Theming (https://github.com/tinted-theming)"
variant: "dark"
palette:
  black: "#2e3440"
"##;

    const TINTED8_BOTH: &str = r##"
scheme:
  system: "tinted8"
  name: "Nord"
  author: "Tinted Theming"
  family: "Nordic"
  style: "Polar"
variant: "dark"
palette:
  black: "#2e3440"
"##;

    fn base16(id: &str) -> Scheme {
        Scheme::parse(System::Base16, id, BASE16).unwrap()
    }

    #[test]
    fn reads_the_name_of_a_base16_scheme() {
        assert_eq!(base16("nord").name(), "Nord");
    }

    #[test]
    fn reads_the_author_of_a_base16_scheme() {
        assert_eq!(base16("nord").author(), "arcticicestudio");
    }

    #[test]
    fn reads_the_variant_of_a_base16_scheme() {
        assert_eq!(base16("nord").variant(), Variant::Dark);
    }

    #[test]
    fn takes_the_system_from_the_caller_and_not_from_the_file() {
        assert_eq!(
            Scheme::parse(System::Base24, "nord", BASE16)
                .unwrap()
                .system(),
            System::Base24
        );
    }

    #[test]
    fn qualifies_the_identifier_with_the_system() {
        assert_eq!(base16("nord").qualified(), "base16/nord");
    }

    #[test]
    fn reads_a_tinted8_name_from_family_and_style() {
        let scheme = Scheme::parse(System::Tinted8, "catppuccin-latte", TINTED8_FAMILY).unwrap();
        assert_eq!(scheme.name(), "Catppuccin Latte");
    }

    #[test]
    fn reads_a_tinted8_author_from_under_scheme() {
        let scheme = Scheme::parse(System::Tinted8, "catppuccin-latte", TINTED8_FAMILY).unwrap();
        assert_eq!(scheme.author(), "https://github.com/catppuccin/catppuccin");
    }

    #[test]
    fn reads_a_tinted8_name_from_the_name_field() {
        let scheme = Scheme::parse(System::Tinted8, "nord", TINTED8_NAME).unwrap();
        assert_eq!(scheme.name(), "Nord");
    }

    #[test]
    fn prefers_a_tinted8_name_field_over_family_and_style() {
        let scheme = Scheme::parse(System::Tinted8, "nord", TINTED8_BOTH).unwrap();
        assert_eq!(scheme.name(), "Nord");
    }

    #[test]
    fn reads_a_variant_that_is_not_quoted() {
        let source = "name: \"Linux VT\"\nauthor: \"j-c-m\"\nvariant: dark\n";
        let scheme = Scheme::parse(System::Base16, "linux-vt", source).unwrap();
        assert_eq!(scheme.variant(), Variant::Dark);
    }

    #[test]
    fn reports_a_file_that_is_not_yaml() {
        let problem = Scheme::parse(System::Base16, "x", "name: \"a\"\n\tauthor: b\n").unwrap_err();
        assert!(matches!(problem, Problem::Syntax(_)), "{problem:?}");
    }

    #[test]
    fn reports_an_empty_file() {
        assert_eq!(
            Scheme::parse(System::Base16, "x", "").unwrap_err(),
            Problem::Empty
        );
    }

    #[test]
    fn reports_a_missing_name() {
        let source = "author: \"a\"\nvariant: \"dark\"\n";
        assert_eq!(
            Scheme::parse(System::Base16, "x", source).unwrap_err(),
            Problem::Missing("name")
        );
    }

    /// A file that writes a mapping under `name` has a `name` field. Reporting it as absent
    /// would name a defect the file does not have, which is what reading the header through
    /// `yaml` stops.
    #[test]
    fn separates_a_name_of_the_wrong_type_from_a_name_that_is_absent() {
        let source = "name:\n  first: \"Nord\"\nauthor: \"a\"\nvariant: \"dark\"\n";
        assert_eq!(
            Scheme::parse(System::Base16, "x", source).unwrap_err(),
            Problem::Type {
                key: "name".to_owned(),
                expected: "a string",
            }
        );
    }

    #[test]
    fn reports_a_file_whose_document_is_not_a_mapping() {
        assert_eq!(
            Scheme::parse(System::Base16, "x", "- nord\n").unwrap_err(),
            Problem::Document
        );
    }

    #[test]
    fn reports_a_missing_author() {
        let source = "name: \"A\"\nvariant: \"dark\"\n";
        assert_eq!(
            Scheme::parse(System::Base16, "x", source).unwrap_err(),
            Problem::Missing("author")
        );
    }

    #[test]
    fn reports_a_missing_variant() {
        let source = "name: \"A\"\nauthor: \"a\"\n";
        assert_eq!(
            Scheme::parse(System::Base16, "x", source).unwrap_err(),
            Problem::Missing("variant")
        );
    }

    #[test]
    fn reports_a_variant_that_is_neither_dark_nor_light() {
        let source = "name: \"A\"\nauthor: \"a\"\nvariant: \"dusk\"\n";
        assert_eq!(
            Scheme::parse(System::Base16, "x", source).unwrap_err(),
            Problem::Variant("dusk".to_owned())
        );
    }

    #[test]
    fn reports_a_tinted8_file_with_no_scheme_mapping() {
        assert_eq!(
            Scheme::parse(System::Tinted8, "x", BASE16).unwrap_err(),
            Problem::Missing("scheme")
        );
    }

    #[test]
    fn reports_a_tinted8_file_with_neither_a_name_nor_a_family() {
        let source = "scheme:\n  author: \"a\"\nvariant: \"dark\"\n";
        assert_eq!(
            Scheme::parse(System::Tinted8, "x", source).unwrap_err(),
            Problem::Missing("scheme.name")
        );
    }

    #[test]
    fn reports_a_tinted8_file_with_no_author() {
        let source = "scheme:\n  name: \"A\"\nvariant: \"dark\"\n";
        assert_eq!(
            Scheme::parse(System::Tinted8, "x", source).unwrap_err(),
            Problem::Missing("scheme.author")
        );
    }

    #[test]
    fn matches_on_the_qualified_identifier() {
        assert!(base16("nord").matches("base16/no"));
    }

    #[test]
    fn matches_on_the_name_without_case() {
        let scheme = Scheme::parse(System::Base16, "x", BASE16).unwrap();
        assert!(scheme.matches("NORD"));
    }

    #[test]
    fn matches_on_the_author() {
        assert!(base16("nord").matches("arctic"));
    }

    #[test]
    fn matches_on_the_variant() {
        assert!(base16("nord").matches("dark"));
    }

    #[test]
    fn does_not_match_a_query_no_cell_holds() {
        assert!(!base16("nord").matches("solarized"));
    }

    #[test]
    fn names_every_system() {
        let names: Vec<&str> = System::ALL.iter().map(|system| system.as_str()).collect();
        assert_eq!(names, ["base16", "base24", "tinted8"]);
    }

    #[test]
    fn parses_a_system_by_name() {
        assert_eq!(System::parse("tinted8"), Some(System::Tinted8));
    }

    #[test]
    fn parses_no_system_from_a_name_that_is_not_one() {
        assert_eq!(System::parse("base8"), None);
    }
}
