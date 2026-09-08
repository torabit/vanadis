//! The config file: what to render, where to write it, and what to run afterwards.
//!
//! `docs/config.md` decides the format. Every problem the file has is reported in one pass.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toml_edit::{Document, Item, Key, Table, TableLike};

use crate::theme::{ThemeId, Variant};
use crate::token::is_segment;

/// A target's identifier: what `check` prints and what an error message names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetName(String);

impl TargetName {
    /// Parses `text` as a target name, returning `None` when it is not a single segment.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        is_segment(text).then(|| Self(text.to_owned()))
    }

    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TargetName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A shell that sources a target's output.
///
/// `vanadis hook <shell>` prints the snippet that sources every target carrying this shell,
/// and sources it again when its output changes. See `docs/hook.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shell {
    /// zsh, which reads an mtime with `zstat` and hooks `precmd`.
    Zsh,
    /// fish, which reads an mtime with `path mtime` and hooks the `fish_prompt` event.
    Fish,
    /// bash, which forks `stat` and appends to `PROMPT_COMMAND`.
    Bash,
}

impl Shell {
    /// Every shell `hook` emits for, in the order `--help` lists them.
    pub const ALL: [Self; 3] = [Self::Zsh, Self::Fish, Self::Bash];

    /// Parses `text` as a shell name, returning `None` when it names none of them.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            "bash" => Some(Self::Bash),
            _ => None,
        }
    }

    /// The name the config file and the command line both write.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Bash => "bash",
        }
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The themes a bare `vanadis apply --variant` resolves through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auto {
    light: ThemeId,
    dark: ThemeId,
}

impl Auto {
    /// The theme this table names for `variant`.
    #[must_use]
    pub fn theme(&self, variant: Variant) -> &ThemeId {
        match variant {
            Variant::Dark => &self.dark,
            Variant::Light => &self.light,
        }
    }
}

/// The themes `vanadis cycle` steps through, in the order the file writes them.
///
/// At least two, and no repeats. Both are the parser's to enforce and both follow from how
/// [`Cycle::next`] finds where it is: it looks the applied theme up in the list rather than
/// reading an index out of the state file, and a list that is empty or that writes one theme
/// twice has no answer to give it.
///
/// The head is held apart from the tail so that "at least two" is true of the type. Nothing
/// here can be asked for an element that is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle {
    head: ThemeId,
    tail: Vec<ThemeId>,
}

impl Cycle {
    /// Every theme, in the order the file writes them.
    fn themes(&self) -> impl Iterator<Item = &ThemeId> {
        std::iter::once(&self.head).chain(&self.tail)
    }

    /// The theme that follows `current`, wrapping at the end of the list.
    ///
    /// `current` is what the state file records, and `None` is a machine that has applied
    /// nothing yet. Both that and a theme the list does not name answer with the first entry:
    /// a cycle that cannot say where it is starts at the beginning, which is what a user who
    /// has just written the table is asking for.
    #[must_use]
    pub fn next(&self, current: Option<&ThemeId>) -> &ThemeId {
        let Some(at) = current.and_then(|theme| self.themes().position(|entry| entry == theme))
        else {
            return &self.head;
        };
        self.themes().nth(at + 1).unwrap_or(&self.head)
    }
}

/// One target's own themes, which win over the theme being applied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Themes(BTreeMap<Variant, ThemeId>);

impl Themes {
    /// The theme this table names for `variant`, or `None` when it names none.
    #[must_use]
    pub fn theme(&self, variant: Variant) -> Option<&ThemeId> {
        self.0.get(&variant)
    }
}

/// One template, where it is written, and what runs afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    name: TargetName,
    template: PathBuf,
    output: PathBuf,
    reload: Vec<String>,
    shell: Option<Shell>,
    themes: Option<Themes>,
}

impl Target {
    /// The target's identifier.
    #[must_use]
    pub fn name(&self) -> &TargetName {
        &self.name
    }

    /// The template to render.
    #[must_use]
    pub fn template(&self) -> &Path {
        &self.template
    }

    /// The file to write.
    #[must_use]
    pub fn output(&self) -> &Path {
        &self.output
    }

    /// What to run after the write, empty when nothing runs.
    #[must_use]
    pub fn reload(&self) -> &[String] {
        &self.reload
    }

    /// The shell whose `hook` sources this output, or `None` when no shell does.
    #[must_use]
    pub fn shell(&self) -> Option<Shell> {
        self.shell
    }

    /// The target's own themes, or `None` when it follows the theme being applied.
    #[must_use]
    pub fn themes(&self) -> Option<&Themes> {
        self.themes.as_ref()
    }
}

/// The config file, parsed and with its paths resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    auto: Option<Auto>,
    cycle: Option<Cycle>,
    targets: Vec<Target>,
}

/// One thing wrong with a config file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Problem {
    /// A key holding the wrong kind of value.
    #[error("line {line}: {key} is a {found}, and the format wants something else")]
    Type {
        /// The key, qualified by the table it sits in.
        key: String,
        /// The line the key is written on.
        line: usize,
        /// The TOML type found instead.
        found: &'static str,
    },
    /// A required key that is not written.
    #[error("line {line}: {key} is required")]
    Missing {
        /// The key, qualified by the table it sits in.
        key: String,
        /// The line the table it belongs to opens on.
        line: usize,
    },
    /// A key the format does not define.
    #[error("line {line}: the format has no key {key}")]
    Unknown {
        /// The key, qualified by the table it sits in.
        key: String,
        /// The line the key is written on.
        line: usize,
    },
    /// A target name that is not a single segment.
    #[error("line {line}: `{name}` is not a target name: lowercase, digits, internal hyphens")]
    Name {
        /// The name, as the file writes it.
        name: String,
        /// The line the name is written on.
        line: usize,
    },
    /// Two targets written with the same name.
    #[error("line {line}: `{name}` names two targets")]
    Duplicate {
        /// The name both targets carry.
        name: String,
        /// The line the second one is written on.
        line: usize,
    },
    /// One theme written twice in the cycle.
    #[error("line {line}: `{theme}` is written twice in cycle.themes")]
    Repeated {
        /// The theme both entries name.
        theme: String,
        /// The line the table is written on.
        line: usize,
    },
    /// A cycle of fewer than two themes.
    #[error("line {line}: cycle.themes names fewer than two themes")]
    Short {
        /// The line the table is written on.
        line: usize,
    },
    /// A theme name that is not a theme identifier.
    #[error("line {line}: {key} is `{value}`, which is not a theme identifier")]
    Theme {
        /// The key, qualified by the table it sits in.
        key: String,
        /// The line the key is written on.
        line: usize,
        /// The value, as the file writes it.
        value: String,
    },
    /// A `shell` naming a shell `hook` does not emit for.
    #[error("line {line}: targets.shell is `{value}`, and the shells are zsh, fish and bash")]
    Shell {
        /// The value, as the file writes it.
        value: String,
        /// The line the key is written on.
        line: usize,
    },
    /// A path starting with `~` when there is no home directory to expand it to.
    #[error("line {line}: {key} starts with `~`, and there is no home directory")]
    Home {
        /// The key, qualified by the table it sits in.
        key: String,
        /// The line the key is written on.
        line: usize,
    },
}

impl Problem {
    /// The line the problem is on, which is what problems are ordered by.
    #[must_use]
    pub fn line(&self) -> usize {
        match self {
            Self::Type { line, .. }
            | Self::Missing { line, .. }
            | Self::Unknown { line, .. }
            | Self::Name { line, .. }
            | Self::Duplicate { line, .. }
            | Self::Repeated { line, .. }
            | Self::Short { line, .. }
            | Self::Theme { line, .. }
            | Self::Shell { line, .. }
            | Self::Home { line, .. } => *line,
        }
    }
}

/// Loading the config failed.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The file could not be read.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The file the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// The file is not valid TOML.
    #[error("{}: {source}", .path.display())]
    Syntax {
        /// The file the parse failed in.
        path: PathBuf,
        /// The parse error.
        source: toml_edit::TomlError,
    },
    /// The file parses as TOML but is not a valid config.
    #[error("{}: invalid config\n{}", .path.display(), listed(.problems))]
    Invalid {
        /// The file the problems were found in.
        path: PathBuf,
        /// Every problem, in source order.
        problems: Vec<Problem>,
    },
}

/// Every problem on its own line, indented.
fn listed(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(|problem| format!("  {problem}"))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Config {
    /// Reads `config.toml` out of `directory`.
    ///
    /// `home` expands a leading `~`, and `directory` is what a relative path resolves
    /// against.
    ///
    /// # Errors
    ///
    /// Returns the read error, or every problem the file has, in one pass.
    pub fn load(directory: &Path, home: Option<&Path>) -> Result<Self, ConfigError> {
        let path = directory.join("config.toml");
        let source = std::fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.clone(),
            source,
        })?;
        Self::parse(&path, &source, directory, home)
    }

    /// Parses `source` as the config stored at `path`.
    ///
    /// # Errors
    ///
    /// Returns every problem found, in one pass.
    pub fn parse(
        path: &Path,
        source: &str,
        directory: &Path,
        home: Option<&Path>,
    ) -> Result<Self, ConfigError> {
        let document = Document::parse(source).map_err(|source| ConfigError::Syntax {
            path: path.to_owned(),
            source,
        })?;

        let mut parse = Parse {
            source,
            directory,
            home,
            problems: Vec::new(),
        };
        let config = parse.config(document.as_table());
        parse.problems.sort_by_key(Problem::line);

        if parse.problems.is_empty() {
            Ok(config)
        } else {
            Err(ConfigError::Invalid {
                path: path.to_owned(),
                problems: parse.problems,
            })
        }
    }

    /// The themes `--variant` resolves through, when the file names them.
    #[must_use]
    pub fn auto(&self) -> Option<&Auto> {
        self.auto.as_ref()
    }

    /// The themes `cycle` steps through, when the file names them.
    #[must_use]
    pub fn cycle(&self) -> Option<&Cycle> {
        self.cycle.as_ref()
    }

    /// Every target, in the order the file writes them.
    #[must_use]
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }
}

/// Reads the document, collecting every problem rather than stopping at the first.
struct Parse<'a> {
    source: &'a str,
    directory: &'a Path,
    home: Option<&'a Path>,
    problems: Vec<Problem>,
}

impl Parse<'_> {
    /// Reads the whole document.
    fn config(&mut self, table: &Table) -> Config {
        let mut auto = None;
        let mut cycle = None;
        let mut targets = Vec::new();
        let mut names = BTreeSet::new();

        for (key, item) in table {
            let line = self.line_of(table.key(key));
            match key {
                "auto" => {
                    if let Some(inner) = item.as_table_like() {
                        auto = self.auto(inner, line);
                    } else {
                        self.wrong_type("auto", line, item);
                    }
                }
                "cycle" => {
                    if let Some(inner) = item.as_table_like() {
                        cycle = self.cycle(inner, line);
                    } else {
                        self.wrong_type("cycle", line, item);
                    }
                }
                "targets" => {
                    let Some(array) = item.as_array_of_tables() else {
                        self.wrong_type("targets", line, item);
                        continue;
                    };
                    for entry in array {
                        let header = entry.span().map_or(line, |span| self.line_at(span.start));
                        let Some(target) = self.target(entry, header) else {
                            continue;
                        };
                        if names.insert(target.name.clone()) {
                            targets.push(target);
                        } else {
                            self.problems.push(Problem::Duplicate {
                                name: target.name.as_str().to_owned(),
                                line: self.line_of(entry.key("name")),
                            });
                        }
                    }
                }
                other => self.problems.push(Problem::Unknown {
                    key: other.to_owned(),
                    line,
                }),
            }
        }

        Config {
            auto,
            cycle,
            targets,
        }
    }

    /// Reads `[auto]`, which names a theme for each mode.
    fn auto(&mut self, table: &dyn TableLike, line: usize) -> Option<Auto> {
        for (key, item) in table.iter() {
            if key != "light" && key != "dark" {
                self.problems.push(Problem::Unknown {
                    key: format!("auto.{key}"),
                    line: self.line_of(table.key(key)),
                });
            }
            let _ = item;
        }

        let light = self.theme(table, "auto", "light", line);
        let dark = self.theme(table, "auto", "dark", line);
        Some(Auto {
            light: light?,
            dark: dark?,
        })
    }

    /// Reads `[cycle]`, whose one key is the list of themes to step through.
    ///
    /// Every entry is reported on its own, the way the rest of this file reports problems, so
    /// a table with two things wrong with it says both. The `Cycle` built from the survivors
    /// is only ever returned alongside an empty problem list, so it is never a cycle stepping
    /// through an order the file does not write.
    fn cycle(&mut self, table: &dyn TableLike, line: usize) -> Option<Cycle> {
        for (key, item) in table.iter() {
            if key != "themes" {
                self.problems.push(Problem::Unknown {
                    key: format!("cycle.{key}"),
                    line: self.line_of(table.key(key)),
                });
            }
            let _ = item;
        }

        let Some(item) = table.get("themes") else {
            self.problems.push(Problem::Missing {
                key: "cycle.themes".to_owned(),
                line,
            });
            return None;
        };
        let at = self.line_of(table.key("themes"));
        let Some(array) = item.as_array() else {
            self.wrong_type("cycle.themes", at, item);
            return None;
        };

        // Shortness is read off what the file writes, not off what parsed. An entry that is
        // not a theme identifier is already reported as itself, and counting the survivors
        // would report the same defect a second time as a cycle that is too short.
        if array.len() < 2 {
            self.problems.push(Problem::Short { line: at });
        }

        let mut themes = Vec::new();
        let mut written = BTreeSet::new();
        for value in array {
            let Some(text) = value.as_str() else {
                self.problems.push(Problem::Type {
                    key: "cycle.themes".to_owned(),
                    line: at,
                    found: value.type_name(),
                });
                continue;
            };
            let Some(theme) = ThemeId::parse(text) else {
                self.problems.push(Problem::Theme {
                    key: "cycle.themes".to_owned(),
                    line: at,
                    value: text.to_owned(),
                });
                continue;
            };
            if written.insert(theme.clone()) {
                themes.push(theme);
            } else {
                self.problems.push(Problem::Repeated {
                    theme: text.to_owned(),
                    line: at,
                });
            }
        }

        let mut themes = themes.into_iter();
        let (Some(head), Some(second)) = (themes.next(), themes.next()) else {
            return None;
        };
        Some(Cycle {
            head,
            tail: std::iter::once(second).chain(themes).collect(),
        })
    }

    /// Reads one `[[targets]]` entry, whose header is on `line`.
    fn target(&mut self, table: &Table, line: usize) -> Option<Target> {
        let mut themes = None;

        for (key, item) in table {
            let at = self.line_of(table.key(key));
            match key {
                "name" | "template" | "output" | "reload" | "shell" => {}
                "themes" => {
                    if let Some(inner) = item.as_table_like() {
                        themes = Some(self.themes(inner));
                    } else {
                        self.wrong_type("targets.themes", at, item);
                    }
                }
                other => self.problems.push(Problem::Unknown {
                    key: format!("targets.{other}"),
                    line: at,
                }),
            }
        }

        let name = self.name(table, line);
        let template = self.path(table, "template", line);
        let output = self.path(table, "output", line);
        let reload = self.reload(table);
        let shell = self.shell(table);

        Some(Target {
            name: name?,
            template: template?,
            output: output?,
            reload,
            shell,
            themes,
        })
    }

    /// Reads a target's `name`.
    fn name(&mut self, table: &Table, line: usize) -> Option<TargetName> {
        let text = self.string(table, "targets", "name", line)?;
        let at = self.line_of(table.key("name"));
        let name = TargetName::parse(&text);
        if name.is_none() {
            self.problems.push(Problem::Name {
                name: text,
                line: at,
            });
        }
        name
    }

    /// Reads a target's `reload`, which is empty when the target has none.
    fn reload(&mut self, table: &Table) -> Vec<String> {
        let Some(item) = table.get("reload") else {
            return Vec::new();
        };
        let at = self.line_of(table.key("reload"));
        let Some(array) = item.as_array() else {
            self.wrong_type("targets.reload", at, item);
            return Vec::new();
        };

        let mut reload = Vec::new();
        for value in array {
            match value.as_str() {
                Some(argument) => reload.push(argument.to_owned()),
                None => self.problems.push(Problem::Type {
                    key: "targets.reload".to_owned(),
                    line: at,
                    found: value.type_name(),
                }),
            }
        }
        reload
    }

    /// Reads a target's `shell`, which is `None` when the target has none.
    ///
    /// A value that names no shell is a problem rather than a `None`, so a typo is a config
    /// error and not a target that silently never sources.
    fn shell(&mut self, table: &Table) -> Option<Shell> {
        let item = table.get("shell")?;
        let at = self.line_of(table.key("shell"));
        let Some(text) = item.as_str() else {
            self.wrong_type("targets.shell", at, item);
            return None;
        };

        let shell = Shell::parse(text);
        if shell.is_none() {
            self.problems.push(Problem::Shell {
                value: text.to_owned(),
                line: at,
            });
        }
        shell
    }

    /// Reads a target's `themes`, which names a theme for one mode or both.
    fn themes(&mut self, table: &dyn TableLike) -> Themes {
        let mut themes = BTreeMap::new();
        for (key, item) in table.iter() {
            let at = self.line_of(table.key(key));
            let Some(variant) = Variant::parse(key) else {
                self.problems.push(Problem::Unknown {
                    key: format!("targets.themes.{key}"),
                    line: at,
                });
                continue;
            };
            let qualified = format!("targets.themes.{key}");
            if let Some(theme) = self.identifier(item, &qualified, at) {
                themes.insert(variant, theme);
            }
        }
        Themes(themes)
    }

    /// Reads a required theme name out of `table`.
    fn theme(
        &mut self,
        table: &dyn TableLike,
        prefix: &str,
        key: &str,
        line: usize,
    ) -> Option<ThemeId> {
        let qualified = format!("{prefix}.{key}");
        let Some(item) = table.get(key) else {
            self.problems.push(Problem::Missing {
                key: qualified,
                line,
            });
            return None;
        };
        let at = self.line_of(table.key(key));
        self.identifier(item, &qualified, at)
    }

    /// Reads `item` as a theme identifier.
    fn identifier(&mut self, item: &Item, key: &str, line: usize) -> Option<ThemeId> {
        let Some(text) = item.as_str() else {
            self.wrong_type(key, line, item);
            return None;
        };
        let theme = ThemeId::parse(text);
        if theme.is_none() {
            self.problems.push(Problem::Theme {
                key: key.to_owned(),
                line,
                value: text.to_owned(),
            });
        }
        theme
    }

    /// Reads a required path, expanding `~` and resolving a relative path.
    fn path(&mut self, table: &Table, key: &str, line: usize) -> Option<PathBuf> {
        let text = self.string(table, "targets", key, line)?;
        let at = self.line_of(table.key(key));
        let qualified = format!("targets.{key}");

        let Some(rest) = text.strip_prefix('~') else {
            let path = Path::new(&text);
            return Some(if path.is_absolute() {
                path.to_owned()
            } else {
                self.directory.join(path)
            });
        };

        // `~user` is someone else's home directory, which the format does not promise to
        // expand, so it stays a literal path.
        if !rest.is_empty() && !rest.starts_with('/') {
            return Some(self.directory.join(&text));
        }

        let Some(home) = self.home else {
            self.problems.push(Problem::Home {
                key: qualified,
                line: at,
            });
            return None;
        };
        Some(home.join(rest.trim_start_matches('/')))
    }

    /// Reads a required string out of `table`.
    fn string(&mut self, table: &Table, prefix: &str, key: &str, line: usize) -> Option<String> {
        let qualified = format!("{prefix}.{key}");
        let Some(item) = table.get(key) else {
            self.problems.push(Problem::Missing {
                key: qualified,
                line,
            });
            return None;
        };
        let at = self.line_of(table.key(key));
        let Some(text) = item.as_str() else {
            self.wrong_type(&qualified, at, item);
            return None;
        };
        Some(text.to_owned())
    }

    /// Records a value of the wrong kind.
    fn wrong_type(&mut self, key: &str, line: usize, item: &Item) {
        self.problems.push(Problem::Type {
            key: key.to_owned(),
            line,
            found: item.type_name(),
        });
    }

    /// The line `key` is written on, or 0 when the document does not say.
    fn line_of(&self, key: Option<&Key>) -> usize {
        key.and_then(Key::span)
            .map_or(0, |span| self.line_at(span.start))
    }

    /// The 1-based line `offset` falls on.
    fn line_at(&self, offset: usize) -> usize {
        self.source[..offset]
            .bytes()
            .filter(|&b| b == b'\n')
            .count()
            + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: &str = "[[targets]]\nname = \"bat\"\ntemplate = \"t.in\"\noutput = \"o\"\n";

    fn parse(source: &str) -> Result<Config, ConfigError> {
        Config::parse(
            Path::new("/cfg/config.toml"),
            source,
            Path::new("/cfg"),
            Some(Path::new("/home/ada")),
        )
    }

    fn problems(source: &str) -> Vec<Problem> {
        match parse(source) {
            Err(ConfigError::Invalid { problems, .. }) => problems,
            other => panic!("expected problems, got {other:?}"),
        }
    }

    fn target(source: &str) -> Target {
        parse(source).unwrap().targets()[0].clone()
    }

    fn theme(text: &str) -> ThemeId {
        ThemeId::parse(text).unwrap()
    }

    #[test]
    fn reads_a_target_name() {
        assert_eq!(target(TARGET).name().as_str(), "bat");
    }

    #[test]
    fn resolves_a_relative_path_against_the_config_directory() {
        assert_eq!(target(TARGET).template(), Path::new("/cfg/t.in"));
    }

    #[test]
    fn expands_a_leading_tilde_to_the_home_directory() {
        let source = "[[targets]]\nname = \"bat\"\ntemplate = \"t.in\"\noutput = \"~/.config/o\"\n";
        assert_eq!(target(source).output(), Path::new("/home/ada/.config/o"));
    }

    #[test]
    fn keeps_an_absolute_path() {
        let source = "[[targets]]\nname = \"bat\"\ntemplate = \"/t.in\"\noutput = \"o\"\n";
        assert_eq!(target(source).template(), Path::new("/t.in"));
    }

    #[test]
    fn reads_reload_as_an_argument_vector() {
        let source = format!("{TARGET}reload = [\"bat\", \"cache\", \"--build\"]\n");
        assert_eq!(target(&source).reload(), ["bat", "cache", "--build"]);
    }

    #[test]
    fn leaves_reload_empty_when_a_target_has_none() {
        assert!(target(TARGET).reload().is_empty());
    }

    #[test]
    fn reads_the_shell_whose_hook_sources_a_target() {
        let source = format!("{TARGET}shell = \"fish\"\n");
        assert_eq!(target(&source).shell(), Some(Shell::Fish));
    }

    #[test]
    fn leaves_a_target_no_shell_sources_unmarked() {
        assert_eq!(target(TARGET).shell(), None);
    }

    #[test]
    fn reports_a_shell_hook_does_not_emit_for() {
        // A typo is a config error rather than a target that silently never sources.
        let source = format!("{TARGET}shell = \"nu\"\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Shell {
                value: "nu".to_owned(),
                line: 5,
            }]
        );
    }

    #[test]
    fn reads_the_per_target_theme_override() {
        let source =
            format!("{TARGET}themes = {{ light = \"gruvbox-light\", dark = \"gruvbox-dark\" }}\n");
        let target = target(&source);
        let themes = target.themes().unwrap();
        assert_eq!(themes.theme(Variant::Dark), Some(&theme("gruvbox-dark")));
    }

    #[test]
    fn leaves_a_target_without_an_override_unpinned() {
        assert!(target(TARGET).themes().is_none());
    }

    #[test]
    fn reads_the_auto_table() {
        let source = format!("[auto]\nlight = \"paper\"\ndark = \"ink\"\n{TARGET}");
        let config = parse(&source).unwrap();
        assert_eq!(
            config.auto().unwrap().theme(Variant::Light),
            &theme("paper")
        );
    }

    #[test]
    fn allows_a_config_with_no_auto_table() {
        assert!(parse(TARGET).unwrap().auto().is_none());
    }

    #[test]
    fn reports_a_target_with_no_output() {
        let source = "[[targets]]\nname = \"bat\"\ntemplate = \"t.in\"\n";
        assert_eq!(
            problems(source),
            vec![Problem::Missing {
                key: "targets.output".to_owned(),
                line: 1,
            }]
        );
    }

    #[test]
    fn reports_every_key_a_target_is_missing() {
        assert_eq!(problems("[[targets]]\nname = \"bat\"\n").len(), 2);
    }

    #[test]
    fn rejects_a_key_the_format_does_not_define() {
        let source = format!("{TARGET}enabled = false\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Unknown {
                key: "targets.enabled".to_owned(),
                line: 5,
            }]
        );
    }

    #[test]
    fn rejects_a_target_name_that_is_not_an_identifier() {
        let source = "[[targets]]\nname = \"Bat Theme\"\ntemplate = \"t.in\"\noutput = \"o\"\n";
        assert_eq!(
            problems(source),
            vec![Problem::Name {
                name: "Bat Theme".to_owned(),
                line: 2,
            }]
        );
    }

    #[test]
    fn rejects_two_targets_sharing_a_name() {
        let source = format!("{TARGET}{TARGET}");
        assert_eq!(
            problems(&source),
            vec![Problem::Duplicate {
                name: "bat".to_owned(),
                line: 6,
            }]
        );
    }

    #[test]
    fn rejects_an_auto_table_missing_a_mode() {
        let source = format!("[auto]\nlight = \"paper\"\n{TARGET}");
        assert_eq!(
            problems(&source),
            vec![Problem::Missing {
                key: "auto.dark".to_owned(),
                line: 1,
            }]
        );
    }

    #[test]
    fn rejects_a_theme_name_that_is_not_an_identifier() {
        let source = format!("[auto]\nlight = \"Paper\"\ndark = \"ink\"\n{TARGET}");
        assert_eq!(
            problems(&source),
            vec![Problem::Theme {
                key: "auto.light".to_owned(),
                line: 2,
                value: "Paper".to_owned(),
            }]
        );
    }

    #[test]
    fn rejects_a_value_that_is_not_a_string() {
        let source = "[[targets]]\nname = \"bat\"\ntemplate = 1\noutput = \"o\"\n";
        assert_eq!(
            problems(source),
            vec![Problem::Type {
                key: "targets.template".to_owned(),
                line: 3,
                found: "integer",
            }]
        );
    }

    #[test]
    fn rejects_a_reload_entry_that_is_not_a_string() {
        let source = format!("{TARGET}reload = [1]\n");
        assert_eq!(
            problems(&source),
            vec![Problem::Type {
                key: "targets.reload".to_owned(),
                line: 5,
                found: "integer",
            }]
        );
    }

    #[test]
    fn reports_a_tilde_it_cannot_expand() {
        let source = "[[targets]]\nname = \"bat\"\ntemplate = \"~/t.in\"\noutput = \"o\"\n";
        let error = Config::parse(
            Path::new("/cfg/config.toml"),
            source,
            Path::new("/cfg"),
            None,
        );
        assert!(
            matches!(&error, Err(ConfigError::Invalid { problems, .. })
                if problems == &[Problem::Home { key: "targets.template".to_owned(), line: 3 }]),
            "{error:?}"
        );
    }

    #[test]
    fn reports_every_problem_in_one_pass() {
        let source = "[[targets]]\nname = \"Bat\"\ntemplate = 1\n";
        assert_eq!(problems(source).len(), 3);
    }

    #[test]
    fn reports_problems_in_source_order() {
        let source = "[[targets]]\nname = \"Bat\"\ntemplate = 1\n";
        let lines: Vec<usize> = problems(source).iter().map(Problem::line).collect();
        assert_eq!(lines, vec![1, 2, 3]);
    }

    const CYCLE: &str = "[cycle]\nthemes = [\"papercolor-light\", \"nord\", \"gruvbox-dark\"]\n";

    fn cycle(source: &str) -> Cycle {
        parse(&format!("{source}{TARGET}"))
            .unwrap()
            .cycle()
            .unwrap()
            .clone()
    }

    #[test]
    fn reads_no_cycle_from_a_file_that_writes_none() {
        assert_eq!(parse(TARGET).unwrap().cycle(), None);
    }

    #[test]
    fn steps_to_the_theme_after_the_one_in_use() {
        assert_eq!(
            *cycle(CYCLE).next(Some(&theme("nord"))),
            theme("gruvbox-dark")
        );
    }

    #[test]
    fn wraps_from_the_last_theme_to_the_first() {
        assert_eq!(
            *cycle(CYCLE).next(Some(&theme("gruvbox-dark"))),
            theme("papercolor-light")
        );
    }

    /// A machine that has applied nothing has no position, so the cycle starts.
    #[test]
    fn starts_at_the_first_theme_when_nothing_is_applied() {
        assert_eq!(*cycle(CYCLE).next(None), theme("papercolor-light"));
    }

    /// The applied theme need not be in the list: `apply` names any theme it likes.
    #[test]
    fn starts_at_the_first_theme_when_the_cycle_does_not_name_the_one_in_use() {
        assert_eq!(
            *cycle(CYCLE).next(Some(&theme("solarized"))),
            theme("papercolor-light")
        );
    }

    #[test]
    fn steps_through_a_cycle_of_two() {
        let cycle = cycle("[cycle]\nthemes = [\"a\", \"b\"]\n");
        assert_eq!(*cycle.next(Some(&theme("a"))), theme("b"));
        assert_eq!(*cycle.next(Some(&theme("b"))), theme("a"));
    }

    /// The position is looked up by theme, so a theme written twice has two positions and
    /// the cycle can never pass the first of them.
    #[test]
    fn reports_a_theme_the_cycle_writes_twice() {
        let problems = problems(&format!(
            "[cycle]\nthemes = [\"a\", \"b\", \"a\"]\n{TARGET}"
        ));
        assert!(
            problems
                .iter()
                .any(|problem| matches!(problem, Problem::Repeated { theme, .. } if theme == "a")),
            "{problems:?}"
        );
    }

    #[test]
    fn reports_a_cycle_of_one_theme() {
        let problems = problems(&format!("[cycle]\nthemes = [\"a\"]\n{TARGET}"));
        assert!(
            problems
                .iter()
                .any(|problem| matches!(problem, Problem::Short { .. })),
            "{problems:?}"
        );
    }

    #[test]
    fn reports_a_cycle_of_no_themes() {
        let problems = problems(&format!("[cycle]\nthemes = []\n{TARGET}"));
        assert!(
            problems
                .iter()
                .any(|problem| matches!(problem, Problem::Short { .. })),
            "{problems:?}"
        );
    }

    /// An entry that is not an identifier is reported as itself. Counting the survivors
    /// would call the same defect a cycle that is too short as well.
    #[test]
    fn reports_only_the_entry_that_is_not_a_theme_identifier() {
        let problems = problems(&format!(
            "[cycle]\nthemes = [\"a\", \"Not An Id\"]\n{TARGET}"
        ));
        assert!(
            problems.iter().any(
                |problem| matches!(problem, Problem::Theme { value, .. } if value == "Not An Id")
            ),
            "{problems:?}"
        );
        assert!(
            !problems
                .iter()
                .any(|problem| matches!(problem, Problem::Short { .. })),
            "{problems:?}"
        );
    }

    #[test]
    fn reports_an_entry_that_is_not_a_string() {
        let problems = problems(&format!("[cycle]\nthemes = [\"a\", 3]\n{TARGET}"));
        assert!(
            problems.iter().any(
                |problem| matches!(problem, Problem::Type { key, .. } if key == "cycle.themes")
            ),
            "{problems:?}"
        );
    }

    #[test]
    fn reports_a_cycle_that_writes_no_themes_key() {
        let problems = problems(&format!("[cycle]\n{TARGET}"));
        assert!(
            problems.iter().any(
                |problem| matches!(problem, Problem::Missing { key, .. } if key == "cycle.themes")
            ),
            "{problems:?}"
        );
    }

    #[test]
    fn reports_a_key_the_cycle_table_does_not_have() {
        let problems = problems(&format!(
            "[cycle]\nthemes = [\"a\", \"b\"]\nstep = 2\n{TARGET}"
        ));
        assert!(
            problems.iter().any(
                |problem| matches!(problem, Problem::Unknown { key, .. } if key == "cycle.step")
            ),
            "{problems:?}"
        );
    }
}
