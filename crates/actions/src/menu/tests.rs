use pretty_assertions::assert_eq;
use tgg_model::{HeadState, Oid, Stash};

use super::*;
use crate::{
    Action, ChangesLayout, ChangesSort, ChangesView, CommitScope, CommitTarget, GraphColumn,
    Operation, OperationAction, PickPurpose, RepoAction, ResetMode, Rev, StashAction, TagAction,
    UiEffect, WorkingTreeAction, WorktreeAction,
};
use tgg_git::worktree::Worktree;

fn oid(c: char) -> Oid {
    Oid::parse(&c.to_string().repeat(40)).unwrap()
}

/// On `main` at commit `a`, with `origin`.
fn ctx() -> MenuContext {
    MenuContext {
        head: HeadState::Branch("main".into()),
        head_commit: Some(oid('a')),
        remotes: vec!["origin".into()],
        branch_filter: Vec::new(),
        compare_base: None,
        operation: None,
        changes: ChangesLayout::default(),
    }
}

/// Labels, with `---` for separators and `(disabled: why)` for disabled entries.
fn labels(menu: &[MenuItem]) -> Vec<String> {
    menu.iter()
        .map(|item| match item {
            MenuItem::Separator => "---".to_owned(),
            MenuItem::Entry(MenuEntry {
                label,
                disabled: None,
                ..
            }) => label.clone(),
            MenuItem::Entry(MenuEntry {
                label,
                disabled: Some(why),
                ..
            }) => format!("{label} (disabled: {why})"),
            MenuItem::Submenu { label, .. } => format!("{label} ▸"),
        })
        .collect()
}

fn submenu<'m>(menu: &'m [MenuItem], label: &str) -> &'m [MenuItem] {
    menu.iter()
        .find_map(|item| match item {
            MenuItem::Submenu { label: l, items } if l == label => Some(items.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no submenu {label:?} in {:?}", labels(menu)))
}

fn action<'m>(menu: &'m [MenuItem], label: &str) -> &'m Action {
    menu.iter()
        .find_map(|item| match item {
            MenuItem::Entry(entry) if entry.label == label => Some(&entry.action),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no entry {label:?} in {:?}", labels(menu)))
}

fn local(name: &str) -> Target {
    Target::LocalBranch {
        name: name.into(),
        target: oid('b'),
    }
}

fn commit(id: char, parent_count: usize, is_head_ancestor: bool) -> Target {
    Target::Commit(CommitTarget {
        id: oid(id),
        subject: "Add lib".into(),
        parent_count,
        is_head_ancestor,
        linear_to_head: is_head_ancestor,
    })
}

// ---- branch menu (S8) ----

#[test]
fn branch_menu_matches_the_vs_code_order() {
    assert_eq!(
        labels(&menu_for(&local("develop"), &ctx())),
        [
            "Checkout Branch",
            "Rename Branch...",
            "Delete Branch...",
            "Merge into current branch...",
            "Rebase current branch on Branch...",
            "Push Branch...",
            "Create Pull Request",
            "---",
            "Create Archive",
            "Select in Branches Dropdown",
            "---",
            "Copy Branch Name to Clipboard",
        ]
    );
}

#[test]
fn current_branch_menu_hides_what_does_not_apply() {
    assert_eq!(
        labels(&menu_for(&local("main"), &ctx())),
        [
            "Rename Branch...",
            "Push Branch...",
            "Create Pull Request",
            "---",
            "Create Archive",
            "Select in Branches Dropdown",
            "---",
            "Copy Branch Name to Clipboard",
        ]
    );
}

#[test]
fn without_remotes_there_is_nothing_to_push_to() {
    let ctx = MenuContext {
        remotes: Vec::new(),
        ..ctx()
    };
    let menu = labels(&menu_for(&local("develop"), &ctx));
    assert!(!menu.contains(&"Push Branch...".to_owned()), "{menu:?}");
    assert!(
        !menu.contains(&"Create Pull Request".to_owned()),
        "{menu:?}"
    );
}

#[test]
fn detached_head_has_no_current_branch_to_merge_into() {
    let ctx = MenuContext {
        head: HeadState::Detached,
        ..ctx()
    };
    assert_eq!(
        labels(&menu_for(&local("develop"), &ctx)),
        [
            "Checkout Branch",
            "Rename Branch...",
            "Delete Branch...",
            "Push Branch...",
            "Create Pull Request",
            "---",
            "Create Archive",
            "Select in Branches Dropdown",
            "---",
            "Copy Branch Name to Clipboard",
        ]
    );
}

#[test]
fn branch_in_the_filter_offers_unselect() {
    let ctx = MenuContext {
        branch_filter: vec!["develop".into()],
        ..ctx()
    };
    let menu = menu_for(&local("develop"), &ctx);
    assert_eq!(
        action(&menu, "Unselect in Branches Dropdown"),
        &Action::ToggleBranchFilter {
            branch: "develop".into(),
            selected: true
        }
    );
}

#[test]
fn branch_actions_are_prefilled() {
    let menu = menu_for(&local("feature/login"), &ctx());
    assert_eq!(
        action(&menu, "Checkout Branch"),
        &Action::CheckoutBranch {
            name: "feature/login".into()
        }
    );
    assert_eq!(
        action(&menu, "Rename Branch..."),
        &Action::RenameBranch {
            from: "feature/login".into(),
            to: "feature/login".into()
        }
    );
    assert_eq!(
        action(&menu, "Delete Branch..."),
        &Action::DeleteBranch {
            name: "feature/login".into(),
            force: false
        }
    );
    assert_eq!(
        action(&menu, "Merge into current branch..."),
        &Action::MergeIntoCurrent {
            source: Rev::Local("feature/login".into()),
            current: Some("main".into()),
            no_ff: true
        }
    );
    assert_eq!(
        action(&menu, "Push Branch..."),
        &Action::PushBranch {
            branch: "feature/login".into(),
            remote: "origin".into(),
            set_upstream: true,
            force_with_lease: false
        }
    );
    // Next to the repository, not inside the working tree.
    assert_eq!(
        action(&menu, "Create Archive"),
        &Action::CreateArchive {
            rev: Rev::Local("feature/login".into()),
            output: "../feature-login.zip".into()
        }
    );
}

#[test]
fn remote_branch_menu_checks_out_a_tracking_branch() {
    let target = Target::RemoteBranch {
        remote: "origin".into(),
        branch: "feature/x".into(),
        target: oid('b'),
    };
    let menu = menu_for(&target, &ctx());
    assert_eq!(
        labels(&menu),
        [
            "Checkout Branch...",
            "Delete Remote Branch...",
            "Fetch into local branch...",
            "Merge into current branch...",
            "Pull into current branch...",
            "Rebase current branch on Branch...",
            "Create Pull Request",
            "---",
            "Create Archive",
            "Select in Branches Dropdown",
            "---",
            "Copy Branch Name to Clipboard",
        ]
    );
    assert_eq!(
        action(&menu, "Checkout Branch..."),
        &Action::CheckoutRemoteBranch {
            remote: "origin".into(),
            branch: "feature/x".into(),
            local_name: "feature/x".into()
        }
    );
    assert_eq!(
        action(&menu, "Copy Branch Name to Clipboard"),
        &Action::CopyBranchName {
            name: "origin/feature/x".into()
        }
    );
}

#[test]
fn fetch_into_local_branch_defaults_to_the_remote_branch_name() {
    let target = Target::RemoteBranch {
        remote: "origin".into(),
        branch: "feature/x".into(),
        target: oid('b'),
    };
    assert_eq!(
        action(&menu_for(&target, &ctx()), "Fetch into local branch..."),
        &Action::FetchIntoLocal {
            remote: "origin".into(),
            branch: "feature/x".into(),
            local_name: "feature/x".into(),
            force: false,
        }
    );
}

#[test]
fn fetch_into_the_current_branch_is_disabled() {
    let target = Target::RemoteBranch {
        remote: "origin".into(),
        branch: "main".into(),
        target: oid('b'),
    };
    let menu = menu_for(&target, &ctx());
    assert!(
        labels(&menu).contains(
            &"Fetch into local branch... (disabled: 'main' is the current branch, and git can't fetch into a checked-out branch. Use Pull instead.)"
                .to_owned()
        ),
        "{:?}",
        labels(&menu)
    );
}

// ---- commit menu (S9) ----

#[test]
fn commit_menu_matches_the_vs_code_order() {
    // An older commit on a side branch: not reachable from HEAD.
    assert_eq!(
        labels(&menu_for(&commit('c', 1, false), &ctx())),
        [
            "Add Tag...",
            "Create Branch...",
            "---",
            "Checkout...",
            "Cherry Pick...",
            "Revert...",
            "Drop... (disabled: Only commits on the current branch can be dropped.)",
            "---",
            "Merge into current branch...",
            "Rebase current branch on this Commit...",
            "Interactive Rebase from Here... (disabled: Only commits on the current branch can be rebased interactively.)",
            "Reset current branch to this Commit...",
            "---",
            "Select for Compare",
            "Compare with Working Tree",
            "---",
            "Copy Commit Hash to Clipboard",
            "Copy Commit Subject to Clipboard",
        ]
    );
}

#[test]
fn commit_on_the_current_branch_can_be_dropped_and_explains_merge_and_pick() {
    assert_eq!(
        labels(&menu_for(&commit('c', 1, true), &ctx())),
        [
            "Add Tag...",
            "Create Branch...",
            "---",
            "Checkout...",
            "Cherry Pick... (disabled: This commit is already on the current branch.)",
            "Revert...",
            "Drop...",
            "---",
            "Merge into current branch... (disabled: This commit is already on the current branch.)",
            "Rebase current branch on this Commit...",
            "Interactive Rebase from Here...",
            "Reset current branch to this Commit...",
            "---",
            "Select for Compare",
            "Compare with Working Tree",
            "---",
            "Copy Commit Hash to Clipboard",
            "Copy Commit Subject to Clipboard",
        ]
    );
}

#[test]
fn head_commit_menu() {
    assert_eq!(
        labels(&menu_for(&commit('a', 1, true), &ctx())),
        [
            "Add Tag...",
            "Create Branch...",
            "---",
            "Checkout...",
            "Cherry Pick... (disabled: This commit is already on the current branch.)",
            "Revert...",
            "Drop...",
            "---",
            "Merge into current branch... (disabled: This commit is already on the current branch.)",
            "Rebase current branch on this Commit... (disabled: The current branch is already at this commit.)",
            "Interactive Rebase from Here... (disabled: This is HEAD: pick an older commit, the rebase edits the commits after it.)",
            "Reset current branch to this Commit... (disabled: The current branch is already at this commit.)",
            "---",
            "Select for Compare",
            "Compare with Working Tree",
            "---",
            "Copy Commit Hash to Clipboard",
            "Copy Commit Subject to Clipboard",
        ]
    );
}

#[test]
fn compare_entries_mark_a_commit_and_compare_it_with_another() {
    let menu = menu_for(&commit('c', 1, false), &ctx());
    assert_eq!(
        action(&menu, "Select for Compare"),
        &Action::Ui(UiEffect::SelectForCompare { commit: oid('c') })
    );
    assert_eq!(
        action(&menu, "Compare with Working Tree"),
        &Action::Ui(UiEffect::CompareWithWorkingTree { commit: oid('c') })
    );

    // With another commit marked, "Compare with Selected" follows "Select for Compare".
    let marked = MenuContext {
        compare_base: Some(oid('b')),
        ..ctx()
    };
    let menu = menu_for(&commit('c', 1, false), &marked);
    let names = labels(&menu);
    let select = names
        .iter()
        .position(|l| l == "Select for Compare")
        .unwrap();
    assert_eq!(names[select + 1], "Compare with Selected");
    assert_eq!(
        action(&menu, "Compare with Selected"),
        &Action::Ui(UiEffect::CompareWithSelected {
            base: oid('b'),
            commit: oid('c'),
        })
    );

    // Not on the marked commit itself.
    let names = labels(&menu_for(&commit('b', 1, false), &marked));
    assert!(
        names.contains(&"Select for Compare".to_owned()),
        "{names:?}"
    );
    assert!(
        !names.contains(&"Compare with Selected".to_owned()),
        "{names:?}"
    );
}

#[test]
fn merges_and_root_commits_cannot_be_dropped() {
    let drop = |target: Target| {
        labels(&menu_for(&target, &ctx()))
            .into_iter()
            .find(|l| l.starts_with("Drop..."))
            .unwrap()
    };
    assert_eq!(
        drop(commit('c', 2, true)),
        "Drop... (disabled: Merge commits can't be dropped.)"
    );
    assert_eq!(
        drop(commit('c', 0, true)),
        "Drop... (disabled: The first commit can't be dropped.)"
    );
}

#[test]
fn detached_head_hides_checkout_here_and_disables_branch_actions() {
    let ctx = MenuContext {
        head: HeadState::Detached,
        ..ctx()
    };
    let menu = labels(&menu_for(&commit('a', 1, true), &ctx));
    assert!(!menu.contains(&"Checkout...".to_owned()), "{menu:?}");
    let other = labels(&menu_for(&commit('c', 1, false), &ctx));
    assert!(other.contains(&"Checkout...".to_owned()), "{other:?}");
    for label in [
        "Merge into current branch...",
        "Rebase current branch on this Commit...",
        "Reset current branch to this Commit...",
    ] {
        let disabled = format!("{label} (disabled: HEAD isn't on a branch.)");
        assert!(other.contains(&disabled), "{other:?}");
    }
}

/// The "Interactive Rebase from Here..." entry of `target`'s menu.
fn interactive_rebase(target: &Target, ctx: &MenuContext) -> String {
    labels(&menu_for(target, ctx))
        .into_iter()
        .find(|l| l.starts_with("Interactive Rebase from Here..."))
        .unwrap()
}

#[test]
fn interactive_rebase_opens_the_panel_for_an_older_commit_on_the_branch() {
    let menu = menu_for(&commit('c', 1, true), &ctx());
    assert_eq!(
        action(&menu, "Interactive Rebase from Here..."),
        &Action::Ui(UiEffect::OpenInteractiveRebase { base: oid('c') })
    );
    // A merge commit as the base is fine: only the commits after it change.
    assert_eq!(
        interactive_rebase(&commit('c', 2, true), &ctx()),
        "Interactive Rebase from Here..."
    );
}

#[test]
fn interactive_rebase_is_disabled_where_it_cannot_run() {
    let disabled = |why: &str| format!("Interactive Rebase from Here... (disabled: {why})");
    assert_eq!(
        interactive_rebase(&commit('a', 1, true), &ctx()),
        disabled("This is HEAD: pick an older commit, the rebase edits the commits after it.")
    );
    let detached = MenuContext {
        head: HeadState::Detached,
        ..ctx()
    };
    assert_eq!(
        interactive_rebase(&commit('c', 1, true), &detached),
        disabled("Interactive rebase needs HEAD on a branch.")
    );
    let merged = Target::Commit(CommitTarget {
        id: oid('c'),
        subject: "Add lib".into(),
        parent_count: 1,
        is_head_ancestor: true,
        linear_to_head: false,
    });
    assert_eq!(
        interactive_rebase(&merged, &ctx()),
        disabled("There are merge commits between this commit and HEAD.")
    );
    let rebasing = MenuContext {
        operation: Some(crate::Operation::Rebase),
        ..ctx()
    };
    assert_eq!(
        interactive_rebase(&commit('c', 1, true), &rebasing),
        disabled("A rebase is in progress: continue or abort it first.")
    );
}

#[test]
fn commit_actions_are_prefilled() {
    let menu = menu_for(&commit('c', 2, false), &ctx());
    assert_eq!(
        action(&menu, "Cherry Pick..."),
        &Action::CherryPick {
            commit: oid('c'),
            subject: "Add lib".into(),
            is_merge: true
        }
    );
    assert_eq!(
        action(&menu, "Reset current branch to this Commit..."),
        &Action::ResetCurrentTo {
            commit: oid('c'),
            subject: "Add lib".into(),
            current: "main".into(),
            mode: ResetMode::Mixed
        }
    );
    assert_eq!(
        action(&menu, "Rebase current branch on this Commit..."),
        &Action::RebaseCurrentOnto {
            onto: Rev::Commit(oid('c')),
            current: Some("main".into())
        }
    );
    assert_eq!(
        action(&menu, "Copy Commit Hash to Clipboard"),
        &Action::CopyCommitHash { commit: oid('c') }
    );
}

// ---- other targets ----

fn stash_target() -> Target {
    Target::Stash(Stash {
        id: oid('d'),
        index: 0,
        subject: "WIP".into(),
    })
}

fn tag_target() -> Target {
    Target::Tag {
        name: "v1".into(),
        target: oid('a'),
    }
}

#[test]
fn tag_menu() {
    let menu = menu_for(&tag_target(), &ctx());
    assert_eq!(
        labels(&menu),
        [
            "View Details",
            "Delete Tag...",
            "Push Tag...",
            "---",
            "Create Archive",
            "---",
            "Copy Tag Name to Clipboard",
        ]
    );
    assert_eq!(
        action(&menu, "Delete Tag..."),
        &Action::Tag(TagAction::Delete {
            name: "v1".into(),
            remote: Some("origin".into()),
            on_remote: false
        })
    );
    assert_eq!(
        action(&menu, "Create Archive"),
        &Action::CreateArchive {
            rev: Rev::Tag("v1".into()),
            output: "../v1.zip".into()
        }
    );
}

#[test]
fn tag_menu_without_remotes_cannot_push() {
    let ctx = MenuContext {
        remotes: Vec::new(),
        ..ctx()
    };
    let menu = menu_for(&tag_target(), &ctx);
    assert!(!labels(&menu).contains(&"Push Tag...".to_owned()));
    assert_eq!(
        action(&menu, "Delete Tag..."),
        &Action::Tag(TagAction::Delete {
            name: "v1".into(),
            remote: None,
            on_remote: false
        })
    );
}

#[test]
fn stash_menu() {
    let menu = menu_for(&stash_target(), &ctx());
    assert_eq!(
        labels(&menu),
        [
            "Apply Stash...",
            "Create Branch from Stash...",
            "Pop Stash...",
            "Drop Stash...",
            "---",
            "Copy Stash Name to Clipboard",
            "Copy Stash Hash to Clipboard",
        ]
    );
    let Target::Stash(stash) = stash_target() else {
        unreachable!()
    };
    assert_eq!(
        action(&menu, "Pop Stash..."),
        &Action::Stash(StashAction::Pop {
            stash: stash.clone(),
            reinstate_index: false
        })
    );
    assert_eq!(
        action(&menu, "Copy Stash Hash to Clipboard"),
        &Action::CopyCommitHash { commit: oid('d') }
    );
}

#[test]
fn uncommitted_menu() {
    let menu = menu_for(&Target::Uncommitted, &ctx());
    assert_eq!(
        labels(&menu),
        [
            "Stash uncommitted changes...",
            "---",
            "Reset uncommitted changes...",
            "Clean untracked files...",
            "---",
            "Open Source Control View",
        ]
    );
    assert_eq!(
        action(&menu, "Stash uncommitted changes..."),
        &Action::WorkingTree(WorkingTreeAction::Stash {
            message: String::new(),
            include_untracked: true
        })
    );
    assert_eq!(
        action(&menu, "Reset uncommitted changes..."),
        &Action::WorkingTree(WorkingTreeAction::Reset {
            mode: ResetMode::Mixed
        })
    );
}

#[test]
fn separators_never_lead_trail_or_repeat() {
    let items = vec![
        MenuItem::Separator,
        MenuItem::Separator,
        MenuItem::Entry(MenuEntry::new(
            "A",
            Action::CopyCommitSubject {
                subject: "s".into(),
            },
        )),
        MenuItem::Separator,
        MenuItem::Separator,
        MenuItem::Entry(MenuEntry::new(
            "B",
            Action::CopyCommitSubject {
                subject: "s".into(),
            },
        )),
        MenuItem::Separator,
    ];
    assert_eq!(labels(&tidy(items)), ["A", "---", "B"]);
}

// ---- repository "..." menu (S11) ----

#[test]
fn repository_menu_matches_the_vs_code_order() {
    let menu = menu_for(&Target::Repository, &ctx());
    assert_eq!(
        labels(&menu),
        [
            "View as Tree",
            "---",
            "View & Sort ▸",
            "---",
            "Pull",
            "Push",
            "Clone",
            "Checkout to...",
            "Fetch",
            "---",
            "Commit ▸",
            "Changes ▸",
            "Pull, Push ▸",
            "Branch ▸",
            "Remote ▸",
            "Stash ▸",
            "Tags ▸",
            "---",
            "Show Git Output",
            "Show Reflog",
            "Worktrees...",
        ]
    );
    assert_eq!(
        labels(submenu(&menu, "Branch")),
        [
            "Merge...",
            "Rebase Branch...",
            "---",
            "Create Branch...",
            "Create Branch From...",
            "---",
            "Rename Branch...",
            "Delete Branch...",
            "Delete Remote Branch...",
            "---",
            "Publish Branch...",
        ]
    );
    assert_eq!(
        labels(submenu(&menu, "Remote")),
        ["Add Remote...", "Remove Remote", "Fetch", "Prune"]
    );
    assert_eq!(
        labels(submenu(&menu, "Stash")),
        [
            "Stash",
            "Stash (Include Untracked)",
            "Stash Staged",
            "---",
            "Apply Latest Stash",
            "Apply Stash...",
            "---",
            "Pop Latest Stash",
            "Pop Stash...",
            "---",
            "Drop Stash...",
            "Drop All Stashes...",
            "---",
            "View Stash...",
        ]
    );
    assert_eq!(
        labels(submenu(&menu, "Tags")),
        [
            "Create Tag...",
            "Delete Tag...",
            "Delete Remote Tag...",
            "---",
            "Push Tags"
        ]
    );
}

#[test]
fn commit_and_changes_submenus_match_vs_code() {
    let menu = menu_for(&Target::Repository, &ctx());
    let no_rebase = " (disabled: No rebase is in progress.)";
    assert_eq!(
        labels(submenu(&menu, "Commit")),
        [
            "Commit",
            "Commit Staged",
            "Commit All",
            "Undo Last Commit",
            &format!("Abort Rebase{no_rebase}"),
            "---",
            "Commit (Amend)",
            "Commit Staged (Amend)",
            "Commit All (Amend)",
            "---",
            "Commit (Signed Off)",
            "Commit Staged (Signed Off)",
            "Commit All (Signed Off)",
        ]
    );
    let commit = submenu(&menu, "Commit");
    let effect = |scope, amend, signoff| {
        Action::Ui(UiEffect::Commit {
            scope,
            amend,
            signoff,
        })
    };
    assert_eq!(
        action(commit, "Commit"),
        &effect(CommitScope::Auto, false, false)
    );
    assert_eq!(
        action(commit, "Commit Staged (Amend)"),
        &effect(CommitScope::Staged, true, false)
    );
    assert_eq!(
        action(commit, "Commit All (Signed Off)"),
        &effect(CommitScope::All, false, true)
    );
    assert_eq!(
        action(commit, "Undo Last Commit"),
        &Action::WorkingTree(WorkingTreeAction::UndoLastCommit)
    );

    assert_eq!(
        labels(submenu(&menu, "Changes")),
        [
            "Stage All Changes",
            "Unstage All Changes",
            "Discard All Changes"
        ]
    );
    let changes = submenu(&menu, "Changes");
    assert_eq!(
        action(changes, "Stage All Changes"),
        &Action::WorkingTree(WorkingTreeAction::Stage { paths: Vec::new() })
    );
    assert_eq!(
        action(changes, "Unstage All Changes"),
        &Action::WorkingTree(WorkingTreeAction::Unstage { paths: Vec::new() })
    );
    assert_eq!(
        action(changes, "Discard All Changes"),
        &Action::WorkingTree(WorkingTreeAction::DiscardAll)
    );
}

#[test]
fn pull_push_submenu_matches_vs_code() {
    let menu = menu_for(&Target::Repository, &ctx());
    let sub = submenu(&menu, "Pull, Push");
    assert_eq!(
        labels(sub),
        [
            "Sync",
            "---",
            "Pull",
            "Pull (Rebase)",
            "Pull from...",
            "---",
            "Push",
            "Push to...",
            "---",
            "Fetch",
            "Fetch (Prune)",
            "Fetch From All Remotes",
        ]
    );
    assert_eq!(action(sub, "Sync"), &Action::Repo(RepoAction::Sync));
    assert_eq!(
        action(sub, "Pull (Rebase)"),
        &Action::Repo(RepoAction::PullUpstream { rebase: true })
    );
    assert_eq!(
        action(sub, "Pull from..."),
        &Action::Repo(RepoAction::PullFrom {
            remote: "origin".into(),
            branch: "main".into(),
        })
    );
    assert_eq!(
        action(sub, "Push to..."),
        &Action::PushBranch {
            branch: "main".into(),
            remote: "origin".into(),
            set_upstream: true,
            force_with_lease: false
        }
    );
    assert_eq!(
        action(sub, "Fetch (Prune)"),
        &Action::Repo(RepoAction::FetchDefault { prune: true })
    );
    assert_eq!(
        action(sub, "Fetch From All Remotes"),
        &Action::Repo(RepoAction::Fetch { prune: false })
    );

    // Without a remote there is nothing to pull, push or fetch.
    let local = MenuContext {
        remotes: Vec::new(),
        ..ctx()
    };
    let names = labels(&menu_for(&Target::Repository, &local));
    assert!(!names.iter().any(|l| l.starts_with("Pull")), "{names:?}");
}

#[test]
fn branch_submenu_entries_pick_a_ref_or_act_on_the_current_branch() {
    let menu = menu_for(&Target::Repository, &ctx());
    let branch = submenu(&menu, "Branch");
    let pick_for = |purpose| Action::Ui(UiEffect::PickRef { purpose });
    assert_eq!(action(branch, "Merge..."), &pick_for(PickPurpose::Merge));
    assert_eq!(
        action(branch, "Rebase Branch..."),
        &pick_for(PickPurpose::Rebase)
    );
    assert_eq!(
        action(branch, "Create Branch From..."),
        &pick_for(PickPurpose::CreateBranchFrom)
    );
    assert_eq!(
        action(branch, "Delete Remote Branch..."),
        &pick_for(PickPurpose::DeleteRemoteBranch)
    );
    // As in VS Code, a new branch is checked out.
    assert_eq!(
        action(branch, "Create Branch..."),
        &Action::CreateBranch {
            at: oid('a'),
            name: String::new(),
            checkout: true
        }
    );
    assert_eq!(
        action(branch, "Rename Branch..."),
        &Action::RenameBranch {
            from: "main".into(),
            to: "main".into()
        }
    );
    assert_eq!(
        action(branch, "Publish Branch..."),
        &Action::PushBranch {
            branch: "main".into(),
            remote: "origin".into(),
            set_upstream: true,
            force_with_lease: false
        }
    );
}

#[test]
fn remote_branch_menu_deletes_and_pulls_like_vs_code() {
    let target = Target::RemoteBranch {
        remote: "origin".into(),
        branch: "feature/x".into(),
        target: oid('b'),
    };
    let menu = menu_for(&target, &ctx());
    assert_eq!(
        action(&menu, "Delete Remote Branch..."),
        &Action::Repo(RepoAction::DeleteRemoteBranch {
            remote: "origin".into(),
            branch: "feature/x".into()
        })
    );
    assert_eq!(
        action(&menu, "Pull into current branch..."),
        &Action::Repo(RepoAction::PullFrom {
            remote: "origin".into(),
            branch: "feature/x".into()
        })
    );
}

#[test]
fn stash_and_tags_submenus_stash_or_pick() {
    let menu = menu_for(&Target::Repository, &ctx());
    let stash = submenu(&menu, "Stash");
    let pick_for = |purpose| Action::Ui(UiEffect::PickRef { purpose });
    assert_eq!(
        action(stash, "Stash"),
        &Action::WorkingTree(WorkingTreeAction::Stash {
            message: String::new(),
            include_untracked: false
        })
    );
    assert_eq!(
        action(stash, "Stash (Include Untracked)"),
        &Action::WorkingTree(WorkingTreeAction::Stash {
            message: String::new(),
            include_untracked: true
        })
    );
    assert_eq!(
        action(stash, "Stash Staged"),
        &Action::WorkingTree(WorkingTreeAction::StashStaged {
            message: String::new()
        })
    );
    assert_eq!(
        action(stash, "Apply Stash..."),
        &pick_for(PickPurpose::ApplyStash)
    );
    assert_eq!(
        action(stash, "Pop Stash..."),
        &pick_for(PickPurpose::PopStash)
    );
    assert_eq!(
        action(stash, "Drop Stash..."),
        &pick_for(PickPurpose::DropStash)
    );
    assert_eq!(
        action(stash, "Drop All Stashes..."),
        &Action::Repo(RepoAction::DropAllStashes)
    );
    assert_eq!(
        action(stash, "View Stash..."),
        &pick_for(PickPurpose::ViewStash)
    );
    let tags = submenu(&menu, "Tags");
    assert_eq!(
        action(tags, "Delete Tag..."),
        &pick_for(PickPurpose::DeleteTag)
    );
    assert_eq!(
        action(tags, "Delete Remote Tag..."),
        &pick_for(PickPurpose::DeleteRemoteTag)
    );
    assert_eq!(
        action(tags, "Push Tags"),
        &Action::Repo(RepoAction::PushTags {
            remote: "origin".into()
        })
    );
}

#[test]
fn view_and_sort_switch_the_source_control_list() {
    let menu = menu_for(&Target::Repository, &ctx());
    assert_eq!(
        action(&menu, "View as Tree"),
        &Action::Ui(UiEffect::SetChangesView(ChangesView::Tree))
    );
    let sort = submenu(&menu, "View & Sort");
    assert_eq!(
        labels(sort),
        [
            "✓ View as List",
            "  View as Tree",
            "---",
            "  Sort Changes by Name",
            "✓ Sort Changes by Path",
            "  Sort Changes by Status",
        ]
    );
    assert_eq!(
        action(sort, "  Sort Changes by Status"),
        &Action::Ui(UiEffect::SetChangesSort(ChangesSort::Status))
    );

    // In tree view the top entry switches back, and the check moves.
    let tree = MenuContext {
        changes: ChangesLayout {
            view: ChangesView::Tree,
            sort: ChangesSort::Name,
        },
        ..ctx()
    };
    let menu = menu_for(&Target::Repository, &tree);
    assert_eq!(
        action(&menu, "View as List"),
        &Action::Ui(UiEffect::SetChangesView(ChangesView::List))
    );
    let sort = labels(submenu(&menu, "View & Sort"));
    assert!(sort.contains(&"✓ View as Tree".to_owned()), "{sort:?}");
    assert!(
        sort.contains(&"✓ Sort Changes by Name".to_owned()),
        "{sort:?}"
    );
}

#[test]
fn abort_rebase_and_undo_depend_on_the_repository_state() {
    let rebasing = MenuContext {
        operation: Some(Operation::Rebase),
        ..ctx()
    };
    let menu = menu_for(&Target::Repository, &rebasing);
    assert_eq!(
        action(submenu(&menu, "Commit"), "Abort Rebase"),
        &Action::Operation(OperationAction::Abort(Operation::Rebase))
    );
    let unborn = MenuContext {
        head: HeadState::Unborn,
        head_commit: None,
        ..ctx()
    };
    let menu = menu_for(&Target::Repository, &unborn);
    assert!(
        labels(submenu(&menu, "Commit"))
            .contains(&"Undo Last Commit (disabled: There is no commit yet.)".to_owned())
    );
}

#[test]
fn repository_actions_are_prefilled() {
    let menu = menu_for(&Target::Repository, &ctx());
    assert_eq!(
        action(&menu, "Pull"),
        &Action::Repo(RepoAction::PullUpstream { rebase: false })
    );
    assert_eq!(action(&menu, "Push"), &Action::Repo(RepoAction::Push));
    // The app fills in the folder that holds the repository.
    assert_eq!(
        action(&menu, "Clone"),
        &Action::Repo(RepoAction::Clone {
            url: String::new(),
            parent: String::new()
        })
    );
    assert_eq!(
        action(&menu, "Fetch"),
        &Action::Repo(RepoAction::FetchDefault { prune: false })
    );
    assert_eq!(
        action(&menu, "Checkout to..."),
        &Action::Ui(UiEffect::PickRef {
            purpose: PickPurpose::Checkout
        })
    );
    let branch = submenu(&menu, "Branch");
    assert_eq!(
        action(branch, "Create Branch..."),
        &Action::CreateBranch {
            at: oid('a'),
            name: String::new(),
            checkout: true
        }
    );
    assert_eq!(
        action(branch, "Delete Branch..."),
        &Action::Ui(UiEffect::PickRef {
            purpose: PickPurpose::DeleteBranch
        })
    );
    assert_eq!(
        action(submenu(&menu, "Remote"), "Prune"),
        &Action::Repo(RepoAction::PruneRemotes {
            remotes: vec!["origin".into()]
        })
    );
    assert_eq!(
        action(&menu, "Show Git Output"),
        &Action::Ui(UiEffect::ShowGitOutput)
    );
    assert_eq!(
        action(&menu, "Show Reflog"),
        &Action::Ui(UiEffect::ShowReflog)
    );
    assert_eq!(
        action(&menu, "Worktrees..."),
        &Action::Ui(UiEffect::ShowWorktrees)
    );
}

#[test]
fn repository_menu_without_remotes_or_branch() {
    let ctx = MenuContext {
        head: HeadState::Detached,
        remotes: Vec::new(),
        ..ctx()
    };
    let menu = menu_for(&Target::Repository, &ctx);
    assert_eq!(
        labels(&menu),
        [
            "View as Tree",
            "---",
            "View & Sort ▸",
            "---",
            "Clone",
            "Checkout to...",
            "---",
            "Commit ▸",
            "Changes ▸",
            "Branch ▸",
            "Remote ▸",
            "Stash ▸",
            "Tags ▸",
            "---",
            "Show Git Output",
            "Show Reflog",
            "Worktrees...",
        ]
    );
    assert_eq!(
        labels(submenu(&menu, "Branch")),
        [
            "Create Branch...",
            "Create Branch From...",
            "---",
            "Delete Branch..."
        ]
    );
    assert_eq!(labels(submenu(&menu, "Remote")), ["Add Remote..."]);
    assert_eq!(
        labels(submenu(&menu, "Tags")),
        ["Create Tag...", "Delete Tag..."]
    );
}

#[test]
fn empty_submenus_are_dropped() {
    let ctx = MenuContext {
        head: HeadState::Unborn,
        head_commit: None,
        remotes: Vec::new(),
        branch_filter: Vec::new(),
        compare_base: None,
        operation: None,
        changes: ChangesLayout::default(),
    };
    let menu = menu_for(&Target::Repository, &ctx);
    assert!(
        !labels(&menu).contains(&"Tags ▸".to_owned()),
        "{:?}",
        labels(&menu)
    );
}

#[test]
fn create_pull_request_targets_the_branch_on_its_remote() {
    let menu = menu_for(&local("develop"), &ctx());
    assert_eq!(
        action(&menu, "Create Pull Request"),
        &Action::Ui(UiEffect::CreatePullRequest {
            remote: "origin".into(),
            branch: "develop".into()
        })
    );
    let remote = Target::RemoteBranch {
        remote: "upstream".into(),
        branch: "feature/x".into(),
        target: oid('b'),
    };
    assert_eq!(
        action(&menu_for(&remote, &ctx()), "Create Pull Request"),
        &Action::Ui(UiEffect::CreatePullRequest {
            remote: "upstream".into(),
            branch: "feature/x".into()
        })
    );
}

// ---- column header menu (S20 1) ----

#[test]
fn the_column_menu_checks_the_shown_columns() {
    let menu = column_menu(&[GraphColumn::Author]);
    assert_eq!(labels(&menu), ["✓ Date", "  Author", "✓ Commit"]);
    assert_eq!(
        action(&menu, "  Author"),
        &Action::Ui(UiEffect::ToggleColumn {
            column: GraphColumn::Author
        })
    );
    assert_eq!(
        action(&menu, "✓ Commit"),
        &Action::Ui(UiEffect::ToggleColumn {
            column: GraphColumn::Commit
        })
    );
}

#[test]
fn column_names_round_trip() {
    for column in GraphColumn::ALL {
        assert_eq!(GraphColumn::from_name(column.name()), Some(column));
    }
    assert_eq!(GraphColumn::Commit.name(), "commit");
    assert_eq!(GraphColumn::from_name("graph"), None, "always shown");
}

// ---- reflog entry menu (S20 4) ----

#[test]
fn reflog_menu_recovers_the_entrys_commit() {
    let menu = reflog_menu(&oid('c'), "Lost work", &ctx());
    assert_eq!(
        labels(&menu),
        [
            "Create Branch Here...",
            "Checkout (detached)...",
            "Reset current branch to here...",
            "---",
            "Copy Commit Hash to Clipboard",
        ]
    );
    assert_eq!(
        action(&menu, "Create Branch Here..."),
        &Action::CreateBranch {
            at: oid('c'),
            name: String::new(),
            checkout: false
        }
    );
    assert_eq!(
        action(&menu, "Checkout (detached)..."),
        &Action::CheckoutCommit {
            commit: oid('c'),
            subject: "Lost work".into()
        }
    );
    assert_eq!(
        action(&menu, "Reset current branch to here..."),
        &Action::ResetCurrentTo {
            commit: oid('c'),
            subject: "Lost work".into(),
            current: "main".into(),
            mode: ResetMode::default()
        }
    );
    assert_eq!(
        action(&menu, "Copy Commit Hash to Clipboard"),
        &Action::CopyCommitHash { commit: oid('c') }
    );
}

#[test]
fn reflog_menu_on_a_detached_head_cannot_reset_a_branch() {
    let ctx = MenuContext {
        head: HeadState::Detached,
        ..ctx()
    };
    let menu = reflog_menu(&oid('c'), "Lost work", &ctx);
    assert!(
        !labels(&menu).contains(&"Reset current branch to here...".to_owned()),
        "{:?}",
        labels(&menu)
    );
}

// ---- worktree entry menu (S20 4) ----

fn linked_worktree() -> Worktree {
    Worktree {
        path: "/src/app-dev".into(),
        head: Some(oid('a')),
        branch: Some("dev".into()),
        ..Worktree::default()
    }
}

#[test]
fn worktree_menu_offers_copy_lock_remove_add_and_prune() {
    let menu = worktree_menu(&linked_worktree(), false);
    assert_eq!(
        labels(&menu),
        [
            "Copy Path to Clipboard",
            "Lock",
            "---",
            "Remove Worktree...",
            "---",
            "Add Worktree...",
            "Prune Worktrees",
        ]
    );
    let path = || "/src/app-dev".to_owned();
    assert_eq!(
        action(&menu, "Copy Path to Clipboard"),
        &Action::Worktree(WorktreeAction::CopyPath { path: path() })
    );
    assert_eq!(
        action(&menu, "Lock"),
        &Action::Worktree(WorktreeAction::Lock { path: path() })
    );
    assert_eq!(
        action(&menu, "Remove Worktree..."),
        &Action::Worktree(WorktreeAction::Remove {
            path: path(),
            force: false,
            locked: false
        })
    );
    assert_eq!(
        action(&menu, "Add Worktree..."),
        &Action::Worktree(WorktreeAction::add())
    );
    assert_eq!(
        action(&menu, "Prune Worktrees"),
        &Action::Worktree(WorktreeAction::Prune)
    );
}

#[test]
fn a_locked_worktree_unlocks_and_removes_with_its_lock_known() {
    let locked = Worktree {
        locked: Some("usb".into()),
        ..linked_worktree()
    };
    let menu = worktree_menu(&locked, false);
    assert_eq!(
        action(&menu, "Unlock"),
        &Action::Worktree(WorktreeAction::Unlock {
            path: "/src/app-dev".into()
        })
    );
    assert!(matches!(
        action(&menu, "Remove Worktree..."),
        Action::Worktree(WorktreeAction::Remove { locked: true, .. })
    ));
}

#[test]
fn the_main_and_the_current_worktree_cannot_be_removed() {
    let main = Worktree {
        is_main: true,
        ..linked_worktree()
    };
    assert_eq!(
        labels(&worktree_menu(&main, true)),
        [
            "Copy Path to Clipboard",
            "---",
            "Remove Worktree... (disabled: The main worktree can't be removed.)",
            "---",
            "Add Worktree...",
            "Prune Worktrees",
        ]
    );
    assert!(labels(&worktree_menu(&linked_worktree(), true)).contains(
        &"Remove Worktree... (disabled: tui-git-graph is running in this worktree.)".to_owned()
    ));
}
