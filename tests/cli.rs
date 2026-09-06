//! The commands that read: `list`, `current` and `get`, driven as the user drives them.

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
fn lists_a_theme_that_does_not_define_the_whole_core() {
    // Both themes in this fixture define one `role` token and no `[ansi]` at all. `check` is
    // the only place the core is enforced, so an unfinished theme costs the user that theme
    // and never a command that enumerates the directory.
    let state = state_home("list-incomplete");
    let output = vanadis(&config(), &state, &["list"]);
    assert!(output.status.success(), "{}", stderr(&output));
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

#[test]
fn prints_the_value_a_token_resolves_to() {
    let state = state_home("get");
    applied(&state, "gruvbox-dark");
    let output = vanadis(&config(), &state, &["get", "role.fg"]);
    assert_eq!(stdout(&output), "#ebdbb2\n");
}

#[test]
fn follows_a_reference_before_printing_it() {
    let state = state_home("get-reference");
    applied(&state, "papercolor-light");
    let output = vanadis(&config(), &state, &["get", "role.bg"]);
    assert_eq!(stdout(&output), "#eeeeee\n");
}

#[test]
fn reads_the_theme_named_instead_of_the_applied_one() {
    let state = state_home("get-theme");
    applied(&state, "gruvbox-dark");
    let output = vanadis(
        &config(),
        &state,
        &["get", "role.fg", "--theme", "papercolor-light"],
    );
    assert_eq!(stdout(&output), "#444444\n");
}

#[test]
fn prints_nothing_for_a_token_the_theme_does_not_define() {
    let state = state_home("get-undefined");
    applied(&state, "gruvbox-dark");
    let output = vanadis(&config(), &state, &["get", "role.acent"]);
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("role.acent"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn prints_nothing_for_a_name_that_is_not_a_token_path() {
    let state = state_home("get-malformed");
    applied(&state, "gruvbox-dark");
    let output = vanadis(&config(), &state, &["get", "Role.BG"]);
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
}

#[test]
fn reports_that_no_theme_has_been_applied_when_reading_a_token() {
    let state = state_home("get-unset");
    let output = vanadis(&config(), &state, &["get", "role.fg"]);
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert!(!stderr(&output).is_empty());
}

#[test]
fn prints_every_token_the_theme_defines_as_json() {
    let state = state_home("get-json");
    applied(&state, "gruvbox-dark");
    let output = vanadis(&config(), &state, &["get", "--json"]);
    let tokens: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(
        tokens,
        [
            ("colors.bg", "#282828"),
            ("meta.format", "1"),
            ("meta.id", "gruvbox-dark"),
            ("meta.name", "Gruvbox Dark"),
            ("meta.variant", "dark"),
            ("role.accent", "#282828"),
            ("role.bg", "#282828"),
            ("role.fg", "#ebdbb2"),
        ]
        .into_iter()
        .map(|(path, value)| (path.to_owned(), value.to_owned()))
        .collect()
    );
}

#[test]
fn refuses_a_token_path_and_json_together() {
    let state = state_home("get-both");
    applied(&state, "gruvbox-dark");
    let output = vanadis(&config(), &state, &["get", "role.fg", "--json"]);
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
}

#[test]
fn refuses_to_read_neither_a_token_nor_the_whole_theme() {
    let state = state_home("get-neither");
    applied(&state, "gruvbox-dark");
    let output = vanadis(&config(), &state, &["get"]);
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
}
