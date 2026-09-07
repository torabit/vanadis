//! The state file: which theme was applied last.
//!
//! It lives outside the config directory, under `$XDG_STATE_HOME`, because it records what
//! this machine is showing rather than what the user configured. See `docs/config.md`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::{Document, Item};

use crate::config::TargetName;
use crate::theme::ThemeId;

/// What vanadis last applied.
///
/// `theme` is what a whole apply wrote. `targets` holds the targets a later `--only` apply
/// moved off it, which is the only way the two can disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    theme: ThemeId,
    targets: BTreeMap<TargetName, ThemeId>,
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
    /// A key under `[targets]` is not a target name.
    #[error("{}: `{name}` is not a target name", .path.display())]
    Target {
        /// The file the name came from.
        path: PathBuf,
        /// The key, as the file writes it.
        name: String,
    },
    /// A theme name that is not a theme identifier.
    #[error("{}: `{theme}` is not a theme identifier", .path.display())]
    Id {
        /// The file the identifier came from.
        path: PathBuf,
        /// The value `theme` holds.
        theme: String,
    },
}

impl State {
    /// The state that records `theme` as applied to every target.
    #[must_use]
    pub fn new(theme: ThemeId) -> Self {
        Self {
            theme,
            targets: BTreeMap::new(),
        }
    }

    /// The theme that was applied.
    #[must_use]
    pub fn theme(&self) -> &ThemeId {
        &self.theme
    }

    /// The targets carrying a theme of their own, and which.
    #[must_use]
    pub fn targets(&self) -> &BTreeMap<TargetName, ThemeId> {
        &self.targets
    }

    /// The theme `target` is on: the one recorded for it alone, or the applied theme.
    ///
    /// A partial apply is the only thing that puts the two apart, and every command that
    /// renders what is already on the machine has to ask this rather than read `theme`.
    #[must_use]
    pub fn theme_for(&self, target: &TargetName) -> &ThemeId {
        self.targets.get(target).unwrap_or(&self.theme)
    }

    /// Records `theme` as applied to `name` alone.
    ///
    /// A target brought back to the theme every other target carries stops being recorded,
    /// so the table holds exactly what diverges.
    pub fn record(&mut self, name: TargetName, theme: ThemeId) {
        if theme == self.theme {
            self.targets.remove(&name);
        } else {
            self.targets.insert(name, theme);
        }
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
        let write = |path: &Path| {
            let path = path.to_owned();
            move |source| StateError::Write { path, source }
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(write(parent))?;
        }

        let staged = path.with_extension("toml.new");
        std::fs::write(&staged, self.to_toml()).map_err(write(&staged))?;
        std::fs::rename(&staged, path).map_err(write(path))
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

        let theme = ThemeId::parse(theme).ok_or_else(|| StateError::Id {
            path: path.to_owned(),
            theme: theme.to_owned(),
        })?;

        let mut targets = BTreeMap::new();
        if let Some(table) = document
            .as_table()
            .get("targets")
            .and_then(Item::as_table_like)
        {
            for (key, item) in table.iter() {
                let name = TargetName::parse(key).ok_or_else(|| StateError::Target {
                    path: path.to_owned(),
                    name: key.to_owned(),
                })?;
                // A value that is not a string cannot be an identifier either, so it is
                // reported as the type it is.
                let value = item.as_str().unwrap_or_else(|| item.type_name());
                let theme = ThemeId::parse(value).ok_or_else(|| StateError::Id {
                    path: path.to_owned(),
                    theme: value.to_owned(),
                })?;
                targets.insert(name, theme);
            }
        }

        Ok(Self { theme, targets })
    }

    /// The file's contents.
    ///
    /// A `ThemeId` and a `TargetName` are lowercase, digits and hyphens, so neither needs
    /// escaping.
    fn to_toml(&self) -> String {
        let mut file = format!(
            "# written by vanadis; the theme it last applied\ntheme = \"{}\"\n",
            self.theme.as_str()
        );
        if !self.targets.is_empty() {
            file.push_str("\n[targets]\n");
            for (name, theme) in &self.targets {
                let _ = writeln!(file, "{name} = \"{}\"", theme.as_str());
            }
        }
        file
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
    #[test]
    fn reads_a_target_applied_on_its_own() {
        let state = parse("theme = \"paper\"\n[targets]\nnvim = \"nord\"\n").unwrap();
        assert_eq!(
            state.targets().get(&TargetName::parse("nvim").unwrap()),
            Some(&ThemeId::parse("nord").unwrap())
        );
    }

    #[test]
    fn leaves_targets_empty_when_the_file_names_none() {
        assert!(parse("theme = \"paper\"\n").unwrap().targets().is_empty());
    }

    #[test]
    fn records_a_target_that_diverges_from_the_theme() {
        let mut state = State::new(ThemeId::parse("paper").unwrap());
        state.record(
            TargetName::parse("nvim").unwrap(),
            ThemeId::parse("nord").unwrap(),
        );
        assert_eq!(state.targets().len(), 1);
    }

    #[test]
    fn drops_a_target_brought_back_to_the_theme() {
        let mut state = State::new(ThemeId::parse("paper").unwrap());
        let nvim = TargetName::parse("nvim").unwrap();
        state.record(nvim.clone(), ThemeId::parse("nord").unwrap());
        state.record(nvim, ThemeId::parse("paper").unwrap());
        assert!(state.targets().is_empty());
    }

    #[test]
    fn reads_back_the_targets_it_writes() {
        let mut state = State::new(ThemeId::parse("paper").unwrap());
        state.record(
            TargetName::parse("nvim").unwrap(),
            ThemeId::parse("nord").unwrap(),
        );
        assert_eq!(parse(&state.to_toml()).unwrap(), state);
    }

    #[test]
    fn reports_a_target_name_that_is_not_an_identifier() {
        let source = "theme = \"paper\"\n[targets]\n\"Nvim Editor\" = \"nord\"\n";
        assert!(matches!(parse(source), Err(StateError::Target { .. })));
    }

    #[test]
    fn reports_a_target_theme_that_is_not_an_identifier() {
        let source = "theme = \"paper\"\n[targets]\nnvim = \"Nord\"\n";
        assert!(matches!(parse(source), Err(StateError::Id { .. })));
    }
}
