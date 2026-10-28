//! Test fixtures for tui-git-graph.
//!
//! Builds throwaway git repositories in temporary directories (branches,
//! merges, tags, stashes, a local bare `origin`) so tests never touch a real
//! repository. Used only as a dev-dependency.
//!
//! Repositories are deterministic: a fixed identity, a fixed clock that
//! advances one minute per commit, and no global or system git config. The
//! same steps always produce the same commit hashes, which keeps snapshot
//! tests stable across machines.

// Fixture helpers panic on failure: a broken fixture must fail the test loudly.
#![expect(clippy::expect_used, reason = "test fixtures panic on setup failure")]

use std::cell::{Cell, OnceCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// 2026-01-01T00:00:00Z. Commit `n` is dated `EPOCH + n` minutes.
const EPOCH: i64 = 1_767_225_600;
const AUTHOR_NAME: &str = "Fixture Author";
const AUTHOR_EMAIL: &str = "fixture@example.com";

/// A throwaway git repository. Deleted when dropped.
#[derive(Debug)]
pub struct FixtureRepo {
    dir: TempDir,
    origin: OnceCell<TempDir>,
    clock: Cell<i64>,
}

impl FixtureRepo {
    /// An empty repository on branch `main`.
    #[must_use]
    pub fn new() -> Self {
        let repo = Self {
            dir: TempDir::new().expect("create temp dir"),
            origin: OnceCell::new(),
            clock: Cell::new(EPOCH),
        };
        repo.git(&["init", "--quiet", "--initial-branch=main"]);
        for (key, value) in [
            ("user.name", AUTHOR_NAME),
            ("user.email", AUTHOR_EMAIL),
            ("commit.gpgsign", "false"),
            ("tag.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            repo.git(&["config", key, value]);
        }
        repo
    }

    /// A repository with every shape the graph view needs:
    ///
    /// - `main` with a merge from `develop`, tag `v0.1.0`, pushed to `origin`
    /// - `develop` one commit ahead of the merge
    /// - `feature/login` branched from `develop`, not merged
    /// - a stash and an uncommitted change on `main`
    #[must_use]
    pub fn sample() -> Self {
        let repo = Self::new();
        repo.commit_file("README.md", "# sample\n", "Initial commit");
        repo.commit_file("src/main.rs", "fn main() {}\n", "Add main");
        repo.branch("develop");
        repo.commit_file("src/lib.rs", "pub fn lib() {}\n", "Add lib");
        repo.checkout("main");
        repo.commit_file("README.md", "# sample\n\nDocs.\n", "Update docs");
        repo.merge_no_ff("develop", "Merge branch 'develop'");
        repo.tag("v0.1.0");
        repo.add_origin();
        repo.push("main");
        repo.checkout("develop");
        repo.commit_file("src/lib.rs", "pub fn lib() -> u8 { 1 }\n", "Return a value");
        repo.branch("feature/login");
        repo.commit_file("src/login.rs", "pub fn login() {}\n", "Add login");
        repo.checkout("main");
        repo.write_file("README.md", "# sample\n\nStashed edit.\n");
        repo.stash("WIP on docs");
        repo.write_file("notes.txt", "uncommitted\n");
        repo
    }

    /// Path of the working tree.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Runs `git` in the repository and returns trimmed stdout.
    ///
    /// # Panics
    ///
    /// If git can't be started or exits with a non-zero status.
    pub fn git(&self, args: &[&str]) -> String {
        run_git(self.path(), args, self.clock.get())
    }

    /// Writes `contents` to `relative_path` (creating parent directories) without committing.
    pub fn write_file(&self, relative_path: &str, contents: &str) {
        let path = self.path().join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(path, contents).expect("write file");
    }

    /// Writes a file, commits it with `message`, and returns the new commit hash.
    pub fn commit_file(&self, relative_path: &str, contents: &str, message: &str) -> String {
        self.write_file(relative_path, contents);
        self.git(&["add", "--", relative_path]);
        self.tick();
        self.git(&["commit", "--quiet", "--message", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// Creates `name` at HEAD and switches to it.
    pub fn branch(&self, name: &str) {
        self.git(&["switch", "--quiet", "--create", name]);
    }

    /// Switches to an existing branch.
    pub fn checkout(&self, name: &str) {
        self.git(&["switch", "--quiet", name]);
    }

    /// Merges `branch` into the current branch, always creating a merge commit.
    pub fn merge_no_ff(&self, branch: &str, message: &str) -> String {
        self.tick();
        self.git(&["merge", "--quiet", "--no-ff", "--message", message, branch]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// Creates a lightweight tag at HEAD.
    pub fn tag(&self, name: &str) {
        self.git(&["tag", name]);
    }

    /// Creates an annotated tag at HEAD.
    pub fn annotated_tag(&self, name: &str, message: &str) {
        self.tick();
        self.git(&["tag", "--annotate", name, "--message", message]);
    }

    /// Stashes the current changes with `message`.
    pub fn stash(&self, message: &str) {
        self.tick();
        self.git(&["stash", "push", "--quiet", "--message", message]);
    }

    /// Creates a bare repository in its own temp dir and adds it as `origin`.
    ///
    /// # Panics
    ///
    /// If called twice on the same repository.
    pub fn add_origin(&self) -> PathBuf {
        let bare = TempDir::new().expect("create temp dir for origin");
        run_git(
            bare.path(),
            &["init", "--quiet", "--bare", "--initial-branch=main"],
            EPOCH,
        );
        let url = bare.path().to_str().expect("temp path is UTF-8").to_owned();
        self.git(&["remote", "add", "origin", &url]);
        let path = bare.path().to_path_buf();
        assert!(self.origin.set(bare).is_ok(), "origin already added");
        path
    }

    /// Pushes `branch` to `origin` and sets it as upstream.
    pub fn push(&self, branch: &str) {
        self.git(&["push", "--quiet", "--set-upstream", "origin", branch]);
    }

    /// Advances the fixture clock by one minute.
    fn tick(&self) {
        self.clock.set(self.clock.get() + 60);
    }
}

impl Default for FixtureRepo {
    fn default() -> Self {
        Self::new()
    }
}

fn run_git(dir: &Path, args: &[&str], timestamp: i64) -> String {
    let date = format!("@{timestamp} +0000");
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        // Ignore the developer's global and system config (signing, hooks, aliases).
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", AUTHOR_NAME)
        .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
        .env("GIT_COMMITTER_NAME", AUTHOR_NAME)
        .env("GIT_COMMITTER_EMAIL", AUTHOR_EMAIL)
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .output()
        .expect("start git");
    assert!(
        output.status.success(),
        "git {} failed in {}:\n{}",
        args.join(" "),
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim_end()
        .to_owned()
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}
