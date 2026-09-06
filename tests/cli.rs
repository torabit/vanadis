//! `vanadis list` and `vanadis current`, driven as the user drives them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The fixture config directory, whose `themes/` holds two themes and one broken file.
fn config() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config")
}

/// An empty directory to hand the binary as `$XDG_STATE_HOME`.
fn state_home(test: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Writes a state file recording `theme` under `state_home`.
fn applied(state_home: &Path, theme: &str) {
    let directory = state_home.join("vanadis");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("state.toml"),
        format!("theme = \"{theme}\"\n"),
    )
    .unwrap();
}

fn vanadis(config: &Path, state_home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vanadis"))
        .args(args)
        .env("VANADIS_CONFIG", config)
        .env("XDG_STATE_HOME", state_home)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn lists_every_theme_with_its_variant_and_display_name() {
    let state = state_home("list");
    let output = vanadis(&config(), &state, &["list"]);
    assert_eq!(
        stdout(&output),
        "  gruvbox-dark      dark   Gruvbox Dark\n  papercolor-light  light  PaperColor Light\n"
    );
}

#[test]
fn marks_the_theme_that_was_applied_last() {
    let state = state_home("list-current");
    applied(&state, "papercolor-light");
    let output = vanadis(&config(), &state, &["list"]);
    assert_eq!(
        stdout(&output),
        "  gruvbox-dark      dark   Gruvbox Dark\n* papercolor-light  light  PaperColor Light\n"
    );
}

#[test]
fn lists_only_the_themes_written_for_one_background() {
    let state = state_home("list-variant");
    let output = vanadis(&config(), &state, &["list", "--variant", "dark"]);
    assert_eq!(stdout(&output), "  gruvbox-dark  dark  Gruvbox Dark\n");
}

#[test]
fn warns_about_a_theme_it_could_not_load_without_failing() {
    let state = state_home("list-broken");
    let output = vanadis(&config(), &state, &["list"]);
    assert!(
        stderr(&output).contains("broken.toml"),
        "{}",
        stderr(&output)
    );
    assert!(output.status.success());
}

#[test]
fn reports_the_directory_it_cannot_find_themes_in() {
    let state = state_home("list-nowhere");
    let elsewhere = state.join("nowhere");
    let output = vanadis(&elsewhere, &state, &["list"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("themes"), "{}", stderr(&output));
}

#[test]
fn prints_the_theme_that_was_applied_last() {
    let state = state_home("current");
    applied(&state, "papercolor-light");
    let output = vanadis(&config(), &state, &["current"]);
    assert_eq!(stdout(&output), "papercolor-light\n");
}

#[test]
fn reports_that_no_theme_has_been_applied_yet() {
    let state = state_home("current-unset");
    let output = vanadis(&config(), &state, &["current"]);
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert!(!stderr(&output).is_empty());
}
