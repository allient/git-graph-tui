//! Drives the real `ggt` binary in a terminal with tui-test-rs:
//! real keys and mouse events in, the drawn screen and the repository's git
//! state out. Run with `sh e2e/run.sh` (it builds the binary first).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use tempfile::TempDir;
use tui_test::{
    KeyAction, LocatorClickOptions, MouseAction, MouseButton, MouseOptions, OpenOptions, Operation,
    OperationResult, RunOptions, Session,
};

const WAIT_MS: u64 = 5_000;

/// A scratch repository: `main` (2 commits, tag `v0.1.0` on the first, one
/// stash) and `feature` (1 commit on top of the first).
struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().expect("temp dir"),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.name", "E2E"]);
        repo.git(&["config", "user.email", "e2e@example.com"]);
        repo.commit("a.txt", "first commit");
        repo.git(&["tag", "v0.1.0"]);
        repo.git(&["switch", "-q", "-c", "feature"]);
        repo.commit("f.txt", "feature work");
        repo.git(&["switch", "-q", "main"]);
        repo.commit("b.txt", "second commit");
        std::fs::write(repo.path().join("b.txt"), "changed\n").expect("write");
        repo.git(&["stash", "push", "-q", "-m", "wip on b"]);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn commit(&self, file: &str, subject: &str) {
        std::fs::write(self.path().join(file), format!("{subject}\n")).expect("write");
        self.git(&["add", file]);
        self.git(&["commit", "-q", "-m", subject]);
    }

    /// Runs git in the repository and returns its trimmed stdout.
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

/// The app running on `repo` in a 120x32 terminal, with its own empty HOME so
/// the user's config never applies.
struct App {
    session: Session,
    _home: TempDir,
}

impl App {
    fn launch(repo: &Repo) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let home = tempfile::tempdir().expect("temp home");
        let home_path = home.path().to_string_lossy().into_owned();
        let session = Session::new(format!(
            "ggt-e2e-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let defaults = OpenOptions::default();
        session
            .run(RunOptions {
                backend: defaults.backend,
                program: binary().to_string_lossy().into_owned(),
                // Plain Unicode icons, so the screen text doesn't depend on
                // the terminal's fonts.
                args: vec![
                    "--icons".into(),
                    "unicode".into(),
                    repo.path().to_string_lossy().into_owned(),
                ],
                profile: defaults.profile,
                style: defaults.style,
                cols: 120,
                rows: 32,
                cwd: Some(repo.path().to_string_lossy().into_owned()),
                env: vec![
                    ("HOME".into(), home_path.clone()),
                    ("XDG_CONFIG_HOME".into(), format!("{home_path}/.config")),
                    ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
                    ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
                ],
                wait_ready: Some(false),
                restart: true,
                timeouts: defaults.timeouts,
                recording: defaults.recording,
            })
            .expect("run git-graph-tui");
        let app = Self {
            session,
            _home: home,
        };
        app.wait_for("first commit");
        app
    }

    fn screen(&self) -> String {
        match self.session.execute(Operation::Text { full: false }) {
            Ok(OperationResult::Text(text)) => text,
            other => panic!("read screen: {other:?}"),
        }
    }

    fn wait_for(&self, text: &str) {
        if let Err(err) = self
            .session
            .get_by_text(text)
            .first()
            .wait_with_timeout(Some(WAIT_MS))
        {
            panic!("{text:?} never appeared: {err}\n{}", self.screen());
        }
    }

    fn wait_gone(&self, text: &str) {
        if let Err(err) = self
            .session
            .get_by_text(text)
            .first()
            .wait_hidden(Some(WAIT_MS))
        {
            panic!("{text:?} never went away: {err}\n{}", self.screen());
        }
    }

    fn press(&self, key: &str) {
        self.session
            .execute(Operation::Key {
                keys: vec![key.into()],
                action: KeyAction::Press,
            })
            .expect("press key");
    }

    fn click(&self, text: &str, button: MouseButton) {
        let options = LocatorClickOptions {
            mouse: MouseOptions {
                button,
                ..MouseOptions::default()
            },
            timeout_ms: Some(WAIT_MS),
            ..LocatorClickOptions::default()
        };
        if let Err(err) = self.session.get_by_text(text).first().click_with(options) {
            panic!("click {text:?}: {err}\n{}", self.screen());
        }
    }
}

impl App {
    fn type_text(&self, text: &str) {
        self.session
            .execute(Operation::Write { data: text.into() })
            .expect("type text");
    }

    /// Left-clicks the first `label` on the same row as `row_text`, such as
    /// a file's stage button.
    fn click_on_row(&self, row_text: &str, label: &str) {
        self.wait_for(row_text);
        let row = self
            .session
            .get_by_text(row_text)
            .first()
            .location()
            .expect("row")
            .start
            .row;
        let line = self
            .screen()
            .lines()
            .nth(row as usize)
            .unwrap_or_default()
            .to_string();
        let column = line
            .char_indices()
            .filter(|&(byte, _)| line[byte..].starts_with(label))
            .map(|(byte, _)| line[..byte].chars().count())
            .next_back()
            .unwrap_or_else(|| panic!("no {label:?} on row {row_text:?}: {line}"));
        self.session
            .execute(Operation::Mouse {
                action: MouseAction::Click {
                    x: Some(u16::try_from(column).expect("column")),
                    y: Some(u16::try_from(row).expect("row")),
                    on_text: None,
                    options: MouseOptions::default(),
                    clicks: 1,
                },
            })
            .expect("click");
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.session.close();
    }
}

/// `GIT_GRAPH_TUI_BIN`, or the workspace's debug build.
fn binary() -> PathBuf {
    std::env::var_os("GIT_GRAPH_TUI_BIN").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/debug/ggt"),
        PathBuf::from,
    )
}

#[test]
fn graph_shows_commits_branches_tags_and_the_stash() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    for text in [
        "second commit",
        "feature work",
        "first commit",
        "main",
        "feature",
        "v0.1.0",
        "wip on b",
    ] {
        app.wait_for(text);
    }
}

#[test]
fn right_click_on_a_branch_badge_opens_the_branch_menu() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    app.click("feature", MouseButton::Right);
    for entry in [
        "Checkout Branch",
        "Rename Branch...",
        "Delete Branch...",
        "Merge into current branch...",
        "Copy Branch Name to Clipboard",
    ] {
        app.wait_for(entry);
    }
    app.press("Escape");
    app.wait_gone("Checkout Branch");
}

#[test]
fn checkout_branch_from_the_menu_switches_head() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    app.click("feature", MouseButton::Right);
    app.click("Checkout Branch", MouseButton::Left);
    app.wait_for("✓ Checkout Branch");
    assert_eq!(repo.git(&["branch", "--show-current"]), "feature");
}

#[test]
fn delete_tag_deletes_it_after_confirming() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    app.click("v0.1.0", MouseButton::Right);
    app.click("Delete Tag...", MouseButton::Left);
    app.wait_for("Delete the local tag 'v0.1.0'?");
    app.click("[ Delete ]", MouseButton::Left);
    app.wait_gone("v0.1.0");
    assert_eq!(repo.git(&["tag", "--list"]), "");
}

#[test]
fn cancelling_a_confirmation_keeps_the_tag() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    app.click("v0.1.0", MouseButton::Right);
    app.click("Delete Tag...", MouseButton::Left);
    app.wait_for("Delete the local tag 'v0.1.0'?");
    app.press("Escape");
    app.wait_for("Delete Tag cancelled.");
    assert_eq!(repo.git(&["tag", "--list"]), "v0.1.0");
}

#[test]
fn drop_stash_from_the_stash_row_menu() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    app.click("wip on b", MouseButton::Right);
    app.click("Drop Stash...", MouseButton::Left);
    app.wait_for("[ Drop ]");
    app.click("[ Drop ]", MouseButton::Left);
    app.wait_gone("wip on b");
    assert_eq!(repo.git(&["stash", "list"]), "");
}

#[test]
fn keyboard_menu_reaches_the_branch_menu_of_the_selected_row() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    app.press("H");
    app.press("m");
    app.wait_for("Branch main");
    app.press("Escape");
    app.wait_gone("Branch main");
}

#[test]
fn stage_a_file_and_commit_it_from_source_control() {
    let repo = Repo::new();
    std::fs::write(repo.path().join("new.txt"), "new\n").expect("write");
    let app = App::launch(&repo);
    app.wait_for("Uncommitted Changes (1)");
    app.click_on_row("new.txt", "+");
    app.wait_for("Staged Changes");
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "new.txt");

    // Focus is on the file list now, so click the message box.
    app.click("Message (Enter commits", MouseButton::Left);
    app.type_text("add new file");
    app.press("Enter");
    app.wait_for("add new file");
    app.wait_gone("Uncommitted Changes");
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "add new file");
    assert_eq!(repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn q_quits() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    app.press("q");
    app.session
        .execute(Operation::WaitExit {
            timeout_ms: Some(WAIT_MS),
        })
        .expect("git-graph-tui exits on q");
}

#[test]
fn a_commit_made_in_another_terminal_appears_on_its_own() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    repo.commit("outside.txt", "made in another terminal");
    app.wait_for("made in another terminal");
}

#[test]
fn a_branch_made_in_another_terminal_appears_on_its_own() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    repo.git(&["branch", "from-outside"]);
    app.wait_for("from-outside");
}

#[test]
fn a_file_edited_elsewhere_shows_up_as_uncommitted() {
    let repo = Repo::new();
    let app = App::launch(&repo);
    std::fs::write(repo.path().join("a.txt"), "edited in an editor\n").expect("write");
    app.wait_for("Uncommitted Changes (1)");
}
