//! Running git: the process executor, the background runner and repository discovery.

mod discover;
mod process;
// The event loop (ODD task T7) is the runner's first non-test user.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired into the event loop in task T7")
)]
mod runner;

pub use discover::discover_repo;
pub use process::ProcessExecutor;
