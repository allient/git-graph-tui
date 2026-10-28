use pretty_assertions::assert_eq;
use tgg_testkit::FixtureRepo;

#[test]
fn new_repo_starts_on_main_with_no_commits() {
    let repo = FixtureRepo::new();
    assert_eq!(repo.git(&["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(repo.git(&["rev-list", "--all", "--count"]), "0");
}

#[test]
fn commit_file_creates_a_commit_and_returns_its_hash() {
    let repo = FixtureRepo::new();
    let hash = repo.commit_file("a.txt", "hello\n", "Add a");
    assert_eq!(hash.len(), 40);
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), hash);
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "Add a");
}

#[test]
fn same_steps_produce_the_same_hashes() {
    let build = || {
        let repo = FixtureRepo::new();
        repo.commit_file("a.txt", "1\n", "First");
        repo.commit_file("a.txt", "2\n", "Second")
    };
    assert_eq!(build(), build());
}

#[test]
fn commits_have_increasing_timestamps() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "1\n", "First");
    repo.commit_file("a.txt", "2\n", "Second");
    let times = repo.git(&["log", "--format=%at"]);
    let times: Vec<i64> = times.lines().map(|t| t.parse().unwrap()).collect();
    assert!(times[0] > times[1], "newest first: {times:?}");
}

#[test]
fn branches_merges_and_tags() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "1\n", "Base");
    repo.branch("develop");
    repo.commit_file("b.txt", "1\n", "Work on develop");
    repo.checkout("main");
    repo.merge_no_ff("develop", "Merge branch 'develop'");
    repo.tag("v0.1.0");

    let parents = repo.git(&["log", "-1", "--format=%P"]);
    assert_eq!(
        parents.split(' ').count(),
        2,
        "merge commit has two parents"
    );
    assert_eq!(repo.git(&["tag", "--list"]), "v0.1.0");
}

#[test]
fn uncommitted_changes_and_stash() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "1\n", "Base");
    repo.write_file("a.txt", "dirty\n");
    repo.stash("WIP");
    assert_eq!(repo.git(&["stash", "list", "--format=%s"]), "On main: WIP");
    repo.write_file("new.txt", "x\n");
    assert_eq!(repo.git(&["status", "--porcelain"]), "?? new.txt");
}

#[test]
fn origin_is_a_local_bare_repo_with_pushed_branches() {
    let repo = FixtureRepo::new();
    repo.commit_file("a.txt", "1\n", "Base");
    repo.add_origin();
    repo.push("main");
    assert_eq!(
        repo.git(&["rev-parse", "origin/main"]),
        repo.git(&["rev-parse", "main"])
    );
}

#[test]
fn global_git_config_is_ignored() {
    let repo = FixtureRepo::new();
    assert_eq!(repo.git(&["config", "user.name"]), "Fixture Author");
    assert_eq!(repo.git(&["config", "--get", "commit.gpgsign"]), "false");
}

#[test]
fn sample_repo_has_the_shapes_the_graph_needs() {
    let repo = FixtureRepo::sample();
    let refs = repo.git(&["for-each-ref", "--format=%(refname)"]);
    for expected in [
        "refs/heads/main",
        "refs/heads/develop",
        "refs/heads/feature/login",
        "refs/remotes/origin/main",
        "refs/tags/v0.1.0",
        "refs/stash",
    ] {
        assert!(
            refs.lines().any(|r| r == expected),
            "missing {expected} in:\n{refs}"
        );
    }
    assert!(
        !repo.git(&["status", "--porcelain"]).is_empty(),
        "has uncommitted changes"
    );
}
