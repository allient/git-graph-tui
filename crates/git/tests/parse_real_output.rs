//! Parsers run against output from a real git, on a deterministic fixture repo.

#![expect(clippy::unwrap_used, reason = "test helpers")]

use pretty_assertions::assert_eq;
use tgg_git::{log, refs, stash, status};
use tgg_model::{HeadState, RefKind};
use tgg_testkit::FixtureRepo;

fn run(repo: &FixtureRepo, args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    repo.git_raw(&args)
}

#[test]
fn log_of_the_sample_repo() {
    let repo = FixtureRepo::sample();
    let commits = log::parse(&run(
        &repo,
        &log::args(&log::LogQuery {
            max_count: 100,
            skip: 0,
            revisions: log::Revisions::default(),
        }),
    ))
    .unwrap();
    let subjects: Vec<&str> = commits.iter().map(|c| c.subject.as_str()).collect();
    insta::assert_snapshot!(subjects.join("\n"));
    assert_eq!(commits.iter().filter(|c| c.is_merge()).count(), 1);
    assert!(commits.last().unwrap().is_root());
}

#[test]
fn log_pagination_skips_and_limits() {
    let repo = FixtureRepo::sample();
    let all = log::parse(&run(
        &repo,
        &log::args(&log::LogQuery {
            max_count: 100,
            skip: 0,
            revisions: log::Revisions::default(),
        }),
    ))
    .unwrap();
    let page = log::parse(&run(
        &repo,
        &log::args(&log::LogQuery {
            max_count: 2,
            skip: 1,
            revisions: log::Revisions::default(),
        }),
    ))
    .unwrap();
    assert_eq!(page, all[1..3]);
}

#[test]
fn refs_of_the_sample_repo() {
    let repo = FixtureRepo::sample();
    let refs = refs::parse(&run(&repo, &refs::args())).unwrap();
    let lines: Vec<String> = refs
        .iter()
        .map(|r| {
            format!(
                "{:?} {} head={} upstream={:?}",
                r.kind,
                r.short_name(),
                r.is_head,
                r.upstream
            )
        })
        .collect();
    insta::assert_snapshot!(lines.join("\n"));
    let main = refs.iter().find(|r| r.short_name() == "main").unwrap();
    let origin_main = refs
        .iter()
        .find(|r| r.short_name() == "origin/main")
        .unwrap();
    assert_eq!(main.target, origin_main.target);
    assert_eq!(origin_main.kind, RefKind::RemoteBranch);
}

#[test]
fn annotated_tag_resolves_to_its_commit() {
    let repo = FixtureRepo::new();
    let head = repo.commit_file("a.txt", "1\n", "Base");
    repo.annotated_tag("v2.0", "Release 2.0");
    let refs = refs::parse(&run(&repo, &refs::args())).unwrap();
    let tag = refs.iter().find(|r| r.kind == RefKind::Tag).unwrap();
    assert_eq!(tag.target.as_str(), head);
}

#[test]
fn status_of_the_sample_repo() {
    let repo = FixtureRepo::sample();
    let status = status::parse(&run(&repo, &status::args())).unwrap();
    assert_eq!(status.head, HeadState::Branch("main".into()));
    assert_eq!(status.files, 1);
    assert_eq!(status.untracked, 1);
}

#[test]
fn status_with_staged_rename_and_unicode_path() {
    let repo = FixtureRepo::new();
    repo.commit_file("old name.txt", "1\n", "Base");
    repo.git(&["mv", "old name.txt", "nuevo año.txt"]);
    repo.write_file("dirty.txt", "x\n");
    let status = status::parse(&run(&repo, &status::args())).unwrap();
    assert_eq!(status.staged, 1);
    assert_eq!(status.untracked, 1);
    assert_eq!(status.files, 2);
}

#[test]
fn stash_list_of_the_sample_repo() {
    let repo = FixtureRepo::sample();
    let stashes = stash::parse(&run(&repo, &stash::args())).unwrap();
    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].subject, "On main: WIP on docs");
}

fn subjects(repo: &FixtureRepo, revisions: log::Revisions) -> Vec<String> {
    let commits = log::parse(&run(
        repo,
        &log::args(&log::LogQuery {
            max_count: 100,
            skip: 0,
            revisions,
        }),
    ))
    .unwrap();
    commits.into_iter().map(|c| c.subject).collect()
}

#[test]
fn log_of_only_some_refs_follows_just_their_history() {
    let repo = FixtureRepo::sample();
    let only = subjects(
        &repo,
        log::Revisions::Only(vec!["refs/heads/main".to_owned()]),
    );
    assert_eq!(only[0], "Merge branch 'develop'");
    assert!(
        !only
            .iter()
            .any(|s| s == "Add login" || s == "Return a value")
    );
    assert!(only.iter().any(|s| s == "Initial commit"));
}

#[test]
fn log_without_remotes_hides_commits_only_a_remote_has() {
    let repo = FixtureRepo::sample();
    repo.checkout("feature/login");
    repo.push("feature/login");
    repo.checkout("main");
    repo.git(&["branch", "--delete", "--force", "feature/login"]);

    let with = subjects(&repo, log::Revisions::All { remotes: true });
    let without = subjects(&repo, log::Revisions::All { remotes: false });
    assert!(with.iter().any(|s| s == "Add login"));
    assert!(!without.iter().any(|s| s == "Add login"));
}
