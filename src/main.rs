//! The command line. Subcommands are added by the issues that define them.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
use vanadis::init::{Answer, Binding, Colour, Draft};
use vanadis::{
    Cache, Catalog, Config, Disk, Environment, Plan, State, System, TargetName, Theme, ThemeId,
    TokenPath, Tokens, Variant,
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
    /// Turn a config file that already exists into a template, a theme and a target.
    Init {
        /// The file to read. It is never modified, and becomes the target's output.
        file: PathBuf,
        /// The theme to write the colours into, instead of being asked.
        #[arg(long, value_name = "ID")]
        theme: Option<String>,
        /// What to call the target, instead of being asked.
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
        /// The background a new theme is written for, instead of being asked.
        #[arg(long)]
        variant: Option<Background>,
    },
    /// Fetch and cache the tinted-theming scheme collection.
    Remote {
        #[command(subcommand)]
        command: RemoteCommand,
    },
    /// Find a cached scheme by identifier, variant, name or author.
    Search {
        /// What to look for, compared without case against every column printed.
        query: String,
    },
    /// Print what a token resolves to, for a tool that can shell out.
    #[command(group(ArgGroup::new("reads").required(true)))]
    Get {
        /// The token path to read.
        #[arg(value_name = "TOKEN", group = "reads")]
        token: Option<String>,
        /// Print every token the theme defines, as JSON.
        #[arg(long, group = "reads")]
        json: bool,
        /// Read this theme instead of the one that was applied.
        #[arg(long)]
        theme: Option<String>,
    },
}

#[derive(Subcommand)]
enum RemoteCommand {
    /// Download the collection, replacing what is cached.
    Update,
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
        Command::Get { token, json, theme } => {
            get(&environment, token.as_deref(), json, theme.as_deref())
        }
        Command::Init {
            file,
            theme,
            name,
            variant,
        } => init(
            &environment,
            &file,
            theme.as_deref(),
            name.as_deref(),
            variant.map(Variant::from),
        ),
        Command::Remote {
            command: RemoteCommand::Update,
        } => remote_update(&environment),
        Command::Search { query } => search(&environment, &query),
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

/// Prints what one token resolves to, or every token the theme defines.
///
/// Only the theme asked for is read. Scanning `themes/` would parse every other file and
/// warn about a broken one, and this is the command a shell prompt hook calls.
///
/// Nothing reaches stdout unless the whole lookup succeeded, so `$(vanadis get role.bg)` is
/// empty exactly when there is no value to substitute.
fn get(
    environment: &Environment,
    token: Option<&str>,
    json: bool,
    theme: Option<&str>,
) -> anyhow::Result<ExitCode> {
    let id = match theme {
        Some(name) => {
            ThemeId::parse(name).with_context(|| format!("`{name}` is not a theme identifier"))?
        }
        None => State::load(&environment.state_file()?)?
            .context("no theme has been applied yet, so pass --theme")?
            .theme()
            .clone(),
    };
    let theme = vanadis::catalog::load(&environment.themes_dir()?, &id)?;

    let mut out = std::io::stdout().lock();
    if json {
        let tokens: BTreeMap<&str, &str> = theme
            .tokens()
            .iter()
            .map(|(path, value)| (path.as_str(), value))
            .collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&tokens)?)?;
        return Ok(ExitCode::SUCCESS);
    }

    // The `reads` group has already rejected neither being given.
    let token = token.context("name a token, or pass --json")?;
    let path = TokenPath::parse(token).with_context(|| format!("`{token}` is not a token path"))?;
    let value = theme
        .tokens()
        .get(&path)
        .with_context(|| format!("`{id}` does not define `{path}`"))?;
    writeln!(out, "{value}")?;
    Ok(ExitCode::SUCCESS)
}

/// The width of the widest of `values`.
///
/// Byte length is the printed width here: the two columns this pads are an identifier and a
/// variant, both ASCII by construction. The display name is last and never padded.
fn width<'a>(values: impl Iterator<Item = &'a str>) -> usize {
    values.map(str::len).max().unwrap_or_default()
}

/// Turns a config file that already exists into a template, a theme and a target.
///
/// `docs/init.md` decides the dialogue and decides that the file being read is never
/// modified. Nothing at all is written until the template it built renders back to that
/// file byte for byte, which is [`vanadis::init::plan`]'s to establish.
fn init(
    environment: &Environment,
    file: &Path,
    theme: Option<&str>,
    name: Option<&str>,
    variant: Option<Variant>,
) -> anyhow::Result<ExitCode> {
    let directory = environment.config_dir()?;
    let output = std::path::absolute(file)
        .with_context(|| format!("{}: cannot be resolved", file.display()))?;
    let source = std::fs::read_to_string(&output)
        .with_context(|| format!("{}: cannot be read", output.display()))?;

    let config_path = directory.join("config.toml");
    let written = read(&config_path)?.unwrap_or_default();
    let config = Config::parse(&config_path, &written, &directory, environment.home())?;
    if let Some(target) = config
        .targets()
        .iter()
        .find(|target| target.output() == output)
    {
        anyhow::bail!(
            "{} is already the output of target `{}`",
            output.display(),
            target.name()
        );
    }

    let scan = vanadis::init::scan(&source);
    let mut dialogue = Dialogue::new();
    Dialogue::found(&scan)?;

    let id = match theme {
        Some(name) => {
            ThemeId::parse(name).with_context(|| format!("`{name}` is not a theme identifier"))?
        }
        None => dialogue.theme(&environment.themes_dir()?)?,
    };
    let existing = read(&directory.join("themes").join(format!("{id}.toml")))?;
    let loaded = existing
        .as_deref()
        .map(|source| Theme::parse(&directory.join(format!("themes/{id}.toml")), source))
        .transpose()?;
    let variant = match (&loaded, variant) {
        (Some(theme), _) => theme.variant(),
        (None, Some(variant)) => variant,
        (None, None) => dialogue.variant()?,
    };

    let name = match name {
        Some(name) => {
            TargetName::parse(name).with_context(|| format!("`{name}` is not a target name"))?
        }
        None => dialogue.name(&output, &config)?,
    };
    if config.targets().iter().any(|target| *target.name() == name) {
        anyhow::bail!("config.toml already has a target named `{name}`");
    }

    let known = loaded
        .as_ref()
        .map_or_else(Tokens::default, |theme| theme.tokens().clone());
    let (bindings, tokens) = dialogue.colours(&source, &scan, &known)?;

    let plan = vanadis::init::plan(Draft {
        directory: &directory,
        home: environment.home(),
        name: &name,
        output: &output,
        source: &source,
        id: &id,
        variant,
        existing: existing.as_deref(),
        config: &written,
        bindings: &bindings,
        tokens: &tokens,
    })?;
    plan.commit()?;

    let home = environment.home();
    let mut out = std::io::stdout().lock();
    writeln!(out, "\nwrote:")?;
    writeln!(out, "  {}", shown(plan.theme().path(), home))?;
    writeln!(out, "  {}", shown(plan.template().path(), home))?;
    writeln!(
        out,
        "  {}   (added 1 target)",
        shown(plan.config().path(), home)
    )?;

    if !plan.missing().is_empty() {
        writeln!(
            out,
            "\ncore tokens still undefined ({}):",
            plan.missing().len()
        )?;
        for line in plan.missing().chunks(4) {
            let paths: Vec<&str> = line.iter().map(TokenPath::as_str).collect();
            writeln!(out, "  {}", paths.join(" "))?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Downloads the scheme collection and replaces the cache with it.
///
/// This is the one command that prints its own failure. `docs/schemes.md` decides that a
/// failed fetch closes by saying the cached schemes are still readable, which is only true
/// when there is a cache, and no other command has a closing line that depends on disk.
fn remote_update(environment: &Environment) -> anyhow::Result<ExitCode> {
    let schemes = environment.schemes_dir()?;
    let installed = match vanadis::remote::update(&schemes) {
        Ok(installed) => installed,
        Err(error) => {
            eprintln!("error: {error}");
            let mut cause: &dyn std::error::Error = &error;
            while let Some(next) = cause.source() {
                eprintln!("  caused by: {next}");
                cause = next;
            }
            if schemes.is_dir() {
                eprintln!("the cached schemes are unchanged; `vanadis search` still reads them");
            }
            return Ok(ExitCode::FAILURE);
        }
    };

    let counts: Vec<String> = System::ALL
        .iter()
        .filter(|system| installed.count(**system) > 0)
        .map(|system| format!("{} {system}", installed.count(*system)))
        .collect();
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "fetched {} schemes: {}",
        installed.total(),
        counts.join(", ")
    )?;
    Ok(ExitCode::SUCCESS)
}

/// Prints every cached scheme `query` matches.
///
/// A scheme that does not load is reported and skipped, which is what `list` does with a
/// theme that does not load and for the same reason.
fn search(environment: &Environment, query: &str) -> anyhow::Result<ExitCode> {
    let cache = Cache::scan(&environment.schemes_dir()?)?;
    for error in cache.broken() {
        eprintln!("warning: {error}");
    }

    let found = cache.search(query);
    if found.is_empty() {
        return Ok(ExitCode::FAILURE);
    }

    let qualified: Vec<String> = found.iter().map(|scheme| scheme.qualified()).collect();
    let id = width(qualified.iter().map(String::as_str));
    let background = width(found.iter().map(|scheme| scheme.variant().as_str()));
    let name = width(found.iter().map(|scheme| scheme.name()));

    let mut out = std::io::stdout().lock();
    for (scheme, qualified) in found.iter().zip(&qualified) {
        writeln!(
            out,
            "  {qualified:id$}  {:background$}  {:name$}  {}",
            scheme.variant().as_str(),
            scheme.name(),
            scheme.author()
        )?;
    }

    Ok(ExitCode::SUCCESS)
}

/// `path` as it is printed, with the home directory written as `~`.
fn shown(path: &Path, home: Option<&Path>) -> String {
    home.and_then(|home| path.strip_prefix(home).ok())
        .map_or_else(
            || path.display().to_string(),
            |rest| format!("~/{}", rest.display()),
        )
}

/// The contents of `path`, or `None` when there is no such file.
fn read(path: &Path) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("{}: cannot be read", path.display())),
    }
}

/// The questions `init` asks, and the answers read back off stdin.
///
/// Reaching the end of the input answers every remaining question with nothing, so a run
/// with no input attached skips every value rather than hanging.
struct Dialogue {
    input: std::io::Lines<std::io::StdinLock<'static>>,
}

impl Dialogue {
    fn new() -> Self {
        Self {
            input: std::io::stdin().lock().lines(),
        }
    }

    /// Prints `prompt` and reads one line back.
    fn ask(&mut self, prompt: &str) -> anyhow::Result<String> {
        let mut out = std::io::stdout().lock();
        write!(out, "{prompt}")?;
        out.flush()?;
        drop(out);
        Ok(self.input.next().transpose()?.unwrap_or_default())
    }

    /// Says what the file holds, and names every notation that cannot be substituted.
    fn found(scan: &vanadis::Scan) -> anyhow::Result<()> {
        let mut out = std::io::stdout().lock();
        let count = scan.colours().len();
        writeln!(out, "found {count} hex {}", plural(count, "value"))?;
        if !scan.unhandled().is_empty() {
            writeln!(out, "\nunhandled colour notations:")?;
            for unhandled in scan.unhandled() {
                writeln!(out, "  {unhandled}")?;
            }
        }
        writeln!(out)?;
        Ok(())
    }

    /// Asks which theme the colours go into.
    ///
    /// A directory holding exactly one theme offers it, which is what makes the second file
    /// onwards a return key.
    fn theme(&mut self, themes: &Path) -> anyhow::Result<ThemeId> {
        let only = Catalog::scan(themes)
            .ok()
            .and_then(|catalog| match catalog.themes() {
                [theme] => Some(theme.id().clone()),
                _ => None,
            });
        let prompt = only
            .as_ref()
            .map_or_else(|| "theme? > ".to_owned(), |id| format!("theme? [{id}] > "));
        loop {
            let answer = self.ask(&prompt)?;
            let answer = answer.trim();
            if answer.is_empty() {
                if let Some(id) = only {
                    return Ok(id);
                }
            } else if let Some(id) = ThemeId::parse(answer) {
                return Ok(id);
            }
            anyhow::ensure!(!answer.is_empty(), "name a theme, or pass --theme");
            eprintln!("`{answer}` is not a theme identifier: lowercase, digits, internal hyphens");
        }
    }

    /// Asks which background a new theme is written for.
    fn variant(&mut self) -> anyhow::Result<Variant> {
        loop {
            let answer = self.ask("variant? [dark/light] > ")?;
            let answer = answer.trim();
            if let Some(variant) = Variant::parse(answer) {
                return Ok(variant);
            }
            anyhow::ensure!(!answer.is_empty(), "say dark or light, or pass --variant");
            eprintln!("`{answer}` is neither dark nor light");
        }
    }

    /// Asks what the target is called, offering the name the path suggests.
    fn name(&mut self, output: &Path, config: &Config) -> anyhow::Result<TargetName> {
        let taken = |name: &TargetName| config.targets().iter().any(|target| target.name() == name);
        let suggested = vanadis::init::suggest(output).filter(|name| !taken(name));
        let prompt = suggested.as_ref().map_or_else(
            || "target name? > ".to_owned(),
            |name| format!("target name? [{name}] > "),
        );
        loop {
            let answer = self.ask(&prompt)?;
            let answer = answer.trim();
            if answer.is_empty() {
                if let Some(name) = suggested {
                    return Ok(name);
                }
            } else if let Some(name) = TargetName::parse(answer) {
                return Ok(name);
            }
            anyhow::ensure!(!answer.is_empty(), "name the target, or pass --name");
            eprintln!("`{answer}` is not a target name: lowercase, digits, internal hyphens");
        }
    }

    /// Walks every value the file holds, and returns what each occurrence is bound to.
    ///
    /// A value the theme already names offers that name. An answer naming more than one
    /// token opens the per-occurrence pass `docs/init.md` decides on.
    fn colours(
        &mut self,
        source: &str,
        scan: &vanadis::Scan,
        known: &Tokens,
    ) -> anyhow::Result<(Vec<Binding>, BTreeMap<TokenPath, String>)> {
        let mut bindings = Vec::new();
        let mut tokens: BTreeMap<TokenPath, String> = BTreeMap::new();

        for colour in scan.colours() {
            for (occurrence, token) in colour
                .occurrences()
                .iter()
                .zip(self.colour(source, colour, known)?)
            {
                let Some(token) = token else { continue };
                match (known.get(&token), tokens.get(&token).map(String::as_str)) {
                    (Some(value), _) | (_, Some(value)) if value != colour.value() => {
                        anyhow::bail!(
                            "`{token}` is already {value}, so it cannot also be {}",
                            colour.value()
                        );
                    }
                    (Some(_), _) => {}
                    _ => {
                        tokens.insert(token.clone(), colour.value().to_owned());
                    }
                }
                bindings.push(Binding::new(occurrence, token));
            }
        }
        Ok((bindings, tokens))
    }

    /// Asks about one value, and returns the token each of its occurrences takes.
    fn colour(
        &mut self,
        source: &str,
        colour: &Colour,
        known: &Tokens,
    ) -> anyhow::Result<Vec<Option<TokenPath>>> {
        let count = colour.occurrences().len();
        let default = offered(known, colour.value());
        let prompt = format!(
            "  {}  {count} {}   token name?{} > ",
            colour.value(),
            plural(count, "occurrence"),
            default
                .as_ref()
                .map_or_else(String::new, |path| format!(" [{path}]"))
        );

        let names = loop {
            let answer = self.ask(&prompt)?;
            match vanadis::init::answer(&answer) {
                Ok(Answer::Skip) => return Ok(vec![None; count]),
                Ok(Answer::Default) => match default {
                    Some(path) => break vec![path],
                    None => return Ok(vec![None; count]),
                },
                Ok(Answer::Tokens(names)) => break names,
                Err(error) => eprintln!("{error}"),
            }
        };

        match names.as_slice() {
            [only] => Ok(vec![Some(only.clone()); count]),
            several => colour
                .occurrences()
                .iter()
                .map(|occurrence| self.which(source, occurrence, several))
                .collect(),
        }
    }

    /// Asks which of `names` one occurrence takes, showing the line it sits on.
    fn which(
        &mut self,
        source: &str,
        occurrence: &vanadis::Occurrence,
        names: &[TokenPath],
    ) -> anyhow::Result<Option<TokenPath>> {
        let short: Vec<&str> = names.iter().map(TokenPath::leaf).collect();
        let mut out = std::io::stdout().lock();
        writeln!(
            out,
            "    line {}  {}",
            occurrence.line(),
            occurrence.context(source)
        )?;
        drop(out);

        let prompt = format!("      which? [{}] > ", short.join("/"));
        loop {
            let answer = self.ask(&prompt)?;
            let answer = answer.trim();
            if answer.is_empty() {
                return Ok(names.first().cloned());
            }
            if answer == "-" {
                return Ok(None);
            }
            if let Some(path) = names
                .iter()
                .find(|path| path.as_str() == answer || path.leaf() == answer)
            {
                return Ok(Some(path.clone()));
            }
            eprintln!("`{answer}` is not one of {}", short.join(", "));
        }
    }
}

/// The token to offer for `value`: one the theme already resolves to it.
///
/// A `role` token wins over any other, and ties break in path order, so the name a template
/// should be reading is the one the return key takes.
fn offered(known: &Tokens, value: &str) -> Option<TokenPath> {
    known
        .iter()
        .filter(|(_, resolved)| *resolved == value)
        .min_by_key(|(path, _)| (path.root() != "role", path.as_str().to_owned()))
        .map(|(path, _)| path.clone())
}

/// `word`, pluralised for `count`.
fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}
