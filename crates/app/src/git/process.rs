use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use ggt_git::{CancelToken, ExecError, Executor, GitCommand, Output};

/// How often a running command is checked for exit, timeout and cancellation.
const POLL: Duration = Duration::from_millis(10);

/// Runs git as a child process. Timeouts and cancellation kill the process.
#[derive(Debug, Clone)]
pub struct ProcessExecutor {
    program: String,
}

impl ProcessExecutor {
    pub fn new() -> Self {
        Self::with_program("git")
    }

    /// Uses another executable (for tests, or a custom git path).
    pub fn with_program(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
        }
    }

    /// Runs the program with `args` instead of the command's git argv
    /// (environment, timeout and cancellation still come from `command`).
    /// Only the Unix tests use it (they run `sleep`).
    #[cfg(all(test, unix))]
    pub fn run_raw(
        &self,
        command: &GitCommand,
        args: &[&str],
        cancel: &CancelToken,
    ) -> Result<Output, ExecError> {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        self.execute(command, &args, cancel)
    }

    fn execute(
        &self,
        command: &GitCommand,
        args: &[String],
        cancel: &CancelToken,
    ) -> Result<Output, ExecError> {
        let mut child = Command::new(&self.program)
            .args(args)
            .envs(command.env())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ExecError::Spawn)?;

        // Drain both pipes on their own threads so a large output can't fill a
        // pipe buffer and deadlock the child while we wait for it.
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());

        let deadline = Instant::now() + command.timeout();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(ExecError::Io)? {
                break status;
            }
            if cancel.is_cancelled() {
                kill(&mut child);
                return Err(ExecError::Cancelled);
            }
            if Instant::now() >= deadline {
                kill(&mut child);
                return Err(ExecError::Timeout(command.timeout()));
            }
            thread::sleep(POLL);
        };

        let stdout = join(stdout)?;
        let stderr = join(stderr)?;
        if status.success() || command.accepts_exit(status.code()) {
            Ok(Output { stdout, stderr })
        } else {
            // A signal-terminated process has no code; report it as -1.
            Err(ExecError::failed_with_stdout(
                status.code().unwrap_or(-1),
                stderr,
                &stdout,
            ))
        }
    }
}

impl Default for ProcessExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor for ProcessExecutor {
    fn run(&self, command: &GitCommand, cancel: &CancelToken) -> Result<Output, ExecError> {
        self.execute(command, &command.argv(), cancel)
    }
}

type Drain = Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>;

fn drain(pipe: Option<impl Read + Send + 'static>) -> Drain {
    pipe.map(|mut pipe| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            pipe.read_to_end(&mut buf).map(|_| buf)
        })
    })
}

fn join(handle: Drain) -> Result<String, ExecError> {
    let Some(handle) = handle else {
        return Ok(String::new());
    };
    let bytes = handle
        .join()
        .map_err(|_| ExecError::Io(std::io::Error::other("output reader panicked")))?
        .map_err(ExecError::Io)?;
    String::from_utf8(bytes).map_err(|_| ExecError::InvalidUtf8)
}

fn kill(child: &mut Child) {
    // The process may have exited between the check and the kill; both are fine.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use ggt_git::{CancelToken, ExecError, Executor, Failure, GitCommand};
    use ggt_testkit::FixtureRepo;

    #[test]
    fn runs_git_and_captures_stdout() {
        let repo = FixtureRepo::new();
        let head = repo.commit_file("a.txt", "1\n", "Base");
        let cmd = GitCommand::read(repo.path(), ["rev-parse", "HEAD"]);
        let out = ProcessExecutor::new()
            .run(&cmd, &CancelToken::new())
            .unwrap();
        assert_eq!(out.stdout.trim(), head);
    }

    #[test]
    fn large_output_does_not_deadlock() {
        let repo = FixtureRepo::new();
        repo.commit_file("big.txt", &"x".repeat(300_000), "Big");
        let cmd = GitCommand::read(repo.path(), ["show", "HEAD:big.txt"]);
        let out = ProcessExecutor::new()
            .run(&cmd, &CancelToken::new())
            .unwrap();
        assert_eq!(out.stdout.len(), 300_000);
    }

    #[test]
    fn non_zero_exit_is_a_classified_failure() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = GitCommand::read(dir.path(), ["status"]);
        let err = ProcessExecutor::new()
            .run(&cmd, &CancelToken::new())
            .unwrap_err();
        assert!(
            matches!(
                err,
                ExecError::Failed {
                    failure: Failure::NotARepository,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn slow_commands_are_killed_at_the_timeout() {
        // `sleep` stands in for a hanging git; the executor treats it the same way.
        let cmd = GitCommand::read(std::path::Path::new("."), ["5"])
            .with_timeout(Duration::from_millis(200));
        let start = Instant::now();
        let err = ProcessExecutor::with_program("sleep")
            .run_raw(&cmd, &["5"], &CancelToken::new())
            .unwrap_err();
        assert!(matches!(err, ExecError::Timeout(_)), "{err:?}");
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn cancelled_commands_are_killed() {
        let cmd = GitCommand::read(std::path::Path::new("."), ["5"]);
        let token = CancelToken::new();
        let canceller = token.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            canceller.cancel();
        });
        let start = Instant::now();
        let err = ProcessExecutor::with_program("sleep")
            .run_raw(&cmd, &["5"], &token)
            .unwrap_err();
        assert!(matches!(err, ExecError::Cancelled), "{err:?}");
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn missing_program_is_a_spawn_error() {
        let cmd = GitCommand::read(std::path::Path::new("."), ["status"]);
        let err = ProcessExecutor::with_program("definitely-not-git-xyz")
            .run(&cmd, &CancelToken::new())
            .unwrap_err();
        assert!(matches!(err, ExecError::Spawn(_)), "{err:?}");
    }
}
