//! `import`, driven the way somebody at a terminal drives it.
//!
//! The two acceptance conditions the issue states are here: `apply` works on a theme the
//! moment `import` has written it, and a scheme on disk converts with no network.
//!
//! The cache is built from `tests/fixtures/schemes/`, the upstream files
//! `tests/scheme.rs` already documents. `tests/fixtures/cache/` is not used: its schemes are
//! headers with two palette entries, enough for `search` to print a row and not enough to
//! convert.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh config directory, and a cache holding the schemes a test names.
struct Machine {
    config: PathBuf,
    state: PathBuf,
    cache: PathBuf,
}

impl Machine {
    /// A machine whose cache holds `schemes`, each named as `<system>/<id>.yaml`.
    fn new(test: &str, schemes: &[&str]) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("import-{test}"));
        let _ = fs::remove_dir_all(&root);
        let machine = Self {
            config: root.join("config"),
            state: root.join("state"),
            cache: root.join("cache"),
        };
        for directory in [&machine.config, &machine.state, &machine.cache] {
            fs::create_dir_all(directory).unwrap();
        }
        for scheme in schemes {
            let destination = machine.cache.join("vanadis/schemes").join(scheme);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(fixture(scheme), &destination).unwrap();
        }
        machine
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_vanadis"))
            .args(args)
            .env("VANADIS_CONFIG", &self.config)
            .env("XDG_STATE_HOME", &self.state)
            .env("XDG_CACHE_HOME", &self.cache)
            .output()
            .unwrap()
    }

    /// The theme file `id` was written to.
    fn theme(&self, id: &str) -> PathBuf {
        self.config.join("themes").join(format!("{id}.toml"))
    }

    fn written(&self, id: &str) -> String {
        fs::read_to_string(self.theme(id)).unwrap()
    }
}

/// One upstream scheme out of `tests/fixtures/schemes/`.
fn fixture(scheme: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schemes")
        .join(scheme)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn writes_a_theme_from_a_cached_scheme() {
    let machine = Machine::new("cached", &["base16/nord.yaml"]);
    let output = machine.run(&["import", "base16/nord"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(machine.theme("nord").is_file());
}

#[test]
fn says_what_it_imported_and_where_it_put_it() {
    let machine = Machine::new("says", &["base16/nord.yaml"]);
    let output = machine.run(&["import", "base16/nord"]);
    let said = stdout(&output);
    assert!(said.contains("imported base16/nord as nord"), "{said}");
    assert!(said.contains("dark  Nord  arcticicestudio"), "{said}");
    assert!(
        said.contains(machine.theme("nord").to_str().unwrap()),
        "{said}"
    );
}

/// `docs/schemes.md`: the first line of the file records where the scheme came from.
#[test]
fn records_where_the_scheme_came_from() {
    let machine = Machine::new("provenance", &["base16/nord.yaml"]);
    machine.run(&["import", "base16/nord"]);
    assert!(
        machine
            .written("nord")
            .starts_with("# imported by vanadis from base16/nord\n[meta]\n"),
        "{}",
        machine.written("nord")
    );
}

#[test]
fn keeps_the_author_the_scheme_credits() {
    let machine = Machine::new("author", &["base16/nord.yaml"]);
    machine.run(&["import", "base16/nord"]);
    assert!(
        machine
            .written("nord")
            .contains("author = \"arcticicestudio\""),
        "{}",
        machine.written("nord")
    );
}

/// The identifier is the filename, so a bare one is looked for in every system.
#[test]
fn finds_a_bare_identifier_in_a_system_that_is_not_the_first() {
    let machine = Machine::new("bare-later", &["base24/dracula.yaml"]);
    let output = machine.run(&["import", "dracula"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("imported base24/dracula"),
        "{}",
        stdout(&output)
    );
}

/// `nord` is a base16 scheme and a tinted8 one. The bare identifier takes the first system
/// that holds it, which `docs/schemes.md` orders base16, base24, tinted8.
#[test]
fn takes_the_first_system_that_holds_a_bare_identifier() {
    let machine = Machine::new("bare-order", &["base16/nord.yaml", "tinted8/nord.yaml"]);
    let output = machine.run(&["import", "nord"]);
    assert!(
        stdout(&output).contains("imported base16/nord"),
        "{}",
        stdout(&output)
    );
}

/// Qualifying it is how the other one is reached.
#[test]
fn reaches_the_other_system_through_the_qualified_identifier() {
    let machine = Machine::new("qualified", &["base16/nord.yaml", "tinted8/nord.yaml"]);
    let output = machine.run(&["import", "tinted8/nord"]);
    assert!(
        stdout(&output).contains("imported tinted8/nord"),
        "{}",
        stdout(&output)
    );
    // `colors.orange` is a tinted8 palette key. No base16 scheme carries one.
    assert!(machine.written("nord").contains("orange = "));
}

#[test]
fn refuses_a_theme_that_is_already_there() {
    let machine = Machine::new("exists", &["base16/nord.yaml", "tinted8/nord.yaml"]);
    machine.run(&["import", "base16/nord"]);
    let before = machine.written("nord");

    let output = machine.run(&["import", "tinted8/nord"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains(machine.theme("nord").to_str().unwrap()),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("--force"), "{}", stderr(&output));
    assert_eq!(machine.written("nord"), before);
}

#[test]
fn writes_over_a_theme_when_force_says_to() {
    let machine = Machine::new("force", &["base16/nord.yaml", "tinted8/nord.yaml"]);
    machine.run(&["import", "base16/nord"]);
    let output = machine.run(&["import", "tinted8/nord", "--force"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        machine
            .written("nord")
            .starts_with("# imported by vanadis from tinted8/nord\n"),
        "{}",
        machine.written("nord")
    );
}

/// `docs/schemes.md`: converting a file on disk reads no cache and no network.
#[test]
fn converts_a_file_on_disk_with_no_cache() {
    let machine = Machine::new("file", &[]);
    let scheme = fixture("base16/solarized-light.yaml");
    let output = machine.run(&["import", scheme.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(machine.theme("solarized-light").is_file());
}

/// A loose file has no directory to take a system from, so its own `system` is read.
#[test]
fn reads_the_system_a_file_on_disk_declares() {
    let machine = Machine::new("file-system", &[]);
    let scheme = fixture("tinted8/gruvbox-dark.yaml");
    machine.run(&["import", scheme.to_str().unwrap()]);
    assert!(machine.written("gruvbox-dark").contains("orange = "));
}

#[test]
fn records_a_file_on_disk_by_its_path() {
    let machine = Machine::new("file-provenance", &[]);
    let scheme = fixture("base16/solarized-light.yaml");
    machine.run(&["import", scheme.to_str().unwrap()]);
    assert!(
        machine.written("solarized-light").starts_with(&format!(
            "# imported by vanadis from {}\n",
            scheme.display()
        )),
        "{}",
        machine.written("solarized-light")
    );
}

#[test]
fn reports_a_file_that_declares_no_system() {
    let machine = Machine::new("no-system", &[]);
    let scheme = machine.config.join("plain.yaml");
    fs::write(&scheme, "name: \"Plain\"\nvariant: \"dark\"\n").unwrap();
    let output = machine.run(&["import", scheme.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no `system` field"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn says_how_to_fill_a_cache_that_is_not_there() {
    let machine = Machine::new("no-cache", &[]);
    let output = machine.run(&["import", "nord"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("vanadis remote update"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn says_how_to_look_for_an_identifier_the_cache_does_not_hold() {
    let machine = Machine::new("unknown", &["base16/nord.yaml"]);
    let output = machine.run(&["import", "solarized"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("vanadis search"),
        "{}",
        stderr(&output)
    );
}

/// The acceptance condition: an `apply` straight after an `import` renders the new theme.
#[test]
fn applies_a_theme_the_moment_it_is_imported() {
    let machine = Machine::new("apply", &["base16/nord.yaml"]);
    let templates = machine.config.join("templates");
    fs::create_dir_all(&templates).unwrap();
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/templates/zsh/palette.zsh.in"),
        templates.join("palette.zsh.in"),
    )
    .unwrap();
    fs::write(
        machine.config.join("config.toml"),
        "[[targets]]\nname = \"palette\"\ntemplate = \"templates/palette.zsh.in\"\noutput = \"out/palette.zsh\"\n",
    )
    .unwrap();

    let imported = machine.run(&["import", "base16/nord"]);
    assert!(imported.status.success(), "{}", stderr(&imported));

    let applied = machine.run(&["apply", "nord"]);
    assert!(applied.status.success(), "{}", stderr(&applied));

    let rendered = fs::read_to_string(machine.config.join("out/palette.zsh")).unwrap();
    assert!(rendered.contains("#2e3440"), "{rendered}");
}
