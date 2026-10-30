//! Notices when something outside the app changes the repository, so the
//! graph stays current without a manual reload.
//!
//! Only git's own files are watched (`HEAD`, `index`, `refs/`, ...), never the
//! working tree: a working tree can hold hundreds of thousands of files
//! (`node_modules`, `target`) and watching it can exhaust inotify watches on
//! Linux. Edits in the working tree are found by polling `git status`
//! instead (see `main.rs`), only while the terminal has focus.
//!
//! The watcher uses the platform's file events (`notify`). Where those don't
//! work (network drives, WSL `/mnt`, some containers, an inotify limit), it
//! falls back to comparing the modification times of those same files.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use ggt_git::{CancelToken, Executor, GitCommand};

/// Quiet time before a burst of events is reported: one checkout or rebase
/// writes many files, and should cause one reload.
const SETTLE: Duration = Duration::from_millis(250);
/// Report anyway after this long, so a long rebase still shows progress.
const MAX_WAIT: Duration = Duration::from_secs(1);
/// How often the fallback compares modification times.
const POLL_EVERY: Duration = Duration::from_secs(1);

/// What changed, and so what to reload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    /// The staging area: only the working-tree status needs a reload.
    Index,
    /// HEAD, branches, tags, stashes, remotes or an operation in progress:
    /// reload everything.
    Refs,
}

/// Where git keeps its files. In a linked worktree, `HEAD` and `index` live
/// in its own git dir and the refs in the common one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDirs {
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
}

impl GitDirs {
    /// Asks git for the repository's git dirs.
    pub fn resolve(executor: &impl Executor, repo: &Path) -> Option<Self> {
        let command = GitCommand::read(
            repo,
            [
                "rev-parse",
                "--path-format=absolute",
                "--git-dir",
                "--git-common-dir",
            ],
        );
        let output = executor.run(&command, &CancelToken::new()).ok()?;
        let mut lines = output.stdout.lines();
        let git_dir = PathBuf::from(lines.next()?);
        let common_dir = PathBuf::from(lines.next()?);
        // Events carry resolved paths (`/private/var/...` on macOS), so
        // compare against resolved ones.
        Some(Self {
            git_dir: git_dir.canonicalize().unwrap_or(git_dir),
            common_dir: common_dir.canonicalize().unwrap_or(common_dir),
        })
    }

    fn refs(&self) -> PathBuf {
        self.common_dir.join("refs")
    }

    /// What a change to `path` means, or `None` for files that don't affect
    /// what the app shows (lock files, logs, objects, `COMMIT_EDITMSG`, ...).
    pub fn classify(&self, path: &Path) -> Option<Change> {
        let name = path.file_name()?.to_str()?;
        // git writes `<file>.lock` and renames it over `<file>`: the rename
        // is the change that matters.
        if path.extension().is_some_and(|ext| ext == "lock") {
            return None;
        }
        if path.starts_with(self.refs()) {
            return Some(Change::Refs);
        }
        let parent = path.parent()?;
        if parent != self.git_dir && parent != self.common_dir {
            return None;
        }
        match name {
            "index" => Some(Change::Index),
            // Not FETCH_HEAD: editors fetch in the background and rewrite it
            // even when nothing changed; a fetch that brings anything also
            // updates `refs/remotes`.
            "HEAD" | "packed-refs" | "ORIG_HEAD" | "MERGE_HEAD" | "CHERRY_PICK_HEAD"
            | "REVERT_HEAD" | "rebase-merge" | "rebase-apply" | "config" => Some(Change::Refs),
            _ => None,
        }
    }
}

/// Groups a burst of changes into one report.
#[derive(Debug, Default)]
struct Debounce {
    /// The strongest change so far, when the burst started, and the last event.
    pending: Option<(Change, Instant, Instant)>,
}

impl Debounce {
    fn record(&mut self, change: Change, at: Instant) {
        self.pending = Some(match self.pending {
            Some((old, first, _)) => (old.max(change), first, at),
            None => (change, at, at),
        });
    }

    /// The change and its last event, once the burst has settled. A reload
    /// asked for after that last event has seen every change in the burst.
    fn take(&mut self, now: Instant) -> Option<(Change, Instant)> {
        let (change, first, last) = self.pending?;
        let settled = now.duration_since(last) >= SETTLE;
        let overdue = now.duration_since(first) >= MAX_WAIT;
        if settled || overdue {
            self.pending = None;
            return Some((change, last));
        }
        None
    }
}

type Stamp = Option<(SystemTime, u64)>;

/// Modification times of the files `GitDirs::classify` cares about.
#[derive(Debug, Default, PartialEq, Eq)]
struct Snapshot {
    index: Stamp,
    refs: Vec<(PathBuf, Stamp)>,
}

impl Snapshot {
    fn take(dirs: &GitDirs) -> Self {
        let mut refs = Vec::new();
        for dir in [&dirs.git_dir, &dirs.common_dir] {
            for name in [
                "HEAD",
                "packed-refs",
                "ORIG_HEAD",
                "MERGE_HEAD",
                "CHERRY_PICK_HEAD",
                "REVERT_HEAD",
                "rebase-merge",
                "rebase-apply",
                "config",
            ] {
                let path = dir.join(name);
                refs.push((path.clone(), stamp(&path)));
            }
        }
        walk(&dirs.refs(), &mut refs);
        Self {
            index: stamp(&dirs.git_dir.join("index")),
            refs,
        }
    }

    /// What changed since `old`.
    fn changes_since(&self, old: &Self) -> Option<Change> {
        if self.refs != old.refs {
            Some(Change::Refs)
        } else if self.index != old.index {
            Some(Change::Index)
        } else {
            None
        }
    }
}

fn stamp(path: &Path) -> Stamp {
    let meta = fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Every file and folder under `dir`. Folders count too: git renames a
/// new ref into place, which changes its folder's modification time.
fn walk(dir: &Path, out: &mut Vec<(PathBuf, Stamp)>) {
    out.push((dir.to_path_buf(), stamp(dir)));
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push((path.clone(), stamp(&path)));
        }
    }
}

enum Backend {
    Events {
        // Kept alive: dropping it stops the events.
        _watcher: RecommendedWatcher,
        events: mpsc::Receiver<notify::Result<notify::Event>>,
    },
    Poll {
        last: Snapshot,
        next: Instant,
    },
}

/// Watches a repository's git files. Call [`RepoWatch::poll`] from the
/// event loop; it never blocks.
pub struct RepoWatch {
    dirs: GitDirs,
    backend: Backend,
    debounce: Debounce,
}

impl RepoWatch {
    /// Watches with file events, or by polling when they can't be set up.
    pub fn start(dirs: GitDirs) -> Self {
        let backend = events(&dirs).unwrap_or_else(|| poll_backend(&dirs));
        Self {
            dirs,
            backend,
            debounce: Debounce::default(),
        }
    }

    /// Watches by polling only (also what `start` falls back to).
    #[cfg(test)]
    pub fn polling(dirs: GitDirs) -> Self {
        let backend = poll_backend(&dirs);
        Self {
            dirs,
            backend,
            debounce: Debounce::default(),
        }
    }

    /// A settled change and when it was last seen, if there is one.
    pub fn poll(&mut self, now: Instant) -> Option<(Change, Instant)> {
        let mut broken = false;
        match &mut self.backend {
            Backend::Events { events, .. } => {
                while let Ok(event) = events.try_recv() {
                    match event {
                        Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                            for path in &event.paths {
                                if let Some(change) = self.dirs.classify(path) {
                                    self.debounce.record(change, now);
                                }
                            }
                        }
                        Ok(_) => {}
                        // Events were lost or the watch broke (e.g. the
                        // inotify queue overflowed): reload, then poll.
                        Err(_) => broken = true,
                    }
                }
            }
            Backend::Poll { last, next } => {
                if now >= *next {
                    *next = now + POLL_EVERY;
                    let current = Snapshot::take(&self.dirs);
                    if let Some(change) = current.changes_since(last) {
                        self.debounce.record(change, now);
                    }
                    *last = current;
                }
            }
        }
        if broken {
            self.backend = poll_backend(&self.dirs);
            self.debounce.record(Change::Refs, now);
        }
        self.debounce.take(now)
    }
}

fn poll_backend(dirs: &GitDirs) -> Backend {
    Backend::Poll {
        last: Snapshot::take(dirs),
        next: Instant::now() + POLL_EVERY,
    }
}

/// File events on the git dirs: each folder on its own, plus `refs/` with
/// everything under it. `objects/` and `logs/` are never watched.
fn events(dirs: &GitDirs) -> Option<Backend> {
    let (sender, events) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        // The receiver is gone only when the app is exiting.
        let _ = sender.send(event);
    })
    .ok()?;
    watcher
        .watch(&dirs.git_dir, RecursiveMode::NonRecursive)
        .ok()?;
    if dirs.common_dir != dirs.git_dir {
        watcher
            .watch(&dirs.common_dir, RecursiveMode::NonRecursive)
            .ok()?;
    }
    watcher.watch(&dirs.refs(), RecursiveMode::Recursive).ok()?;
    Some(Backend::Events {
        _watcher: watcher,
        events,
    })
}

#[cfg(test)]
mod tests;
