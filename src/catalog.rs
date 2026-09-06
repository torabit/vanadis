//! Theme discovery: what `themes/` holds.
//!
//! `docs/config.md` decides that `themes/` is the only place scanned, that a file is a
//! theme when it ends in `.toml`, and that the filename minus `.toml` is the identifier.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::theme::{Theme, ThemeError};

/// Every theme a directory holds, and every file in it that is not one.
#[derive(Debug)]
pub struct Catalog {
    themes: Vec<Theme>,
    broken: Vec<ThemeError>,
}

/// The themes directory could not be scanned.
#[derive(Debug, Error)]
pub enum CatalogError {
    /// The directory could not be read.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The directory the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
}

impl Catalog {
    /// Loads every theme in `directory`, in identifier order.
    ///
    /// A file that does not load costs the user that theme, not the command: it lands in
    /// [`Catalog::broken`] and the rest of the directory is still read. Subdirectories and
    /// files that do not end in `.toml` are ignored.
    ///
    /// # Errors
    ///
    /// Returns the read error when the directory itself cannot be listed.
    pub fn scan(directory: &Path) -> Result<Self, CatalogError> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
            .map_err(|source| CatalogError::Read {
                path: directory.to_owned(),
                source,
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|kind| kind == "toml") && path.is_file())
            .collect();
        files.sort();

        let mut themes = Vec::new();
        let mut broken = Vec::new();
        for file in files {
            match Theme::load(&file) {
                Ok(theme) => themes.push(theme),
                Err(error) => broken.push(error),
            }
        }

        Ok(Self { themes, broken })
    }

    /// The themes that loaded, in identifier order.
    #[must_use]
    pub fn themes(&self) -> &[Theme] {
        &self.themes
    }

    /// Why each of the other files is not a theme, in the same order.
    #[must_use]
    pub fn broken(&self) -> &[ThemeError] {
        &self.broken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config/themes");
        Catalog::scan(&directory).unwrap()
    }

    fn ids() -> Vec<String> {
        catalog()
            .themes()
            .iter()
            .map(|theme| theme.id().as_str().to_owned())
            .collect()
    }

    #[test]
    fn finds_every_theme_the_directory_holds() {
        assert_eq!(ids(), vec!["gruvbox-dark", "papercolor-light"]);
    }

    #[test]
    fn ignores_a_file_that_is_not_a_toml_file() {
        assert!(!ids().contains(&"notes".to_owned()));
    }

    #[test]
    fn ignores_a_subdirectory() {
        assert!(!ids().contains(&"deep".to_owned()));
    }

    #[test]
    fn keeps_a_theme_it_cannot_load_out_of_the_list() {
        assert!(!ids().contains(&"broken".to_owned()));
    }

    #[test]
    fn reports_the_theme_it_could_not_load() {
        let broken = catalog();
        let broken = broken.broken();
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(broken[0].to_string().contains("broken.toml"), "{broken:?}");
    }

    #[test]
    fn reports_a_directory_it_cannot_scan() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nowhere");
        assert!(Catalog::scan(&directory).is_err());
    }
}
