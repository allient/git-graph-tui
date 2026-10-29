use tgg_model::Stash;

use crate::{
    Action, CommitTarget, MenuContext, PickPurpose, PullMode, RepoAction, ResetMode, Rev,
    StashAction, TagAction, Target, UiEffect, WorkingTreeAction,
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
/// (S8, S9 and its tag, stash and uncommitted-changes menus). Entries that
/// can't apply are left out rather than disabled, except Drop, which
/// explains why it isn't possible.
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

/// Adds the entry only when `show` is true.
fn entry_if(show: bool, label: &str, action: impl FnOnce() -> Action) -> Option<MenuItem> {
    show.then(|| MenuItem::Entry(MenuEntry::new(label, action())))
}

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
        entry_if(current.is_some(), "Merge into current branch...", || {
            Action::MergeIntoCurrent {
                source: rev(),
                current: current.clone(),
                no_ff: true,
            }
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

fn commit_menu(commit: &CommitTarget, ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let id = &commit.id;
    let subject = &commit.subject;
    let is_head = ctx.is_head_commit(id);
    let current = ctx.current_branch().map(str::to_owned);
    let detached_here = is_head && current.is_none();
    let on_branch = current.is_some();
    vec![
        entry(
            "Add Tag...",
            Action::AddTag {
                at: id.clone(),
                name: String::new(),
                message: String::new(),
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
        entry_if(!commit.is_head_ancestor, "Cherry Pick...", || {
            Action::CherryPick {
                commit: id.clone(),
                subject: subject.clone(),
                is_merge: commit.is_merge(),
            }
        }),
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
        entry_if(
            on_branch && !commit.is_head_ancestor,
            "Merge into current branch...",
            || Action::MergeIntoCurrent {
                source: Rev::Commit(id.clone()),
                current: current.clone(),
                no_ff: true,
            },
        ),
        entry_if(
            on_branch && !is_head,
            "Rebase current branch on this Commit...",
            || Action::RebaseCurrentOnto {
                onto: Rev::Commit(id.clone()),
                current: current.clone(),
            },
        ),
        current.as_ref().filter(|_| !is_head).and_then(|branch| {
            entry(
                "Reset current branch to this Commit...",
                Action::ResetCurrentTo {
                    commit: id.clone(),
                    subject: subject.clone(),
                    current: branch.clone(),
                    mode: ResetMode::default(),
                },
            )
        }),
        Some(MenuItem::Separator),
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

/// The repository "..." menu (S11), without Clone, Commit and Changes
/// (README non-goals). Remote entries need a remote; current-branch entries
/// need HEAD on a branch; create entries need a HEAD commit.
fn repository_menu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    let current = ctx.current_branch();
    let remote = ctx.default_remote();
    vec![
        current.filter(|_| remote.is_some()).and_then(|branch| {
            entry(
                "Pull...",
                Action::Repo(RepoAction::Pull {
                    branch: branch.to_owned(),
                    mode: PullMode::default(),
                }),
            )
        }),
        current.zip(remote).and_then(|(branch, remote)| {
            entry(
                "Push...",
                Action::PushBranch {
                    branch: branch.to_owned(),
                    remote: remote.to_owned(),
                    set_upstream: true,
                    force_with_lease: false,
                },
            )
        }),
        entry("Checkout to...", pick(PickPurpose::Checkout)),
        entry_if(remote.is_some(), "Fetch", || {
            Action::Repo(RepoAction::Fetch { prune: false })
        }),
        Some(MenuItem::Separator),
        Some(submenu("Branch", branch_submenu(ctx))),
        Some(submenu("Remote", remote_submenu(ctx))),
        Some(submenu("Stash", stash_submenu())),
        Some(submenu("Tags", tags_submenu(ctx))),
        Some(MenuItem::Separator),
        entry("Show Git Output", Action::Ui(UiEffect::ShowGitOutput)),
    ]
}

fn branch_submenu(ctx: &MenuContext) -> Vec<Option<MenuItem>> {
    vec![
        ctx.head_commit.as_ref().and_then(|at| {
            entry(
                "Create Branch...",
                Action::CreateBranch {
                    at: at.clone(),
                    name: String::new(),
                    checkout: false,
                },
            )
        }),
        ctx.current_branch().and_then(|branch| {
            entry(
                "Rename Current Branch...",
                Action::RenameBranch {
                    from: branch.to_owned(),
                    to: branch.to_owned(),
                },
            )
        }),
        entry("Delete Branch...", pick(PickPurpose::DeleteBranch)),
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
        entry_if(has_remotes, "Remove Remote...", || {
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
                    message: String::new(),
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
