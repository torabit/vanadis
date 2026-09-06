//! Fetching the tinted-theming collection and caching it.
//!
//! `docs/schemes.md` decides the source, that it is a constant, and that extraction is a
//! whitelist of path shapes rather than a sanitising pass: an entry that does not match one
//! of the shapes is discarded, so nothing outside the cache is ever a write target.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::scheme::System;

/// The archive every scheme is read out of.
pub const SOURCE: &str =
    "https://github.com/tinted-theming/schemes/archive/refs/heads/spec-0.11.tar.gz";

/// What one run of [`install`] wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    counts: BTreeMap<System, usize>,
}

/// The collection could not be cached.
#[derive(Debug, Error)]
pub enum RemoteError {
    /// The archive could not be read as a gzipped tar.
    #[error("{url}: not a readable archive: {source}")]
    Archive {
        /// Where the archive came from.
        url: String,
        /// What reading it failed on.
        source: std::io::Error,
    },
    /// The archive was read but holds no scheme.
    #[error("{url}: the archive holds no scheme")]
    Empty {
        /// Where the archive came from.
        url: String,
    },
    /// A file or directory could not be written.
    #[error("{}: {source}", .path.display())]
    Write {
        /// The path the write failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
}

impl Installed {
    /// How many schemes were written for `system`.
    #[must_use]
    pub fn count(&self, system: System) -> usize {
        self.counts.get(&system).copied().unwrap_or_default()
    }

    /// How many schemes were written in all.
    #[must_use]
    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }
}

/// Where one archive entry belongs in the cache, when it belongs anywhere.
enum Destination {
    /// A scheme, under its system's directory.
    Scheme(System, String),
    /// The collection's licence file.
    License,
}

/// Writes the schemes `archive` holds into `directory`, replacing what is there.
///
/// The archive is unpacked into `directory` with `.incoming` appended to its name, and that
/// directory replaces `directory` by rename once every entry is written. A failure part way
/// through leaves the previous cache exactly as it was.
///
/// # Errors
///
/// Returns [`RemoteError::Archive`] when the bytes are not a readable gzipped tar,
/// [`RemoteError::Empty`] when they hold no scheme, and [`RemoteError::Write`] when the
/// cache cannot be written.
pub fn install(archive: &[u8], directory: &Path) -> Result<Installed, RemoteError> {
    let staging = staging(directory);
    remove(&staging)?;

    let mut installed = Installed::default();
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let entries = tar.entries().map_err(|source| RemoteError::Archive {
        url: SOURCE.to_owned(),
        source,
    })?;

    for entry in entries {
        let mut entry = entry.map_err(|source| RemoteError::Archive {
            url: SOURCE.to_owned(),
            source,
        })?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|source| RemoteError::Archive {
            url: SOURCE.to_owned(),
            source,
        })?;
        let Some(destination) = destination(&path) else {
            continue;
        };

        let target = match &destination {
            Destination::Scheme(system, file) => staging.join(system.as_str()).join(file),
            Destination::License => staging.join("LICENSE"),
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|source| RemoteError::Write {
                path: parent.to_owned(),
                source,
            })?;
        }
        let mut file = std::fs::File::create(&target).map_err(|source| RemoteError::Write {
            path: target.clone(),
            source,
        })?;
        std::io::copy(&mut entry, &mut file).map_err(|source| RemoteError::Write {
            path: target.clone(),
            source,
        })?;

        if let Destination::Scheme(system, _) = destination {
            *installed.counts.entry(system).or_default() += 1;
        }
    }

    if installed.total() == 0 {
        remove(&staging)?;
        return Err(RemoteError::Empty {
            url: SOURCE.to_owned(),
        });
    }

    remove(directory)?;
    std::fs::rename(&staging, directory).map_err(|source| RemoteError::Write {
        path: directory.to_owned(),
        source,
    })?;
    Ok(installed)
}

/// The directory the archive is unpacked into before it replaces the cache.
fn staging(directory: &Path) -> PathBuf {
    let mut name = directory.as_os_str().to_owned();
    name.push(".incoming");
    PathBuf::from(name)
}

/// Removes `directory` and everything under it, treating an absent one as removed.
fn remove(directory: &Path) -> Result<(), RemoteError> {
    match std::fs::remove_dir_all(directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RemoteError::Write {
            path: directory.to_owned(),
            source,
        }),
    }
}

/// Where `path` belongs in the cache, or `None` when the archive should not have it.
///
/// This is the whitelist. Every component must be an ordinary name, so `..`, `.`, a root
/// and a prefix all fail before anything is matched, and the shapes accept exactly one
/// leading directory. Nothing outside the cache can be named.
fn destination(path: &Path) -> Option<Destination> {
    let parts: Vec<&str> = path
        .components()
        .map(|component| match component {
            Component::Normal(name) => name.to_str(),
            Component::RootDir
            | Component::Prefix(_)
            | Component::CurDir
            | Component::ParentDir => None,
        })
        .collect::<Option<Vec<_>>>()?;

    match parts.as_slice() {
        [_, directory, file] => {
            let system = System::parse(directory)?;
            let extension = Path::new(file).extension()?;
            (extension == "yaml" || extension == "yml")
                .then(|| Destination::Scheme(system, (*file).to_owned()))
        }
        [_, "LICENSE"] => Some(Destination::License),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write as _;

    /// A gzipped tar holding `entries`, each a path and its contents.
    ///
    /// The path is written directly into the header's name field rather than through
    /// `Builder::append_data`, because the `tar` crate's own path-setting refuses to write a
    /// path that starts with `/` or contains `..`, exactly the hostile shapes these fixtures
    /// need to hold. A real hostile archive is not built by a well-behaved writer, so the
    /// fixture must be free to hold what the crate's safe API will not construct.
    fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, body) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            let name = &mut header.as_old_mut().name;
            name.fill(0);
            name[..path.len()].copy_from_slice(path.as_bytes());
            header.set_cksum();
            builder.append(&header, body.as_bytes()).unwrap();
        }
        let tarball = builder.into_inner().unwrap();

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tarball).unwrap();
        encoder.finish().unwrap()
    }

    const NORD: &str = "system: \"base16\"\nname: \"Nord\"\nauthor: \"a\"\nvariant: \"dark\"\n";

    /// An empty directory to install into, named after the test.
    ///
    /// `CARGO_TARGET_TMPDIR` is only set for integration test and bench targets, not for a
    /// library's own unit tests, so this falls back to the OS temp directory instead.
    fn target(test: &str) -> PathBuf {
        let directory = std::env::temp_dir().join("vanadis-remote-tests").join(test);
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory.join("schemes")
    }

    fn one() -> Vec<u8> {
        archive(&[
            ("schemes-spec-0.11/base16/nord.yaml", NORD),
            ("schemes-spec-0.11/LICENSE", "MIT\n"),
        ])
    }

    #[test]
    fn writes_a_scheme_under_its_system() {
        let schemes = target("writes-a-scheme");
        install(&one(), &schemes).unwrap();
        assert_eq!(
            std::fs::read_to_string(schemes.join("base16/nord.yaml")).unwrap(),
            NORD
        );
    }

    #[test]
    fn drops_the_leading_directory_the_archive_wraps_everything_in() {
        let schemes = target("drops-the-leading-directory");
        install(&one(), &schemes).unwrap();
        assert!(!schemes.join("schemes-spec-0.11").exists());
    }

    #[test]
    fn keeps_the_licence_file() {
        let schemes = target("keeps-the-licence");
        install(&one(), &schemes).unwrap();
        assert!(schemes.join("LICENSE").is_file());
    }

    #[test]
    fn counts_what_it_wrote() {
        let schemes = target("counts");
        let installed = install(&one(), &schemes).unwrap();
        assert_eq!(installed.count(System::Base16), 1);
    }

    #[test]
    fn counts_no_scheme_for_a_system_the_archive_does_not_carry() {
        let schemes = target("counts-none");
        let installed = install(&one(), &schemes).unwrap();
        assert_eq!(installed.count(System::Tinted8), 0);
    }

    #[test]
    fn writes_a_scheme_whose_file_ends_in_yml() {
        let schemes = target("writes-yml");
        let bytes = archive(&[("schemes-spec-0.11/base16/cyberpunk.yml", NORD)]);
        install(&bytes, &schemes).unwrap();
        assert!(schemes.join("base16/cyberpunk.yml").is_file());
    }

    #[test]
    fn writes_nothing_for_an_entry_that_climbs_out_of_the_cache() {
        let schemes = target("climbs-out");
        let bytes = archive(&[
            ("schemes-spec-0.11/base16/nord.yaml", NORD),
            ("schemes-spec-0.11/base16/../../escaped.yaml", NORD),
        ]);
        install(&bytes, &schemes).unwrap();
        assert!(!schemes.parent().unwrap().join("escaped.yaml").exists());
    }

    #[test]
    fn writes_nothing_for_an_entry_named_by_an_absolute_path() {
        let schemes = target("absolute");
        let bytes = archive(&[
            ("schemes-spec-0.11/base16/nord.yaml", NORD),
            ("/etc/vanadis-escaped.yaml", NORD),
        ]);
        let installed = install(&bytes, &schemes).unwrap();
        assert_eq!(installed.total(), 1);
    }

    #[test]
    fn writes_nothing_for_a_directory_the_collection_does_not_have() {
        let schemes = target("unknown-directory");
        let bytes = archive(&[
            ("schemes-spec-0.11/base16/nord.yaml", NORD),
            ("schemes-spec-0.11/.github/workflows/test.yml", "on: push\n"),
            ("schemes-spec-0.11/README.md", "# schemes\n"),
        ]);
        install(&bytes, &schemes).unwrap();
        assert!(!schemes.join(".github").exists());
        assert!(!schemes.join("README.md").exists());
    }

    #[test]
    fn writes_nothing_for_a_scheme_nested_one_level_deeper() {
        let schemes = target("nested");
        let bytes = archive(&[
            ("schemes-spec-0.11/base16/nord.yaml", NORD),
            ("schemes-spec-0.11/base16/deep/nested.yaml", NORD),
        ]);
        install(&bytes, &schemes).unwrap();
        assert!(!schemes.join("base16/deep").exists());
    }

    #[test]
    fn replaces_a_cache_that_is_already_there() {
        let schemes = target("replaces");
        install(&one(), &schemes).unwrap();
        std::fs::write(schemes.join("base16/stale.yaml"), NORD).unwrap();

        install(&one(), &schemes).unwrap();
        assert!(!schemes.join("base16/stale.yaml").exists());
        assert!(schemes.join("base16/nord.yaml").is_file());
    }

    #[test]
    fn leaves_the_previous_cache_alone_when_the_archive_is_not_readable() {
        let schemes = target("leaves-alone");
        install(&one(), &schemes).unwrap();

        assert!(install(b"not a gzip stream at all", &schemes).is_err());
        assert!(schemes.join("base16/nord.yaml").is_file());
    }

    #[test]
    fn reports_an_archive_it_cannot_read() {
        let schemes = target("unreadable");
        assert!(matches!(
            install(b"not a gzip stream at all", &schemes),
            Err(RemoteError::Archive { .. })
        ));
    }

    #[test]
    fn reports_an_archive_that_holds_no_scheme() {
        let schemes = target("no-scheme");
        let bytes = archive(&[("schemes-spec-0.11/README.md", "# schemes\n")]);
        assert!(matches!(
            install(&bytes, &schemes),
            Err(RemoteError::Empty { .. })
        ));
    }
}
