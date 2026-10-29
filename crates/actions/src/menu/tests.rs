use pretty_assertions::assert_eq;
use tgg_model::{HeadState, Oid, Stash};

use super::*;
use crate::{
    Action, CommitTarget, PickPurpose, PullMode, RepoAction, ResetMode, Rev, StashAction,
    TagAction, UiEffect, WorkingTreeAction,
};

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
            "Merge into current branch...",
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
            "Reset current branch to this Commit...",
            "---",
            "Copy Commit Hash to Clipboard",
            "Copy Commit Subject to Clipboard",
        ]
    );
}

#[test]
fn commit_on_the_current_branch_can_be_dropped_but_not_merged_or_picked() {
    assert_eq!(
        labels(&menu_for(&commit('c', 1, true), &ctx())),
        [
            "Add Tag...",
            "Create Branch...",
            "---",
            "Checkout...",
            "Revert...",
            "Drop...",
            "---",
            "Rebase current branch on this Commit...",
            "Reset current branch to this Commit...",
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
            "Revert...",
            "Drop...",
            "---",
            "Copy Commit Hash to Clipboard",
            "Copy Commit Subject to Clipboard",
        ]
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
fn detached_head_at_the_commit_hides_checkout_and_reset() {
    let ctx = MenuContext {
        head: HeadState::Detached,
        ..ctx()
    };
    let menu = labels(&menu_for(&commit('a', 1, true), &ctx));
    assert!(!menu.contains(&"Checkout...".to_owned()), "{menu:?}");
    let other = labels(&menu_for(&commit('c', 1, false), &ctx));
    assert!(other.contains(&"Checkout...".to_owned()), "{other:?}");
    assert!(
        !other.contains(&"Reset current branch to this Commit...".to_owned()),
        "{other:?}"
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
            "Pull...",
            "Push...",
            "Checkout to...",
            "Fetch",
            "---",
            "Branch ▸",
            "Remote ▸",
            "Stash ▸",
            "Tags ▸",
            "---",
            "Show Git Output",
        ]
    );
    assert_eq!(
        labels(submenu(&menu, "Branch")),
        [
            "Create Branch...",
            "Rename Current Branch...",
            "Delete Branch..."
        ]
    );
    assert_eq!(
        labels(submenu(&menu, "Remote")),
        ["Add Remote...", "Remove Remote...", "Fetch", "Prune"]
    );
    assert_eq!(
        labels(submenu(&menu, "Stash")),
        ["Stash Changes...", "Apply Latest Stash", "Pop Latest Stash"]
    );
    assert_eq!(
        labels(submenu(&menu, "Tags")),
        ["Create Tag...", "Push All Tags"]
    );
}

#[test]
fn repository_actions_are_prefilled() {
    let menu = menu_for(&Target::Repository, &ctx());
    assert_eq!(
        action(&menu, "Pull..."),
        &Action::Repo(RepoAction::Pull {
            branch: "main".into(),
            mode: PullMode::FastForwardOnly
        })
    );
    assert_eq!(
        action(&menu, "Push..."),
        &Action::PushBranch {
            branch: "main".into(),
            remote: "origin".into(),
            set_upstream: true,
            force_with_lease: false
        }
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
            checkout: false
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
            "Checkout to...",
            "---",
            "Branch ▸",
            "Remote ▸",
            "Stash ▸",
            "Tags ▸",
            "---",
            "Show Git Output",
        ]
    );
    assert_eq!(
        labels(submenu(&menu, "Branch")),
        ["Create Branch...", "Delete Branch..."]
    );
    assert_eq!(labels(submenu(&menu, "Remote")), ["Add Remote..."]);
    assert_eq!(labels(submenu(&menu, "Tags")), ["Create Tag..."]);
}

#[test]
fn empty_submenus_are_dropped() {
    let ctx = MenuContext {
        head: HeadState::Unborn,
        head_commit: None,
        remotes: Vec::new(),
        branch_filter: Vec::new(),
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
