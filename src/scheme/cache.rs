//! The cached scheme collection.
//!
//! `docs/schemes.md` decides the layout: one directory per system under the cache, holding
//! the collection's files unchanged. A file that does not load costs the user that scheme
//! and not the command, which is the call `docs/config.md` makes for `themes/`.

use std::path::{Path, PathBuf};

use thiserror::Error;

use super::{Scheme, SchemeError, System};

/// Every scheme the cache holds, and every file in it that is not one.
#[derive(Debug)]
pub struct Cache {
    schemes: Vec<Scheme>,
    broken: Vec<SchemeError>,
}

/// The cache could not be scanned.
#[derive(Debug, Error)]
pub enum CacheError {
    /// Nothing has been cached yet.
    #[error("no scheme cache under {}\nrun `coloris remote update` to fetch it", .path.display())]
    Missing {
        /// Where the cache would be.
        path: PathBuf,
    },
    /// A system directory could not be listed.
    #[error("{}: {source}", .path.display())]
    Read {
        /// The directory the read failed on.
        path: PathBuf,
        /// The IO error.
        source: std::io::Error,
    },
}

impl Cache {
    /// Loads every scheme under `directory`, in system then identifier order.
    ///
    /// A system with no directory is not an error: a cache written before that system
    /// existed simply holds no schemes for it. Files that are not `.yaml` or `.yml`, and
    /// anything that is not a file, are ignored, which is what keeps the collection's
    /// `LICENSE` out of the results.
    ///
    /// Sorting compares the file stem, not the whole path: the identifier is the stem, and
    /// `docs/schemes.md` promises identifier order. Sorting the whole filename instead would
    /// let the extension take part in the comparison, and it can disagree with the stem —
    /// `nord-light.yaml` sorts before `nord.yaml` because `-` (0x2D) is less than `.` (0x2E),
    /// even though `nord` is the earlier identifier. The full path is kept as a tiebreak, so
    /// `x.yaml` and `x.yml` sort in a fixed order rather than whatever `read_dir` happened to
    /// yield, a pair the collection does not have today but could.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError::Missing`] when `directory` does not exist, and
    /// [`CacheError::Read`] when a system directory exists but cannot be listed.
    pub fn scan(directory: &Path) -> Result<Self, CacheError> {
        if !directory.is_dir() {
            return Err(CacheError::Missing {
                path: directory.to_owned(),
            });
        }

        let mut schemes = Vec::new();
        let mut broken = Vec::new();
        for system in System::ALL {
            let inside = directory.join(system.as_str());
            if !inside.is_dir() {
                continue;
            }
            let mut files: Vec<PathBuf> = std::fs::read_dir(&inside)
                .map_err(|source| CacheError::Read {
                    path: inside.clone(),
                    source,
                })?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|kind| kind == "yaml" || kind == "yml")
                        && path.is_file()
                })
                .collect();
            files.sort_by(|left, right| (left.file_stem(), left).cmp(&(right.file_stem(), right)));

            for file in files {
                match Scheme::load(system, &file) {
                    Ok(scheme) => schemes.push(scheme),
                    Err(error) => broken.push(error),
                }
            }
        }

        Ok(Self { schemes, broken })
    }

    /// The schemes that loaded, in system then identifier order.
    #[must_use]
    pub fn schemes(&self) -> &[Scheme] {
        &self.schemes
    }

    /// Why each of the other files is not a scheme, in the same order.
    #[must_use]
    pub fn broken(&self) -> &[SchemeError] {
        &self.broken
    }

    /// Every scheme `query` matches, in the order [`Cache::schemes`] holds them.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&Scheme> {
        self.schemes
            .iter()
            .filter(|scheme| scheme.matches(query))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cache/coloris/schemes")
    }

    fn cache() -> Cache {
        Cache::scan(&directory()).unwrap()
    }

    fn qualified(schemes: &[&Scheme]) -> Vec<String> {
        schemes.iter().map(|scheme| scheme.qualified()).collect()
    }

    #[test]
    fn finds_every_scheme_in_system_then_identifier_order() {
        let cache = cache();
        let all: Vec<&Scheme> = cache.schemes().iter().collect();
        assert_eq!(
            qualified(&all),
            [
                "base16/cyberpunk",
                "base16/nord",
                "base16/nord-light",
                "base24/dracula",
                "tinted8/catppuccin-latte"
            ]
        );
    }

    #[test]
    fn orders_a_prefix_pair_by_identifier_and_not_by_filename() {
        assert_eq!(
            qualified(&cache().search("nord")),
            ["base16/nord", "base16/nord-light"]
        );
    }

    #[test]
    fn reads_a_scheme_whose_file_ends_in_yml() {
        assert_eq!(
            qualified(&cache().search("cyberpunk")),
            ["base16/cyberpunk"]
        );
    }

    #[test]
    fn keeps_a_scheme_it_cannot_read_out_of_the_list() {
        assert!(!qualified(&cache().search("broken")).contains(&"base16/broken".to_owned()));
    }

    #[test]
    fn reports_the_scheme_it_could_not_read() {
        let cache = cache();
        let broken = cache.broken();
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(broken[0].to_string().contains("broken.yaml"), "{broken:?}");
    }

    #[test]
    fn ignores_the_licence_file_beside_the_systems() {
        assert!(
            cache()
                .broken()
                .iter()
                .all(|error| !error.to_string().contains("LICENSE"))
        );
    }

    #[test]
    fn finds_a_scheme_by_name() {
        assert_eq!(
            qualified(&cache().search("nord light")),
            ["base16/nord-light"]
        );
    }

    #[test]
    fn finds_a_scheme_by_author() {
        assert_eq!(qualified(&cache().search("clach04")), ["base24/dracula"]);
    }

    #[test]
    fn finds_every_scheme_of_one_system() {
        assert_eq!(
            qualified(&cache().search("base16/")),
            ["base16/cyberpunk", "base16/nord", "base16/nord-light"]
        );
    }

    #[test]
    fn finds_every_scheme_of_one_variant() {
        assert_eq!(
            qualified(&cache().search("light")),
            ["base16/nord-light", "tinted8/catppuccin-latte"]
        );
    }

    #[test]
    fn finds_nothing_for_a_query_no_scheme_holds() {
        assert!(cache().search("solarized").is_empty());
    }

    #[test]
    fn reports_a_cache_that_is_not_there() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nowhere");
        assert!(matches!(
            Cache::scan(&missing),
            Err(CacheError::Missing { .. })
        ));
    }

    #[test]
    fn says_how_to_fill_a_cache_that_is_not_there() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nowhere");
        let error = Cache::scan(&missing).unwrap_err();
        assert!(
            error.to_string().contains("coloris remote update"),
            "{error}"
        );
    }
}
