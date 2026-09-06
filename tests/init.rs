//! `init`, driven the way somebody at a terminal drives it.
//!
//! The acceptance condition `docs/init.md` states is here: an `apply` straight after an
//! `init` reproduces the file `init` read, byte for byte. It is checked against the golden
//! outputs in `tests/fixtures/expected/`, which are real config files.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A fresh config directory and a copy of the file being adopted.
struct Machine {
    config: PathBuf,
    state: PathBuf,
    home: PathBuf,
}

impl Machine {
    fn new(test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("init-{test}"));
        let _ = fs::remove_dir_all(&root);
        let machine = Self {
            config: root.join("config"),
            state: root.join("state"),
            home: root.join("home"),
        };
        for directory in [&machine.config, &machine.state, &machine.home] {
            fs::create_dir_all(directory).unwrap();
        }
        machine
    }

    /// Copies a golden output into the machine, as the file somebody already has.
    fn adopt(&self, fixture: &str) -> PathBuf {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/expected")
            .join(fixture);
        let destination = self.home.join(".config").join(fixture);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(&source, &destination).unwrap();
        destination
    }

    /// What `expected/<fixture>` holds, which is what an apply has to reproduce.
    fn original(fixture: &str) -> String {
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/expected")
                .join(fixture),
        )
        .unwrap()
    }

    fn run(&self, args: &[&str], input: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_vanadis"))
            .args(args)
            .env("VANADIS_CONFIG", &self.config)
            .env("XDG_STATE_HOME", &self.state)
            .env("HOME", &self.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn succeeded(output: &Output) -> bool {
    output.status.success()
}

/// One answer per line, for a run that names every value in the file.
fn answers(names: &[&str]) -> String {
    names.iter().fold(String::new(), |mut input, name| {
        input.push_str(name);
        input.push('\n');
        input
    })
}

/// The twelve values in `hunk/config.toml`, in the order the file first writes them, named
/// the way `docs/examples/papercolor-light.toml` names the same colours.
const HUNK: [&str; 12] = [
    "bg",
    "hover-bg",
    "border",
    "accent",
    "comment",
    "fg",
    "selection-bg",
    "ok",
    "error",
    "linenr",
    "warn",
    "visual",
];

/// The fourteen values in `rio/config.toml`, in the same order.
///
/// Ten are empty, because `hunk/config.toml` has already named those colours and `init`
/// offers the name back. That is the claim `docs/init.md` makes about the second file
/// onwards, checked rather than asserted.
const RIO: [&str; 14] = [
    "",
    "",
    "accent-alt",
    "",
    "",
    "string",
    "",
    "",
    "",
    "colors.crimson",
    "",
    "colors.purple",
    "",
    "keyword",
];

#[test]
fn writes_the_three_files_it_says_it_writes() {
    let machine = Machine::new("writes");
    let file = machine.adopt("hunk/config.toml");
    let output = machine.run(
        &["init", file.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );
    assert!(succeeded(&output), "{}", stderr(&output));

    assert!(machine.config.join("themes/paper-light.toml").is_file());
    assert!(
        machine
            .config
            .join("templates/hunk/config.toml.in")
            .is_file()
    );
    assert!(machine.config.join("config.toml").is_file());
}

#[test]
fn leaves_the_file_it_read_exactly_as_it_found_it() {
    let machine = Machine::new("untouched");
    let file = machine.adopt("hunk/config.toml");
    machine.run(
        &["init", file.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        Machine::original("hunk/config.toml")
    );
}

#[test]
fn applying_straight_after_an_init_reproduces_the_original_file() {
    let machine = Machine::new("round-trip");
    let file = machine.adopt("hunk/config.toml");
    let output = machine.run(
        &["init", file.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );
    assert!(succeeded(&output), "{}", stderr(&output));

    // Something other than the original, so a write that does not happen is not mistaken
    // for a write that reproduced it.
    fs::write(&file, "clobbered\n").unwrap();

    let applied = machine.run(&["apply", "paper-light"], "");
    assert!(succeeded(&applied), "{}", stderr(&applied));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        Machine::original("hunk/config.toml")
    );
}

#[test]
fn checks_clean_straight_after_an_init() {
    let machine = Machine::new("check");
    let file = machine.adopt("hunk/config.toml");
    machine.run(
        &["init", file.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );
    machine.run(&["apply", "paper-light"], "");

    let checked = machine.run(&["check"], "");
    assert!(succeeded(&checked), "{}", stdout(&checked));
}

#[test]
fn adopts_a_second_file_into_the_theme_the_first_one_wrote() {
    let machine = Machine::new("second");
    let first = machine.adopt("hunk/config.toml");
    machine.run(
        &["init", first.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );

    // Nine of rio's fourteen values are ones hunk already named, so the answer is the
    // return key. The five that are new are typed, and two of them belong outside `[role]`.
    let second = machine.adopt("rio/config.toml");
    let output = machine.run(
        &["init", second.to_str().unwrap()],
        &format!("\nrio\n{}", answers(&RIO)),
    );
    assert!(succeeded(&output), "{}", stderr(&output));

    let applied = machine.run(&["apply", "paper-light"], "");
    assert!(succeeded(&applied), "{}", stderr(&applied));
    assert_eq!(
        fs::read_to_string(&first).unwrap(),
        Machine::original("hunk/config.toml")
    );
    assert_eq!(
        fs::read_to_string(&second).unwrap(),
        Machine::original("rio/config.toml")
    );
}

#[test]
fn offers_the_name_a_value_already_carries() {
    let machine = Machine::new("offers");
    let first = machine.adopt("hunk/config.toml");
    machine.run(
        &["init", first.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );

    let second = machine.adopt("rio/config.toml");
    let output = machine.run(&["init", second.to_str().unwrap()], "\nrio\n");
    assert!(
        stdout(&output).contains("#444444  4 occurrences   token name? [role.fg]"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn names_the_core_tokens_the_theme_still_lacks() {
    let machine = Machine::new("missing");
    let file = machine.adopt("hunk/config.toml");
    let output = machine.run(
        &["init", file.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );
    let printed = stdout(&output);
    assert!(printed.contains("core tokens still undefined"), "{printed}");
    assert!(printed.contains("ansi.0"), "{printed}");
    assert!(!printed.contains("role.bg "), "{printed}");
}

#[test]
fn keeps_a_literal_the_dialogue_skipped() {
    let machine = Machine::new("skipped");
    let file = machine.adopt("herdr/host-colors.py");
    // The three hex values in the prose describing a terminal bug are not colours.
    let output = machine.run(
        &["init", file.to_str().unwrap()],
        "paper-light\nlight\nherdr\nfg\n-\n-\nbg\n",
    );
    assert!(succeeded(&output), "{}", stderr(&output));

    let template =
        fs::read_to_string(machine.config.join("templates/herdr/host-colors.py.in")).unwrap();
    assert!(template.contains("#ffffff"), "{template}");
    assert!(template.contains("#000000"), "{template}");

    let applied = machine.run(&["apply", "paper-light"], "");
    assert!(succeeded(&applied), "{}", stderr(&applied));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        Machine::original("herdr/host-colors.py")
    );
}

#[test]
fn splits_one_value_across_two_tokens_when_asked_to() {
    let machine = Machine::new("split");
    let file = machine.adopt("hunk/config.toml");
    let mut input = String::from("paper-light\nlight\nhunk\n");
    for name in HUNK {
        if name == "comment" {
            // `#878787` is written four times, and the file's own comments call line 22
            // inactive and line 24 comment.
            input.push_str("comment inactive\ninactive\ncomment\ncomment\ninactive\n");
        } else {
            input.push_str(name);
            input.push('\n');
        }
    }
    let output = machine.run(&["init", file.to_str().unwrap()], &input);
    assert!(succeeded(&output), "{}", stderr(&output));

    let theme = fs::read_to_string(machine.config.join("themes/paper-light.toml")).unwrap();
    assert!(theme.contains("comment = \"#878787\""), "{theme}");
    assert!(theme.contains("inactive = \"#878787\""), "{theme}");

    let template =
        fs::read_to_string(machine.config.join("templates/hunk/config.toml.in")).unwrap();
    assert!(template.contains("{{role.inactive}}"), "{template}");

    let applied = machine.run(&["apply", "paper-light"], "");
    assert!(succeeded(&applied), "{}", stderr(&applied));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        Machine::original("hunk/config.toml")
    );
}

#[test]
fn refuses_a_file_that_is_already_a_target_output() {
    let machine = Machine::new("duplicate");
    let file = machine.adopt("hunk/config.toml");
    machine.run(
        &["init", file.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );

    let output = machine.run(&["init", file.to_str().unwrap()], "paper-light\nhunk-2\n");
    assert!(!succeeded(&output));
    assert!(
        stderr(&output).contains("already the output"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn refuses_to_replace_a_template_that_is_already_there() {
    let machine = Machine::new("occupied");
    let file = machine.adopt("hunk/config.toml");
    let template = machine.config.join("templates/hunk/config.toml.in");
    fs::create_dir_all(template.parent().unwrap()).unwrap();
    fs::write(&template, "somebody else's\n").unwrap();

    let output = machine.run(
        &["init", file.to_str().unwrap()],
        &format!("paper-light\nlight\nhunk\n{}", answers(&HUNK)),
    );
    assert!(!succeeded(&output));
    assert_eq!(
        fs::read_to_string(&template).unwrap(),
        "somebody else's\n",
        "{}",
        stderr(&output)
    );
}

#[test]
fn writes_nothing_when_a_value_is_named_twice_for_two_colours() {
    let machine = Machine::new("conflict");
    let file = machine.adopt("hunk/config.toml");
    let output = machine.run(
        &["init", file.to_str().unwrap()],
        "paper-light\nlight\nhunk\nbg\nbg\n",
    );
    assert!(!succeeded(&output));
    assert!(!machine.config.join("themes/paper-light.toml").exists());
    assert!(!machine.config.join("config.toml").exists());
}

#[test]
fn takes_the_names_from_the_flags_instead_of_asking() {
    let machine = Machine::new("flags");
    let file = machine.adopt("hunk/config.toml");
    let output = machine.run(
        &[
            "init",
            file.to_str().unwrap(),
            "--theme",
            "paper-light",
            "--name",
            "hunk",
            "--variant",
            "light",
        ],
        &answers(&HUNK),
    );
    assert!(succeeded(&output), "{}", stderr(&output));
    assert!(machine.config.join("themes/paper-light.toml").is_file());
}

#[test]
fn suggests_the_target_name_the_path_carries() {
    let machine = Machine::new("suggests");
    let file = machine.adopt("hunk/config.toml");
    let output = machine.run(&["init", file.to_str().unwrap()], "paper-light\nlight\n");
    assert!(
        stdout(&output).contains("target name? [hunk]"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn reports_a_notation_it_cannot_substitute() {
    let machine = Machine::new("unhandled");
    let file = machine.home.join(".config/tool/colors.conf");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        "a = \"#EEEEEE\"\nb = rgb(1, 2, 3)\nc = \"#eee\"\nd = \"bold red\"\n",
    )
    .unwrap();

    let output = machine.run(
        &["init", file.to_str().unwrap()],
        "paper-light\nlight\ntool\n",
    );
    let printed = stdout(&output);
    assert!(printed.contains("unhandled colour notations:"), "{printed}");
    assert!(printed.contains("uppercase hex"), "{printed}");
    assert!(printed.contains("function notation"), "{printed}");
    assert!(printed.contains("three-digit hex"), "{printed}");
    assert!(printed.contains("colour name"), "{printed}");
}

/// Every golden output, with every colour in it bound to a token of its own.
///
/// The dialogue is not involved: this is the renderer's half of the guarantee, checked
/// against all eleven real files rather than the two the runs above walk through.
#[test]
fn reproduces_every_file_in_the_corpus() {
    use std::collections::BTreeMap;
    use vanadis::init::{Binding, Draft};
    use vanadis::{ThemeId, TokenPath, Variant};

    let expected = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/expected");
    let mut checked = 0;
    let mut files: Vec<PathBuf> = walk(&expected);
    files.sort();

    for file in files {
        let source = fs::read_to_string(&file).unwrap();
        let scan = vanadis::init::scan(&source);

        let mut bindings = Vec::new();
        let mut tokens = BTreeMap::new();
        for (index, colour) in scan.colours().iter().enumerate() {
            let token = TokenPath::parse(&format!("role.c{index}")).unwrap();
            tokens.insert(token.clone(), colour.value().to_owned());
            for occurrence in colour.occurrences() {
                bindings.push(Binding::new(occurrence, token.clone()));
            }
        }

        let name = vanadis::TargetName::parse("target").unwrap();
        let id = ThemeId::parse("paper-light").unwrap();
        let planned = vanadis::init::plan(Draft {
            directory: Path::new("/config"),
            home: None,
            name: &name,
            output: &file,
            source: &source,
            id: &id,
            variant: Variant::Light,
            existing: None,
            config: "",
            bindings: &bindings,
            tokens: &tokens,
        });
        assert!(planned.is_ok(), "{}: {:?}", file.display(), planned.err());
        checked += 1;
    }
    assert_eq!(checked, 11);
}

/// Every file under `directory`, however deep.
fn walk(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .flat_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                walk(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}
