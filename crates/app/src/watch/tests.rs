use std::thread;

use ggt_testkit::FixtureRepo;
use pretty_assertions::assert_eq;

use super::*;
use crate::git::ProcessExecutor;

fn dirs() -> GitDirs {
    GitDirs {
        git_dir: PathBuf::from("/r/.git"),
        common_dir: PathBuf::from("/r/.git"),
    }
}

#[test]
fn refs_head_and_operations_reload_everything() {
    let dirs = dirs();
    for path in [
        "/r/.git/HEAD",
        "/r/.git/packed-refs",
        "/r/.git/MERGE_HEAD",
        "/r/.git/rebase-merge",
        "/r/.git/config",
        "/r/.git/refs/heads/main",
        "/r/.git/refs/heads/feature/login",
        "/r/.git/refs/stash",
    ] {
        assert_eq!(dirs.classify(Path::new(path)), Some(Change::Refs), "{path}");
    }
}

#[test]
fn the_index_reloads_only_the_status() {
    assert_eq!(
        dirs().classify(Path::new("/r/.git/index")),
        Some(Change::Index)
    );
}

#[test]
fn locks_logs_objects_and_other_files_are_ignored() {
    let dirs = dirs();
    for path in [
        "/r/.git/index.lock",
        "/r/.git/refs/heads/main.lock",
        "/r/.git/HEAD.lock",
        "/r/.git/COMMIT_EDITMSG",
        "/r/.git/FETCH_HEAD",
        "/r/.git/logs/HEAD",
        "/r/.git/objects/ab/cdef",
        "/r/.git/rebase-merge/done",
        "/r/src/HEAD",
    ] {
        assert_eq!(dirs.classify(Path::new(path)), None, "{path}");
    }
}

#[test]
fn a_linked_worktree_has_its_own_head_and_shared_refs() {
    let dirs = GitDirs {
        git_dir: PathBuf::from("/r/.git/worktrees/wt"),
        common_dir: PathBuf::from("/r/.git"),
    };
    assert_eq!(
        dirs.classify(Path::new("/r/.git/worktrees/wt/HEAD")),
        Some(Change::Refs)
    );
    assert_eq!(
        dirs.classify(Path::new("/r/.git/worktrees/wt/index")),
        Some(Change::Index)
    );
    assert_eq!(
        dirs.classify(Path::new("/r/.git/refs/heads/main")),
        Some(Change::Refs)
    );
}

#[test]
fn a_burst_is_reported_once_after_it_settles_with_its_strongest_change() {
    let start = Instant::now();
    let mut debounce = Debounce::default();
    debounce.record(Change::Index, start);
    debounce.record(Change::Refs, start + Duration::from_millis(100));
    debounce.record(Change::Index, start + Duration::from_millis(200));
    assert_eq!(debounce.take(start + Duration::from_millis(300)), None);
    assert_eq!(
        debounce.take(start + Duration::from_millis(200) + SETTLE),
        Some((Change::Refs, start + Duration::from_millis(200)))
    );
    assert_eq!(debounce.take(start + Duration::from_secs(10)), None);
}

#[test]
fn a_long_burst_is_still_reported_every_so_often() {
    let start = Instant::now();
    let mut debounce = Debounce::default();
    let mut at = start;
    while at < start + MAX_WAIT {
        debounce.record(Change::Refs, at);
        assert_eq!(debounce.take(at), None);
        at += Duration::from_millis(100);
    }
    debounce.record(Change::Refs, at);
    assert_eq!(debounce.take(at), Some((Change::Refs, at)));
}

fn watched(repo: &FixtureRepo) -> GitDirs {
    GitDirs::resolve(&ProcessExecutor::new(), repo.path()).expect("git dirs")
}

/// Polls `watch` until it reports a change, for up to five seconds.
fn wait_for_change(watch: &mut RepoWatch) -> Option<Change> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some((change, _)) = watch.poll(Instant::now()) {
            return Some(change);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn resolves_the_git_dirs_of_a_repository() {
    let repo = FixtureRepo::sample();
    let dirs = watched(&repo);
    let git = repo.path().join(".git").canonicalize().unwrap();
    assert_eq!(dirs.git_dir, git);
    assert_eq!(dirs.common_dir, git);
}

#[test]
fn polling_sees_a_new_branch_and_a_staged_file() {
    let repo = FixtureRepo::sample();
    let mut watch = RepoWatch::polling(watched(&repo));
    repo.git(&["branch", "polled"]);
    assert_eq!(wait_for_change(&mut watch), Some(Change::Refs));
    fs::write(repo.path().join("staged.txt"), "x\n").unwrap();
    repo.git(&["add", "staged.txt"]);
    assert_eq!(wait_for_change(&mut watch), Some(Change::Index));
}

#[test]
fn polling_ignores_working_tree_edits() {
    let repo = FixtureRepo::sample();
    let mut watch = RepoWatch::polling(watched(&repo));
    fs::write(repo.path().join("untracked.txt"), "x\n").unwrap();
    let deadline = Instant::now() + POLL_EVERY * 2 + SETTLE;
    while Instant::now() < deadline {
        assert_eq!(watch.poll(Instant::now()), None);
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn file_events_see_a_commit_made_outside_the_app() {
    let repo = FixtureRepo::sample();
    let mut watch = RepoWatch::start(watched(&repo));
    // FSEvents can report changes from just before the watch started:
    // let those settle and drop them.
    let quiet = Instant::now() + MAX_WAIT + SETTLE;
    while Instant::now() < quiet {
        watch.poll(Instant::now());
        thread::sleep(Duration::from_millis(20));
    }
    assert!(matches!(watch.backend, Backend::Events { .. }));
    repo.commit_file("outside.txt", "x\n", "Made outside");
    assert_eq!(wait_for_change(&mut watch), Some(Change::Refs));
}
