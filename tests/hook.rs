//! `vanadis hook`, driven by evaluating what it prints in the shell it printed for.
//!
//! Asserting the snippet as a string would establish that the generator did not change and
//! nothing about whether a shell can parse the result, so every test here starts a real
//! interactive shell and reads what it prints. `docs/hook.md` decides the behaviour.
//!
//! A shell that is not installed is skipped, and skipping is refused under `CI`, so removing
//! the step that installs the shells fails the run rather than quietly passing it.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// One shell, and the lines that ask it what this test needs to know.
struct Shell {
    /// What `vanadis hook` is asked for, which is also the program to run.
    name: &'static str,
    /// The arguments that start it interactive, reading commands from stdin, with no rc file.
    arguments: &'static [&'static str],
    /// Evaluating the snippet, given the command that prints it.
    evaluate: &'static str,
    /// Printing the colour the sourced output sets, prefixed `bg=`.
    show: &'static str,
    /// Printing every prompt hook it has registered, one per line.
    hooks: &'static str,
    /// Printing the exit status of the command before it, prefixed `status=`.
    status: &'static str,
    /// Whether it needs a terminal before it will draw a prompt at all.
    ///
    /// fish emits `fish_prompt` only when it is drawing a prompt, and it draws none for a
    /// pipe, `-i` or not. zsh and bash run their prompt hooks either way, and giving them a
    /// terminal would only add the escape sequences a redraw writes.
    terminal: bool,
}

const ZSH: Shell = Shell {
    name: "zsh",
    arguments: &["-f", "-is"],
    evaluate: "eval \"$(@vanadis@ hook zsh)\"",
    show: "print \"bg=$VANADIS_BG\"",
    hooks: "print -l $precmd_functions",
    status: "print \"status=$?\"",
    terminal: false,
};

const BASH: Shell = Shell {
    name: "bash",
    arguments: &["--norc", "--noprofile", "-i"],
    evaluate: "eval \"$(@vanadis@ hook bash)\"",
    show: "echo \"bg=$VANADIS_BG\"",
    hooks: "echo \"$PROMPT_COMMAND\" | tr ';' '\\n'",
    status: "echo \"status=$?\"",
    terminal: false,
};

const FISH: Shell = Shell {
    name: "fish",
    arguments: &["--no-config", "-i"],
    // A fish snippet is piped into `source`, which is what fish users write for every tool in
    // this shape. `eval` would need the output collected into one argument first.
    evaluate: "@vanadis@ hook fish | source",
    show: "echo \"bg=$VANADIS_BG\"",
    hooks: "functions --handlers-type fish_prompt; or functions --handlers",
    status: "echo \"status=$status\"",
    terminal: true,
};

/// A copy of the `hook` fixture, and an empty state directory beside it.
fn workspace(test: &str) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = fs::remove_dir_all(&root);

    let config = root.join("config");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hook"),
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

fn vanadis(config: &Path, state: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vanadis"))
        .args(arguments)
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

/// Whether this machine has `shell`, refusing to skip under `CI`.
///
/// The CI workflow installs all three. A skip there would mean the gate stopped covering a
/// shell without anything saying so.
fn installed(shell: &Shell) -> bool {
    let found = Command::new(shell.name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok();

    assert!(
        found || std::env::var_os("CI").is_none(),
        "{} is not installed, and CI is supposed to install it",
        shell.name
    );
    if !found {
        eprintln!("skipping: {} is not installed", shell.name);
    }
    found
}

/// Runs `script` in `shell`, with `@vanadis@` standing for the binary under test.
///
/// The shell is interactive and reads its commands from stdin, so a prompt is drawn between
/// them and every prompt hook runs the way it does for a person at a keyboard. Prompts go to
/// stderr, so what comes back is only what the script printed.
fn drive(shell: &Shell, config: &Path, state: &Path, script: &str) -> String {
    let binary = env!("CARGO_BIN_EXE_vanadis");
    let script = script.replace("@vanadis@", binary);

    let mut child = started(shell)
        .env("VANADIS_CONFIG", config)
        .env("XDG_STATE_HOME", state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    // A terminal ends its lines with a carriage return, and prints the prompt and the echo of
    // what was typed around them. Only the lines the script printed are read, so the rest is
    // noise this leaves in place.
    stdout(&output).replace('\r', "")
}

/// The command that starts `shell`, under `script` when it needs a terminal.
///
/// `script` allocates one and connects the child to it, so the shell runs its reader the way
/// it does for a person. Its two spellings differ, and neither accepts the other's.
fn started(shell: &Shell) -> Command {
    if !shell.terminal {
        let mut command = Command::new(shell.name);
        command.args(shell.arguments);
        return command;
    }

    let line = std::iter::once(shell.name)
        .chain(shell.arguments.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    let mut command = Command::new("script");
    if cfg!(target_os = "macos") {
        command
            .args(["-q", "/dev/null"])
            .arg(shell.name)
            .args(shell.arguments);
    } else {
        command.args(["-q", "-c", &line, "/dev/null"]);
    }
    command
}

/// Backdates every file in `directory`, so the write that follows lands on a different mtime.
///
/// An mtime is seconds, and a test that applies twice inside one second would otherwise be
/// asking the hook to see a change `docs/hook.md` says it cannot. A person switching themes
/// never hits that; a test run does.
fn age(directory: &Path) {
    let long_ago = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    for entry in fs::read_dir(directory).unwrap() {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(entry.unwrap().path())
            .unwrap();
        file.set_times(fs::FileTimes::new().set_modified(long_ago))
            .unwrap();
    }
}

/// Every colour the script printed, in the order it printed them.
///
/// The marker is read out of the text rather than off the start of a line, because a terminal
/// writes the prompt and the echo of what was typed around it. `bg=$VANADIS_BG`, which is the
/// echo of the command itself, does not match: the marker is `bg=` followed by a colour.
fn colours(printed: &str) -> Vec<String> {
    printed
        .match_indices("bg=#")
        .map(|(at, _)| printed[at + 3..].chars().take(7).collect())
        .collect()
}

/// The colour the fixture's themes carry, so a test can name the one it expects.
const NORD: &str = "#2e3440";
const PAPER: &str = "#eeeeee";

#[test]
fn sources_the_output_when_the_snippet_is_evaluated() {
    for shell in [ZSH, BASH, FISH] {
        if !installed(&shell) {
            continue;
        }
        let (config, state) = workspace(&format!("hook-eval-{}", shell.name));
        vanadis(&config, &state, &["apply", "nord"]);

        let script = format!("{}\n{}\n", shell.evaluate, shell.show);
        let printed = drive(&shell, &config, &state, &script);
        assert_eq!(colours(&printed), [NORD], "{}: {printed}", shell.name);
    }
}

#[test]
fn sources_again_at_the_next_prompt_when_a_cycle_moves_the_output() {
    for shell in [ZSH, BASH, FISH] {
        if !installed(&shell) {
            continue;
        }
        let (config, state) = workspace(&format!("hook-cycle-{}", shell.name));
        vanadis(&config, &state, &["apply", "nord"]);
        age(&config.join("out"));

        // The cycle steps nord to paper-light. The `bg=` after it is printed at a prompt the
        // hook has run at, which is the whole claim.
        let script = format!("{}\n@vanadis@ cycle\n{}\n", shell.evaluate, shell.show);
        let printed = drive(&shell, &config, &state, &script);
        assert_eq!(colours(&printed), [PAPER], "{}: {printed}", shell.name);
    }
}

#[test]
fn leaves_one_hook_registered_after_a_second_evaluation() {
    for shell in [ZSH, BASH, FISH] {
        if !installed(&shell) {
            continue;
        }
        let (config, state) = workspace(&format!("hook-twice-{}", shell.name));
        vanadis(&config, &state, &["apply", "nord"]);

        let script = format!("{0}\n{0}\n{1}\n", shell.evaluate, shell.hooks);
        let printed = drive(&shell, &config, &state, &script);
        let registered = printed.matches("_vanadis_hook").count();
        assert_eq!(registered, 1, "{}: {printed}", shell.name);
    }
}

#[test]
fn returns_the_exit_status_the_command_before_the_prompt_left() {
    for shell in [ZSH, BASH, FISH] {
        if !installed(&shell) {
            continue;
        }
        let (config, state) = workspace(&format!("hook-status-{}", shell.name));
        vanadis(&config, &state, &["apply", "nord"]);

        // `false` runs, the hook runs at the prompt after it, and the status read on the next
        // line is the one a prompt showing `$?` would display.
        let script = format!("{}\nfalse\n{}\n", shell.evaluate, shell.status);
        let printed = drive(&shell, &config, &state, &script);
        assert!(printed.contains("status=1"), "{}: {printed}", shell.name);
    }
}

#[test]
fn sources_nothing_when_an_apply_names_no_shell_target() {
    for shell in [ZSH, BASH, FISH] {
        if !installed(&shell) {
            continue;
        }
        let (config, state) = workspace(&format!("hook-only-{}", shell.name));
        vanadis(&config, &state, &["apply", "nord"]);
        age(&config.join("out"));

        // `plain` is the target no `hook` sources. Applying paper-light into it alone must
        // leave the shell on nord, because no file the hook watches was written. The cycle
        // after it is what keeps this from passing in a shell where the hook never ran.
        let script = format!(
            "{}\n@vanadis@ apply paper-light --only plain\n{1}\n@vanadis@ cycle\n{1}\n",
            shell.evaluate, shell.show
        );
        let printed = drive(&shell, &config, &state, &script);
        assert_eq!(
            colours(&printed),
            [NORD, PAPER],
            "{}: {printed}",
            shell.name
        );
    }
}

#[test]
fn prints_a_snippet_holding_the_output_of_every_target_that_names_the_shell() {
    let (config, state) = workspace("hook-outputs");
    let output = vanadis(&config, &state, &["hook", "zsh"]);
    assert!(
        stdout(&output).contains("out/colours.zsh"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn reads_no_state_file_and_no_theme() {
    // Nothing has been applied, and the outputs do not exist yet. The snippet holds paths, so
    // it is the same snippet either way.
    let (config, state) = workspace("hook-unapplied");
    let output = vanadis(&config, &state, &["hook", "zsh"]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn says_so_on_stderr_and_exits_zero_when_no_target_names_the_shell() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hook-none");
    let _ = fs::remove_dir_all(&root);
    let config = root.join("config");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/apply"),
        &config,
    );
    let state = root.join("state");
    fs::create_dir_all(&state).unwrap();

    let output = vanadis(&config, &state, &["hook", "zsh"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("shell = \"zsh\""),
        "{}",
        stderr(&output)
    );
}

#[test]
fn refuses_a_shell_it_does_not_emit_for() {
    let (config, state) = workspace("hook-unknown");
    let output = vanadis(&config, &state, &["hook", "nu"]);
    assert!(!output.status.success());
    assert_eq!(stdout(&output), "");
}

#[test]
fn prints_nothing_when_the_config_will_not_load() {
    // A failure that reached stdout would be evaluated by the shell that ran the command.
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hook-broken");
    let _ = fs::remove_dir_all(&root);
    let config = root.join("config");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("config.toml"), "[[targets]]\nname = \"a\"\n").unwrap();
    let state = root.join("state");
    fs::create_dir_all(&state).unwrap();

    let output = vanadis(&config, &state, &["hook", "zsh"]);
    assert!(!output.status.success());
    assert_eq!(stdout(&output), "");
}
