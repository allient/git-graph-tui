//! Git access for tui-git-graph.
//!
//! - Builds `git` argument vectors safely (`-C <repo>`, no shell, user input
//!   passed as separate arguments).
//! - Runs `git` as a child process behind a trait, so callers and tests can
//!   substitute the executor.
//! - Parses machine-readable output (`-z`, `%x1f` separators) into
//!   `tgg-model` types.
//!
//! Each command lives in its own module that owns both its arguments and its
//! parser, so the output format and the code that reads it can't drift apart.
//!
//! This crate never touches the terminal and never spawns threads: it
//! defines the [`Executor`] trait, and the app provides the implementation
//! that runs processes on its worker threads.

mod command;
mod error;
mod exec;
mod failure;
pub mod in_progress;
pub mod log;
pub mod reflog;
pub mod refs;
pub mod stash;
pub mod status;
pub mod worktree;

pub use command::{CommandKind, GitCommand};
pub use error::ParseError;
pub use exec::{CancelToken, ExecError, Executor, Output};
pub use failure::Failure;

/// Field separator used in every `--format` string (ASCII unit separator).
pub(crate) const FIELD_SEP: char = '\u{1f}';
pub mod show;
pub mod tag;
