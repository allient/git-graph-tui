//! Programs that take over the terminal for a moment (S12): a diff pager
//! such as `delta`, fed by git, `git difftool`, or the user's editor on a
//! changed file. The app emits an
//! [`External`]; `main` steps the TUI aside, runs it with [`run`] and comes
//! back.

use std::path::Path;
use std::process::{Command, Stdio};

/// Environment variable naming the diff pager; it wins over the config file.
pub const PAGER_ENV: &str = "GIT_GRAPH_TUI_PAGER";

/// A program to run with the terminal handed over to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct External {
    /// What failed, for the status message: the pager or `git difftool`.
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    /// Reload the repository afterwards: the program may have changed files.
    pub refresh: bool,
}

/// The diff pager to use: `$GIT_GRAPH_TUI_PAGER`, else the config file's
/// `[diff] pager`. None (or an empty value) keeps the built-in diff view.
pub fn pager(env: &dyn Fn(&str) -> Option<String>, config: Option<String>) -> Option<String> {
    env(PAGER_ENV)
        .or(config)
        .map(|pager| pager.trim().to_owned())
        .filter(|pager| !pager.is_empty())
}

/// The editor for "Open File": `$VISUAL`, else `$EDITOR`, else `vi`. It is
/// a shell command line, so it may carry arguments (`code --wait`).
pub fn editor(env: &dyn Fn(&str) -> Option<String>) -> String {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(env)
        .map(|editor| editor.trim().to_owned())
        .find(|editor| !editor.is_empty())
        .unwrap_or_else(|| "vi".to_owned())
}

/// Opens `path` (relative to `repo`) in `editor`, through the shell from the
/// repository's root, and refreshes once it exits. On a POSIX shell the
/// directory and the path are positional parameters, never parsed as shell
/// text.
pub fn editor_command(repo: &Path, editor: &str, path: &str) -> External {
    let repo = repo.to_string_lossy().into_owned();
    let (program, args) = if cfg!(windows) {
        let line = format!("cd /d {} && {editor} {}", quote_cmd(&repo), quote_cmd(path));
        ("cmd", vec!["/C".to_owned(), line])
    } else {
        let line = format!("cd -- \"$1\" && exec {editor} \"$2\"");
        (
            "sh",
            vec![
                "-c".to_owned(),
                line,
                "sh".to_owned(),
                repo,
                path.to_owned(),
            ],
        )
    };
    External {
        label: editor.to_owned(),
        program: program.to_owned(),
        args,
        refresh: true,
    }
}

/// Quotes `arg` for a POSIX shell: in single quotes, with each `'` closed,
/// escaped and reopened.
pub fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

/// Quotes `arg` for `cmd.exe`: in double quotes, with `"` doubled.
fn quote_cmd(arg: &str) -> String {
    format!("\"{}\"", arg.replace('"', "\"\""))
}

/// Pipes the colored output of `git <diff_args>` in `repo` into `pager`,
/// through the shell (`sh -c`, or `cmd /C` on Windows). `diff_args` start
/// with the subcommand (`show` or `diff`), as `show::diff_args` and
/// `show::compare_diff_args` return them.
pub fn pager_command(repo: &Path, diff_args: &[String], pager: &str) -> External {
    let windows = cfg!(windows);
    let quote = if windows { quote_cmd } else { quote };
    let (subcommand, rest) = diff_args
        .split_first()
        .map_or(("", &[][..]), |(s, r)| (s.as_str(), r));
    let mut git = vec![
        "git".to_owned(),
        // git must not page the diff itself: the pager reads it from the pipe.
        "--no-pager".to_owned(),
        "-C".to_owned(),
        quote(&repo.to_string_lossy()),
        quote(subcommand),
        // A pipe isn't a terminal, so ask for colors.
        quote("--color=always"),
    ];
    git.extend(rest.iter().map(|arg| quote(arg)));
    let line = format!("{} | {pager}", git.join(" "));
    let (program, flag) = if windows { ("cmd", "/C") } else { ("sh", "-c") };
    External {
        label: pager.to_owned(),
        program: program.to_owned(),
        args: vec![flag.to_owned(), line],
        refresh: false,
    }
}

/// Runs `git <difftool_args>` in `repo`, no shell involved.
pub fn difftool_command(repo: &Path, difftool_args: Vec<String>) -> External {
    let mut args = vec!["-C".to_owned(), repo.to_string_lossy().into_owned()];
    args.extend(difftool_args);
    External {
        label: "git difftool".to_owned(),
        program: "git".to_owned(),
        args,
        refresh: false,
    }
}

/// Runs `external` with the terminal's input and output, waits for it, and
/// returns the status message when it failed (`delta failed: exit status: 1`).
/// The caller must have handed the terminal over first.
pub fn run(external: &External) -> Option<String> {
    let status = Command::new(&external.program)
        .args(&external.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();
    match status {
        Ok(status) if status.success() => None,
        Ok(status) => Some(format!("{} failed: {status}", external.label)),
        Err(err) => Some(format!("{} failed: {err}", external.label)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    #[test]
    fn the_environment_wins_over_the_config_and_empty_means_built_in() {
        assert_eq!(pager(&env(&[]), None), None);
        assert_eq!(pager(&env(&[]), Some("delta".into())), Some("delta".into()));
        assert_eq!(
            pager(&env(&[(PAGER_ENV, "less -R")]), Some("delta".into())),
            Some("less -R".into())
        );
        assert_eq!(
            pager(&env(&[(PAGER_ENV, "")]), Some("delta".into())),
            None,
            "an empty variable turns the configured pager off"
        );
        assert_eq!(pager(&env(&[]), Some("  ".into())), None);
    }

    #[test]
    fn quoting_keeps_spaces_and_single_quotes_literal() {
        assert_eq!(quote("src/main.rs"), "'src/main.rs'");
        assert_eq!(quote("my file's.rs"), r"'my file'\''s.rs'");
        assert_eq!(quote_cmd("say \"hi\".txt"), "\"say \"\"hi\"\".txt\"");
    }

    #[cfg(unix)]
    #[test]
    fn quoted_arguments_reach_the_program_unchanged() {
        let tricky = "a b'c\"d $HOME `x` \\";
        let out = Command::new("sh")
            .args(["-c", &format!("printf %s {}", quote(tricky))])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), tricky);
    }

    #[cfg(unix)]
    #[test]
    fn the_pager_reads_colored_git_output_through_sh() {
        let args = ["show".to_owned(), "--format=".to_owned(), "abc".to_owned()];
        assert_eq!(
            pager_command(Path::new("/repo dir"), &args, "delta --side-by-side"),
            External {
                label: "delta --side-by-side".into(),
                program: "sh".into(),
                args: vec![
                    "-c".into(),
                    "git --no-pager -C '/repo dir' 'show' '--color=always' '--format=' 'abc' \
                     | delta --side-by-side"
                        .into()
                ],
                refresh: false,
            }
        );
    }

    #[test]
    fn the_editor_comes_from_visual_then_editor_then_vi() {
        assert_eq!(editor(&env(&[])), "vi");
        assert_eq!(editor(&env(&[("EDITOR", "nano")])), "nano");
        assert_eq!(
            editor(&env(&[("EDITOR", "nano"), ("VISUAL", "code --wait")])),
            "code --wait"
        );
        assert_eq!(editor(&env(&[("VISUAL", " "), ("EDITOR", "nano")])), "nano");
    }

    #[cfg(unix)]
    #[test]
    fn the_editor_opens_the_path_from_the_repository_root() {
        let dir = std::env::temp_dir();
        let command = editor_command(&dir, "printf '%s|%s'", "a b's.txt");
        assert!(command.refresh);
        assert_eq!(command.program, "sh");
        // `printf` stands in for the editor: it prints the path it got and
        // the directory it ran in.
        let mut command = command;
        command.args[1] = command.args[1].replace("\"$2\"", "\"$2\" \"$(pwd -P)\"");
        let out = Command::new(&command.program)
            .args(&command.args)
            .output()
            .unwrap();
        let printed = String::from_utf8(out.stdout).unwrap();
        let root = dir.canonicalize().unwrap();
        assert_eq!(printed, format!("a b's.txt|{}", root.display()));
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_run_names_the_program_and_its_exit_status() {
        let failing = External {
            label: "delta".into(),
            program: "sh".into(),
            args: vec!["-c".into(), "exit 3".into()],
            refresh: false,
        };
        assert_eq!(
            run(&failing).as_deref(),
            Some("delta failed: exit status: 3")
        );
        let fine = External {
            args: vec!["-c".into(), "true".into()],
            ..failing
        };
        assert_eq!(run(&fine), None);
        let missing = External {
            label: "nope".into(),
            program: "git-graph-tui-no-such-program".into(),
            args: Vec::new(),
            refresh: false,
        };
        let message = run(&missing).unwrap();
        assert!(message.starts_with("nope failed: "), "{message}");
    }
}
