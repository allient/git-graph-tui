use std::path::Path;

use ggt_git::CommandKind;
use pretty_assertions::assert_eq;

use super::*;
use crate::{InputError, InputId, InputValue, Plan, UiEffect};

fn commands(action: &TagAction) -> Vec<(CommandKind, Vec<String>)> {
    let Plan::Git(commands) = action.plan(Path::new("/r")) else {
        panic!("expected git commands for {action:?}");
    };
    commands
        .iter()
        .map(|c| (c.kind(), c.argv()[6..].to_vec()))
        .collect()
}

fn delete(on_remote: bool) -> TagAction {
    TagAction::Delete {
        name: "v1.0".into(),
        remote: Some("origin".into()),
        on_remote,
    }
}

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn delete_is_local_unless_asked_to_delete_on_the_remote_too() {
    assert_eq!(
        commands(&delete(false)),
        [(CommandKind::Write, argv(&["tag", "--delete", "--", "v1.0"]))]
    );
    assert_eq!(
        commands(&delete(true)),
        [
            (CommandKind::Write, argv(&["tag", "--delete", "--", "v1.0"])),
            (
                CommandKind::Network,
                argv(&["push", "--delete", "--", "origin", "refs/tags/v1.0"])
            ),
        ]
    );
}

#[test]
fn push_sends_exactly_this_tag() {
    let push = TagAction::Push {
        name: "v1.0".into(),
        remote: "origin".into(),
    };
    assert_eq!(
        commands(&push),
        [(
            CommandKind::Network,
            argv(&["push", "--", "origin", "refs/tags/v1.0"])
        )]
    );
}

#[test]
fn details_and_copy_need_no_git() {
    let repo = Path::new("/r");
    assert_eq!(
        TagAction::ShowDetails {
            name: "v1.0".into()
        }
        .plan(repo),
        Plan::Ui(UiEffect::ShowTag {
            name: "v1.0".into()
        })
    );
    assert_eq!(
        TagAction::CopyName {
            name: "v1.0".into()
        }
        .plan(repo),
        Plan::Clipboard("v1.0".into())
    );
}

#[test]
fn deleting_is_destructive_and_says_where() {
    assert!(delete(false).is_destructive());
    assert_eq!(
        delete(false).confirmation().unwrap().message,
        "Delete the local tag 'v1.0'? Its commit is kept, and so is the tag on remotes."
    );
    assert_eq!(
        delete(true).confirmation().unwrap().message,
        "Delete the tag 'v1.0' here and on 'origin'? Everyone who fetches from 'origin' loses it (git push --delete origin refs/tags/v1.0)."
    );
    let push = TagAction::Push {
        name: "v1.0".into(),
        remote: "origin".into(),
    };
    assert!(!push.is_destructive());
    assert_eq!(push.confirmation(), None);
}

#[test]
fn inputs_and_validation() {
    let ids = |a: &TagAction| a.inputs().iter().map(|i| i.id).collect::<Vec<_>>();
    assert_eq!(ids(&delete(false)), [InputId::DeleteOnRemote]);
    let local_only = TagAction::Delete {
        name: "v1.0".into(),
        remote: None,
        on_remote: false,
    };
    assert!(ids(&local_only).is_empty(), "no remote, no option");

    let mut action = delete(false);
    action
        .set_input(InputId::DeleteOnRemote, InputValue::Bool(true))
        .unwrap();
    assert_eq!(action, delete(true));

    let mut push = TagAction::Push {
        name: "v1.0".into(),
        remote: "origin".into(),
    };
    assert_eq!(ids(&push), [InputId::Remote]);
    assert!(matches!(
        push.set_input(InputId::Remote, InputValue::Text("bad remote".into())),
        Err(InputError::InvalidName(_))
    ));
}

#[test]
fn delete_remote_tag_only_touches_the_remote_after_asking() {
    let action = TagAction::DeleteRemote {
        name: "v1.0".into(),
        remote: "origin".into(),
    };
    assert_eq!(
        commands(&action),
        [(
            CommandKind::Network,
            argv(&["push", "--delete", "--", "origin", "refs/tags/v1.0"])
        )]
    );
    assert!(action.is_destructive());
    let confirmation = action.confirmation().unwrap();
    assert!(
        confirmation.message.contains("'v1.0' on 'origin'"),
        "{confirmation:?}"
    );
}
