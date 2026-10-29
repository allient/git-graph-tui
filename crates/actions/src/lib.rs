//! Git actions for tui-git-graph.
//!
//! Each action (checkout, create branch, merge, rebase, cherry-pick, reset,
//! push, ...) is plain data: the git arguments to run, whether it is
//! destructive, the confirmation text to show, and what to refresh afterwards.
//! Context menus are built from the selected target (commit, branch, tag,
//! stash, uncommitted changes), so mouse menus, keyboard menus and tests all
//! share one source of truth.

mod action;
mod input;
mod menu;
mod refname;
mod target;

pub use action::{Action, Confirmation, GraphColumn, Plan, ResetMode, Rev, UiEffect};
pub use input::{InputError, InputId, InputKind, InputSpec, InputValue};
pub use menu::{MenuEntry, MenuItem, column_menu, menu_for, reflog_menu};
pub use refname::{RefNameError, validate_ref_name};
pub use target::{CommitTarget, MenuContext, Target};
mod stash;
mod tag;
mod working_tree;

pub use stash::StashAction;
pub use tag::TagAction;
pub use working_tree::WorkingTreeAction;
mod repo;
pub use repo::{PickPurpose, PullMode, RepoAction};
mod forge;
pub use forge::{IssueRef, issue_refs, issue_url, pull_request_url};
