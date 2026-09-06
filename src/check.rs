//! Verifying what is on disk: render every target in memory, then compare.
//!
//! `check` is the command vanadis has that a theme distribution system cannot: it generated
//! the file, so it can say whether the file is still what it generated. See
//! `docs/core-vocabulary.md` for why that is the differentiator.
//!
//! One target's failure costs that target and nothing else. Every problem lands in the
//! report and the remaining targets are still checked, the same call
//! [`Catalog::broken`](crate::Catalog::broken) makes for a theme file that will not load.

use std::path::PathBuf;

use thiserror::Error;

use crate::apply::{ApplyError, Rendered, render, select};
use crate::catalog::Catalog;
use crate::config::{Config, Target, TargetName};
use crate::state::State;
use crate::theme::ThemeId;

/// What the file a render would be written to holds now.
#[derive(Debug)]
pub enum Disk {
    /// The file holds exactly what the render would write.
    Same,
    /// The file holds something else, which is what this carries.
    Different(String),
    /// There is no file.
    Missing,
    /// The file could not be read.
    Unreadable(std::io::Error),
}

/// Reads the file `render` would be written to and compares it against the render.
#[must_use]
pub fn compare(render: &Rendered) -> Disk {
    match std::fs::read_to_string(render.output()) {
        Ok(current) if current == render.contents() => Disk::Same,
        Ok(current) => Disk::Different(current),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Disk::Missing,
        Err(error) => Disk::Unreadable(error),
    }
}

/// One thing `check` found wrong with a target.
#[derive(Debug, Error)]
pub enum Finding {
    /// The output no longer holds what its template renders to.
    #[error("{target}: {} does not match {}", .output.display(), .template.display())]
    Drift {
        /// The target the output belongs to.
        target: TargetName,
        /// The file that has drifted.
        output: PathBuf,
        /// The template it should hold.
        template: PathBuf,
    },
    /// The output has not been written yet, or has been deleted.
    #[error("{target}: {} does not exist", .output.display())]
    Missing {
        /// The target the output belongs to.
        target: TargetName,
        /// The file that is not there.
        output: PathBuf,
    },
    /// The output exists and could not be read, so nothing can be said about it.
    #[error("{target}: {}: {source}", .output.display())]
    Unreadable {
        /// The target the output belongs to.
        target: TargetName,
        /// The file the read failed on.
        output: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// The target does not render at all, so an apply would fail on it.
    #[error("{target}: {source}")]
    Unrenderable {
        /// The target that does not render.
        target: TargetName,
        /// Why it does not: a missing theme, an unreadable template, undefined tokens.
        source: ApplyError,
    },
}

/// `check` could not run at all.
#[derive(Debug, Error)]
pub enum CheckError {
    /// `--only` named a target the config does not have.
    #[error("config.toml has no target named `{name}`")]
    UnknownTarget {
        /// The name that was asked for.
        name: TargetName,
    },
    /// No theme was named and none has been applied, so there is nothing to check against.
    #[error("no theme has been applied yet, so name a theme")]
    Unapplied,
}

/// What `check` found.
#[derive(Debug)]
pub struct Report {
    checked: usize,
    findings: Vec<Finding>,
}

impl Report {
    /// How many targets were checked.
    #[must_use]
    pub fn checked(&self) -> usize {
        self.checked
    }

    /// Everything wrong, in the order the config writes the targets.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
}

/// The theme each target is checked against.
enum Assigned<'a> {
    /// One theme, named on the command line, for every target.
    Named(&'a ThemeId),
    /// Whatever the state file records the target as being on.
    Recorded(&'a State),
}

impl Assigned<'_> {
    /// The theme `target` is checked against.
    fn theme(&self, target: &Target) -> ThemeId {
        match self {
            Self::Named(theme) => (*theme).clone(),
            Self::Recorded(state) => state
                .targets()
                .get(target.name())
                .unwrap_or_else(|| state.theme())
                .clone(),
        }
    }
}

/// Checks every target, or the ones `only` names, against `theme` or against `state`.
///
/// Naming a theme asks whether the machine looks like that theme. Naming none asks whether
/// it still looks like what was applied, which is the question a partial apply makes
/// interesting: `state` carries the targets that were moved off the theme it records.
///
/// # Errors
///
/// Returns the reason there is nothing to check. A target that cannot be checked is a
/// finding, not an error.
pub fn check(
    config: &Config,
    catalog: &Catalog,
    theme: Option<&ThemeId>,
    state: Option<&State>,
    only: &[TargetName],
) -> Result<Report, CheckError> {
    let assigned = match (theme, state) {
        (Some(theme), _) => Assigned::Named(theme),
        (None, Some(state)) => Assigned::Recorded(state),
        (None, None) => return Err(CheckError::Unapplied),
    };
    let targets = select(config, only).map_err(|name| CheckError::UnknownTarget { name })?;

    let mut findings = Vec::new();
    for target in &targets {
        let name = target.name().clone();
        let rendered = match render(catalog, target, &assigned.theme(target)) {
            Ok(rendered) => rendered,
            Err(source) => {
                findings.push(Finding::Unrenderable {
                    target: name,
                    source,
                });
                continue;
            }
        };

        match compare(&rendered) {
            Disk::Same => {}
            Disk::Different(_) => findings.push(Finding::Drift {
                target: name,
                output: rendered.output().to_owned(),
                template: rendered.template().to_owned(),
            }),
            Disk::Missing => findings.push(Finding::Missing {
                target: name,
                output: rendered.output().to_owned(),
            }),
            Disk::Unreadable(source) => findings.push(Finding::Unreadable {
                target: name,
                output: rendered.output().to_owned(),
                source,
            }),
        }
    }

    Ok(Report {
        checked: targets.len(),
        findings,
    })
}
