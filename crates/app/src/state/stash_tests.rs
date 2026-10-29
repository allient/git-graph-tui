//! Stashes in the graph (VS Code Git Graph shows each one as a row above the
//! commit it was made on), against real fixture repositories.

use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use pretty_assertions::assert_eq;
use tgg_actions::MenuItem;
use tgg_testkit::FixtureRepo;

use super::*;
use crate::testing::Harness;

const W: u16 = 120;
const H: u16 = 30;

fn open(repo: &FixtureRepo) -> Harness {
    let mut h = Harness::open(repo);
    h.draw(W, H);
    h
}

fn right_click(h: &mut Harness, x: u16, y: u16) {
    let commands = h.state.update(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    h.settle(commands);
    h.draw(W, H);
}

fn menu_labels(h: &Harness) -> Vec<String> {
    h.state
        .menu
        .as_ref()
        .map(|m| {
            m.items()
                .iter()
                .filter_map(|item| match item {
                    MenuItem::Entry(e) => Some(e.label.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The sample repository (which has one stash) with two more made on HEAD.
fn stashed() -> FixtureRepo {
    let repo = FixtureRepo::sample();
    repo.write_file("a.txt", "first stash\n");
    repo.git(&["stash", "push", "--include-untracked", "--message", "first"]);
    repo.write_file("b.txt", "second stash\n");
    repo.git(&[
        "stash",
        "push",
        "--include-untracked",
        "--message",
        "second",
    ]);
    repo
}

#[test]
fn each_stash_is_a_row_above_the_commit_it_was_made_on() {
    let repo = stashed();
    let h = open(&repo);
    let head = h.state.head.clone().unwrap();
    let subjects: Vec<&str> = h.state.commits.iter().map(|c| c.subject.as_str()).collect();
    let at = |s: &str| subjects.iter().position(|x| *x == s).unwrap();
    let base = h.state.commits.iter().position(|c| c.id == head).unwrap();
    assert_eq!(
        at("On main: second") + 1,
        at("On main: first"),
        "{subjects:?}"
    );
    // The fixture's own stash (made on the same commit) is the oldest.
    assert_eq!(
        at("On main: first") + 1,
        at("On main: WIP on docs"),
        "{subjects:?}"
    );
    assert_eq!(at("On main: WIP on docs") + 1, base, "{subjects:?}");
    // Their badges name the stash.
    assert!(h.find_on_screen("stash@{0}").is_some());
    assert!(h.find_on_screen("stash@{1}").is_some());
    // Git's own index and untracked commits stay hidden.
    assert!(
        !subjects.iter().any(|s| s.starts_with("index on")),
        "{subjects:?}"
    );
    assert!(
        !subjects.iter().any(|s| s.starts_with("untracked files on")),
        "{subjects:?}"
    );
}

#[test]
fn right_clicking_a_stash_row_or_badge_opens_the_stash_menu() {
    let repo = stashed();
    let mut h = open(&repo);
    let (x, y) = h.find_on_screen("On main: first").unwrap();
    right_click(&mut h, x + 2, y);
    assert_eq!(
        menu_labels(&h)[0],
        "Apply Stash...",
        "{:?}",
        menu_labels(&h)
    );
    h.key(KeyCode::Esc);
    h.draw(W, H);

    let (x, y) = h.find_on_screen("stash@{0}").unwrap();
    right_click(&mut h, x + 2, y);
    assert!(menu_labels(&h).contains(&"Create Branch from Stash...".to_owned()));
}

#[test]
fn history_pages_still_load_after_stash_rows() {
    let repo = stashed();
    let mut h = Harness::with_page_size(&repo, 3);
    h.draw(W, H);
    // Keep paging until history ends: every commit once, plus the stashes.
    for _ in 0..10 {
        h.key(KeyCode::End);
        h.draw(W, H);
    }
    let real = repo.git(&[
        "rev-list",
        "--count",
        "--exclude=refs/stash",
        "--all",
        "HEAD",
    ]);
    let ids: std::collections::HashSet<_> = h.state.commits.iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids.len(), h.state.commits.len(), "no row twice");
    assert!(h.state.commits.len() >= real.parse::<usize>().unwrap() + 2);
}
