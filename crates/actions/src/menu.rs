use tgg_git::worktree::Worktree;
use tgg_model::{Oid, Stash};

use crate::{
    Action, CommitScope, CommitTarget, GraphColumn, MenuContext, Operation, OperationAction,
    PickPurpose, RepoAction, ResetMode, Rev, StashAction, TagAction, Target, UiEffect,
    WorkingTreeAction, WorktreeAction,
};

/// One row of a context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Entry(MenuEntry),
    Separator,
    /// Opens a child menu (`Branch ▸`).
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },
}

/// A selectable menu row. A label ending in `...` opens a dialog
/// (inputs or a confirmation) before anything runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub label: String,
    pub action: Action,
    /// Shown but not selectable, with the reason (e.g. why a commit can't be dropped).
    pub disabled: Option<String>,
}

impl MenuEntry {
    #[must_use]
    pub fn new(label: &str, action: Action) -> Self {
        Self {
            label: label.to_owned(),
            action,
            disabled: None,
        }
    }

    #[must_use]
    pub fn disabled_because(mut self, reason: Option<&str>) -> Self {
        self.disabled = reason.map(str::to_owned);
        self
    }
}

/// Builds the context menu for `target`, in the order of VS Code Git Graph
/// (S8, S9 and its tag, stash and uncommitted-changes menus). Most entries
/// that can't apply are left out; the commit menu's git actions stay and
/// explain why they're disabled.
#[must_use]
pub fn menu_for(target: &Target, ctx: &MenuContext) -> Vec<MenuItem> {
    let items = match target {
        Target::LocalBranch { name, .. } => local_branch_menu(name, ctx),
        Target::RemoteBranch { remote, branch, .. } => remote_branch_menu(remote, branch, ctx),
        Target::Commit(commit) => commit_menu(commit, ctx),
        Target::Tag { name, .. } => tag_menu(name, ctx),
        Target::Stash(stash) => stash_menu(stash),
        Target::Uncommitted => uncommitted_menu(),
        Target::Repository => repository_menu(ctx),
    };
    tidy(items)
}

/// The graph column header's menu (S20): a checked entry per optional
/// column, which shows or hides it.
#[must_use]
pub fn column_menu(hidden: &[GraphColumn]) -> Vec<MenuItem> {
    GraphColumn::ALL
        .into_iter()
        .map(|column| {
            let check = if hidden.contains(&column) { " " } else { "✓" };
            MenuItem::Entry(MenuEntry::new(
                &format!("{check} {}", column.label()),
                Action::Ui(UiEffect::ToggleColumn { column }),
            ))
        })
        .collect()
}

/// Adds the entry only when `show` is true.
fn entry_if(show: bool, label: &str, action: impl FnOnce() -> Action) -> Option<MenuItem> {
    show.then(|| MenuItem::Entry(MenuEntry::new(label, action())))
}

/// An entry that is always shown, disabled with `reason` when there is one,
/// as VS Code Git Graph shows it even where it can't run.
fn entry_unless(reason: Option<&str>, label: &str, action: Action) -> MenuItem {
    MenuItem::Entry(MenuEntry::new(label, action).disabled_because(reason))
}

const ALREADY_ON_CURRENT: &str = "This commit is already on the current branch.";
const ALREADY_AT_COMMIT: &str = "The current branch is already at this commit.";
const NOT_ON_BRANCH: &str = "HEAD isn't on a branch.";

/// An entry that is always shown (`Option` so it lines up with [`entry_if`]).
fn entry(label: &str, action: Action) -> Option<MenuItem> {
    entry_if(true, label, || action)
}

fn local_branch_menu(name: &str, ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let is_current = ctx.is_current_branch(name);
    let current = ctx.current_branch().map(str::to_owned);
    let other_than_current = !is_current && current.is_some();
    let rev = || Rev::Local(name.to_owned());
    vec![
        entry_if(!is_current, "Checkout Branch", || Action::CheckoutBranch {
            name: name.to_owned(),
        }),
        entry(
            "Rename Branch...",
            Action::RenameBranch {
                from: name.to_owned(),
                to: name.to_owned(),
            },
        ),
        entry_if(!is_current, "Delete Branch...", || Action::DeleteBranch {
            name: name.to_owned(),
            force: false,
        }),
        entry_if(other_than_current, "Merge into current branch...", || {
            Action::MergeIntoCurrent {
                source: rev(),
                current: current.clone(),
                no_ff: true,
            }
        }),
        entry_if(
            other_than_current,
            "Rebase current branch on Branch...",
            || Action::RebaseCurrentOnto {
                onto: rev(),
                current: current.clone(),
            },
        ),
        ctx.default_remote().and_then(|remote| {
            entry(
                "Push Branch...",
                Action::PushBranch {
                    branch: name.to_owned(),
                    remote: remote.to_owned(),
                    set_upstream: true,
                    force_with_lease: false,
                },
            )
        }),
        ctx.default_remote()
            .and_then(|remote| entry("Create Pull Request", pull_request(remote, name))),
        Some(MenuItem::Separator),
        entry("Create Archive", archive(rev(), name)),
        filter_toggle(name, ctx),
        Some(MenuItem::Separator),
        entry(
            "Copy Branch Name to Clipboard",
            Action::CopyBranchName {
                name: name.to_owned(),
            },
        ),
    ]
}

fn pull_request(remote: &str, branch: &str) -> Action {
    Action::Ui(UiEffect::CreatePullRequest {
        remote: remote.to_owned(),
        branch: branch.to_owned(),
    })
}

fn remote_branch_menu(remote: &str, branch: &str, ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let full = format!("{remote}/{branch}");
    let current = ctx.current_branch().map(str::to_owned);
    let rev = || Rev::Remote {
        remote: remote.to_owned(),
        branch: branch.to_owned(),
    };
    vec![
        entry(
            "Checkout Branch...",
            Action::CheckoutRemoteBranch {
                remote: remote.to_owned(),
                branch: branch.to_owned(),
                local_name: branch.to_owned(),
            },
        ),
        entry(
            "Delete Remote Branch...",
            Action::Repo(RepoAction::DeleteRemoteBranch {
                remote: remote.to_owned(),
                branch: branch.to_owned(),
            }),
        ),
        Some(fetch_into_local(remote, branch, ctx)),
        entry_if(current.is_some(), "Merge into current branch...", || {
            Action::MergeIntoCurrent {
                source: rev(),
                current: current.clone(),
                no_ff: true,
            }
        }),
        entry_if(current.is_some(), "Pull into current branch...", || {
            Action::Repo(RepoAction::PullFrom {
                remote: remote.to_owned(),
                branch: branch.to_owned(),
            })
        }),
        entry_if(
            current.is_some(),
            "Rebase current branch on Branch...",
            || Action::RebaseCurrentOnto {
                onto: rev(),
                current: current.clone(),
            },
        ),
        entry("Create Pull Request", pull_request(remote, branch)),
        Some(MenuItem::Separator),
        entry("Create Archive", archive(rev(), &full)),
        filter_toggle(&full, ctx),
        Some(MenuItem::Separator),
        entry(
            "Copy Branch Name to Clipboard",
            Action::CopyBranchName { name: full.clone() },
        ),
    ]
}

/// "Fetch into local branch..." for a remote branch, into the local branch
/// of the same name by default. Git refuses to fetch into the checked-out
/// branch, so that case is disabled.
fn fetch_into_local(remote: &str, branch: &str, ctx: &MenuContext) -> MenuItem {
    let why = ctx.is_current_branch(branch).then(|| {
        format!(
            "'{branch}' is the current branch, and git can't fetch into a checked-out branch. Use Pull instead."
        )
    });
    MenuItem::Entry(
        MenuEntry::new(
            "Fetch into local branch...",
            Action::FetchIntoLocal {
                remote: remote.to_owned(),
                branch: branch.to_owned(),
                local_name: branch.to_owned(),
                force: false,
            },
        )
        .disabled_because(why.as_deref()),
    )
}

fn commit_menu(commit: &CommitTarget, ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let id = &commit.id;
    let subject = &commit.subject;
    let is_head = ctx.is_head_commit(id);
    let current = ctx.current_branch().map(str::to_owned);
    let detached_here = is_head && current.is_none();
    vec![
        entry(
            "Add Tag...",
            Action::AddTag {
                at: id.clone(),
                name: String::new(),
                annotated: true,
                message: String::new(),
                remote: ctx.default_remote().map(str::to_owned),
                push: false,
            },
        ),
        entry(
            "Create Branch...",
            Action::CreateBranch {
                at: id.clone(),
                name: String::new(),
                checkout: false,
            },
        ),
        Some(MenuItem::Separator),
        entry_if(!detached_here, "Checkout...", || Action::CheckoutCommit {
            commit: id.clone(),
            subject: subject.clone(),
        }),
        // Already in HEAD's history: picking it again would be empty.
        Some(entry_unless(
            commit.is_head_ancestor.then_some(ALREADY_ON_CURRENT),
            "Cherry Pick...",
            Action::CherryPick {
                commit: id.clone(),
                subject: subject.clone(),
                is_merge: commit.is_merge(),
            },
        )),
        entry(
            "Revert...",
            Action::Revert {
                commit: id.clone(),
                subject: subject.clone(),
                is_merge: commit.is_merge(),
            },
        ),
        Some(MenuItem::Entry(
            MenuEntry::new(
                "Drop...",
                Action::Drop {
                    commit: id.clone(),
                    subject: subject.clone(),
                    current: current.clone(),
                },
            )
            .disabled_because(why_not_droppable(commit)),
        )),
        Some(MenuItem::Separator),
    ]
    .into_iter()
    .chain(current_branch_entries(commit, ctx))
    .chain(compare_entries(id, ctx))
    .chain([
        entry(
            "Copy Commit Hash to Clipboard",
            Action::CopyCommitHash { commit: id.clone() },
        ),
        entry(
            "Copy Commit Subject to Clipboard",
            Action::CopyCommitSubject {
                subject: subject.clone(),
            },
        ),
    ])
    .collect()
}

/// Merge, rebase and reset of the current branch with `commit`, plus
/// interactive rebase; disabled with the reason where they can't run.
fn current_branch_entries(commit: &CommitTarget, ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let id = &commit.id;
    let subject = &commit.subject;
    let is_head = ctx.is_head_commit(id);
    let current = ctx.current_branch().map(str::to_owned);
    let not_on_branch = current.is_none().then_some(NOT_ON_BRANCH);
    vec![
        Some(entry_unless(
            not_on_branch.or(commit.is_head_ancestor.then_some(ALREADY_ON_CURRENT)),
            "Merge into current branch...",
            Action::MergeIntoCurrent {
                source: Rev::Commit(id.clone()),
                current: current.clone(),
                no_ff: true,
            },
        )),
        Some(entry_unless(
            not_on_branch.or(is_head.then_some(ALREADY_AT_COMMIT)),
            "Rebase current branch on this Commit...",
            Action::RebaseCurrentOnto {
                onto: Rev::Commit(id.clone()),
                current: current.clone(),
            },
        )),
        Some(interactive_rebase_entry(commit, ctx)),
        Some(entry_unless(
            not_on_branch.or(is_head.then_some(ALREADY_AT_COMMIT)),
            "Reset current branch to this Commit...",
            Action::ResetCurrentTo {
                commit: id.clone(),
                subject: subject.clone(),
                current: current.clone().unwrap_or_default(),
                mode: ResetMode::default(),
            },
        )),
    ]
}

/// "Select for Compare", then "Compare with Selected" when another commit
/// is marked, then "Compare with Working Tree" (S12), between separators.
fn compare_entries(id: &Oid, ctx: &MenuContext) -> [Option<MenuItem>; 5] {
    [
        Some(MenuItem::Separator),
        entry(
            "Select for Compare",
            Action::Ui(UiEffect::SelectForCompare { commit: id.clone() }),
        ),
        ctx.compare_base
            .as_ref()
            .filter(|base| *base != id)
            .and_then(|base| {
                entry(
                    "Compare with Selected",
                    Action::Ui(UiEffect::CompareWithSelected {
                        base: base.clone(),
                        commit: id.clone(),
                    }),
                )
            }),
        entry(
            "Compare with Working Tree",
            Action::Ui(UiEffect::CompareWithWorkingTree { commit: id.clone() }),
        ),
        Some(MenuItem::Separator),
    ]
}

fn tag_menu(name: &str, ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let tag = |action| Action::Tag(action);
    vec![
        entry(
            "View Details",
            tag(TagAction::ShowDetails {
                name: name.to_owned(),
            }),
        ),
        entry(
            "Delete Tag...",
            tag(TagAction::Delete {
                name: name.to_owned(),
                remote: ctx.default_remote().map(str::to_owned),
                on_remote: false,
            }),
        ),
        ctx.default_remote().and_then(|remote| {
            entry(
                "Push Tag...",
                tag(TagAction::Push {
                    name: name.to_owned(),
                    remote: remote.to_owned(),
                }),
            )
        }),
        Some(MenuItem::Separator),
        entry("Create Archive", archive(Rev::Tag(name.to_owned()), name)),
        Some(MenuItem::Separator),
        entry(
            "Copy Tag Name to Clipboard",
            tag(TagAction::CopyName {
                name: name.to_owned(),
            }),
        ),
    ]
}

fn stash_menu(stash: &Stash) -> Vec<Option<MenuItem>> {
    let action = |action| Action::Stash(action);
    vec![
        entry(
            "Apply Stash...",
            action(StashAction::Apply {
                stash: stash.clone(),
                reinstate_index: false,
            }),
        ),
        entry(
            "Create Branch from Stash...",
            action(StashAction::CreateBranch {
                stash: stash.clone(),
                name: String::new(),
            }),
        ),
        entry(
            "Pop Stash...",
            action(StashAction::Pop {
                stash: stash.clone(),
                reinstate_index: false,
            }),
        ),
        entry(
            "Drop Stash...",
            action(StashAction::Drop {
                stash: stash.clone(),
            }),
        ),
        Some(MenuItem::Separator),
        entry(
            "Copy Stash Name to Clipboard",
            action(StashAction::CopyName {
                stash: stash.clone(),
            }),
        ),
        entry(
            "Copy Stash Hash to Clipboard",
            Action::CopyCommitHash {
                commit: stash.id.clone(),
            },
        ),
    ]
}

fn uncommitted_menu() -> Vec<Option<MenuItem>> {
    let action = |action| Action::WorkingTree(action);
    vec![
        entry(
            "Stash uncommitted changes...",
            action(WorkingTreeAction::Stash {
                message: String::new(),
                include_untracked: true,
            }),
        ),
        Some(MenuItem::Separator),
        entry(
            "Reset uncommitted changes...",
            action(WorkingTreeAction::Reset {
                mode: ResetMode::Mixed,
            }),
        ),
        entry("Clean untracked files...", action(WorkingTreeAction::Clean)),
        Some(MenuItem::Separator),
        entry(
            "Open Source Control View",
            action(WorkingTreeAction::OpenSourceControl),
        ),
    ]
}

fn submenu(label: &str, items: Vec<Option<MenuItem>>) -> MenuItem {
    MenuItem::Submenu {
        label: label.to_owned(),
        items: tidy(items),
    }
}

fn pick(purpose: PickPurpose) -> Action {
    Action::Ui(UiEffect::PickRef { purpose })
}

/// The repository "..." menu (S11), in the order of VS Code's Source
/// Control "..." menu. Remote entries need a remote; current-branch entries
/// need HEAD on a branch; create entries need a HEAD commit.
fn repository_menu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let remote = ctx.default_remote();
    let on_branch_with_remote = ctx.current_branch().is_some() && remote.is_some();
    vec![
        entry_if(on_branch_with_remote, "Pull", || {
            Action::Repo(RepoAction::PullUpstream { rebase: false })
        }),
        entry_if(on_branch_with_remote, "Push", || {
            Action::Repo(RepoAction::Push)
        }),
        entry("Checkout to...", pick(PickPurpose::Checkout)),
        entry_if(remote.is_some(), "Fetch", || {
            Action::Repo(RepoAction::FetchDefault { prune: false })
        }),
        Some(MenuItem::Separator),
        Some(submenu("Commit", commit_submenu(ctx))),
        Some(submenu("Changes", changes_submenu())),
        Some(submenu("Pull, Push", pull_push_submenu(ctx))),
        Some(submenu("Branch", branch_submenu(ctx))),
        Some(submenu("Remote", remote_submenu(ctx))),
        Some(submenu("Stash", stash_submenu())),
        Some(submenu("Tags", tags_submenu(ctx))),
        Some(MenuItem::Separator),
        entry("Show Git Output", Action::Ui(UiEffect::ShowGitOutput)),
        entry("Show Reflog", Action::Ui(UiEffect::ShowReflog)),
        entry("Worktrees...", Action::Ui(UiEffect::ShowWorktrees)),
    ]
}

/// The menu of an entry in the worktrees view (S20 4). The main worktree
/// and the one the app runs in (`is_current`) can't be removed, and git
/// can't lock the main one.
#[must_use]
pub fn worktree_menu(worktree: &Worktree, is_current: bool) -> Vec<MenuItem> {
    let path = || worktree.path.clone();
    let why_not_removed = if worktree.is_main {
        Some("The main worktree can't be removed.")
    } else if is_current {
        Some("tui-git-graph is running in this worktree.")
    } else {
        None
    };
    let remove = MenuEntry::new(
        "Remove Worktree...",
        Action::Worktree(WorktreeAction::Remove {
            path: path(),
            force: false,
            locked: worktree.locked.is_some(),
        }),
    )
    .disabled_because(why_not_removed);
    let lock = if worktree.locked.is_some() {
        entry(
            "Unlock",
            Action::Worktree(WorktreeAction::Unlock { path: path() }),
        )
    } else {
        entry(
            "Lock",
            Action::Worktree(WorktreeAction::Lock { path: path() }),
        )
    };
    tidy([
        entry(
            "Copy Path to Clipboard",
            Action::Worktree(WorktreeAction::CopyPath { path: path() }),
        ),
        lock.filter(|_| !worktree.is_main),
        Some(MenuItem::Separator),
        Some(MenuItem::Entry(remove)),
        Some(MenuItem::Separator),
        entry("Add Worktree...", Action::Worktree(WorktreeAction::add())),
        entry("Prune Worktrees", Action::Worktree(WorktreeAction::Prune)),
    ])
}

/// The menu of an entry in the reflog view (S20 4): ways to get back to its
/// commit, which may be on no branch any more (a reset or deleted branch).
/// `subject` is the commit's subject, for the confirmations.
#[must_use]
pub fn reflog_menu(id: &Oid, subject: &str, ctx: &MenuContext) -> Vec<MenuItem> {
    let is_head = ctx.is_head_commit(id);
    let current = ctx.current_branch();
    tidy([
        entry(
            "Create Branch Here...",
            Action::CreateBranch {
                at: id.clone(),
                name: String::new(),
                checkout: false,
            },
        ),
        entry_if(
            !(is_head && current.is_none()),
            "Checkout (detached)...",
            || Action::CheckoutCommit {
                commit: id.clone(),
                subject: subject.to_owned(),
            },
        ),
        current.filter(|_| !is_head).and_then(|branch| {
            entry(
                "Reset current branch to here...",
                Action::ResetCurrentTo {
                    commit: id.clone(),
                    subject: subject.to_owned(),
                    current: branch.to_owned(),
                    mode: ResetMode::default(),
                },
            )
        }),
        Some(MenuItem::Separator),
        entry(
            "Copy Commit Hash to Clipboard",
            Action::CopyCommitHash { commit: id.clone() },
        ),
    ])
}

/// VS Code's Commit submenu: commit what is staged (or everything when
/// nothing is), only what is staged, or everything; each also as an amend
/// and signed off. The message is the Source Control pane's.
fn commit_submenu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let commits = |amend: bool, signoff: bool, suffix: &str| {
        [
            ("Commit", CommitScope::Auto),
            ("Commit Staged", CommitScope::Staged),
            ("Commit All", CommitScope::All),
        ]
        .map(|(label, scope)| {
            entry(
                &format!("{label}{suffix}"),
                Action::Ui(UiEffect::Commit {
                    scope,
                    amend,
                    signoff,
                }),
            )
        })
    };
    let rebasing = ctx.operation == Some(Operation::Rebase);
    let mut items = commits(false, false, "").to_vec();
    items.extend([
        Some(entry_unless(
            ctx.head_commit
                .is_none()
                .then_some("There is no commit yet."),
            "Undo Last Commit",
            Action::WorkingTree(WorkingTreeAction::UndoLastCommit),
        )),
        Some(entry_unless(
            (!rebasing).then_some("No rebase is in progress."),
            "Abort Rebase",
            Action::Operation(OperationAction::Abort(Operation::Rebase)),
        )),
        Some(MenuItem::Separator),
    ]);
    items.extend(commits(true, false, " (Amend)"));
    items.push(Some(MenuItem::Separator));
    items.extend(commits(false, true, " (Signed Off)"));
    items
}

/// VS Code's "Pull, Push" submenu. Pull, Push and Sync follow the git
/// config (`pull.rebase`, `push.default`); "Pull from..." and "Push to..."
/// ask where. Empty without a remote.
fn pull_push_submenu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let Some(remote) = ctx.default_remote() else {
        return Vec::new();
    };
    let repo = |action| Action::Repo(action);
    let branch_entries = ctx.current_branch().map(|branch| {
        [
            entry("Sync", repo(RepoAction::Sync)),
            Some(MenuItem::Separator),
            entry("Pull", repo(RepoAction::PullUpstream { rebase: false })),
            entry(
                "Pull (Rebase)",
                repo(RepoAction::PullUpstream { rebase: true }),
            ),
            entry(
                "Pull from...",
                repo(RepoAction::PullFrom {
                    remote: remote.to_owned(),
                    branch: branch.to_owned(),
                }),
            ),
            Some(MenuItem::Separator),
            entry("Push", repo(RepoAction::Push)),
            entry(
                "Push to...",
                Action::PushBranch {
                    branch: branch.to_owned(),
                    remote: remote.to_owned(),
                    set_upstream: true,
                    force_with_lease: false,
                },
            ),
            Some(MenuItem::Separator),
        ]
    });
    branch_entries
        .into_iter()
        .flatten()
        .chain([
            entry("Fetch", repo(RepoAction::FetchDefault { prune: false })),
            entry(
                "Fetch (Prune)",
                repo(RepoAction::FetchDefault { prune: true }),
            ),
            entry(
                "Fetch From All Remotes",
                repo(RepoAction::Fetch { prune: false }),
            ),
        ])
        .collect()
}

/// VS Code's Changes submenu, for every file at once.
fn changes_submenu() -> Vec<Option<MenuItem>> {
    vec![
        entry(
            "Stage All Changes",
            Action::WorkingTree(WorkingTreeAction::Stage { paths: Vec::new() }),
        ),
        entry(
            "Unstage All Changes",
            Action::WorkingTree(WorkingTreeAction::Unstage { paths: Vec::new() }),
        ),
        entry(
            "Discard All Changes",
            Action::WorkingTree(WorkingTreeAction::DiscardAll),
        ),
    ]
}

fn branch_submenu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let current = ctx.current_branch();
    let remote = ctx.default_remote();
    vec![
        entry_if(current.is_some(), "Merge...", || pick(PickPurpose::Merge)),
        entry_if(current.is_some(), "Rebase Branch...", || {
            pick(PickPurpose::Rebase)
        }),
        Some(MenuItem::Separator),
        ctx.head_commit.as_ref().and_then(|at| {
            entry(
                "Create Branch...",
                Action::CreateBranch {
                    at: at.clone(),
                    name: String::new(),
                    checkout: true,
                },
            )
        }),
        entry("Create Branch From...", pick(PickPurpose::CreateBranchFrom)),
        Some(MenuItem::Separator),
        current.and_then(|branch| {
            entry(
                "Rename Branch...",
                Action::RenameBranch {
                    from: branch.to_owned(),
                    to: branch.to_owned(),
                },
            )
        }),
        entry("Delete Branch...", pick(PickPurpose::DeleteBranch)),
        entry_if(remote.is_some(), "Delete Remote Branch...", || {
            pick(PickPurpose::DeleteRemoteBranch)
        }),
        Some(MenuItem::Separator),
        current.zip(remote).and_then(|(branch, remote)| {
            entry(
                "Publish Branch...",
                Action::PushBranch {
                    branch: branch.to_owned(),
                    remote: remote.to_owned(),
                    set_upstream: true,
                    force_with_lease: false,
                },
            )
        }),
    ]
}

fn remote_submenu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let has_remotes = !ctx.remotes.is_empty();
    vec![
        entry(
            "Add Remote...",
            Action::Repo(RepoAction::AddRemote {
                name: String::new(),
                url: String::new(),
            }),
        ),
        entry_if(has_remotes, "Remove Remote", || {
            pick(PickPurpose::RemoveRemote)
        }),
        entry_if(has_remotes, "Fetch", || {
            Action::Repo(RepoAction::Fetch { prune: false })
        }),
        entry_if(has_remotes, "Prune", || {
            Action::Repo(RepoAction::PruneRemotes {
                remotes: ctx.remotes.clone(),
            })
        }),
    ]
}

fn stash_submenu() -> Vec<Option<MenuItem>> {
    vec![
        entry(
            "Stash Changes...",
            Action::WorkingTree(WorkingTreeAction::Stash {
                message: String::new(),
                include_untracked: true,
            }),
        ),
        entry(
            "Apply Latest Stash",
            Action::Repo(RepoAction::ApplyLatestStash),
        ),
        entry("Pop Latest Stash", Action::Repo(RepoAction::PopLatestStash)),
    ]
}

fn tags_submenu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    vec![
        ctx.head_commit.as_ref().and_then(|at| {
            entry(
                "Create Tag...",
                Action::AddTag {
                    at: at.clone(),
                    name: String::new(),
                    annotated: true,
                    message: String::new(),
                    remote: ctx.default_remote().map(str::to_owned),
                    push: false,
                },
            )
        }),
        ctx.default_remote().and_then(|remote| {
            entry(
                "Push All Tags",
                Action::Repo(RepoAction::PushTags {
                    remote: remote.to_owned(),
                }),
            )
        }),
    ]
}

/// Drop is `git rebase --onto <commit>^ <commit>`, which needs exactly one
/// parent and the commit in HEAD's history.
/// "Interactive Rebase from Here..." (S20 4): edits the commits after this
/// one up to HEAD. Shown on every commit, disabled with the reason where it
/// can't run.
fn interactive_rebase_entry(commit: &CommitTarget, ctx: &MenuContext) -> MenuItem {
    let why = if let Some(operation) = ctx.operation {
        Some(format!(
            "A {} is in progress: continue or abort it first.",
            operation.label().to_lowercase()
        ))
    } else if ctx.current_branch().is_none() {
        Some("Interactive rebase needs HEAD on a branch.".to_owned())
    } else if ctx.is_head_commit(&commit.id) {
        Some(
            "This is HEAD: pick an older commit, the rebase edits the commits after it.".to_owned(),
        )
    } else if !commit.is_head_ancestor {
        Some("Only commits on the current branch can be rebased interactively.".to_owned())
    } else if !commit.linear_to_head {
        Some("There are merge commits between this commit and HEAD.".to_owned())
    } else {
        None
    };
    MenuItem::Entry(
        MenuEntry::new(
            "Interactive Rebase from Here...",
            Action::Ui(UiEffect::OpenInteractiveRebase {
                base: commit.id.clone(),
            }),
        )
        .disabled_because(why.as_deref()),
    )
}

fn why_not_droppable(commit: &CommitTarget) -> Option<&'static str> {
    if commit.is_merge() {
        Some("Merge commits can't be dropped.")
    } else if commit.is_root() {
        Some("The first commit can't be dropped.")
    } else if !commit.is_head_ancestor {
        Some("Only commits on the current branch can be dropped.")
    } else {
        None
    }
}

fn archive(rev: Rev, name: &str) -> Action {
    // Default next to the repository so the archive doesn't land in the
    // working tree as an untracked file.
    Action::CreateArchive {
        rev,
        output: format!("../{}.zip", name.replace('/', "-")),
    }
}

fn filter_toggle(branch: &str, ctx: &MenuContext) -> Option<MenuItem> {
    let selected = ctx.is_filtered(branch);
    let label = if selected {
        "Unselect in Branches Dropdown"
    } else {
        "Select in Branches Dropdown"
    };
    entry(
        label,
        Action::ToggleBranchFilter {
            branch: branch.to_owned(),
            selected,
        },
    )
}

/// Drops hidden entries, empty submenus and separators that would lead,
/// trail or repeat (in submenus too).
pub(crate) fn tidy(items: impl IntoIterator<Item = impl Into<Option<MenuItem>>>) -> Vec<MenuItem> {
    let mut menu: Vec<MenuItem> = Vec::new();
    for item in items.into_iter().filter_map(Into::into) {
        let item = match item {
            MenuItem::Submenu { label, items } => {
                let items = tidy(items);
                if items.is_empty() {
                    continue;
                }
                MenuItem::Submenu { label, items }
            }
            other => other,
        };
        let after_separator = matches!(menu.last(), None | Some(MenuItem::Separator));
        if item == MenuItem::Separator && after_separator {
            continue;
        }
        menu.push(item);
    }
    if menu.last() == Some(&MenuItem::Separator) {
        menu.pop();
    }
    menu
}

#[cfg(test)]
mod tests;
