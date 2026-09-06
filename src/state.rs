//! The state file: which theme was applied last.
//!
//! It lives outside the config directory, under `$XDG_STATE_HOME`, because it records what
//! this machine is showing rather than what the user configured. See `docs/config.md`.

use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::{Document, Item};

use crate::theme::ThemeId;

/// What vanadis last applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    theme: ThemeId,
}

/// Reading or writing the state file failed.
#[derive(Debug, Error)]
pub enum StateError {
    /// The file could not be read.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The file the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// The file could not be written.
    #[error("{}: {source}", .path.display())]
    Write {
        /// The file the write failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// The file is not valid TOML.
    #[error("{}: {source}", .path.display())]
    Syntax {
        /// The file the parse failed in.
        path: PathBuf,
        /// The parse error.
        source: toml_edit::TomlError,
    },
    /// The file carries no `theme`.
    #[error("{}: no `theme` key", .path.display())]
    Missing {
        /// The file the key is missing from.
        path: PathBuf,
    },
    /// `theme` is not a theme identifier.
    #[error("{}: `{theme}` is not a theme identifier", .path.display())]
    Id {
        /// The file the identifier came from.
        path: PathBuf,
        /// The value `theme` holds.
        theme: String,
    },
}

impl State {
    /// The state that records `theme` as applied.
    #[must_use]
    pub fn new(theme: ThemeId) -> Self {
        Self { theme }
    }

    /// The theme that was applied.
    #[must_use]
    pub fn theme(&self) -> &ThemeId {
        &self.theme
    }

    /// Reads the state stored at `path`, or `None` when nothing has been applied yet.
    ///
    /// # Errors
    ///
    /// Returns the read error, or what is wrong with the file's contents.
    pub fn load(path: &Path) -> Result<Option<Self>, StateError> {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(StateError::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        Self::parse(path, &source).map(Some)
    }

    /// Writes the state to `path`, creating the directories leading to it.
    ///
    /// The file is written beside its destination and renamed over it, so a state file that
    /// exists is a state file that was written whole.
    ///
    /// # Errors
    ///
    /// Returns the write error, naming the file it happened on.
    pub fn store(&self, path: &Path) -> Result<(), StateError> {
        let write = |path: &Path, source: std::io::Error| StateError::Write {
            path: path.to_owned(),
            source,
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| write(parent, source))?;
        }

        let staged = path.with_extension("toml.new");
        std::fs::write(&staged, self.to_toml()).map_err(|source| write(&staged, source))?;
        std::fs::rename(&staged, path).map_err(|source| write(path, source))
    }

    /// Parses `source` as the state stored at `path`.
    ///
    /// # Errors
    ///
    /// Returns what is wrong with the file's contents.
    fn parse(path: &Path, source: &str) -> Result<Self, StateError> {
        let document = Document::parse(source).map_err(|source| StateError::Syntax {
            path: path.to_owned(),
            source,
        })?;
        let theme = document
            .as_table()
            .get("theme")
            .and_then(Item::as_str)
            .ok_or_else(|| StateError::Missing {
                path: path.to_owned(),
            })?;

        ThemeId::parse(theme)
            .map(Self::new)
            .ok_or_else(|| StateError::Id {
                path: path.to_owned(),
                theme: theme.to_owned(),
            })
    }

    /// The file's contents.
    ///
    /// A `ThemeId` is lowercase, digits and hyphens, so it needs no escaping.
    fn to_toml(&self) -> String {
        format!(
            "# written by vanadis; the theme it last applied\ntheme = \"{}\"\n",
            self.theme.as_str()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn parse(source: &str) -> Result<State, StateError> {
        State::parse(Path::new("state.toml"), source)
    }

    #[test]
    fn reads_the_theme_it_records() {
        let state = parse("theme = \"papercolor-light\"\n").unwrap();
        assert_eq!(state.theme().as_str(), "papercolor-light");
    }

    #[test]
    fn reports_a_state_file_that_names_no_theme() {
        assert!(matches!(parse("\n"), Err(StateError::Missing { .. })));
    }

    #[test]
    fn reports_a_theme_that_is_not_an_identifier() {
        let error = parse("theme = \"PaperColor Light\"\n").unwrap_err();
        assert!(
            matches!(error, StateError::Id { ref theme, .. } if theme == "PaperColor Light"),
            "{error:?}"
        );
    }

    #[test]
    fn reports_a_state_file_that_is_not_toml() {
        assert!(matches!(parse("theme ="), Err(StateError::Syntax { .. })));
    }

    #[test]
    fn reads_back_what_it_writes() {
        let state = State::new(ThemeId::parse("gruvbox-dark").unwrap());
        assert_eq!(parse(&state.to_toml()).unwrap(), state);
    }
}
