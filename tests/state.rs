//! Writing the state file, which is how `apply` records what it applied.

use std::fs;
use std::path::{Path, PathBuf};

use vanadis::{State, ThemeId};

/// An empty directory to write state into.
fn directory(test: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn state(theme: &str) -> State {
    State::new(ThemeId::parse(theme).unwrap())
}

#[test]
fn reads_back_the_state_it_wrote() {
    let path = directory("store").join("state.toml");
    state("nord").store(&path).unwrap();
    assert_eq!(State::load(&path).unwrap(), Some(state("nord")));
}

#[test]
fn creates_the_directories_leading_to_the_state_file() {
    let path = directory("store-mkdir").join("vanadis/state.toml");
    state("nord").store(&path).unwrap();
    assert!(path.is_file());
}

#[test]
fn leaves_nothing_beside_the_state_file_it_wrote() {
    let directory = directory("store-clean");
    let path = directory.join("state.toml");
    state("nord").store(&path).unwrap();
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
}

#[test]
fn replaces_the_state_that_was_there() {
    let path = directory("store-replace").join("state.toml");
    state("nord").store(&path).unwrap();
    state("gruvbox-dark").store(&path).unwrap();
    assert_eq!(State::load(&path).unwrap(), Some(state("gruvbox-dark")));
}
