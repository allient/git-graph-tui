//! `reflog` parser run against output from a real git, on fixture repos.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use pretty_assertions::assert_eq;
use tgg_git::GitCommand;
use tgg_git::reflog::{self, ReflogEntry};
use tgg_testkit::FixtureRepo;

fn entries(repo: &FixtureRepo, max_count: usize) -> Vec<ReflogEntry> {
    let full = GitCommand::read(repo.path(), reflog::args(max_count)).argv();
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    reflog::parse(&repo.git_raw(&full)).unwrap()
}

#[test]
fn a_reset_commit_stays_in_the_reflog() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "a\n", "First");
    let lost = repo.commit_file("b.txt", "b\n", "Lost: \"quoted\" ünïcode {x}");
    repo.git(&["reset", "--hard", "HEAD~1"]);
    let first = repo.git(&["rev-parse", "HEAD"]);

    let entries = entries(&repo, 500);
    let summary: Vec<(usize, &str, &str)> = entries
        .iter()
        .map(|e| (e.index, e.id.as_str(), e.subject.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            (0, first.as_str(), "reset: moving to HEAD~1"),
            (1, lost.as_str(), "commit: Lost: \"quoted\" ünïcode {x}"),
            (2, first.as_str(), "commit (initial): First"),
        ]
    );
    assert_eq!(entries[0].selector(), "HEAD@{0}");
    assert_eq!(entries[1].commit_subject, "Lost: \"quoted\" ünïcode {x}");
    assert!(entries[1].time > 0, "{:?}", entries[1]);
}

#[test]
fn max_count_limits_the_entries() {
    let repo = FixtureRepo::sample();
    assert_eq!(entries(&repo, 2).len(), 2);
}
