//! `vanadis render`, in both its forms: a template with a theme named for it, and a target.
//!
//! `docs/theme-format.md` states that writing a base16 scheme out of a vanadis theme is a
//! template and not a feature. `tests/fixtures/export/base16.yaml.in` is that template, and
//! `round_trips_a_base16_scheme` is what holds the claim to account: a scheme imported, written
//! back out through the renderer, and imported again carries the same sixteen colours.
//!
//! The target form answers a different question, and the tests below it hold the one claim
//! that matters: what it prints is what an apply would write, down to the theme a partial
//! apply left that target on.
//!
//! Separate from `tests/render.rs`, which is the renderer's own golden test. This drives the
//! binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

/// A copy of the `apply` fixture tree, and an empty state directory beside it.
///
/// The tree is copied rather than used in place so a render can be shown to leave the targets
/// and the state alone.
fn workspace(test: &str) -> (PathBuf, PathBuf) {
    let workspace = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("export-{test}"));
    let _ = fs::remove_dir_all(&workspace);
    let config = workspace.join("config");
    copy(&root().join("tests/fixtures/apply"), &config);
    let state = workspace.join("state");
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

/// Runs the binary from `root()`, so a relative template resolves against the repository.
fn vanadis(config: &Path, state: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vanadis"))
        .args(args)
        .current_dir(root())
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

/// The `colors.*` tokens of one theme, read back through `get --json`.
fn colours(config: &Path, state: &Path, theme: &str) -> Vec<(String, String)> {
    let output = vanadis(config, state, &["get", "--json", "--theme", theme]);
    assert!(output.status.success(), "{}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let mut colours: Vec<(String, String)> = json
        .as_object()
        .unwrap()
        .iter()
        .filter(|(path, _)| path.starts_with("colors."))
        .map(|(path, value)| (path.clone(), value.as_str().unwrap().to_owned()))
        .collect();
    colours.sort();
    colours
}

/// The acceptance condition: a scheme survives being written out and read back.
///
/// `import` reads the rendered file's own `system` key, which the template writes, so the
/// second import goes down the same path the first one did.
#[test]
fn round_trips_a_base16_scheme() {
    let (config, state) = workspace("round-trip");
    let scheme = root().join("tests/fixtures/schemes/base16/nord.yaml");

    // The fixture tree already carries a hand-written `nord.toml`. `--force` replaces it, so
    // what is rendered below is the converter's output and not somebody's palette.
    let imported = vanadis(
        &config,
        &state,
        &["import", scheme.to_str().unwrap(), "--force"],
    );
    assert!(imported.status.success(), "{}", stderr(&imported));

    let rendered = vanadis(
        &config,
        &state,
        &[
            "render",
            "tests/fixtures/export/base16.yaml.in",
            "--theme",
            "nord",
        ],
    );
    assert!(rendered.status.success(), "{}", stderr(&rendered));

    let written = config.join("nord-exported.yaml");
    fs::write(&written, stdout(&rendered)).unwrap();
    let again = vanadis(&config, &state, &["import", written.to_str().unwrap()]);
    assert!(again.status.success(), "{}", stderr(&again));

    let before = colours(&config, &state, "nord");
    let after = colours(&config, &state, "nord-exported");
    assert_eq!(before.len(), 16, "base16 fills sixteen slots");
    assert_eq!(before, after);
}

/// The whole reason the command exists: a target's file is written from the active theme, and
/// an export must not be.
#[test]
fn renders_the_theme_it_is_given_and_not_the_applied_one() {
    let (config, state) = workspace("named");
    vanadis(&config, &state, &["apply", "paper-light"]);

    let output = vanadis(
        &config,
        &state,
        &[
            "render",
            "tests/fixtures/apply/templates/simple.in",
            "--theme",
            "ink-dark",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "bg=#111111\n");
}

#[test]
fn leaves_the_targets_and_the_state_alone() {
    let (config, state) = workspace("untouched");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let written = fs::read_to_string(config.join("out/one.conf")).unwrap();
    let recorded = fs::read_to_string(state.join("vanadis/state.toml")).unwrap();

    vanadis(
        &config,
        &state,
        &[
            "render",
            "tests/fixtures/apply/templates/simple.in",
            "--theme",
            "ink-dark",
        ],
    );

    assert_eq!(
        fs::read_to_string(config.join("out/one.conf")).unwrap(),
        written
    );
    assert_eq!(
        fs::read_to_string(state.join("vanadis/state.toml")).unwrap(),
        recorded
    );
}

/// `--theme` is required, so there is no way to spell "the applied one" by leaving it out.
#[test]
fn refuses_to_run_without_a_theme() {
    let (config, state) = workspace("no-theme");
    let output = vanadis(
        &config,
        &state,
        &["render", "tests/fixtures/apply/templates/simple.in"],
    );
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--theme"), "{}", stderr(&output));
}

/// The template is rendered whole before anything is printed, so a failure leaves stdout
/// empty rather than holding the part of the file that resolved.
#[test]
fn writes_nothing_when_the_theme_does_not_define_a_token() {
    let (config, state) = workspace("undefined");
    let template = config.join("undefined.in");
    fs::write(
        &template,
        "first = \"{{role.bg}}\"\nsecond = \"{{role.nonesuch}}\"\n",
    )
    .unwrap();

    let output = vanadis(
        &config,
        &state,
        &["render", template.to_str().unwrap(), "--theme", "nord"],
    );
    assert!(!output.status.success());
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("role.nonesuch"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn reports_a_theme_that_is_not_there() {
    let (config, state) = workspace("unknown-theme");
    let output = vanadis(
        &config,
        &state,
        &[
            "render",
            "tests/fixtures/apply/templates/simple.in",
            "--theme",
            "solarized",
        ],
    );
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
}

#[test]
fn reports_a_template_that_is_not_there() {
    let (config, state) = workspace("unknown-template");
    let output = vanadis(
        &config,
        &state,
        &["render", "nowhere.in", "--theme", "nord"],
    );
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("nowhere.in"),
        "{}",
        stderr(&output)
    );
}

/// A target renders to exactly what the apply that wrote it put on disk.
#[test]
fn renders_a_target_the_way_the_apply_wrote_it() {
    let (config, state) = workspace("target-applied");
    let applied = vanadis(&config, &state, &["apply", "paper-light"]);
    assert!(applied.status.success(), "{}", stderr(&applied));

    let output = vanadis(&config, &state, &["render", "--target", "one"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        fs::read_to_string(config.join("out/one.conf")).unwrap()
    );
}

/// The question `init` leaves behind: does this target render, before anything is applied?
#[test]
fn renders_a_target_in_a_config_nothing_has_been_applied_to() {
    let (config, state) = workspace("target-unapplied");
    let output = vanadis(
        &config,
        &state,
        &["render", "--target", "one", "--theme", "nord"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "bg=#2e3440\n");
    assert!(!state.join("vanadis/state.toml").exists());
}

/// A partial apply leaves one target on another theme, and the render follows it there.
#[test]
fn follows_a_partial_apply_off_the_theme_the_state_records() {
    let (config, state) = workspace("target-partial");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let partial = vanadis(&config, &state, &["apply", "ink-dark", "--only", "two"]);
    assert!(partial.status.success(), "{}", stderr(&partial));

    let moved = vanadis(&config, &state, &["render", "--target", "two"]);
    assert_eq!(stdout(&moved), "background = \"#111111\"\n");

    let stayed = vanadis(&config, &state, &["render", "--target", "one"]);
    assert_eq!(stdout(&stayed), "bg=#eeeeee\n");
}

#[test]
fn takes_the_theme_from_auto_for_a_background() {
    let (config, state) = workspace("target-variant");
    let output = vanadis(
        &config,
        &state,
        &["render", "--target", "one", "--variant", "dark"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "bg=#111111\n");
}

/// A target pinned to its own themes renders from the one it names for the mode being asked
/// for, which is what makes this the same render an apply does.
#[test]
fn resolves_the_themes_a_target_pins_itself_to() {
    let (config, state) = workspace("target-pinned");
    let output = vanadis(
        &config,
        &state,
        &["render", "--target", "pinned", "--theme", "paper-light"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "bg=#e0f0ff\n");
}

/// The acceptance loop: write the render back over the output, and `check` is clean.
#[test]
fn writes_back_over_an_output_that_check_then_passes() {
    let (config, state) = workspace("target-round-trip");
    vanadis(&config, &state, &["apply", "paper-light"]);
    fs::write(config.join("out/one.conf"), "bg=#000000\n").unwrap();

    let drifted = vanadis(&config, &state, &["check", "--only", "one"]);
    assert!(!drifted.status.success(), "the output has drifted");

    let output = vanadis(&config, &state, &["render", "--target", "one"]);
    fs::write(config.join("out/one.conf"), stdout(&output)).unwrap();

    let checked = vanadis(&config, &state, &["check", "--only", "one"]);
    assert!(checked.status.success(), "{}", stdout(&checked));
}

#[test]
fn leaves_the_output_and_the_state_alone_for_a_target() {
    let (config, state) = workspace("target-untouched");
    vanadis(&config, &state, &["apply", "paper-light"]);
    let written = fs::read_to_string(config.join("out/one.conf")).unwrap();
    let recorded = fs::read_to_string(state.join("vanadis/state.toml")).unwrap();

    let output = vanadis(
        &config,
        &state,
        &["render", "--target", "one", "--theme", "ink-dark"],
    );
    assert_eq!(stdout(&output), "bg=#111111\n");

    assert_eq!(
        fs::read_to_string(config.join("out/one.conf")).unwrap(),
        written
    );
    assert_eq!(
        fs::read_to_string(state.join("vanadis/state.toml")).unwrap(),
        recorded
    );
}

#[test]
fn reports_a_target_the_config_does_not_have() {
    let (config, state) = workspace("target-unknown");
    let output = vanadis(
        &config,
        &state,
        &["render", "--target", "nonesuch", "--theme", "nord"],
    );
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("nonesuch"), "{}", stderr(&output));
}

/// Naming neither a theme nor a background with nothing applied is the one case `check`
/// answers the same way: there is no theme to render from.
#[test]
fn refuses_a_target_with_no_theme_and_nothing_applied() {
    let (config, state) = workspace("target-no-theme");
    let output = vanadis(&config, &state, &["render", "--target", "one"]);
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("no theme has been applied"),
        "{}",
        stderr(&output)
    );
}

/// The two forms are separate: a template is a path with a theme named for it, and reading
/// `[auto]` is the target form's business.
#[test]
fn refuses_a_background_alongside_a_template() {
    let (config, state) = workspace("template-variant");
    let output = vanadis(
        &config,
        &state,
        &[
            "render",
            "tests/fixtures/apply/templates/simple.in",
            "--variant",
            "dark",
        ],
    );
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
}

#[test]
fn refuses_a_template_and_a_target_at_once() {
    let (config, state) = workspace("both-forms");
    let output = vanadis(
        &config,
        &state,
        &[
            "render",
            "tests/fixtures/apply/templates/simple.in",
            "--target",
            "one",
            "--theme",
            "nord",
        ],
    );
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
}
