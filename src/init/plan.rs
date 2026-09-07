//! What one run will write, and the check it has to pass first.
//!
//! `docs/init.md` decides that an apply straight after an init reproduces the original file
//! byte for byte, and decides that `init` proves it before writing rather than leaving it to
//! a test. [`plan`] is where that proof happens: it builds the theme, loads it back, renders
//! the template against it, and compares.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::config::TargetName;
use crate::diff::unified;
use crate::init::emit::{self, EmitError};
use crate::init::naming::display_name;
use crate::init::weave::{Binding, weave};
use crate::template::{Template, TemplateError};
use crate::theme::{Theme, ThemeError, ThemeId, Variant};
use crate::token::TokenPath;
use crate::vocabulary;

/// Everything one run needs to know, after the dialogue has answered it.
#[derive(Debug, Clone, Copy)]
pub struct Draft<'a> {
    /// The directory holding `config.toml`, `themes/` and `templates/`.
    pub directory: &'a Path,
    /// The home directory, which the output path is written relative to.
    pub home: Option<&'a Path>,
    /// What the target is called.
    pub name: &'a TargetName,
    /// The file being read, which becomes the target's output.
    pub output: &'a Path,
    /// What that file holds.
    pub source: &'a str,
    /// The theme being written into.
    pub id: &'a ThemeId,
    /// The background that theme is for. Ignored when it already exists.
    pub variant: Variant,
    /// What the theme file holds already, or `None` when there is no such theme.
    pub existing: Option<&'a str>,
    /// What `config.toml` holds already, empty when there is no config yet.
    pub config: &'a str,
    /// Which occurrence takes which token.
    pub bindings: &'a [Binding],
    /// The tokens to add to the theme.
    pub tokens: &'a BTreeMap<TokenPath, String>,
}

/// One file a run will write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    path: PathBuf,
    contents: String,
}

impl Written {
    /// Where it goes.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What it will hold.
    #[must_use]
    pub fn contents(&self) -> &str {
        &self.contents
    }
}

/// Three files that have been checked and not yet written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    theme: Written,
    template: Written,
    config: Written,
    missing: Vec<TokenPath>,
}

/// A run could not be planned, or could not be written.
#[derive(Debug, Error)]
pub enum InitError {
    /// The template would replace a file that is already there.
    #[error("{}: already exists", .path.display())]
    Occupied {
        /// The file that is in the way.
        path: PathBuf,
    },
    /// The theme that was built does not load.
    #[error(transparent)]
    Theme(#[from] ThemeError),
    /// The template that was built does not render.
    #[error(transparent)]
    Render(#[from] TemplateError),
    /// A token could not be added to the theme.
    #[error(transparent)]
    Emit(#[from] EmitError),
    /// The template renders to something other than the file it was read from.
    #[error("{}: an apply would not reproduce this file\n{diff}", .output.display())]
    Mismatch {
        /// The file that would not come back.
        output: PathBuf,
        /// What would change, line by line.
        diff: String,
    },
    /// A file could not be written.
    #[error("{}: {source}", .path.display())]
    Write {
        /// The file the write failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
}

/// Builds the three files, and proves they reproduce the original before returning them.
///
/// # Errors
///
/// Returns [`InitError::Mismatch`] when rendering the template gives back anything other
/// than the file it was read from, and the theme's or the template's own error when either
/// will not load or will not render. Nothing is written either way.
pub fn plan(draft: Draft<'_>) -> Result<Plan, InitError> {
    let theme = Written {
        path: draft
            .directory
            .join("themes")
            .join(format!("{}.toml", draft.id)),
        contents: match draft.existing {
            Some(existing) => emit::merged(existing, draft.tokens)?,
            None => emit::theme(&display_name(draft.id), draft.variant, draft.tokens)?,
        },
    };
    let relative = relative(draft.name, draft.output);
    let template = Written {
        path: draft.directory.join(&relative),
        contents: weave(draft.source, draft.bindings),
    };

    // The theme is loaded back rather than trusted, so a bad value or an unresolvable
    // reference is this run's error and not the next apply's.
    let loaded = Theme::parse(&theme.path, &theme.contents)?;
    let rendered = Template::new(&template.path, &template.contents).render(loaded.tokens())?;
    if rendered != draft.source {
        return Err(InitError::Mismatch {
            output: draft.output.to_owned(),
            diff: unified(draft.output, draft.source, &rendered),
        });
    }

    let config = Written {
        path: draft.directory.join("config.toml"),
        contents: emit::target(
            draft.config,
            draft.name,
            &relative.to_string_lossy(),
            &contracted(draft.output, draft.home),
        ),
    };
    Ok(Plan {
        missing: vocabulary::missing(loaded.tokens()),
        theme,
        template,
        config,
    })
}

impl Plan {
    /// The theme file, whether it is being created or added to.
    #[must_use]
    pub fn theme(&self) -> &Written {
        &self.theme
    }

    /// The template.
    #[must_use]
    pub fn template(&self) -> &Written {
        &self.template
    }

    /// `config.toml`, with the new target on the end of it.
    #[must_use]
    pub fn config(&self) -> &Written {
        &self.config
    }

    /// The core tokens the theme still does not define, in core order.
    #[must_use]
    pub fn missing(&self) -> &[TokenPath] {
        &self.missing
    }

    /// Writes the three files.
    ///
    /// All three are staged beside their destinations before any of them replaces the file
    /// it is going to, which is the order `docs/config.md` has an apply write in. A failure
    /// while staging leaves nothing behind and nothing replaced.
    ///
    /// The template is never replaced: a file already sitting at its path is somebody
    /// else's, and `docs/init.md` refuses the run rather than guess.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::Occupied`] when the template is in the way, and
    /// [`InitError::Write`] naming whichever file could not be written.
    pub fn commit(&self) -> Result<(), InitError> {
        if self.template.path.exists() {
            return Err(InitError::Occupied {
                path: self.template.path.clone(),
            });
        }

        let files = [&self.template, &self.theme, &self.config];
        let mut written = Vec::with_capacity(files.len());
        for file in files {
            match stage(file) {
                Ok(staged) => written.push(staged),
                Err(error) => {
                    for staged in written {
                        let _ = std::fs::remove_file(staged);
                    }
                    return Err(error);
                }
            }
        }

        for (file, staged) in files.iter().zip(written) {
            std::fs::rename(&staged, &file.path).map_err(failed(&file.path))?;
        }
        Ok(())
    }
}

/// Writes one file beside its destination, and says where it put it.
fn stage(file: &Written) -> Result<PathBuf, InitError> {
    if let Some(parent) = file.path.parent() {
        std::fs::create_dir_all(parent).map_err(failed(parent))?;
    }
    let mut name = file.path.as_os_str().to_os_string();
    name.push(".coloris-new");
    let staged = PathBuf::from(name);
    std::fs::write(&staged, &file.contents).map_err(failed(&staged))?;
    Ok(staged)
}

/// Reports an IO error against the file it happened on.
fn failed(path: &Path) -> impl FnOnce(std::io::Error) -> InitError {
    let path = path.to_owned();
    |source| InitError::Write { path, source }
}

/// Where the template goes, relative to the config directory.
fn relative(name: &TargetName, output: &Path) -> PathBuf {
    let mut file = output
        .file_name()
        .unwrap_or(output.as_os_str())
        .to_os_string();
    file.push(".in");
    Path::new("templates").join(name.as_str()).join(file)
}

/// `path`, with the home directory written as `~`.
fn contracted(path: &Path, home: Option<&Path>) -> String {
    home.and_then(|home| path.strip_prefix(home).ok())
        .map_or_else(
            || path.display().to_string(),
            |rest| format!("~/{}", rest.display()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::scan;

    const CONFIG: &str = "/home/ada/.config/coloris";
    const OUTPUT: &str = "/home/ada/.config/nvim/lua/palette.lua";

    fn path(text: &str) -> TokenPath {
        TokenPath::parse(text).unwrap()
    }

    /// Binds the first occurrence of the first value to `role.bg`.
    fn bound(source: &str) -> (Vec<Binding>, BTreeMap<TokenPath, String>) {
        let scan = scan::scan(source);
        let Some(colour) = scan.colours().first() else {
            return (Vec::new(), BTreeMap::new());
        };
        let bindings = colour
            .occurrences()
            .iter()
            .map(|occurrence| Binding::new(occurrence, path("role.bg")))
            .collect();
        let tokens = [(path("role.bg"), colour.value().to_owned())]
            .into_iter()
            .collect();
        (bindings, tokens)
    }

    fn draft<'a>(
        source: &'a str,
        bindings: &'a [Binding],
        tokens: &'a BTreeMap<TokenPath, String>,
        existing: Option<&'a str>,
        name: &'a TargetName,
        id: &'a ThemeId,
    ) -> Draft<'a> {
        Draft {
            directory: Path::new(CONFIG),
            home: Some(Path::new("/home/ada")),
            name,
            output: Path::new(OUTPUT),
            source,
            id,
            variant: Variant::Light,
            existing,
            config: "",
            bindings,
            tokens,
        }
    }

    fn planned(source: &str) -> Result<Plan, InitError> {
        let (bindings, tokens) = bound(source);
        let name = TargetName::parse("nvim").unwrap();
        let id = ThemeId::parse("papercolor-light").unwrap();
        plan(draft(source, &bindings, &tokens, None, &name, &id))
    }

    #[test]
    fn puts_the_template_under_the_config_directory() {
        let plan = planned("bg = \"#eeeeee\"\n").unwrap();
        assert_eq!(
            plan.template().path(),
            Path::new("/home/ada/.config/coloris/templates/nvim/palette.lua.in")
        );
    }

    #[test]
    fn substitutes_the_bound_colour_in_the_template() {
        let plan = planned("bg = \"#eeeeee\"\n").unwrap();
        assert_eq!(plan.template().contents(), "bg = \"{{role.bg}}\"\n");
    }

    #[test]
    fn names_the_theme_file_after_the_identifier() {
        let plan = planned("bg = \"#eeeeee\"\n").unwrap();
        assert_eq!(
            plan.theme().path(),
            Path::new("/home/ada/.config/coloris/themes/papercolor-light.toml")
        );
    }

    #[test]
    fn writes_the_bound_value_into_the_theme() {
        let plan = planned("bg = \"#eeeeee\"\n").unwrap();
        assert!(
            plan.theme().contents().contains("bg = \"#eeeeee\""),
            "{}",
            plan.theme().contents()
        );
    }

    #[test]
    fn writes_the_output_path_with_the_home_directory_contracted() {
        let plan = planned("bg = \"#eeeeee\"\n").unwrap();
        assert!(
            plan.config()
                .contents()
                .contains("output = \"~/.config/nvim/lua/palette.lua\""),
            "{}",
            plan.config().contents()
        );
    }

    #[test]
    fn writes_the_template_path_relative_to_the_config_directory() {
        let plan = planned("bg = \"#eeeeee\"\n").unwrap();
        assert!(
            plan.config()
                .contents()
                .contains("template = \"templates/nvim/palette.lua.in\""),
            "{}",
            plan.config().contents()
        );
    }

    #[test]
    fn names_the_core_tokens_the_theme_still_lacks() {
        let plan = planned("bg = \"#eeeeee\"\n").unwrap();
        assert_eq!(plan.missing().len(), 32);
        assert!(!plan.missing().contains(&path("role.bg")));
    }

    #[test]
    fn reproduces_a_file_whose_colours_are_all_bound() {
        assert!(planned("bg = \"#eeeeee\"\nfg = \"#eeeeee\"\n").is_ok());
    }

    #[test]
    fn reproduces_a_file_that_carries_template_syntax_of_its_own() {
        assert!(planned("title = \"{{columns}}\"\nbg = \"#eeeeee\"\n").is_ok());
    }

    #[test]
    fn reproduces_a_file_with_a_colour_nothing_binds() {
        let source = "bg = \"#eeeeee\"\nnote = \"#444444\"\n";
        let plan = planned(source).unwrap();
        assert!(plan.template().contents().contains("#444444"));
    }

    #[test]
    fn adds_to_a_theme_that_already_exists_rather_than_replacing_it() {
        let source = "fg = \"#444444\"\n";
        let existing = "[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"light\"\n\n[colors]\nink = \"#111111\"\n";
        let scan = scan::scan(source);
        let bindings: Vec<Binding> = scan.colours()[0]
            .occurrences()
            .iter()
            .map(|occurrence| Binding::new(occurrence, path("role.fg")))
            .collect();
        let tokens = [(path("role.fg"), "#444444".to_owned())]
            .into_iter()
            .collect();
        let name = TargetName::parse("nvim").unwrap();
        let id = ThemeId::parse("papercolor-light").unwrap();
        let plan = plan(draft(
            source,
            &bindings,
            &tokens,
            Some(existing),
            &name,
            &id,
        ))
        .unwrap();
        assert!(plan.theme().contents().contains("ink = \"#111111\""));
        assert!(plan.theme().contents().contains("fg = \"#444444\""));
    }

    #[test]
    fn refuses_a_theme_that_would_not_load() {
        let (bindings, _) = bound("bg = \"#eeeeee\"\n");
        let tokens = [(path("role.bg"), "not a colour".to_owned())]
            .into_iter()
            .collect();
        let name = TargetName::parse("nvim").unwrap();
        let id = ThemeId::parse("papercolor-light").unwrap();
        let error = plan(draft(
            "bg = \"#eeeeee\"\n",
            &bindings,
            &tokens,
            None,
            &name,
            &id,
        ))
        .unwrap_err();
        assert!(matches!(error, InitError::Theme(_)), "{error:?}");
    }

    #[test]
    fn refuses_a_binding_that_would_not_reproduce_the_file() {
        let source = "bg = \"#eeeeee\"\n";
        let (bindings, _) = bound(source);
        let tokens = [(path("role.bg"), "#444444".to_owned())]
            .into_iter()
            .collect();
        let name = TargetName::parse("nvim").unwrap();
        let id = ThemeId::parse("papercolor-light").unwrap();
        let error = plan(draft(source, &bindings, &tokens, None, &name, &id)).unwrap_err();
        assert!(matches!(error, InitError::Mismatch { .. }), "{error:?}");
    }

    #[test]
    fn shows_what_would_change_when_it_refuses() {
        let source = "bg = \"#eeeeee\"\n";
        let (bindings, _) = bound(source);
        let tokens = [(path("role.bg"), "#444444".to_owned())]
            .into_iter()
            .collect();
        let name = TargetName::parse("nvim").unwrap();
        let id = ThemeId::parse("papercolor-light").unwrap();
        let error = plan(draft(source, &bindings, &tokens, None, &name, &id)).unwrap_err();
        assert!(error.to_string().contains("#444444"), "{error}");
    }

    #[test]
    fn writes_an_absolute_output_path_that_is_not_under_the_home_directory() {
        let source = "bg = \"#eeeeee\"\n";
        let (bindings, tokens) = bound(source);
        let name = TargetName::parse("rio").unwrap();
        let id = ThemeId::parse("papercolor-light").unwrap();
        let mut draft = draft(source, &bindings, &tokens, None, &name, &id);
        draft.output = Path::new("/etc/rio/config.toml");
        let plan = plan(draft).unwrap();
        assert!(
            plan.config()
                .contents()
                .contains("output = \"/etc/rio/config.toml\""),
            "{}",
            plan.config().contents()
        );
    }
}
