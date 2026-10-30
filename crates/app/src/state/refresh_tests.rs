//! Picking up changes made outside the app: the watcher's `RepoChanged`
//! and the periodic `StatusTick`.

use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

use super::{Command, Event, Request};
use crate::testing::Harness;
use crate::watch::Change;

fn changed(h: &mut Harness, change: Change, since: Instant) -> Vec<Command> {
    h.state.update(Event::RepoChanged { change, since })
}

#[test]
fn a_commit_made_outside_the_app_appears_without_a_reload() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let since = Instant::now();
    repo.commit_file("outside.txt", "x\n", "Made outside");
    let commands = changed(&mut h, Change::Refs, since);
    assert!(
        commands.contains(&Command::Git(Request::Refs)),
        "{commands:?}"
    );
    h.settle(commands);
    assert_eq!(
        h.state.commits.first().map(|c| c.subject.as_str()),
        Some("Made outside")
    );
}

#[test]
fn a_change_the_last_refresh_already_read_is_ignored() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let before = Instant::now();
    let commands = h.state.update(Event::Refresh);
    h.settle(commands);
    assert_eq!(changed(&mut h, Change::Refs, before), Vec::new());
    assert_eq!(changed(&mut h, Change::Index, before), Vec::new());
}

#[test]
fn a_staged_file_reloads_only_the_status() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let staged = h.state.status.staged;
    let since = Instant::now();
    std::fs::write(repo.path().join("new.txt"), "x\n").unwrap();
    repo.git(&["add", "new.txt"]);
    let commands = changed(&mut h, Change::Index, since);
    assert_eq!(commands, vec![Command::Git(Request::Status)]);
    h.settle(commands);
    assert_eq!(h.state.status.staged, staged + 1);
}

#[test]
fn status_ticks_never_stack_up_reads() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let first = h.state.update(Event::StatusTick);
    assert_eq!(first, vec![Command::Git(Request::Status)]);
    let job = h.runner.submit(Request::Status.command(&h.state.repo));
    h.state.track(&job, Request::Status);
    assert_eq!(h.state.update(Event::StatusTick), Vec::new());
    h.settle(Vec::new());
    assert_eq!(
        h.state.update(Event::StatusTick),
        vec![Command::Git(Request::Status)]
    );
}

#[test]
fn slow_status_reads_space_out_the_polling() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let base = Duration::from_secs(3);
    h.state.status_poll.cost = Duration::from_millis(100);
    assert_eq!(h.state.status_poll_interval(base), base);
    h.state.status_poll.cost = Duration::from_secs(1);
    assert_eq!(h.state.status_poll_interval(base), Duration::from_secs(10));
}

#[test]
fn a_refresh_keeps_the_history_already_loaded() {
    let repo = FixtureRepo::new();
    for i in 1..=12 {
        repo.commit_file("log.txt", &format!("{i}\n"), &format!("Commit {i}"));
    }
    let mut h = Harness::with_page_size(&repo, 3);
    for _ in 0..6 {
        h.key(KeyCode::Char('G'));
    }
    assert_eq!(h.state.commits.len(), 12);
    let since = Instant::now();
    repo.commit_file("log.txt", "13\n", "Commit 13");
    let commands = changed(&mut h, Change::Refs, since);
    h.settle(commands);
    assert_eq!(h.state.commits.len(), 13);
    assert_eq!(h.selected_subject(), Some("Commit 1"));
}

#[test]
fn changes_during_an_action_are_left_to_its_own_refresh() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let since = Instant::now();
    let commands = h.state.start_action(
        ggt_actions::Action::CreateBranch {
            at: h.state.commits[1].id.clone(),
            name: "made-here".into(),
            checkout: false,
        },
        "Create Branch".into(),
    );
    // A dialog asks for the name first; submit it as it is.
    let commands = if commands.is_empty() {
        h.state
            .update(Event::Key(crossterm::event::KeyEvent::from(KeyCode::Enter)))
    } else {
        commands
    };
    assert!(
        commands.iter().any(|c| matches!(c, Command::Run(_))),
        "{commands:?}"
    );
    for command in &commands {
        if let Command::Run(git) = command {
            let job = h.runner.submit(git.clone());
            h.state.track_action(&job);
        }
    }
    assert_eq!(changed(&mut h, Change::Refs, since), Vec::new());
    assert_eq!(h.state.update(Event::StatusTick), Vec::new());
    h.settle(Vec::new());
    assert!(h.state.refs.iter().any(|r| r.short_name() == "made-here"));
}

#[test]
fn repeated_reloads_never_grow_the_history() {
    let repo = FixtureRepo::new();
    for i in 1..=30 {
        repo.commit_file("log.txt", &format!("{i}\n"), &format!("Commit {i}"));
    }
    let mut h = Harness::with_page_size(&repo, 3);
    let mut loaded = h.state.commits.len();
    for i in 31..=35 {
        let since = Instant::now();
        repo.commit_file("log.txt", &format!("{i}\n"), &format!("Commit {i}"));
        let commands = changed(&mut h, Change::Refs, since);
        h.settle(commands);
        let now = h.state.commits.len();
        assert!(now <= loaded + 1, "{loaded} commits became {now}");
        loaded = now;
    }
    let commands = h.state.update(Event::Refresh);
    h.settle(commands);
    assert_eq!(h.state.commits.len(), loaded, "a refresh with nothing new");
}

#[test]
fn an_index_change_during_a_status_read_reads_again() {
    let repo = FixtureRepo::sample();
    let mut h = Harness::open(&repo);
    let staged = h.state.status.staged;
    assert_eq!(
        h.state.update(Event::StatusTick),
        vec![Command::Git(Request::Status)]
    );
    let job = h.runner.submit(Request::Status.command(&h.state.repo));
    h.state.track(&job, Request::Status);
    let result = h
        .results
        .recv_timeout(Duration::from_secs(10))
        .expect("status result");
    // Staged after that read finished, seen before its result is applied.
    std::fs::write(repo.path().join("late.txt"), "x\n").unwrap();
    repo.git(&["add", "late.txt"]);
    assert_eq!(changed(&mut h, Change::Index, Instant::now()), Vec::new());
    let commands = h.state.update(Event::Git(result));
    assert_eq!(commands, vec![Command::Git(Request::Status)]);
    h.settle(commands);
    assert_eq!(h.state.status.staged, staged + 1);
}
