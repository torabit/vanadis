//! `vanadis check`, driven as the user drives it.
//!
//! The golden tree is assembled rather than checked in: `tests/fixtures/check/config.toml`
//! points at `tests/fixtures/templates/` and `tests/fixtures/expected/`, and the helper
//! below copies both in beside it. Golden data is written in one place only.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn fixtures() -> PathBuf {
    root().join("tests/fixtures")
}

/// An empty directory to build a workspace in, and an empty state directory beside it.
fn workspace(test: &str) -> (PathBuf, PathBuf) {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = fs::remove_dir_all(&base);

    let state = base.join("state");
    fs::create_dir_all(&state).unwrap();
    (base.join("config"), state)
}

/// A copy of `fixture`, and an empty state directory beside it.
fn tree(test: &str, fixture: &str) -> (PathBuf, PathBuf) {
    let (config, state) = workspace(test);
    copy(&fixtures().join(fixture), &config);
    (config, state)
}

/// The eleven golden templates, their golden outputs, and the theme they were rendered from.
fn golden(test: &str) -> (PathBuf, PathBuf) {
    let (config, state) = tree(test, "check");
    copy(&fixtures().join("templates"), &config.join("templates"));
    copy(&fixtures().join("expected"), &config.join("expected"));

    let themes = config.join("themes");
    fs::create_dir_all(&themes).unwrap();
    fs::copy(
        root().join("docs/examples/papercolor-light.toml"),
        themes.join("papercolor-light.toml"),
    )
    .unwrap();
    (config, state)
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn vanadis(config: &Path, state: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vanadis"))
        .args(args)
        .env("VANADIS_CONFIG", config)
        .env("XDG_STATE_HOME", state)
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
fn finds_nothing_wrong_with_the_golden_outputs() {
    let (config, state) = golden("check-golden");
    let output = vanadis(&config, &state, &["check", "papercolor-light"]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(stdout(&output), "checked 11 targets\n");
}

#[test]
fn reports_an_output_that_was_edited_by_hand() {
    let (config, state) = golden("check-edited");
    let edited = config.join("expected/hunk/config.toml");
    let text = fs::read_to_string(&edited).unwrap();
    fs::write(&edited, text.replace("#eeeeee", "#ffffff")).unwrap();

    let output = vanadis(&config, &state, &["check", "papercolor-light"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("hunk"), "{}", stdout(&output));
}

#[test]
fn leaves_every_other_target_alone_when_one_has_drifted() {
    let (config, state) = golden("check-edited-only");
    let edited = config.join("expected/hunk/config.toml");
    let text = fs::read_to_string(&edited).unwrap();
    fs::write(&edited, text.replace("#eeeeee", "#ffffff")).unwrap();

    let output = vanadis(&config, &state, &["check", "papercolor-light"]);
    assert_eq!(stdout(&output).lines().count(), 1, "{}", stdout(&output));
}

#[test]
fn reports_an_output_that_does_not_exist() {
    let (config, state) = golden("check-missing");
    fs::remove_file(config.join("expected/zsh/palette.zsh")).unwrap();

    let output = vanadis(&config, &state, &["check", "papercolor-light"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("zsh"), "{}", stdout(&output));
}

#[test]
fn reports_every_token_the_assigned_theme_does_not_define() {
    let (config, state) = tree("check-undefined", "apply-broken");
    let output = vanadis(&config, &state, &["check", "paper-light"]);
    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("colors.nope"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn checks_only_the_target_that_was_named() {
    let (config, state) = tree("check-only", "apply-broken");
    let output = vanadis(
        &config,
        &state,
        &["check", "paper-light", "--only", "first"],
    );
    assert!(!output.status.success());
    assert!(stdout(&output).contains("first"), "{}", stdout(&output));
    assert!(
        !stdout(&output).contains("colors.nope"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn checks_the_theme_the_state_file_records() {
    let (config, state) = tree("check-state", "apply");
    let applied = vanadis(&config, &state, &["apply", "paper-light"]);
    assert!(applied.status.success(), "{}", stderr(&applied));

    let output = vanadis(&config, &state, &["check"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn reports_a_target_edited_since_it_was_applied() {
    let (config, state) = tree("check-state-drift", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    fs::write(config.join("out/one.conf"), "bg=#ffffff\n").unwrap();

    let output = vanadis(&config, &state, &["check"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("one"), "{}", stdout(&output));
}

#[test]
fn follows_the_theme_a_partial_apply_moved_a_target_to() {
    let (config, state) = tree("check-state-only", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    vanadis(&config, &state, &["apply", "nord", "--only", "two"]);

    let output = vanadis(&config, &state, &["check"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn refuses_a_check_that_names_no_theme_before_one_is_applied() {
    let (config, state) = tree("check-unapplied", "apply");
    let output = vanadis(&config, &state, &["check"]);
    assert!(!output.status.success());
    assert!(!stderr(&output).is_empty());
}

#[test]
fn resolves_a_theme_through_the_auto_table() {
    let (config, state) = tree("check-auto", "apply");
    vanadis(&config, &state, &["apply", "--variant", "dark"]);

    let output = vanadis(&config, &state, &["check", "--variant", "dark"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn refuses_an_only_that_names_no_target() {
    let (config, state) = tree("check-unknown-target", "apply");
    let output = vanadis(&config, &state, &["check", "paper-light", "--only", "nope"]);
    assert!(!output.status.success());
    assert!(!stderr(&output).is_empty());
}
