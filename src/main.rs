//! The command line. Subcommands are added by the issues that define them.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::io::Write as _;
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{Parser, Subcommand, ValueEnum};
use vanadis::{
    Catalog, Config, Disk, Environment, Plan, State, TargetName, Theme, ThemeId, Variant,
};

#[derive(Parser)]
#[command(name = "vanadis", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render every target from a theme.
    Apply {
        /// The theme to apply.
        theme: Option<String>,
        /// Take the theme from `[auto]` for this background instead.
        #[arg(long, conflicts_with = "theme")]
        variant: Option<Background>,
        /// Write only these targets, leaving every other one alone.
        #[arg(long, value_name = "NAME")]
        only: Vec<String>,
        /// Render and report what would change, without writing anything.
        #[arg(long)]
        dry_run: bool,
        /// Show a unified diff of what would change. Writes nothing.
        #[arg(long)]
        diff: bool,
    },
    /// Report the outputs that have drifted and the tokens no theme defines.
    Check {
        /// The theme to check against, instead of the one that was applied.
        theme: Option<String>,
        /// Check against the theme `[auto]` names for this background.
        #[arg(long, conflicts_with = "theme")]
        variant: Option<Background>,
        /// Check only these targets.
        #[arg(long, value_name = "NAME")]
        only: Vec<String>,
    },
    /// List the themes in `themes/`.
    List {
        /// Show only the themes written for this background.
        #[arg(long)]
        variant: Option<Background>,
    },
    /// Print the theme that was applied last.
    Current,
}

/// [`Variant`], spelled as a command line argument.
#[derive(Clone, Copy, ValueEnum)]
enum Background {
    Dark,
    Light,
}

impl From<Background> for Variant {
    fn from(background: Background) -> Self {
        match background {
            Background::Dark => Self::Dark,
            Background::Light => Self::Light,
        }
    }
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let environment = Environment::read();

    match cli.command {
        Command::Apply {
            theme,
            variant,
            only,
            dry_run,
            diff,
        } => apply(
            &environment,
            theme.as_deref(),
            variant.map(Variant::from),
            &only,
            dry_run || diff,
            diff,
        ),
        Command::Check {
            theme,
            variant,
            only,
        } => check(
            &environment,
            theme.as_deref(),
            variant.map(Variant::from),
            &only,
        ),
        Command::List { variant } => list(&environment, variant.map(Variant::from)),
        Command::Current => current(&environment),
    }
}

/// The config and the themes it names, with every unloadable theme reported.
fn load(environment: &Environment) -> anyhow::Result<(Config, Catalog)> {
    let config = Config::load(&environment.config_dir()?, environment.home())?;
    let catalog = Catalog::scan(&environment.themes_dir()?)?;
    for error in catalog.broken() {
        eprintln!("warning: {error}");
    }
    Ok((config, catalog))
}

/// The theme a command was pointed at, or `None` when it names neither a theme nor a mode.
fn wanted(
    config: &Config,
    theme: Option<&str>,
    variant: Option<Variant>,
) -> anyhow::Result<Option<ThemeId>> {
    match (theme, variant) {
        (Some(name), _) => {
            Ok(Some(ThemeId::parse(name).with_context(|| {
                format!("`{name}` is not a theme identifier")
            })?))
        }
        (None, Some(variant)) => Ok(Some(
            config
                .auto()
                .context("config.toml has no [auto] table, so name a theme")?
                .theme(variant)
                .clone(),
        )),
        (None, None) => Ok(None),
    }
}

/// The names `--only` was given, parsed.
fn selected(only: &[String]) -> anyhow::Result<Vec<TargetName>> {
    only.iter()
        .map(|name| {
            TargetName::parse(name).with_context(|| format!("`{name}` is not a target name"))
        })
        .collect()
}

/// Renders every target, or the ones `only` names, from one theme.
fn apply(
    environment: &Environment,
    theme: Option<&str>,
    variant: Option<Variant>,
    only: &[String],
    dry_run: bool,
    diff: bool,
) -> anyhow::Result<ExitCode> {
    let (config, catalog) = load(environment)?;
    let theme = wanted(&config, theme, variant)?.context("name a theme, or pass --variant")?;
    let only = selected(only)?;

    let plan = vanadis::plan(&config, &catalog, &theme, &only, &environment.state_file()?)?;
    if dry_run {
        return preview(&plan, diff);
    }

    let applied = plan.commit()?;

    let mut out = std::io::stdout().lock();
    writeln!(out, "applied {}", applied.theme())?;
    if !applied.written().is_empty() {
        writeln!(out, "wrote {}", names(applied.written()))?;
    }
    if !applied.reloaded().is_empty() {
        writeln!(out, "reloaded {}", names(applied.reloaded()))?;
    }

    for failure in applied.failures() {
        eprintln!("error: {failure}");
    }
    if !applied.failures().is_empty() {
        anyhow::bail!("a reload failed; the files it would have reloaded are written");
    }
    Ok(ExitCode::SUCCESS)
}

/// Says what an apply would do, and with `diff` shows the change line by line.
///
/// Only the targets whose output would change are named. That is the question `--dry-run`
/// answers, and it is why the list is shorter than the one `apply` prints afterwards: an
/// apply writes every target it rendered, whether or not the bytes moved.
fn preview(plan: &Plan, diff: bool) -> anyhow::Result<ExitCode> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "would apply {}", plan.theme())?;

    let mut changing = Vec::new();
    for render in plan.renders() {
        // An output that cannot be read is still going to be replaced, so it is named. It
        // cannot be diffed against, and diffing against nothing would call every line new.
        let current = match vanadis::compare(render) {
            Disk::Same => continue,
            Disk::Missing => Some(String::new()),
            Disk::Different(current) => Some(current),
            Disk::Unreadable(error) => {
                eprintln!("warning: {}: {error}", render.output().display());
                None
            }
        };
        changing.push(render.name().clone());
        if let (true, Some(current)) = (diff, current) {
            write!(
                out,
                "{}",
                vanadis::unified(render.output(), &current, render.contents())
            )?;
        }
    }

    if changing.is_empty() {
        writeln!(out, "nothing would change")?;
    } else {
        writeln!(out, "would write {}", names(&changing))?;
    }
    for render in plan.renders() {
        if !render.reload().is_empty() {
            writeln!(out, "would run: {}", render.reload().join(" "))?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Reports every target whose output has drifted, and every one that will not render.
///
/// Findings go to stdout, one per finding, and any at all exits non-zero. That is what makes
/// it a CI step.
fn check(
    environment: &Environment,
    theme: Option<&str>,
    variant: Option<Variant>,
    only: &[String],
) -> anyhow::Result<ExitCode> {
    let (config, catalog) = load(environment)?;
    let theme = wanted(&config, theme, variant)?;
    let only = selected(only)?;
    let state = State::load(&environment.state_file()?)?;

    let report = vanadis::check(&config, &catalog, theme.as_ref(), state.as_ref(), &only)?;

    let mut out = std::io::stdout().lock();
    for finding in report.findings() {
        writeln!(out, "{finding}")?;
    }
    if !report.findings().is_empty() {
        return Ok(ExitCode::FAILURE);
    }

    let checked = report.checked();
    let plural = if checked == 1 { "" } else { "s" };
    writeln!(out, "checked {checked} target{plural}")?;
    Ok(ExitCode::SUCCESS)
}

/// Target names, separated by spaces.
fn names(targets: &[TargetName]) -> String {
    targets
        .iter()
        .map(TargetName::as_str)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Prints every theme, or every theme written for `variant`.
///
/// A file that is not a theme is reported and skipped: a broken theme costs the user that
/// theme, not the command. So does a state file that cannot be read, which only costs the
/// marker naming the applied theme.
fn list(environment: &Environment, variant: Option<Variant>) -> anyhow::Result<ExitCode> {
    let catalog = Catalog::scan(&environment.themes_dir()?)?;
    for error in catalog.broken() {
        eprintln!("warning: {error}");
    }

    let applied = match State::load(&environment.state_file()?) {
        Ok(state) => state.map(|state| state.theme().clone()),
        Err(error) => {
            eprintln!("warning: {error}");
            None
        }
    };

    let themes: Vec<&Theme> = catalog
        .themes()
        .iter()
        .filter(|theme| variant.is_none_or(|variant| theme.variant() == variant))
        .collect();
    let id = width(themes.iter().map(|theme| theme.id().as_str()));
    let background = width(themes.iter().map(|theme| theme.variant().as_str()));

    let mut out = std::io::stdout().lock();
    for theme in themes {
        let marker = if applied.as_ref() == Some(theme.id()) {
            '*'
        } else {
            ' '
        };
        writeln!(
            out,
            "{marker} {:id$}  {:background$}  {}",
            theme.id().as_str(),
            theme.variant().as_str(),
            theme.name()
        )?;
    }

    Ok(ExitCode::SUCCESS)
}

/// Prints the theme the state file records, and every target that is not on it.
fn current(environment: &Environment) -> anyhow::Result<ExitCode> {
    let path = environment.state_file()?;
    let Some(state) = State::load(&path)? else {
        anyhow::bail!("no theme has been applied yet");
    };

    let mut out = std::io::stdout().lock();
    writeln!(out, "{}", state.theme())?;

    let width = width(state.targets().keys().map(TargetName::as_str));
    for (name, theme) in state.targets() {
        writeln!(out, "{:width$}  {theme}", name.as_str())?;
    }
    Ok(ExitCode::SUCCESS)
}

/// The width of the widest of `values`.
///
/// Byte length is the printed width here: the two columns this pads are an identifier and a
/// variant, both ASCII by construction. The display name is last and never padded.
fn width<'a>(values: impl Iterator<Item = &'a str>) -> usize {
    values.map(str::len).max().unwrap_or_default()
}
