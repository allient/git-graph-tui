//! Git access for tui-git-graph.
//!
//! - Builds `git` argument vectors safely (`-C <repo>`, no shell, user input
//!   passed as separate arguments).
//! - Runs `git` as a child process behind a trait, so callers and tests can
//!   substitute the executor.
//! - Parses machine-readable output (`-z`, `%x1f` separators) into
//!   `tgg-model` types.
//!
//! This crate never touches the terminal and never spawns threads; the app
//! decides where commands run.
