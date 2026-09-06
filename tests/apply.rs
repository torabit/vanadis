//! `vanadis apply`, driven as the user drives it, against a copy of a fixture tree.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A copy of `fixture`, and an empty state directory beside it.
fn workspace(test: &str, fixture: &str) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = fs::remove_dir_all(&root);

    let config = root.join("config");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture),
        &config,
    );

    let state = root.join("state");
    fs::create_dir_all(&state).unwrap();
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

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn writes_every_target_from_the_theme_it_is_given() {
    let (config, state) = workspace("apply-all", "apply");
    let output = vanadis(&config, &state, &["apply", "paper-light"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(read(&config.join("out/one.conf")), "bg=#eeeeee\n");
    assert_eq!(
        read(&config.join("out/two.conf")),
        "background = \"#eeeeee\"\n"
    );
}

#[test]
fn follows_a_target_pinned_to_its_own_theme() {
    let (config, state) = workspace("apply-pinned", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    assert_eq!(read(&config.join("out/pinned.conf")), "bg=#e0f0ff\n");
}

#[test]
fn resolves_a_theme_through_the_auto_table() {
    let (config, state) = workspace("apply-auto", "apply");
    let output = vanadis(&config, &state, &["apply", "--variant", "dark"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(read(&config.join("out/one.conf")), "bg=#111111\n");
}

#[test]
fn flips_a_pinned_target_with_everything_else() {
    let (config, state) = workspace("apply-auto-pinned", "apply");
    vanadis(&config, &state, &["apply", "--variant", "dark"]);
    assert_eq!(read(&config.join("out/pinned.conf")), "bg=#2e3440\n");
}

#[test]
fn records_the_theme_it_applied() {
    let (config, state) = workspace("apply-records", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let output = vanadis(&config, &state, &["current"]);
    assert_eq!(stdout(&output), "paper-light\n");
}

#[test]
fn reports_what_it_wrote() {
    let (config, state) = workspace("apply-reports", "apply");
    let output = vanadis(&config, &state, &["apply", "paper-light"]);
    assert!(stdout(&output).contains("one"), "{}", stdout(&output));
}

#[test]
fn gives_a_new_output_the_mode_of_its_template() {
    let (config, state) = workspace("apply-mode-new", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    assert_eq!(mode(&config.join("out/host-colors.sh")), 0o755);
}

#[test]
fn leaves_an_output_the_mode_it_already_had() {
    let (config, state) = workspace("apply-mode-kept", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);

    let script = config.join("out/host-colors.sh");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    vanadis(&config, &state, &["apply", "ink-dark"]);
    assert_eq!(mode(&script), 0o700);
}

#[test]
fn writes_nothing_when_one_template_reads_a_token_no_theme_defines() {
    let (config, state) = workspace("apply-undefined", "apply-broken");
    let output = vanadis(&config, &state, &["apply", "paper-light"]);
    assert!(!output.status.success());
    assert!(!config.join("out").exists());
}

#[test]
fn keeps_the_write_when_a_reload_fails() {
    let (config, state) = workspace("apply-reload", "apply-reload");
    let output = vanadis(&config, &state, &["apply", "paper-light"]);
    assert!(!output.status.success());
    assert_eq!(read(&config.join("out/one.conf")), "bg=#eeeeee\n");
}

#[test]
fn writes_only_the_target_that_was_named() {
    let (config, state) = workspace("apply-only", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    vanadis(&config, &state, &["apply", "nord", "--only", "two"]);

    assert_eq!(read(&config.join("out/one.conf")), "bg=#eeeeee\n");
    assert_eq!(
        read(&config.join("out/two.conf")),
        "background = \"#2e3440\"\n"
    );
}

#[test]
fn shows_the_target_a_partial_apply_moved() {
    let (config, state) = workspace("apply-only-current", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    vanadis(&config, &state, &["apply", "nord", "--only", "two"]);

    let output = vanadis(&config, &state, &["current"]);
    assert_eq!(stdout(&output), "paper-light\ntwo  nord\n");
}

#[test]
fn refuses_a_partial_apply_before_a_whole_one() {
    let (config, state) = workspace("apply-only-first", "apply");
    let output = vanadis(&config, &state, &["apply", "nord", "--only", "two"]);
    assert!(!output.status.success());
    assert!(!config.join("out").exists());
}

#[test]
fn refuses_a_theme_that_is_not_in_the_themes_directory() {
    let (config, state) = workspace("apply-unknown-theme", "apply");
    let output = vanadis(&config, &state, &["apply", "nope"]);
    assert!(!output.status.success());
    assert!(!config.join("out").exists());
}

#[test]
fn refuses_an_only_that_names_no_target() {
    let (config, state) = workspace("apply-unknown-target", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let output = vanadis(&config, &state, &["apply", "nord", "--only", "nope"]);
    assert!(!output.status.success());
}

#[test]
fn refuses_an_apply_that_names_neither_a_theme_nor_a_variant() {
    let (config, state) = workspace("apply-bare", "apply");
    let output = vanadis(&config, &state, &["apply"]);
    assert!(!output.status.success());
    assert!(!config.join("out").exists());
}
