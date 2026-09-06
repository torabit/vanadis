//! `vanadis render`, and the export loop it exists for.
//!
//! `docs/theme-format.md` states that writing a base16 scheme out of a vanadis theme is a
//! template and not a feature. `tests/fixtures/export/base16.yaml.in` is that template, and
//! `round_trips_a_base16_scheme` is what holds the claim to account: a scheme imported, written
//! back out through the renderer, and imported again carries the same sixteen colours.
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
