use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};
use ggt_git::{CancelToken, ExecError, Executor, GitCommand, Output};

/// Identifies a submitted job. Ids only grow, so the app can drop results
/// from jobs it has since superseded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(u64);

/// A finished job, delivered on the results channel.
#[derive(Debug)]
pub struct JobResult {
    pub id: JobId,
    pub command: GitCommand,
    pub result: Result<Output, ExecError>,
}

/// A submitted job: its id and a token to cancel it.
#[derive(Debug, Clone)]
pub struct JobHandle {
    pub id: JobId,
    pub cancel: CancelToken,
}

/// Runs git commands off the UI thread, one thread per job, and sends each
/// result to the channel returned by [`Runner::new`].
#[derive(Debug, Clone)]
pub struct Runner<E> {
    executor: Arc<E>,
    results: Sender<JobResult>,
    next_id: Arc<AtomicU64>,
}

impl<E: Executor + 'static> Runner<E> {
    /// Creates a runner and the channel its results arrive on.
    pub fn new(executor: E) -> (Self, Receiver<JobResult>) {
        let (results, receiver) = unbounded();
        let runner = Self {
            executor: Arc::new(executor),
            results,
            next_id: Arc::new(AtomicU64::new(1)),
        };
        (runner, receiver)
    }

    /// Starts `command` in the background.
    pub fn submit(&self, command: GitCommand) -> JobHandle {
        let id = JobId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let cancel = CancelToken::new();
        let executor = Arc::clone(&self.executor);
        let results = self.results.clone();
        let token = cancel.clone();
        thread::spawn(move || {
            let result = executor.run(&command, &token);
            // The receiver is gone only when the app is shutting down.
            let _ = results.send(JobResult {
                id,
                command,
                result,
            });
        });
        JobHandle { id, cancel }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::ProcessExecutor;
    use ggt_git::GitCommand;
    use ggt_testkit::FixtureRepo;
    use std::time::Duration;

    #[test]
    fn delivers_results_tagged_with_their_job_id() {
        let repo = FixtureRepo::new();
        let head = repo.commit_file("a.txt", "1\n", "Base");
        let (runner, results) = Runner::new(ProcessExecutor::new());

        let first = runner.submit(GitCommand::read(repo.path(), ["rev-parse", "HEAD"]));
        let second = runner.submit(GitCommand::read(
            repo.path(),
            ["symbolic-ref", "--short", "HEAD"],
        ));
        assert_ne!(first.id, second.id);

        let mut got: Vec<JobResult> = (0..2)
            .map(|_| results.recv_timeout(Duration::from_secs(10)).unwrap())
            .collect();
        got.sort_by_key(|r| r.id);
        assert_eq!(got[0].id, first.id);
        assert_eq!(got[0].command.to_string(), "git rev-parse HEAD");
        assert_eq!(got[0].result.as_ref().unwrap().stdout.trim(), head);
        assert_eq!(got[1].result.as_ref().unwrap().stdout.trim(), "main");
    }

    #[test]
    fn handles_can_cancel_their_job() {
        let repo = FixtureRepo::new();
        let (runner, results) = Runner::new(ProcessExecutor::new());
        let job = runner.submit(GitCommand::read(repo.path(), ["status"]));
        job.cancel.cancel();
        // Either the job finished before the cancel landed or it reports Cancelled; it never hangs.
        let result = results.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(result.id, job.id);
    }
}
