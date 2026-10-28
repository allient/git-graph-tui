use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::{Failure, GitCommand};

/// What a successful command printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

/// Why a command didn't succeed.
#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error("could not start git: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("git timed out after {0:?}")]
    Timeout(Duration),
    #[error("git was cancelled")]
    Cancelled,
    #[error("git exited with code {code:?}: {}", failure.message())]
    Failed {
        code: Option<i32>,
        stderr: String,
        failure: Failure,
    },
    #[error("git printed invalid UTF-8")]
    InvalidUtf8,
    #[error("reading git output failed: {0}")]
    Io(#[source] std::io::Error),
}

impl ExecError {
    /// A non-zero exit, with stderr classified into a [`Failure`].
    #[must_use]
    pub fn failed(code: i32, stderr: String) -> Self {
        Self::Failed {
            code: Some(code),
            failure: Failure::classify(&stderr),
            stderr,
        }
    }

    /// A short message for the status bar.
    #[must_use]
    pub fn user_message(&self) -> String {
        match self {
            Self::Failed { failure, .. } => failure.message(),
            other => other.to_string(),
        }
    }
}

/// Cooperative cancellation shared between the UI and a running command.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Runs git commands. The app implements it with real processes; tests can
/// substitute their own.
pub trait Executor: Send + Sync {
    /// Runs `command` to completion, honoring its timeout and `cancel`.
    fn run(&self, command: &GitCommand, cancel: &CancelToken) -> Result<Output, ExecError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_is_shared_between_clones() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }

    #[test]
    fn failed_errors_carry_a_classified_failure() {
        let err = ExecError::failed(128, "fatal: not a git repository".into());
        assert!(matches!(
            err,
            ExecError::Failed {
                failure: Failure::NotARepository,
                ..
            }
        ));
        assert_eq!(err.user_message(), Failure::NotARepository.message());
    }
}
