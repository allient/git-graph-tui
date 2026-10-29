use std::path::Path;

use pretty_assertions::assert_eq;
use tgg_git::CommandKind;

use super::*;
use crate::{Action, InputError, InputId, InputValue, Plan};

/// The commands' kind and arguments after `-C <repo>` and the config overrides.
fn commands(action: &WorktreeAction) -> Vec<(CommandKind, Vec<String>)> {
    let Plan::Git(commands) = action.plan(Path::new("/src/app")) else {
        panic!("expected git commands for {action:?}");
    };
    commands
        .iter()
        .map(|c| (c.kind(), c.argv()[6..].to_vec()))
        .collect()
}

fn write(argv: &[&str]) -> (CommandKind, Vec<String>) {
    (
        CommandKind::Write,
        argv.iter().map(|a| (*a).to_owned()).collect(),
    )
}

fn add(path: &str, branch: &str, new_branch: bool) -> WorktreeAction {
    WorktreeAction::Add {
        path: path.into(),
        branch: branch.into(),
        new_branch,
    }
}

fn remove(force: bool, locked: bool) -> WorktreeAction {
    WorktreeAction::Remove {
        path: "/src/app-dev".into(),
        force,
        locked,
    }
}

#[test]
fn add_creates_a_new_branch_or_checks_out_an_existing_one() {
    assert_eq!(
        commands(&add("../wt", "topic", true)),
        [write(&["worktree", "add", "-b", "topic", "--", "../wt"])]
    );
    assert_eq!(
        commands(&add("../wt", "develop", false)),
        [write(&["worktree", "add", "--", "../wt", "develop"])]
    );
}

#[test]
fn an_empty_folder_defaults_next_to_the_repository() {
    assert_eq!(
        commands(&add("", "feature/login", false)),
        [write(&[
            "worktree",
            "add",
            "--",
            "../app-feature-login",
            "feature/login"
        ])]
    );
    assert_eq!(
        commands(&add("  ", "topic", true)),
        [write(&[
            "worktree",
            "add",
            "-b",
            "topic",
            "--",
            "../app-topic"
        ])]
    );
}

#[test]
fn add_asks_for_branch_new_branch_and_folder() {
    let mut action = WorktreeAction::add();
    let ids: Vec<InputId> = action.inputs().iter().map(|s| s.id).collect();
    assert_eq!(ids, [InputId::Name, InputId::NewBranch, InputId::Path]);
    action
        .set_input(InputId::Name, InputValue::Text("dev".into()))
        .unwrap();
    action
        .set_input(InputId::NewBranch, InputValue::Bool(false))
        .unwrap();
    action
        .set_input(InputId::Path, InputValue::Text("/tmp/dev".into()))
        .unwrap();
    assert_eq!(action, add("/tmp/dev", "dev", false));
    assert!(matches!(
        action.set_input(InputId::Name, InputValue::Text("-x".into())),
        Err(InputError::InvalidName(_))
    ));
    assert!(
        Action::Worktree(WorktreeAction::add()).validate().is_err(),
        "the branch is required"
    );
}

#[test]
fn remove_forces_once_or_twice_for_a_locked_worktree() {
    assert_eq!(
        commands(&remove(false, true)),
        [write(&["worktree", "remove", "--", "/src/app-dev"])]
    );
    assert_eq!(
        commands(&remove(true, false)),
        [write(&[
            "worktree",
            "remove",
            "--force",
            "--",
            "/src/app-dev"
        ])]
    );
    assert_eq!(
        commands(&remove(true, true)),
        [write(&[
            "worktree",
            "remove",
            "--force",
            "--force",
            "--",
            "/src/app-dev"
        ])]
    );
}

#[test]
fn remove_is_destructive_and_confirmed() {
    let action = Action::Worktree(remove(false, false));
    assert!(action.is_destructive());
    let confirmation = action.confirmation().unwrap();
    assert_eq!(confirmation.title, "Remove Worktree");
    assert!(
        confirmation.message.contains("'/src/app-dev'")
            && confirmation.message.contains("folder is deleted"),
        "{confirmation:?}"
    );
    let forced = Action::Worktree(remove(true, true)).confirmation().unwrap();
    assert_eq!(forced.confirm_label, "Force Remove");
    assert!(forced.message.contains("locked"), "{forced:?}");

    let mut action = remove(false, false);
    action
        .set_input(InputId::Force, InputValue::Bool(true))
        .unwrap();
    assert_eq!(action, remove(true, false));
}

#[test]
fn prune_lock_unlock_and_copy() {
    assert_eq!(
        commands(&WorktreeAction::Prune),
        [write(&["worktree", "prune"])]
    );
    let path = || "/src/app-dev".to_owned();
    assert_eq!(
        commands(&WorktreeAction::Lock { path: path() }),
        [write(&["worktree", "lock", "--", "/src/app-dev"])]
    );
    assert_eq!(
        commands(&WorktreeAction::Unlock { path: path() }),
        [write(&["worktree", "unlock", "--", "/src/app-dev"])]
    );
    assert_eq!(
        WorktreeAction::CopyPath { path: path() }.plan(Path::new("/r")),
        Plan::Clipboard(path())
    );
    for action in [
        WorktreeAction::Prune,
        WorktreeAction::Lock { path: path() },
        WorktreeAction::CopyPath { path: path() },
    ] {
        let action = Action::Worktree(action);
        assert!(!action.is_destructive() && action.confirmation().is_none());
        assert!(action.inputs().is_empty());
    }
}
