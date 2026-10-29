use std::path::Path;

use pretty_assertions::assert_eq;
use tgg_git::CommandKind;

use super::*;
use crate::Action;

const ALL: [Operation; 4] = [
    Operation::Merge,
    Operation::Rebase,
    Operation::CherryPick,
    Operation::Revert,
];

fn single(action: OperationAction) -> GitCommand {
    let Plan::Git(commands) = action.plan(Path::new("/repo")) else {
        panic!("expected git commands for {action:?}");
    };
    assert_eq!(commands.len(), 1, "{action:?}");
    commands[0].clone()
}

#[test]
fn continue_runs_the_operations_own_continue_without_an_editor() {
    for operation in ALL {
        let command = single(OperationAction::Continue(operation));
        assert_eq!(command.kind(), CommandKind::Write);
        assert_eq!(
            command.argv()[6..],
            [operation.subcommand(), "--continue"],
            "{operation:?}"
        );
        assert!(
            command.env().contains(&("GIT_EDITOR", "true")),
            "{operation:?}"
        );
    }
}

#[test]
fn abort_runs_the_operations_own_abort() {
    for operation in ALL {
        let command = single(OperationAction::Abort(operation));
        assert_eq!(command.argv()[6..], [operation.subcommand(), "--abort"]);
    }
}

#[test]
fn continue_is_safe_and_runs_without_asking() {
    for operation in ALL {
        let action = Action::Operation(OperationAction::Continue(operation));
        assert!(!action.is_destructive());
        assert_eq!(action.confirmation(), None);
    }
}

#[test]
fn abort_is_destructive_and_says_what_is_lost() {
    for operation in ALL {
        let action = Action::Operation(OperationAction::Abort(operation));
        assert!(action.is_destructive(), "{operation:?}");
        let confirmation = action.confirmation().unwrap();
        assert_eq!(confirmation.title, format!("Abort {}", operation.label()));
        assert_eq!(confirmation.confirm_label, "Abort");
        assert!(
            confirmation.message.contains("are lost"),
            "{}",
            confirmation.message
        );
    }
    let merge = Action::Operation(OperationAction::Abort(Operation::Merge));
    assert_eq!(
        merge.confirmation().unwrap().message,
        "Abort the merge (git merge --abort)? The working tree and index go back to how they were before it started: your conflict resolutions are lost."
    );
}

#[test]
fn continuing_or_aborting_never_conflicts_with_the_operation() {
    for operation in ALL {
        for action in [
            OperationAction::Continue(operation),
            OperationAction::Abort(operation),
        ] {
            assert!(!Action::Operation(action).conflicts_with_operation());
        }
    }
}
