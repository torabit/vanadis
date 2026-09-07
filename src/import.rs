//! Turning one upstream scheme into a theme file under `themes/`.
//!
//! `docs/schemes.md` decides what an argument names, what the written file is called, what
//! records where it came from, and what happens when the file is already there.
//!
//! Nothing here reads a palette. Conversion is `scheme::convert` and emission is
//! `init::theme`; this module decides where the bytes come from and whether they may be
//! written.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::init::{EmitError, theme as emit};
use crate::remote::{self, RemoteError};
use crate::scheme::{self, ConvertError, Problem, System};
use crate::theme::{ThemeId, Variant};
use crate::token::TokenPath;

/// Both spellings the collection uses for a scheme file.
///
/// `base16/cyberpunk.yml` is the one file spelled `.yml`, which is why looking a cached
/// identifier up cannot assume the other.
const EXTENSIONS: [&str; 2] = ["yaml", "yml"];

/// What a theme is written under before it is renamed into place.
///
/// The same suffix `init` stages with, so a run interrupted by either leaves the same kind of
/// file behind.
const STAGED: &str = ".coloris-new";

/// What one run of [`import`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    id: ThemeId,
    path: PathBuf,
    origin: String,
    name: String,
    author: Option<String>,
    variant: Variant,
}

impl Imported {
    /// The identifier the theme is reached by, which is its filename.
    #[must_use]
    pub fn id(&self) -> &ThemeId {
        &self.id
    }

    /// The file that was written.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where the scheme came from, as the theme file's own comment records it.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The display name the scheme carried.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Who the scheme credits itself to, when it credits anyone.
    #[must_use]
    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }

    /// The background the theme is written for.
    #[must_use]
    pub fn variant(&self) -> Variant {
        self.variant
    }
}

/// A scheme could not be imported.
#[derive(Debug, Error)]
pub enum ImportError {
    /// Nothing has been cached yet.
    #[error("no scheme cache under {}", .directory.display())]
    NoCache {
        /// Where the cache would be.
        directory: PathBuf,
    },
    /// The cache holds no scheme of that name.
    #[error("the cache holds no scheme called `{argument}`")]
    Unknown {
        /// The argument as it was written.
        argument: String,
    },
    /// A file could not be read.
    #[error("{}", .path.display())]
    Read {
        /// The file the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
    /// A URL could not be fetched.
    #[error(transparent)]
    Fetch {
        /// Why it could not be fetched.
        #[from]
        source: RemoteError,
    },
    /// The scheme does not say which system it is written for.
    ///
    /// The reason hangs off `source` alone. `main` walks the cause chain and prints it, so a
    /// message that interpolated it would show it on both lines.
    #[error("{origin}: cannot be read as a scheme")]
    System {
        /// Where the scheme came from.
        origin: String,
        /// What the file does not say.
        source: Problem,
    },
    /// The scheme could not be converted.
    ///
    /// The reason hangs off `source` alone, for the reason [`ImportError::System`] states.
    #[error("{origin}: cannot be converted")]
    Convert {
        /// Where the scheme came from.
        origin: String,
        /// Why the conversion failed.
        source: ConvertError,
    },
    /// The name the theme would take is not an identifier.
    #[error("`{name}` is not a theme identifier")]
    Name {
        /// The name the source gave.
        name: String,
    },
    /// The theme file is already there.
    #[error("{} already exists", .path.display())]
    Exists {
        /// The file that would have been written over.
        path: PathBuf,
    },
    /// The theme could not be written out.
    #[error(transparent)]
    Emit {
        /// Why it could not be emitted.
        #[from]
        source: EmitError,
    },
    /// A file could not be written.
    #[error("{}", .path.display())]
    Write {
        /// The file the write failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
}

/// Where the bytes of one scheme come from.
///
/// The system travels with them: a cached scheme takes it from the directory it sits in and
/// a loose file has to declare it, which is the whole difference between the two.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// One file out of the cache, whose directory names its system.
    Cached {
        /// The system the directory names.
        system: System,
        /// The file the directory holds.
        path: PathBuf,
        /// The qualified identifier, as `search` prints it.
        qualified: String,
    },
    /// A file on disk, which declares its own system.
    File {
        /// The file, as it was written on the command line.
        path: PathBuf,
    },
    /// A URL, whose answer declares its own system.
    Url {
        /// The URL, as it was written on the command line.
        url: String,
    },
}

/// Converts the scheme `argument` names and writes it as a theme under `themes`.
///
/// `schemes` is the cache directory, which is read only when `argument` names a cached
/// scheme. `force` writes over a theme that is already there.
///
/// # Errors
///
/// Returns [`ImportError`] when the scheme cannot be found, read, fetched, converted or
/// written, when it declares no system coloris reads, when the name it would take is not a
/// theme identifier, or when the theme is already there and `force` is not set.
pub fn import(
    argument: &str,
    schemes: &Path,
    themes: &Path,
    force: bool,
) -> Result<Imported, ImportError> {
    let source = resolve(argument, schemes)?;
    let bytes = read(&source)?;
    let origin = origin(&source);

    let system = match &source {
        Source::Cached { system, .. } => *system,
        Source::File { .. } | Source::Url { .. } => declared(&bytes, &origin)?,
    };

    let converted = scheme::convert(system, &bytes).map_err(|source| ImportError::Convert {
        origin: origin.clone(),
        source,
    })?;

    let name = stem(&source);
    let id = ThemeId::parse(&name).ok_or(ImportError::Name { name })?;
    let path = themes.join(format!("{id}.toml"));
    if !force && path.exists() {
        return Err(ImportError::Exists { path });
    }

    let file = format!(
        "# imported by coloris from {origin}\n{}",
        emit(converted.name(), converted.variant(), converted.tokens())?
    );
    write(&path, &file)?;

    Ok(Imported {
        id,
        path,
        origin,
        name: converted.name().to_owned(),
        author: converted
            .tokens()
            .get(&TokenPath::from_segments(["meta", "author"]))
            .cloned(),
        variant: converted.variant(),
    })
}

/// The system a loose scheme declares itself written for.
///
/// A cached scheme takes its system from the directory it sits in, so this is reached only
/// by a path and a URL.
///
/// # Errors
///
/// Returns [`ImportError::Convert`] when the bytes are not UTF-8 and [`ImportError::System`]
/// when they do not declare a system coloris reads.
fn declared(bytes: &[u8], origin: &str) -> Result<System, ImportError> {
    let text = std::str::from_utf8(bytes).map_err(|source| ImportError::Convert {
        origin: origin.to_owned(),
        source: ConvertError::Utf8 { source },
    })?;
    scheme::declared_system(text).map_err(|source| ImportError::System {
        origin: origin.to_owned(),
        source,
    })
}

/// What `argument` names.
///
/// The four shapes are told apart in the order `docs/schemes.md` lists them. A single token
/// segment always means the cache, so a file of that name is reached by writing a path to it.
///
/// # Errors
///
/// Returns [`ImportError::NoCache`] when a cached scheme is named and nothing is cached, and
/// [`ImportError::Unknown`] when the cache holds no scheme of that name.
fn resolve(argument: &str, schemes: &Path) -> Result<Source, ImportError> {
    if argument.starts_with("http://") || argument.starts_with("https://") {
        return Ok(Source::Url {
            url: argument.to_owned(),
        });
    }

    if let Some((head, id)) = argument.split_once('/')
        && let Some(system) = System::parse(head)
        && ThemeId::parse(id).is_some()
    {
        return cached(argument, schemes, &[system], id);
    }

    if ThemeId::parse(argument).is_some() {
        return cached(argument, schemes, &System::ALL, argument);
    }

    Ok(Source::File {
        path: PathBuf::from(argument),
    })
}

/// The first of `systems` whose directory holds a scheme called `id`.
///
/// The lookup is by filename and not a scan of the cache. The identifier is the file stem,
/// which `docs/schemes.md` decides, so six paths answer what reading five hundred files
/// would.
fn cached(
    argument: &str,
    schemes: &Path,
    systems: &[System],
    id: &str,
) -> Result<Source, ImportError> {
    if !schemes.is_dir() {
        return Err(ImportError::NoCache {
            directory: schemes.to_owned(),
        });
    }
    for system in systems {
        for extension in EXTENSIONS {
            let path = schemes
                .join(system.as_str())
                .join(format!("{id}.{extension}"));
            if path.is_file() {
                return Ok(Source::Cached {
                    system: *system,
                    path,
                    qualified: format!("{system}/{id}"),
                });
            }
        }
    }
    Err(ImportError::Unknown {
        argument: argument.to_owned(),
    })
}

/// The bytes of the scheme `source` names.
///
/// # Errors
///
/// Returns [`ImportError::Read`] for a file that cannot be read and [`ImportError::Fetch`]
/// for a URL that cannot be reached.
fn read(source: &Source) -> Result<Vec<u8>, ImportError> {
    match source {
        Source::Cached { path, .. } | Source::File { path } => {
            std::fs::read(path).map_err(|error| ImportError::Read {
                path: path.clone(),
                source: error,
            })
        }
        Source::Url { url } => Ok(remote::fetch(url)?),
    }
}

/// Where the scheme came from, as the theme file records it.
///
/// A cached scheme is named the way `search` prints it, which is the form that reaches it
/// again. A file is named by the path it was reached through and a URL by itself.
fn origin(source: &Source) -> String {
    match source {
        Source::Cached { qualified, .. } => qualified.clone(),
        Source::File { path } => std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.clone())
            .display()
            .to_string(),
        Source::Url { url } => url.clone(),
    }
}

/// The name the theme takes, which is the source's filename minus its extension.
///
/// A URL is its path, so the last segment of it is the filename. A URL ending in a slash, or
/// carrying none, leaves this empty, which is not a theme identifier and is reported as one.
fn stem(source: &Source) -> String {
    let name = |path: &Path| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    match source {
        Source::Cached { path, .. } | Source::File { path } => name(path),
        Source::Url { url } => {
            let path = url.split(['?', '#']).next().unwrap_or(url);
            name(Path::new(path.rsplit('/').next().unwrap_or_default()))
        }
    }
}

/// Writes `contents` to `path`, through a file beside it.
///
/// The staged file is renamed over the destination, the way every other write in coloris is
/// done, so a `--force` that fails part-way leaves the theme that was there intact.
///
/// # Errors
///
/// Returns [`ImportError::Write`] for whichever step failed, naming the file it failed on.
fn write(path: &Path, contents: &str) -> Result<(), ImportError> {
    let failed = |path: &Path| {
        let path = path.to_owned();
        |source| ImportError::Write { path, source }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(failed(parent))?;
    }
    let mut name = path.as_os_str().to_os_string();
    name.push(STAGED);
    let staged = PathBuf::from(name);
    std::fs::write(&staged, contents).map_err(failed(&staged))?;
    std::fs::rename(&staged, path).map_err(failed(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schemes() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cache/coloris/schemes")
    }

    fn resolved(argument: &str) -> Source {
        resolve(argument, &schemes()).unwrap()
    }

    #[test]
    fn reads_a_url_as_a_url() {
        assert_eq!(
            resolved("https://example.com/nord.yaml"),
            Source::Url {
                url: "https://example.com/nord.yaml".to_owned()
            }
        );
    }

    #[test]
    fn reads_a_qualified_identifier_as_the_cached_scheme_it_names() {
        assert_eq!(
            resolved("base16/nord"),
            Source::Cached {
                system: System::Base16,
                path: schemes().join("base16/nord.yaml"),
                qualified: "base16/nord".to_owned(),
            }
        );
    }

    /// `base16/cyberpunk` is spelled `.yml` upstream, so both extensions are looked for.
    #[test]
    fn finds_a_cached_scheme_spelled_yml() {
        assert_eq!(
            resolved("cyberpunk"),
            Source::Cached {
                system: System::Base16,
                path: schemes().join("base16/cyberpunk.yml"),
                qualified: "base16/cyberpunk".to_owned(),
            }
        );
    }

    #[test]
    fn searches_the_systems_in_order_for_a_bare_identifier() {
        assert_eq!(
            resolved("nord"),
            Source::Cached {
                system: System::Base16,
                path: schemes().join("base16/nord.yaml"),
                qualified: "base16/nord".to_owned(),
            }
        );
    }

    #[test]
    fn reaches_a_system_the_search_order_puts_last() {
        assert_eq!(
            resolved("catppuccin-latte"),
            Source::Cached {
                system: System::Tinted8,
                path: schemes().join("tinted8/catppuccin-latte.yaml"),
                qualified: "tinted8/catppuccin-latte".to_owned(),
            }
        );
    }

    /// A qualified identifier names one system and does not fall through to another.
    #[test]
    fn does_not_search_past_the_system_a_qualified_identifier_names() {
        assert!(matches!(
            resolve("base24/nord", &schemes()),
            Err(ImportError::Unknown { .. })
        ));
    }

    #[test]
    fn reads_a_path_as_a_file() {
        assert_eq!(
            resolved("./nord.yaml"),
            Source::File {
                path: PathBuf::from("./nord.yaml")
            }
        );
    }

    /// A single segment always means the cache, so a file of that name needs a path written
    /// to it.
    #[test]
    fn reads_a_relative_path_to_a_bare_name_as_a_file() {
        assert_eq!(
            resolved("./nord"),
            Source::File {
                path: PathBuf::from("./nord")
            }
        );
    }

    /// A prefix that is not a system is part of a path, not a qualification.
    #[test]
    fn reads_an_unqualified_slash_as_a_file() {
        assert_eq!(
            resolved("schemes/nord.yaml"),
            Source::File {
                path: PathBuf::from("schemes/nord.yaml")
            }
        );
    }

    #[test]
    fn reports_a_bare_identifier_the_cache_does_not_hold() {
        assert!(matches!(
            resolve("solarized", &schemes()),
            Err(ImportError::Unknown { ref argument }) if argument == "solarized"
        ));
    }

    #[test]
    fn reports_a_cache_that_is_not_there() {
        let missing = schemes().join("nowhere");
        assert!(matches!(
            resolve("nord", &missing),
            Err(ImportError::NoCache { .. })
        ));
    }

    /// Nothing about a URL or a path is looked up in the cache, so neither needs one.
    #[test]
    fn needs_no_cache_to_name_a_file() {
        let missing = schemes().join("nowhere");
        assert!(resolve("./nord.yaml", &missing).is_ok());
        assert!(resolve("https://example.com/nord.yaml", &missing).is_ok());
    }

    #[test]
    fn names_the_theme_after_the_cached_identifier() {
        assert_eq!(stem(&resolved("base16/nord")), "nord");
    }

    #[test]
    fn names_the_theme_after_the_file() {
        assert_eq!(stem(&resolved("../schemes/my-theme.yaml")), "my-theme");
    }

    #[test]
    fn names_the_theme_after_the_last_segment_of_a_url() {
        assert_eq!(stem(&resolved("https://example.com/raw/nord.yaml")), "nord");
    }

    #[test]
    fn drops_a_query_string_from_the_name_a_url_gives() {
        assert_eq!(
            stem(&resolved("https://example.com/nord.yaml?raw=1")),
            "nord"
        );
    }

    #[test]
    fn takes_no_name_from_a_url_that_ends_in_a_slash() {
        assert_eq!(stem(&resolved("https://example.com/schemes/")), "");
    }

    #[test]
    fn records_a_cached_scheme_by_its_qualified_identifier() {
        assert_eq!(origin(&resolved("nord")), "base16/nord");
    }

    #[test]
    fn records_a_url_by_itself() {
        let url = "https://example.com/nord.yaml";
        assert_eq!(origin(&resolved(url)), url);
    }
}
