use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What a command does, which decides its environment and timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Only reads the repository (log, status, refs). Skips optional locks so
    /// background refreshes never block the user's own git commands.
    Read,
    /// Changes the repository locally (checkout, merge, reset, ...).
    Write,
    /// Talks to a remote (fetch, pull, push): longer timeout.
    Network,
}

/// A fully specified `git` invocation: repository, arguments, environment and
/// timeout. Building it does no I/O; an executor runs it.
///
/// Arguments are passed to the process as-is (never through a shell), so a
/// branch name like `x; rm -rf ~` stays one harmless argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommand {
    repo: PathBuf,
    args: Vec<String>,
    kind: CommandKind,
    timeout: Duration,
}

impl GitCommand {
    /// A read-only command.
    pub fn read<I, S>(repo: &Path, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new(repo, args, CommandKind::Read)
    }

    /// A command that changes the repository locally.
    pub fn write<I, S>(repo: &Path, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new(repo, args, CommandKind::Write)
    }

    /// A command that talks to a remote.
    pub fn network<I, S>(repo: &Path, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new(repo, args, CommandKind::Network)
    }

    fn new<I, S>(repo: &Path, args: I, kind: CommandKind) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let timeout = match kind {
            CommandKind::Read | CommandKind::Write => Duration::from_secs(30),
            CommandKind::Network => Duration::from_secs(300),
        };
        Self {
            repo: repo.to_path_buf(),
            args: args.into_iter().map(Into::into).collect(),
            kind,
            timeout,
        }
    }

    /// Replaces the default timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Arguments for the `git` executable, including `-C <repo>` and config overrides.
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        let mut argv = vec![
            "-C".to_owned(),
            self.repo.to_string_lossy().into_owned(),
            "-c".to_owned(),
            "color.ui=never".to_owned(),
            "-c".to_owned(),
            "core.quotepath=off".to_owned(),
        ];
        argv.extend(self.args.iter().cloned());
        argv
    }

    /// Environment variables to set on the process.
    #[must_use]
    pub fn env(&self) -> Vec<(&'static str, &'static str)> {
        let mut env = vec![("GIT_TERMINAL_PROMPT", "0")];
        if self.kind == CommandKind::Read {
            env.push(("GIT_OPTIONAL_LOCKS", "0"));
        }
        env
    }

    #[must_use]
    pub fn kind(&self) -> CommandKind {
        self.kind
    }

    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    #[must_use]
    pub fn repo(&self) -> &Path {
        &self.repo
    }
}

/// `git <args>` without the repository and config overrides, for the git log view.
impl fmt::Display for GitCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("git")?;
        for arg in &self.args {
            write!(f, " {arg}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn argv_targets_the_repo_and_disables_colors() {
        let cmd = GitCommand::read(Path::new("/repo"), ["log", "-1"]);
        assert_eq!(
            cmd.argv(),
            [
                "-C",
                "/repo",
                "-c",
                "color.ui=never",
                "-c",
                "core.quotepath=off",
                "log",
                "-1"
            ]
        );
    }

    #[test]
    fn user_input_stays_a_single_argument() {
        let name = "feature/x; rm -rf ~";
        let cmd = GitCommand::write(Path::new("/r"), ["branch", "--", name]);
        assert_eq!(cmd.argv().last().map(String::as_str), Some(name));
    }

    #[test]
    fn every_command_disables_prompts() {
        let cmd = GitCommand::write(Path::new("/r"), ["checkout", "main"]);
        assert!(cmd.env().contains(&("GIT_TERMINAL_PROMPT", "0")));
    }

    #[test]
    fn read_commands_skip_optional_locks_but_writes_do_not() {
        let read = GitCommand::read(Path::new("/r"), ["status"]);
        let write = GitCommand::write(Path::new("/r"), ["commit"]);
        assert!(read.env().contains(&("GIT_OPTIONAL_LOCKS", "0")));
        assert!(!write.env().iter().any(|(k, _)| *k == "GIT_OPTIONAL_LOCKS"));
    }

    #[test]
    fn timeouts_depend_on_the_kind_of_command() {
        let read = GitCommand::read(Path::new("/r"), ["log"]);
        let network = GitCommand::network(Path::new("/r"), ["fetch"]);
        assert!(network.timeout() > read.timeout());
        let custom = read.with_timeout(Duration::from_secs(1));
        assert_eq!(custom.timeout(), Duration::from_secs(1));
    }

    #[test]
    fn display_shows_the_command_for_the_git_log_view() {
        let cmd = GitCommand::write(Path::new("/r"), ["merge", "--no-ff", "develop"]);
        assert_eq!(cmd.to_string(), "git merge --no-ff develop");
    }
}
