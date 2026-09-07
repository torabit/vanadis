//! `coloris check`, driven as the user drives it.
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

fn coloris(config: &Path, state: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_coloris"))
        .args(args)
        .env("COLORIS_CONFIG", config)
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
    let output = coloris(&config, &state, &["check", "papercolor-light"]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(stdout(&output), "checked 11 targets\n");
}

#[test]
fn reports_an_output_that_was_edited_by_hand() {
    let (config, state) = golden("check-edited");
    let edited = config.join("expected/hunk/config.toml");
    let text = fs::read_to_string(&edited).unwrap();
    fs::write(&edited, text.replace("#eeeeee", "#ffffff")).unwrap();

    let output = coloris(&config, &state, &["check", "papercolor-light"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("hunk"), "{}", stdout(&output));
}

#[test]
fn leaves_every_other_target_alone_when_one_has_drifted() {
    let (config, state) = golden("check-edited-only");
    let edited = config.join("expected/hunk/config.toml");
    let text = fs::read_to_string(&edited).unwrap();
    fs::write(&edited, text.replace("#eeeeee", "#ffffff")).unwrap();

    let output = coloris(&config, &state, &["check", "papercolor-light"]);
    assert_eq!(stdout(&output).lines().count(), 1, "{}", stdout(&output));
}

#[test]
fn reports_an_output_that_does_not_exist() {
    let (config, state) = golden("check-missing");
    fs::remove_file(config.join("expected/zsh/palette.zsh")).unwrap();

    let output = coloris(&config, &state, &["check", "papercolor-light"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("zsh"), "{}", stdout(&output));
}

#[test]
fn reports_every_token_the_assigned_theme_does_not_define() {
    let (config, state) = tree("check-undefined", "apply-broken");
    let output = coloris(&config, &state, &["check", "paper-light"]);
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
    let output = coloris(
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
    let applied = coloris(&config, &state, &["apply", "paper-light"]);
    assert!(applied.status.success(), "{}", stderr(&applied));

    let output = coloris(&config, &state, &["check"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn reports_a_target_edited_since_it_was_applied() {
    let (config, state) = tree("check-state-drift", "apply");
    coloris(&config, &state, &["apply", "paper-light"]);
    fs::write(config.join("out/one.conf"), "bg=#ffffff\n").unwrap();

    let output = coloris(&config, &state, &["check"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("one"), "{}", stdout(&output));
}

#[test]
fn follows_the_theme_a_partial_apply_moved_a_target_to() {
    let (config, state) = tree("check-state-only", "apply");
    coloris(&config, &state, &["apply", "paper-light"]);
    coloris(&config, &state, &["apply", "nord", "--only", "two"]);

    let output = coloris(&config, &state, &["check"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn refuses_a_check_that_names_no_theme_before_one_is_applied() {
    let (config, state) = tree("check-unapplied", "apply");
    let output = coloris(&config, &state, &["check"]);
    assert!(!output.status.success());
    assert!(!stderr(&output).is_empty());
}

#[test]
fn resolves_a_theme_through_the_auto_table() {
    let (config, state) = tree("check-auto", "apply");
    coloris(&config, &state, &["apply", "--variant", "dark"]);

    let output = coloris(&config, &state, &["check", "--variant", "dark"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn refuses_an_only_that_names_no_target() {
    let (config, state) = tree("check-unknown-target", "apply");
    let output = coloris(&config, &state, &["check", "paper-light", "--only", "nope"]);
    assert!(!output.status.success());
    assert!(!stderr(&output).is_empty());
}

/// Removes `token` from the theme `id` in `config`, leaving the theme otherwise as written.
fn undefine(config: &Path, id: &str, token: &str) {
    let path = config.join("themes").join(format!("{id}.toml"));
    let text = fs::read_to_string(&path).unwrap();
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with(&format!("{token} = ")))
        .collect();
    fs::write(&path, kept.join("\n") + "\n").unwrap();
}

/// An applied tree, so the only thing left for `check` to find is what a test puts there.
fn applied(test: &str) -> (PathBuf, PathBuf) {
    let (config, state) = tree(test, "apply");
    let output = coloris(&config, &state, &["apply", "paper-light"]);
    assert!(output.status.success(), "{}", stderr(&output));
    (config, state)
}

#[test]
fn reports_a_core_role_the_theme_does_not_define() {
    let (config, state) = applied("check-core-role");
    undefine(&config, "paper-light", "linenr");

    let output = coloris(&config, &state, &["check", "paper-light"]);
    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("role.linenr"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn reports_a_core_ansi_slot_the_theme_does_not_define() {
    let (config, state) = applied("check-core-ansi");
    undefine(&config, "paper-light", "7");

    let output = coloris(&config, &state, &["check", "paper-light"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("ansi.7"), "{}", stdout(&output));
}

#[test]
fn passes_a_theme_that_defines_the_whole_core() {
    let (config, state) = applied("check-core-whole");
    let output = coloris(&config, &state, &["check", "paper-light"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn names_the_incomplete_theme_once_however_many_targets_are_on_it() {
    let (config, state) = applied("check-core-once");
    undefine(&config, "paper-light", "linenr");

    let output = coloris(&config, &state, &["check", "paper-light"]);
    let named = stdout(&output)
        .lines()
        .filter(|line| line.starts_with("paper-light:"))
        .count();
    assert_eq!(named, 1, "{}", stdout(&output));
}

#[test]
fn says_nothing_about_a_theme_no_target_in_the_run_is_on() {
    let (config, state) = applied("check-core-unused");
    undefine(&config, "sea-light", "linenr");

    // Only `pinned` is on sea-light, and `--only two` leaves it out of the run.
    let output = coloris(&config, &state, &["check", "paper-light", "--only", "two"]);
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn reports_the_theme_a_pinned_target_is_on() {
    let (config, state) = applied("check-core-pinned");
    undefine(&config, "sea-light", "linenr");

    let output = coloris(&config, &state, &["check", "paper-light"]);
    assert!(!output.status.success());
    assert!(stdout(&output).contains("sea-light"), "{}", stdout(&output));
}
