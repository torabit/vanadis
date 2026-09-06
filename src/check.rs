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

use crate::apply::{ApplyError, Rendered, pinned, render, select};
use crate::catalog::Catalog;
use crate::config::{Config, Target, TargetName};
use crate::state::State;
use crate::theme::{Theme, ThemeId};
use crate::token::TokenPath;
use crate::vocabulary;

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

/// `s` when there is not exactly one.
fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// The missing paths, four to a line and indented, the way `init` prints the same list.
fn undefined(missing: &[TokenPath]) -> String {
    missing
        .chunks(4)
        .map(|line| {
            let paths: Vec<&str> = line.iter().map(TokenPath::as_str).collect();
            format!("  {}", paths.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One thing `check` found wrong: four about a target, one about a theme.
#[derive(Debug, Error)]
pub enum Finding {
    /// A theme the run resolves to does not define the whole core vocabulary.
    ///
    /// This is about the theme rather than any one target, so it is reported once however
    /// many targets are on it. `docs/core-vocabulary.md` makes `check` the only place the
    /// core is enforced.
    #[error("{theme}: {} core token{} undefined\n{}", .missing.len(), plural(.missing.len()), undefined(.missing))]
    Incomplete {
        /// The theme that is short of the core.
        theme: ThemeId,
        /// Every core token it does not define, in core order.
        missing: Vec<TokenPath>,
    },
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

    /// Everything wrong: the incomplete themes first, then the targets in config order.
    ///
    /// A theme short of the core is why several of the target findings below it exist, so it
    /// is reported before them rather than after.
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
/// Every theme the run resolves to is also measured against the core vocabulary. That is the
/// only place the core is enforced; a theme is free to be incomplete and still load, list and
/// apply to targets whose templates never read what it is short of.
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

    // The core is enforced against the themes this run resolves to, not against everything in
    // `themes/`. `check` asks whether the machine is consistent, and a theme no target is on is
    // not part of that: making one break the run is the shape `docs/core-vocabulary.md` rejects
    // for the loader, where an unfinished file costs the user that theme rather than the tool.
    // `vanadis check <theme>` is how a theme that is not applied yet gets the same question.
    let mut resolved: Vec<ThemeId> = Vec::new();
    for target in &targets {
        // The same resolution `render` does: the applied theme states the mode, and a target
        // with its own `themes` takes the one it names for that mode. Both ways this can fail
        // — a theme that is not in `themes/`, a `themes` table short of the mode — are already
        // that target's own `Unrenderable`, so they are passed over rather than reported twice.
        let applied = assigned.theme(target);
        let Some(mode) = catalog.get(&applied).map(Theme::variant) else {
            continue;
        };
        let Ok(id) = pinned(target, &applied, mode) else {
            continue;
        };
        if resolved.contains(&id) {
            continue;
        }
        resolved.push(id.clone());
        if let Some(theme) = catalog.get(&id) {
            let missing = vocabulary::missing(theme.tokens());
            if !missing.is_empty() {
                findings.push(Finding::Incomplete { theme: id, missing });
            }
        }
    }

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
