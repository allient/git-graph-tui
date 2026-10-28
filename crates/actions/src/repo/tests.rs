use std::path::Path;

use pretty_assertions::assert_eq;
use tgg_git::CommandKind;

use super::*;
use crate::{Action, InputError, InputId, InputValue, Plan};

fn commands(action: &RepoAction) -> Vec<(CommandKind, Vec<String>)> {
    let Plan::Git(commands) = action.plan(Path::new("/r")) else {
        panic!("expected git commands for {action:?}");
    };
    commands
        .iter()
        .map(|c| (c.kind(), c.argv()[6..].to_vec()))
        .collect()
}

fn network(argv: &[&str]) -> (CommandKind, Vec<String>) {
    (
        CommandKind::Network,
        argv.iter().map(|a| (*a).to_owned()).collect(),
    )
}

fn write(argv: &[&str]) -> (CommandKind, Vec<String>) {
    (
        CommandKind::Write,
        argv.iter().map(|a| (*a).to_owned()).collect(),
    )
}

fn pull(mode: PullMode) -> RepoAction {
    RepoAction::Pull {
        branch: "main".into(),
        mode,
    }
}

#[test]
fn pull_is_fast_forward_only_by_default() {
    assert_eq!(PullMode::default(), PullMode::FastForwardOnly);
    assert_eq!(
        commands(&pull(PullMode::FastForwardOnly)),
        [network(&["pull", "--ff-only"])]
    );
    assert_eq!(
        commands(&pull(PullMode::Rebase)),
        [network(&["pull", "--rebase"])]
    );
    assert_eq!(
        commands(&pull(PullMode::Merge)),
        [network(&["pull", "--no-rebase"])]
    );
}

#[test]
fn pull_mode_is_a_choice() {
    let mut action = pull(PullMode::FastForwardOnly);
    assert_eq!(action.inputs()[0].id, InputId::Mode);
    action
        .set_input(InputId::Mode, InputValue::Text("rebase".into()))
        .unwrap();
    assert_eq!(action, pull(PullMode::Rebase));
    assert_eq!(
        action.set_input(InputId::Mode, InputValue::Text("squash".into())),
        Err(InputError::NotAChoice("squash".into()))
    );
}

#[test]
fn fetch_and_prune_talk_to_every_remote() {
    assert_eq!(
        commands(&RepoAction::Fetch { prune: false }),
        [network(&["fetch", "--all"])]
    );
    assert_eq!(
        commands(&RepoAction::Fetch { prune: true }),
        [network(&["fetch", "--all", "--prune"])]
    );
    assert_eq!(
        commands(&RepoAction::PruneRemotes {
            remotes: vec!["origin".into(), "upstream".into()]
        }),
        [
            network(&["remote", "prune", "--", "origin"]),
            network(&["remote", "prune", "--", "upstream"]),
        ]
    );
}

#[test]
fn add_and_remove_remotes() {
    let mut add = RepoAction::AddRemote {
        name: String::new(),
        url: String::new(),
    };
    assert_eq!(
        add.inputs().iter().map(|i| i.id).collect::<Vec<_>>(),
        [InputId::Name, InputId::Url]
    );
    assert!(matches!(
        add.set_input(InputId::Name, InputValue::Text("bad name".into())),
        Err(InputError::InvalidName(_))
    ));
    assert_eq!(
        add.set_input(InputId::Url, InputValue::Text(" ".into())),
        Err(InputError::Required(InputId::Url))
    );
    add.set_input(InputId::Name, InputValue::Text("upstream".into()))
        .unwrap();
    add.set_input(
        InputId::Url,
        InputValue::Text("git@example.com:team/app.git".into()),
    )
    .unwrap();
    assert_eq!(
        commands(&add),
        [write(&[
            "remote",
            "add",
            "--",
            "upstream",
            "git@example.com:team/app.git"
        ])]
    );

    let remove = RepoAction::RemoveRemote {
        name: "upstream".into(),
    };
    assert_eq!(
        commands(&remove),
        [write(&["remote", "remove", "--", "upstream"])]
    );
    assert!(remove.is_destructive());
    assert_eq!(
        remove.confirmation().unwrap().message,
        "Remove the remote 'upstream'? Its remote-tracking branches (upstream/*) are deleted here; the remote repository is not touched."
    );
}

#[test]
fn tags_and_latest_stash() {
    assert_eq!(
        commands(&RepoAction::PushTags {
            remote: "origin".into()
        }),
        [network(&["push", "--tags", "--", "origin"])]
    );
    assert_eq!(
        commands(&RepoAction::ApplyLatestStash),
        [write(&["stash", "apply"])]
    );
    assert_eq!(
        commands(&RepoAction::PopLatestStash),
        [write(&["stash", "pop"])]
    );
}

#[test]
fn only_remove_remote_needs_confirmation() {
    for action in [
        pull(PullMode::Rebase),
        RepoAction::Fetch { prune: true },
        RepoAction::PushTags {
            remote: "origin".into(),
        },
        RepoAction::PopLatestStash,
    ] {
        assert!(!action.is_destructive(), "{action:?}");
        assert_eq!(action.confirmation(), None, "{action:?}");
    }
}

#[test]
fn picked_names_become_actions() {
    assert_eq!(
        PickPurpose::DeleteBranch.action_for("topic"),
        Some(Action::DeleteBranch {
            name: "topic".into(),
            force: false
        })
    );
    assert_eq!(
        PickPurpose::RemoveRemote.action_for("upstream"),
        Some(Action::Repo(RepoAction::RemoveRemote {
            name: "upstream".into()
        }))
    );
    assert_eq!(
        PickPurpose::Checkout.action_for("main"),
        None,
        "checkout depends on the ref kind, which the app knows"
    );
}
