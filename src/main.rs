//! The command line. Subcommands are added by the issues that define them.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::io::Write as _;

use clap::{Parser, Subcommand, ValueEnum};
use vanadis::{Catalog, Environment, State, Theme, Variant};

#[derive(Parser)]
#[command(name = "vanadis", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
        Command::List { variant } => list(&environment, variant.map(Variant::from)),
        Command::Current => current(&environment),
    }
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

/// Prints the theme the state file records.
fn current(environment: &Environment) -> anyhow::Result<()> {
    let path = environment.state_file()?;
    let Some(state) = State::load(&path)? else {
        anyhow::bail!("no theme has been applied yet");
    };
    println!("{}", state.theme().as_str());
    Ok(())
}

/// The width of the widest of `values`.
///
/// Byte length is the printed width here: the two columns this pads are an identifier and a
/// variant, both ASCII by construction. The display name is last and never padded.
fn width<'a>(values: impl Iterator<Item = &'a str>) -> usize {
    values.map(str::len).max().unwrap_or_default()
}
