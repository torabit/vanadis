//! The command line. Subcommands are added by the issues that define them.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::io::Write as _;

use anyhow::Context as _;
use clap::{Parser, Subcommand, ValueEnum};
use vanadis::{Catalog, Config, Environment, State, TargetName, Theme, ThemeId, Variant};

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let environment = Environment::read();

    match cli.command {
        Command::Apply {
            theme,
            variant,
            only,
        } => apply(
            &environment,
            theme.as_deref(),
            variant.map(Variant::from),
            &only,
        ),
        Command::List { variant } => list(&environment, variant.map(Variant::from)),
        Command::Current => current(&environment),
    }
}

/// Renders every target, or the ones `only` names, from one theme.
fn apply(
    environment: &Environment,
    theme: Option<&str>,
    variant: Option<Variant>,
    only: &[String],
) -> anyhow::Result<()> {
    let config = Config::load(&environment.config_dir()?, environment.home())?;
    let catalog = Catalog::scan(&environment.themes_dir()?)?;
    for error in catalog.broken() {
        eprintln!("warning: {error}");
    }

    let theme = match (theme, variant) {
        (Some(name), _) => {
            ThemeId::parse(name).with_context(|| format!("`{name}` is not a theme identifier"))?
        }
        (None, Some(variant)) => config
            .auto()
            .context("config.toml has no [auto] table, so name a theme")?
            .theme(variant)
            .clone(),
        (None, None) => anyhow::bail!("name a theme, or pass --variant"),
    };

    let only: Vec<TargetName> = only
        .iter()
        .map(|name| {
            TargetName::parse(name).with_context(|| format!("`{name}` is not a target name"))
        })
        .collect::<anyhow::Result<_>>()?;

    let applied = vanadis::apply(&config, &catalog, &theme, &only, &environment.state_file()?)?;

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
    Ok(())
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
fn list(environment: &Environment, variant: Option<Variant>) -> anyhow::Result<()> {
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

    Ok(())
}

/// Prints the theme the state file records, and every target that is not on it.
fn current(environment: &Environment) -> anyhow::Result<()> {
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
    Ok(())
}

/// The width of the widest of `values`.
///
/// Byte length is the printed width here: the two columns this pads are an identifier and a
/// variant, both ASCII by construction. The display name is last and never padded.
fn width<'a>(values: impl Iterator<Item = &'a str>) -> usize {
    values.map(str::len).max().unwrap_or_default()
}
