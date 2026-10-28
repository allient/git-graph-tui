use pretty_assertions::assert_eq;
use tgg_model::{HeadState, Oid, Stash};

use super::*;
use crate::{Action, CommitTarget, ResetMode, Rev};

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
        })
        .collect()
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

#[test]
fn targets_outside_s8_and_s9_have_no_menu_yet() {
    let stash = Target::Stash(Stash {
        id: oid('d'),
        index: 0,
        subject: "WIP".into(),
    });
    let tag = Target::Tag {
        name: "v1".into(),
        target: oid('a'),
    };
    for target in [stash, tag, Target::Uncommitted] {
        assert_eq!(menu_for(&target, &ctx()), Vec::new(), "{target:?}");
    }
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
