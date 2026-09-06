//! Applying a theme: render every target, then write, then reload.
//!
//! `docs/config.md` decides the order. Every target renders before anything is written, and
//! every write is staged before any of them replaces the file it is going to, so a failure
//! anywhere leaves the machine on the theme it already had.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use thiserror::Error;

use crate::catalog::Catalog;
use crate::config::{Config, Target, TargetName};
use crate::state::{State, StateError};
use crate::template::{Template, TemplateError};
use crate::theme::{ThemeId, Variant};

/// What an apply did.
#[derive(Debug)]
pub struct Applied {
    theme: ThemeId,
    written: Vec<TargetName>,
    reloaded: Vec<TargetName>,
    failures: Vec<ReloadError>,
}

impl Applied {
    /// The theme that was applied.
    #[must_use]
    pub fn theme(&self) -> &ThemeId {
        &self.theme
    }

    /// The targets that were written, in the order the config writes them.
    #[must_use]
    pub fn written(&self) -> &[TargetName] {
        &self.written
    }

    /// The targets whose reload ran and succeeded.
    #[must_use]
    pub fn reloaded(&self) -> &[TargetName] {
        &self.reloaded
    }

    /// Every reload that did not succeed. The files it would have reloaded are written.
    #[must_use]
    pub fn failures(&self) -> &[ReloadError] {
        &self.failures
    }
}

/// A reload that did not run, or ran and failed.
#[derive(Debug, Error)]
pub enum ReloadError {
    /// The command could not be started.
    #[error("{name}: cannot run `{command}`: {source}")]
    Start {
        /// The target the command belongs to.
        name: TargetName,
        /// The command, as the config writes it.
        command: String,
        /// The IO error.
        source: std::io::Error,
    },
    /// The command ran and exited non-zero.
    #[error("{name}: `{command}` {status}")]
    Status {
        /// The target the command belongs to.
        name: TargetName,
        /// The command, as the config writes it.
        command: String,
        /// How it exited.
        status: ExitStatus,
    },
}

/// Applying a theme failed, before anything was written.
#[derive(Debug, Error)]
pub enum ApplyError {
    /// The theme is not in `themes/`.
    #[error("themes/ holds no theme named `{theme}`")]
    UnknownTheme {
        /// The theme that was asked for.
        theme: ThemeId,
    },
    /// A target's own themes name none for the mode being applied.
    #[error("target `{target}` names no theme for {variant}")]
    NoTheme {
        /// The target whose themes are short one mode.
        target: TargetName,
        /// The mode that was being applied.
        variant: Variant,
    },
    /// `--only` named a target the config does not have.
    #[error("config.toml has no target named `{name}`")]
    UnknownTarget {
        /// The name that was asked for.
        name: TargetName,
    },
    /// `--only` was used before any theme was applied to every target.
    #[error("--only needs a theme applied to every target first")]
    NoWholeApply,
    /// A template could not be read.
    #[error("{}: {source}", .path.display())]
    Template {
        /// The template the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// A template could not be rendered.
    #[error(transparent)]
    Render(#[from] TemplateError),
    /// An output could not be written.
    #[error("{}: {source}", .path.display())]
    Write {
        /// The file the write failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// The state file could not be read or written.
    #[error(transparent)]
    State(#[from] StateError),
}

/// One target rendered, waiting to be written.
struct Rendered {
    name: TargetName,
    template: PathBuf,
    output: PathBuf,
    contents: String,
}

/// Applies `theme`, or applies it to the targets `only` names.
///
/// The state file at `state` records what was applied, and is written after every output is
/// in place and before any reload runs.
///
/// # Errors
///
/// Returns what stopped the apply. Nothing is written unless every target renders.
pub fn apply(
    config: &Config,
    catalog: &Catalog,
    theme: &ThemeId,
    only: &[TargetName],
    state: &Path,
) -> Result<Applied, ApplyError> {
    let applied = catalog.get(theme).ok_or_else(|| ApplyError::UnknownTheme {
        theme: theme.clone(),
    })?;
    let variant = applied.variant();
    let targets = select(config, only)?;
    let next = advance(State::load(state)?, theme, &targets, only.is_empty())?;

    let mut renders = Vec::new();
    for target in &targets {
        let id = pinned(target, theme, variant)?;
        let source = catalog
            .get(&id)
            .ok_or_else(|| ApplyError::UnknownTheme { theme: id.clone() })?;
        let text =
            std::fs::read_to_string(target.template()).map_err(|source| ApplyError::Template {
                path: target.template().to_owned(),
                source,
            })?;
        renders.push(Rendered {
            name: target.name().clone(),
            template: target.template().to_owned(),
            output: target.output().to_owned(),
            contents: Template::new(target.template(), text).render(source.tokens())?,
        });
    }

    write(&renders)?;
    next.store(state)?;

    let (reloaded, failures) = reload(&targets);
    Ok(Applied {
        theme: theme.clone(),
        written: renders.into_iter().map(|render| render.name).collect(),
        reloaded,
        failures,
    })
}

/// The targets to write: every one, or the ones `only` names.
fn select<'a>(config: &'a Config, only: &[TargetName]) -> Result<Vec<&'a Target>, ApplyError> {
    if only.is_empty() {
        return Ok(config.targets().iter().collect());
    }

    let mut seen = BTreeSet::new();
    let mut selected = Vec::new();
    for name in only {
        let target = config
            .targets()
            .iter()
            .find(|target| target.name() == name)
            .ok_or_else(|| ApplyError::UnknownTarget { name: name.clone() })?;
        if seen.insert(name.clone()) {
            selected.push(target);
        }
    }
    Ok(selected)
}

/// The theme `target` takes: its own for this mode, or the one being applied.
fn pinned(target: &Target, applied: &ThemeId, variant: Variant) -> Result<ThemeId, ApplyError> {
    let Some(themes) = target.themes() else {
        return Ok(applied.clone());
    };
    themes
        .theme(variant)
        .cloned()
        .ok_or_else(|| ApplyError::NoTheme {
            target: target.name().clone(),
            variant,
        })
}

/// The state after this apply.
///
/// A whole apply replaces it. A partial one moves the targets it names off a theme that is
/// already recorded, so there has to be one.
fn advance(
    previous: Option<State>,
    theme: &ThemeId,
    targets: &[&Target],
    whole: bool,
) -> Result<State, ApplyError> {
    if whole {
        return Ok(State::new(theme.clone()));
    }

    let mut state = previous.ok_or(ApplyError::NoWholeApply)?;
    for target in targets {
        state.record(target.name().clone(), theme.clone());
    }
    Ok(state)
}

/// Writes every render, staging all of them before any replaces the file it is going to.
fn write(renders: &[Rendered]) -> Result<(), ApplyError> {
    let mut staged = Vec::new();
    for render in renders {
        match stage(render) {
            Ok(path) => staged.push((path, &render.output)),
            Err(error) => {
                for (path, _) in &staged {
                    let _ = std::fs::remove_file(path);
                }
                return Err(error);
            }
        }
    }

    for (path, output) in staged {
        std::fs::rename(&path, output).map_err(|source| ApplyError::Write {
            path: output.clone(),
            source,
        })?;
    }
    Ok(())
}

/// Writes one render beside the file it is going to, and returns where it put it.
fn stage(render: &Rendered) -> Result<PathBuf, ApplyError> {
    let failed = |path: &Path, source: std::io::Error| ApplyError::Write {
        path: path.to_owned(),
        source,
    };

    if let Some(parent) = render.output.parent() {
        std::fs::create_dir_all(parent).map_err(|source| failed(parent, source))?;
    }

    let Some(name) = render.output.file_name() else {
        return Err(failed(
            &render.output,
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a file path"),
        ));
    };
    let mut staged = name.to_owned();
    staged.push(".vanadis-new");
    let staged = render.output.with_file_name(staged);

    std::fs::write(&staged, &render.contents).map_err(|source| failed(&staged, source))?;
    keep_mode(&staged, &render.output, &render.template)
        .map_err(|source| failed(&staged, source))?;
    Ok(staged)
}

/// Gives `staged` the mode `output` already has, or the one `template` has when it is new.
#[cfg(unix)]
fn keep_mode(staged: &Path, output: &Path, template: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let mode = std::fs::metadata(output)
        .or_else(|_| std::fs::metadata(template))
        .map(|metadata| metadata.permissions().mode());
    match mode {
        Ok(mode) => std::fs::set_permissions(staged, std::fs::Permissions::from_mode(mode)),
        Err(_) => Ok(()),
    }
}

/// The filesystem records no mode to keep.
#[cfg(not(unix))]
fn keep_mode(_staged: &Path, _output: &Path, _template: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Runs each target's reload, in order. A failure stops nothing.
fn reload(targets: &[&Target]) -> (Vec<TargetName>, Vec<ReloadError>) {
    let mut reloaded = Vec::new();
    let mut failures = Vec::new();

    for target in targets {
        let Some((program, arguments)) = target.reload().split_first() else {
            continue;
        };
        let name = target.name().clone();
        let command = target.reload().join(" ");

        match Command::new(program).args(arguments).status() {
            Ok(status) if status.success() => reloaded.push(name),
            Ok(status) => failures.push(ReloadError::Status {
                name,
                command,
                status,
            }),
            Err(source) => failures.push(ReloadError::Start {
                name,
                command,
                source,
            }),
        }
    }

    (reloaded, failures)
}
