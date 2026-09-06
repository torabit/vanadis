//! Where vanadis keeps its files.
//!
//! `docs/config.md` decides the layout: `$VANADIS_CONFIG` replaces the whole config
//! directory, and the state file stays outside it, under `$XDG_STATE_HOME`.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// The variables the paths are resolved out of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    vanadis_config: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    xdg_state_home: Option<PathBuf>,
    home: Option<PathBuf>,
}

/// A path could not be resolved.
#[derive(Debug, Error)]
pub enum PathsError {
    /// Nothing names the directory and there is no home directory to fall back on.
    #[error("cannot locate the {what}: neither ${variable} nor a home directory is set")]
    NoHome {
        /// What was being located.
        what: &'static str,
        /// The variable that would have answered.
        variable: &'static str,
    },
}

impl Environment {
    /// Reads the process environment.
    #[must_use]
    pub fn read() -> Self {
        Self {
            vanadis_config: variable("VANADIS_CONFIG"),
            xdg_config_home: variable("XDG_CONFIG_HOME"),
            xdg_state_home: variable("XDG_STATE_HOME"),
            home: std::env::home_dir(),
        }
    }

    /// The home directory, when the environment names one.
    #[must_use]
    pub fn home(&self) -> Option<&Path> {
        set(self.home.as_ref()).map(PathBuf::as_path)
    }

    /// The directory holding `config.toml` and `themes/`.
    ///
    /// # Errors
    ///
    /// Returns [`PathsError::NoHome`] when nothing names it and there is no home directory.
    pub fn config_dir(&self) -> Result<PathBuf, PathsError> {
        if let Some(directory) = set(self.vanadis_config.as_ref()) {
            return Ok(directory.clone());
        }
        if let Some(directory) = set(self.xdg_config_home.as_ref()) {
            return Ok(directory.join("vanadis"));
        }
        self.under_home(
            &[".config", "vanadis"],
            "config directory",
            "XDG_CONFIG_HOME",
        )
    }

    /// The directory themes are discovered in.
    ///
    /// # Errors
    ///
    /// Returns whatever [`Environment::config_dir`] returns.
    pub fn themes_dir(&self) -> Result<PathBuf, PathsError> {
        Ok(self.config_dir()?.join("themes"))
    }

    /// The file recording the applied theme.
    ///
    /// # Errors
    ///
    /// Returns [`PathsError::NoHome`] when nothing names it and there is no home directory.
    pub fn state_file(&self) -> Result<PathBuf, PathsError> {
        if let Some(directory) = set(self.xdg_state_home.as_ref()) {
            return Ok(directory.join("vanadis").join("state.toml"));
        }
        self.under_home(
            &[".local", "state", "vanadis", "state.toml"],
            "state file",
            "XDG_STATE_HOME",
        )
    }

    /// `tail` under the home directory.
    fn under_home(
        &self,
        tail: &[&str],
        what: &'static str,
        variable: &'static str,
    ) -> Result<PathBuf, PathsError> {
        let home = set(self.home.as_ref()).ok_or(PathsError::NoHome { what, variable })?;
        Ok(tail
            .iter()
            .fold(home.clone(), |path, segment| path.join(segment)))
    }
}

/// The variable `name`, as the process has it.
fn variable(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

/// `value`, treating the empty string a variable can hold as unset.
fn set(value: Option<&PathBuf>) -> Option<&PathBuf> {
    value.filter(|path| !path.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment() -> Environment {
        Environment {
            vanadis_config: None,
            xdg_config_home: None,
            xdg_state_home: None,
            home: Some(PathBuf::from("/home/ada")),
        }
    }

    #[test]
    fn takes_the_config_directory_from_vanadis_config() {
        let environment = Environment {
            vanadis_config: Some(PathBuf::from("/srv/themes")),
            xdg_config_home: Some(PathBuf::from("/home/ada/.config")),
            ..environment()
        };
        assert_eq!(environment.config_dir().unwrap(), Path::new("/srv/themes"));
    }

    #[test]
    fn takes_the_config_directory_from_xdg_config_home() {
        let environment = Environment {
            xdg_config_home: Some(PathBuf::from("/home/ada/.config")),
            ..environment()
        };
        assert_eq!(
            environment.config_dir().unwrap(),
            Path::new("/home/ada/.config/vanadis")
        );
    }

    #[test]
    fn falls_back_to_dot_config_under_the_home_directory() {
        assert_eq!(
            environment().config_dir().unwrap(),
            Path::new("/home/ada/.config/vanadis")
        );
    }

    #[test]
    fn reports_that_it_cannot_find_the_home_directory() {
        let environment = Environment {
            home: None,
            ..environment()
        };
        assert!(matches!(
            environment.config_dir(),
            Err(PathsError::NoHome { .. })
        ));
    }

    #[test]
    fn scans_themes_under_the_config_directory() {
        assert_eq!(
            environment().themes_dir().unwrap(),
            Path::new("/home/ada/.config/vanadis/themes")
        );
    }

    #[test]
    fn takes_the_state_file_from_xdg_state_home() {
        let environment = Environment {
            xdg_state_home: Some(PathBuf::from("/home/ada/.local/state")),
            ..environment()
        };
        assert_eq!(
            environment.state_file().unwrap(),
            Path::new("/home/ada/.local/state/vanadis/state.toml")
        );
    }

    #[test]
    fn falls_back_to_dot_local_state_under_the_home_directory() {
        assert_eq!(
            environment().state_file().unwrap(),
            Path::new("/home/ada/.local/state/vanadis/state.toml")
        );
    }

    #[test]
    fn keeps_the_state_file_out_of_the_directory_vanadis_config_names() {
        let environment = Environment {
            vanadis_config: Some(PathBuf::from("/srv/themes")),
            ..environment()
        };
        assert_eq!(
            environment.state_file().unwrap(),
            Path::new("/home/ada/.local/state/vanadis/state.toml")
        );
    }

    #[test]
    fn reads_an_empty_variable_as_unset() {
        let environment = Environment {
            xdg_config_home: Some(PathBuf::new()),
            ..environment()
        };
        assert_eq!(
            environment.config_dir().unwrap(),
            Path::new("/home/ada/.config/vanadis")
        );
    }
}
