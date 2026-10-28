//! Running git: the process executor, the background runner and repository discovery.

mod discover;
mod process;
mod runner;

pub use discover::discover_repo;
pub use process::ProcessExecutor;
pub use runner::{JobHandle, JobId, JobResult, Runner};
