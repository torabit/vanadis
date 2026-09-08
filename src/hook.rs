//! The snippet `vanadis hook` prints for a shell to evaluate.
//!
//! `docs/hook.md` decides it. The snippet holds resolved output paths as literals and never
//! runs vanadis again, so a prompt costs no process and a shell that has lost vanadis from
//! `$PATH` keeps following. Three invariants hold in every shell: a second evaluation leaves
//! one hook registered, the hook returns the exit status it found, and every name it defines
//! is prefixed `_vanadis_`.

use std::fmt::Write as _;
use std::path::Path;

use crate::config::Shell;

/// How `stat` is asked for an mtime in seconds.
///
/// Which one to write is settled when the snippet is generated, because vanadis knows the
/// system it is running on. The snippet carries no runtime branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    /// GNU and busybox: `stat -c %Y`.
    Gnu,
    /// The BSDs, macOS among them: `stat -f %m`.
    Bsd,
}

impl Stat {
    /// The one the system this build runs on has.
    pub const HOST: Self = if cfg!(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )) {
        Self::Bsd
    } else {
        Self::Gnu
    };

    /// The arguments that print an mtime in seconds and nothing else.
    #[must_use]
    fn arguments(self) -> &'static str {
        match self {
            Self::Gnu => "-c %Y",
            Self::Bsd => "-f %m",
        }
    }
}

/// The snippet that sources `outputs` and follows them, for `shell`.
///
/// `outputs` is in the order the targets appear in `config.toml`, which is the order
/// [`crate::apply`] writes them in.
#[must_use]
pub fn snippet(shell: Shell, outputs: &[&Path], stat: Stat) -> String {
    match shell {
        Shell::Zsh => zsh(outputs),
        Shell::Fish => fish(outputs),
        Shell::Bash => bash(outputs, stat),
    }
}

/// The comment the snippet opens with, naming what it is and how to refresh it.
fn preamble(shell: Shell) -> String {
    format!(
        "# vanadis hook {shell}. Sources the output of every target marked `shell = \"{shell}\"`,\n\
         # and sources it again when that file changes. Written by `vanadis hook {shell}`: the\n\
         # paths below are literals, so run `eval \"$(vanadis hook {shell})\"` again after moving\n\
         # an output or adding a target.\n"
    )
}

/// The name of the variable holding the mtime last seen for the output at `index`.
fn variable(index: usize) -> String {
    format!("_vanadis_mtime_{index}")
}

/// `text` as one single-quoted word, for a shell that takes a backslash literally there.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// `text` as one single-quoted word, for fish, where a backslash is an escape.
fn quoted_fish(text: &str) -> String {
    format!("'{}'", text.replace('\\', r"\\").replace('\'', r"\'"))
}

/// `path` as the shell writes it, which is lossy only for a path that is not UTF-8.
fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// zsh: `zstat` from `zsh/stat`, and `add-zsh-hook`, which deduplicates by function name.
fn zsh(outputs: &[&Path]) -> String {
    let mut snippet = preamble(Shell::Zsh);

    // Only the builtin, because `zmodload zsh/stat` also defines a `stat` that shadows the
    // user's.
    snippet.push_str(
        "\nzmodload -F zsh/stat b:zstat\n\
         autoload -Uz add-zsh-hook\n\n\
         # Reports whether $1 has moved since the run recorded in the variable named by $2,\n\
         # and records the new mtime when it has. Emulated, so a user's setopt cannot change\n\
         # what it means; the source itself runs outside this, in the options the user set.\n\
         _vanadis_moved() {\n\
         \temulate -L zsh\n\
         \tlocal -a info\n\
         \tzstat -A info +mtime -- $1 2>/dev/null || return 1\n\
         \t[[ ${(P)2} == $info[1] ]] && return 1\n\
         \ttypeset -g $2=$info[1]\n\
         }\n\n",
    );

    for index in 0..outputs.len() {
        let _ = writeln!(snippet, "typeset -g {}=", variable(index));
    }

    snippet.push_str("\n_vanadis_hook() {\n\tlocal _vanadis_status=$?\n");
    for (index, output) in outputs.iter().enumerate() {
        let path = quoted(&display(output));
        let _ = writeln!(
            snippet,
            "\t_vanadis_moved {path} {} && source {path}",
            variable(index)
        );
    }
    snippet.push_str("\treturn $_vanadis_status\n}\n\n");

    snippet.push_str("add-zsh-hook precmd _vanadis_hook\n_vanadis_hook\n");
    snippet
}

/// fish: `path mtime`, and a handler function, which replaces its own previous definition.
fn fish(outputs: &[&Path]) -> String {
    let mut snippet = preamble(Shell::Fish);
    snippet.push('\n');

    for index in 0..outputs.len() {
        let _ = writeln!(snippet, "set -g {} ''", variable(index));
    }

    snippet.push_str(
        "\nfunction _vanadis_hook --on-event fish_prompt \
         --description 'vanadis: follow the applied theme'\n\
         \tset -l _vanadis_status $status\n",
    );
    for (index, output) in outputs.iter().enumerate() {
        let path = quoted_fish(&display(output));
        let variable = variable(index);
        let _ = write!(
            snippet,
            "\tif test -e {path}\n\
             \t\tset -l _vanadis_now (path mtime {path})\n\
             \t\tif test \"$_vanadis_now\" != \"${variable}\"\n\
             \t\t\tset -g {variable} $_vanadis_now\n\
             \t\t\tsource {path}\n\
             \t\tend\n\
             \tend\n"
        );
    }
    snippet.push_str("\treturn $_vanadis_status\nend\n\n_vanadis_hook\n");
    snippet
}

/// bash: one `stat` per prompt, and a `PROMPT_COMMAND` append guarded by the function name.
fn bash(outputs: &[&Path], stat: Stat) -> String {
    let mut snippet = preamble(Shell::Bash);

    let _ = write!(
        snippet,
        "\n# Reports whether $1 has moved since the run recorded in the variable named by $2,\n\
         # and records the new mtime when it has. bash has no builtin that reads an mtime, so\n\
         # this is the one fork a prompt costs.\n\
         _vanadis_moved() {{\n\
         \tlocal now\n\
         \tnow=$(stat {} -- \"$1\" 2>/dev/null) || return 1\n\
         \t[ -n \"$now\" ] || return 1\n\
         \t[ \"$now\" = \"${{!2}}\" ] && return 1\n\
         \tprintf -v \"$2\" %s \"$now\"\n\
         }}\n\n",
        stat.arguments()
    );

    for index in 0..outputs.len() {
        let _ = writeln!(snippet, "{}=", variable(index));
    }

    snippet.push_str("\n_vanadis_hook() {\n\tlocal _vanadis_status=$?\n");
    for (index, output) in outputs.iter().enumerate() {
        let path = quoted(&display(output));
        let _ = writeln!(
            snippet,
            "\t_vanadis_moved {path} {} && . {path}",
            variable(index)
        );
    }
    snippet.push_str("\treturn $_vanadis_status\n}\n\n");

    // `${PROMPT_COMMAND[*]}` reads it whether it is a string or, since 5.1, an array.
    snippet.push_str(
        "[[ ${PROMPT_COMMAND[*]:-} == *_vanadis_hook* ]] ||\n\
         \tPROMPT_COMMAND=\"_vanadis_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}\"\n\
         _vanadis_hook\n",
    );
    snippet
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zsh_snippet(outputs: &[&str]) -> String {
        let paths: Vec<&Path> = outputs.iter().map(Path::new).collect();
        snippet(Shell::Zsh, &paths, Stat::Gnu)
    }

    #[test]
    fn writes_every_output_as_a_quoted_literal() {
        let snippet = zsh_snippet(&["/home/ada/.config/fzf/colours.zsh"]);
        assert!(
            snippet.contains("source '/home/ada/.config/fzf/colours.zsh'"),
            "{snippet}"
        );
    }

    #[test]
    fn closes_a_quote_a_path_opens() {
        // A single quote in a path would otherwise end the literal and hand the rest of the
        // path to the shell as code.
        let snippet = zsh_snippet(&["/home/ada/it's/colours.zsh"]);
        assert!(
            snippet.contains(r"'/home/ada/it'\''s/colours.zsh'"),
            "{snippet}"
        );
    }

    #[test]
    fn escapes_a_backslash_for_fish_only() {
        let path = Path::new(r"/home/ada/a\b.fish");
        assert!(snippet(Shell::Fish, &[path], Stat::Gnu).contains(r"'/home/ada/a\\b.fish'"));
        assert!(snippet(Shell::Zsh, &[path], Stat::Gnu).contains(r"'/home/ada/a\b.fish'"));
    }

    #[test]
    fn gives_each_output_its_own_variable() {
        let snippet = zsh_snippet(&["/one.zsh", "/two.zsh"]);
        assert!(
            snippet.contains("_vanadis_moved '/one.zsh' _vanadis_mtime_0"),
            "{snippet}"
        );
        assert!(
            snippet.contains("_vanadis_moved '/two.zsh' _vanadis_mtime_1"),
            "{snippet}"
        );
    }

    #[test]
    fn writes_the_stat_of_the_system_it_generated_on() {
        let path = Path::new("/one.bash");
        assert!(snippet(Shell::Bash, &[path], Stat::Gnu).contains("stat -c %Y"));
        assert!(snippet(Shell::Bash, &[path], Stat::Bsd).contains("stat -f %m"));
    }

    #[test]
    fn forks_nothing_for_zsh_or_fish() {
        // Both read an mtime from a builtin. Only bash spends the external `stat`.
        let path = Path::new("/one");
        for shell in [Shell::Zsh, Shell::Fish] {
            let snippet = snippet(shell, &[path], Stat::Gnu);
            assert!(!snippet.contains("stat -c"), "{shell}: {snippet}");
            assert!(!snippet.contains("stat -f"), "{shell}: {snippet}");
        }
    }

    #[test]
    fn restores_the_exit_status_in_every_shell() {
        let path = Path::new("/one");
        for shell in Shell::ALL {
            let snippet = snippet(shell, &[path], Stat::Gnu);
            assert!(snippet.contains("_vanadis_status"), "{shell}: {snippet}");
        }
    }
}
