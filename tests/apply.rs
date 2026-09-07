//! `vanadis apply` and `vanadis cycle`, driven as the user drives them, against a copy of a
//! fixture tree.
//!
//! `cycle` is `apply` reached by a different route, so it is here rather than in a file of
//! its own: it shares the fixture, the harness and everything after the theme is chosen.

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

    // git records the executable bit and nothing else, so the rest of the template's mode is
    // whatever umask the working tree was checked out under. The property is the
    // relationship, and a constant here passes under `umask 022` and fails under `umask 002`.
    let template = mode(&config.join("templates/script.in"));
    assert_eq!(
        template & 0o100,
        0o100,
        "the fixture template is executable"
    );

    vanadis(&config, &state, &["apply", "paper-light"]);
    assert_eq!(mode(&config.join("out/host-colors.sh")), template);
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
    assert!(
        stderr(&output).contains("vanadis apply <theme>"),
        "{}",
        stderr(&output)
    );
}

/// The state a partial apply would record is worked out at the top of the commit, so the
/// refusal lands before the first byte.
#[test]
fn writes_nothing_when_a_partial_apply_is_refused() {
    let (config, state) = workspace("apply-only-first-writes", "apply");
    vanadis(&config, &state, &["apply", "nord", "--only", "two"]);
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

#[test]
fn writes_nothing_when_it_is_only_told_what_it_would_do() {
    let (config, state) = workspace("apply-dry-run", "apply");
    let output = vanadis(&config, &state, &["apply", "paper-light", "--dry-run"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!config.join("out").exists());
}

#[test]
fn names_the_targets_it_would_write() {
    let (config, state) = workspace("apply-dry-run-reports", "apply");
    let output = vanadis(&config, &state, &["apply", "paper-light", "--dry-run"]);
    assert!(
        stdout(&output).contains("would write one"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn names_the_reload_it_would_run() {
    let (config, state) = workspace("apply-dry-run-reload", "apply");
    let output = vanadis(&config, &state, &["apply", "paper-light", "--dry-run"]);
    assert!(
        stdout(&output).contains("would run: true"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn records_no_theme_when_it_is_only_told_what_it_would_do() {
    let (config, state) = workspace("apply-dry-run-state", "apply");
    vanadis(&config, &state, &["apply", "paper-light", "--dry-run"]);
    let output = vanadis(&config, &state, &["current"]);
    assert!(!output.status.success());
}

/// Adding targets one at a time is when `--only` is wanted, and is exactly when no whole
/// apply has happened yet. A preview records nothing, so it needs no state to record against.
#[test]
fn previews_a_partial_apply_before_a_whole_one() {
    let (config, state) = workspace("apply-dry-run-only", "apply");
    let output = vanadis(
        &config,
        &state,
        &["apply", "nord", "--only", "two", "--dry-run"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("would write two"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn diffs_one_target_before_a_whole_apply() {
    let (config, state) = workspace("apply-diff-only-first", "apply");
    let output = vanadis(
        &config,
        &state,
        &["apply", "nord", "--only", "two", "--diff"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("+background = \"#2e3440\""),
        "{}",
        stdout(&output)
    );
}

#[test]
fn shows_a_unified_diff_of_what_would_change() {
    let (config, state) = workspace("apply-diff", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let output = vanadis(&config, &state, &["apply", "nord", "--diff"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let two = config.join("out/two.conf");
    let two = two.display();
    let stdout = stdout(&output);
    assert!(
        stdout.contains(&format!("--- {two}\n+++ {two}\n")),
        "{stdout}"
    );
    assert!(stdout.contains("-background = \"#eeeeee\""), "{stdout}");
    assert!(stdout.contains("+background = \"#2e3440\""), "{stdout}");
}

#[test]
fn writes_nothing_when_it_is_only_showing_a_diff() {
    let (config, state) = workspace("apply-diff-writes", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    vanadis(&config, &state, &["apply", "nord", "--diff"]);
    assert_eq!(
        read(&config.join("out/two.conf")),
        "background = \"#eeeeee\"\n"
    );
}

#[test]
fn shows_no_diff_for_a_target_that_would_not_change() {
    let (config, state) = workspace("apply-diff-unchanged", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let output = vanadis(&config, &state, &["apply", "paper-light", "--diff"]);
    assert!(!stdout(&output).contains("---"), "{}", stdout(&output));
    assert!(
        !stdout(&output).contains("would write"),
        "{}",
        stdout(&output)
    );
}

/// `docs/config.md`: the position is the applied theme's place in the list.
#[test]
fn applies_the_theme_after_the_one_in_use() {
    let (config, state) = workspace("cycle-next", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let output = vanadis(&config, &state, &["cycle"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with("applied nord\n"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn wraps_the_cycle_at_the_end_of_the_list() {
    let (config, state) = workspace("cycle-wrap", "apply");
    vanadis(&config, &state, &["apply", "ink-dark"]);
    let output = vanadis(&config, &state, &["cycle"]);
    assert!(
        stdout(&output).starts_with("applied paper-light\n"),
        "{}",
        stdout(&output)
    );
}

/// A machine that has applied nothing has no position, so the cycle starts.
#[test]
fn starts_the_cycle_when_nothing_has_been_applied() {
    let (config, state) = workspace("cycle-fresh", "apply");
    let output = vanadis(&config, &state, &["cycle"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with("applied paper-light\n"),
        "{}",
        stdout(&output)
    );
}

/// `sea-light` is a theme the cycle does not name, which `apply` is free to have applied.
#[test]
fn starts_the_cycle_from_a_theme_the_list_does_not_name() {
    let (config, state) = workspace("cycle-outside", "apply");
    vanadis(&config, &state, &["apply", "sea-light"]);
    let output = vanadis(&config, &state, &["cycle"]);
    assert!(
        stdout(&output).starts_with("applied paper-light\n"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn writes_the_outputs_of_the_theme_it_steps_to() {
    let (config, state) = workspace("cycle-writes", "apply");
    vanadis(&config, &state, &["apply", "ink-dark"]);
    vanadis(&config, &state, &["cycle"]);
    assert_eq!(read(&config.join("out/one.conf")), "bg=#eeeeee\n");
}

/// Two runs step two places, which is the whole point of reading the position back.
#[test]
fn steps_one_place_per_run() {
    let (config, state) = workspace("cycle-twice", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    vanadis(&config, &state, &["cycle"]);
    let output = vanadis(&config, &state, &["cycle"]);
    assert!(
        stdout(&output).starts_with("applied ink-dark\n"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn writes_nothing_on_a_dry_run() {
    let (config, state) = workspace("cycle-dry", "apply");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let before = read(&config.join("out/one.conf"));

    let output = vanadis(&config, &state, &["cycle", "--dry-run"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with("would apply nord\n"),
        "{}",
        stdout(&output)
    );
    assert_eq!(read(&config.join("out/one.conf")), before);

    // The position did not move either, so the next real cycle still steps to `nord`.
    let output = vanadis(&config, &state, &["cycle"]);
    assert!(
        stdout(&output).starts_with("applied nord\n"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn says_a_config_that_writes_no_cycle_has_nothing_to_step_through() {
    let (config, state) = workspace("cycle-none", "apply-reload");
    let output = vanadis(&config, &state, &["cycle"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("[cycle]"), "{}", stderr(&output));
}

/// A directory on the way to an output can be a link, which the path `config.toml` spells
/// does not show. The dry-run names the file the write would land on.
#[test]
fn names_the_file_a_write_resolves_to() {
    let (config, state) = workspace("apply-dry-run-resolved", "apply");
    let elsewhere = config.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, config.join("out")).unwrap();

    let output = vanadis(&config, &state, &["apply", "paper-light", "--dry-run"]);
    let printed = stdout(&output);
    let expected = format!(
        "one: {} resolves to {}",
        config.join("out/one.conf").display(),
        fs::canonicalize(&elsewhere)
            .unwrap()
            .join("one.conf")
            .display()
    );
    assert!(printed.contains(&expected), "{printed}");
}

#[test]
fn names_the_file_a_diff_would_be_written_to() {
    let (config, state) = workspace("apply-diff-resolved", "apply");
    let elsewhere = config.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, config.join("out")).unwrap();

    let output = vanadis(&config, &state, &["apply", "paper-light", "--diff"]);
    assert!(
        stdout(&output).contains("resolves to"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn says_nothing_about_an_output_that_resolves_to_itself() {
    let (config, state) = workspace("apply-dry-run-unresolved", "apply");
    let output = vanadis(&config, &state, &["apply", "paper-light", "--dry-run"]);
    assert!(
        !stdout(&output).contains("resolves to"),
        "{}",
        stdout(&output)
    );
}

/// An output that is itself a link is replaced rather than written through, so the file the
/// write lands on is the link and there is nothing to resolve.
#[test]
fn does_not_follow_an_output_that_is_itself_a_link() {
    let (config, state) = workspace("apply-dry-run-linked-output", "apply");
    let elsewhere = config.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("one.conf"), "bg=#000000\n").unwrap();
    fs::create_dir_all(config.join("out")).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("one.conf"), config.join("out/one.conf")).unwrap();

    let output = vanadis(&config, &state, &["apply", "paper-light", "--dry-run"]);
    assert!(
        !stdout(&output).contains("resolves to"),
        "{}",
        stdout(&output)
    );
}
