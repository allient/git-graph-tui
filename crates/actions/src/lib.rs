//! Git actions for tui-git-graph.
//!
//! Each action (checkout, create branch, merge, rebase, cherry-pick, reset,
//! push, ...) is plain data: the git arguments to run, whether it is
//! destructive, the confirmation text to show, and what to refresh afterwards.
//! Context menus are built from the selected target (commit, branch, tag,
//! stash, uncommitted changes), so mouse menus, keyboard menus and tests all
//! share one source of truth.
