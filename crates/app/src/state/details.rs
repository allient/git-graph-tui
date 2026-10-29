//! The inline commit details panel (S10) and the diff view it opens.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use tgg_git::show::{self, CommitDetails, FileChange};
use tgg_model::Oid;
use tgg_ui::{
    CommitDetailsView, DetailsHit, DetailsHitMap, DetailsOutcome, DiffHitMap, DiffOutcome,
    DiffView, Key,
};

use super::compare::Comparison;
use super::{AppState, Command, Job, Request, SCROLL_LINES, actions::widget_key};
use crate::external;

/// The details of one commit, filled in as its two reads arrive; or a
/// comparison (S12), shown in the same place.
#[derive(Debug)]
pub struct Details {
    /// The commit whose row the panel opens below.
    pub commit: Oid,
    pub(super) info: Option<CommitDetails>,
    pub(super) files: Option<Vec<FileChange>>,
    /// Built once both reads arrived (a comparison: once its files arrived).
    pub view: Option<CommitDetailsView>,
    /// Where the last frame drew the panel.
    pub hits: Option<DetailsHitMap>,
    /// Set when the panel compares two revisions instead of showing `commit`.
    pub compare: Option<Comparison>,
}

impl Details {
    /// Shows the details of `id` (not a comparison anchored at its row).
    fn shows_commit(&self, id: &Oid) -> bool {
        self.compare.is_none() && self.commit == *id
    }
}

/// The open diff and where the last frame drew it.
#[derive(Debug)]
pub struct DiffPanel {
    pub view: DiffView,
    pub hits: Option<DiffHitMap>,
}

impl AppState {
    /// Display row the details panel opens below, while its commit is loaded.
    pub fn details_display_index(&self) -> Option<usize> {
        let commit = &self.details.as_ref()?.commit;
        let index = self.commits.iter().position(|c| &c.id == commit)?;
        Some(index + usize::from(!self.status.is_clean()))
    }

    /// Opens the details of the selected commit, or closes them if they are
    /// already open for it.
    pub(super) fn toggle_details(&mut self) -> Vec<Command> {
        let Some(id) = self.selected_commit().map(|c| c.id.clone()) else {
            return Vec::new();
        };
        if self.details.as_ref().is_some_and(|d| d.commit == id) {
            self.details = None;
            return Vec::new();
        }
        self.open_details(id)
    }

    fn open_details(&mut self, commit: Oid) -> Vec<Command> {
        self.details = Some(Details {
            commit: commit.clone(),
            info: None,
            files: None,
            view: None,
            hits: None,
            compare: None,
        });
        vec![
            Command::Git(Request::Details(commit.clone())),
            Command::Git(Request::Files(commit)),
        ]
    }

    /// Closes the panel when a reload dropped its commit from the history.
    pub(super) fn drop_details_if_gone(&mut self) {
        let gone = self
            .details
            .as_ref()
            .is_some_and(|d| !self.commits.iter().any(|c| c.id == d.commit));
        if gone {
            self.details = None;
        }
    }

    /// Keys for the open panel; None lets the graph handle the key.
    pub(super) fn on_details_key(&mut self, key: KeyEvent) -> Option<Vec<Command>> {
        let key = match key.code {
            KeyCode::Char('j') => Key::Down,
            KeyCode::Char('k') => Key::Up,
            KeyCode::Char('v') => Key::Char('v'),
            KeyCode::Char('t') => return Some(self.difftool_selected()),
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Enter
            | KeyCode::Esc => widget_key(key)?,
            _ => return None,
        };
        let view = self.details.as_mut()?.view.as_mut();
        let outcome = match view {
            Some(view) => view.handle_key(key),
            // Still loading: only Esc does something.
            None if key == Key::Esc => DetailsOutcome::Close,
            None => DetailsOutcome::Stay,
        };
        Some(self.on_details_outcome(outcome))
    }

    /// `t`: opens the selected file in `git difftool` (S12).
    fn difftool_selected(&self) -> Vec<Command> {
        let Some(details) = &self.details else {
            return Vec::new();
        };
        let Some(file) = details.view.as_ref().and_then(|v| v.selected_file()) else {
            return Vec::new();
        };
        let args = match (&details.compare, &details.info) {
            (Some(c), _) => show::compare_difftool_args(&c.base, c.other.as_ref(), file),
            (None, Some(info)) => {
                show::difftool_args(&details.commit, info.commit.parents.first(), file)
            }
            (None, None) => return Vec::new(),
        };
        vec![Command::External(external::difftool_command(
            &self.repo, args,
        ))]
    }

    /// A left click inside the panel; None when the click is outside it.
    pub(super) fn on_details_click(&mut self, x: u16, y: u16) -> Option<Vec<Command>> {
        let details = self.details.as_mut()?;
        let hits = details.hits.as_ref()?;
        if hits.hit(x, y) == DetailsHit::Outside {
            return None;
        }
        let outcome = match &mut details.view {
            Some(view) => view.handle_click(hits, x, y),
            None => DetailsOutcome::Stay,
        };
        Some(self.on_details_outcome(outcome))
    }

    pub(super) fn on_details_outcome(&mut self, outcome: DetailsOutcome) -> Vec<Command> {
        match outcome {
            DetailsOutcome::Stay => Vec::new(),
            DetailsOutcome::Close => {
                self.details = None;
                Vec::new()
            }
            DetailsOutcome::OpenFile(file) => {
                let (args, request) = match &self.details {
                    Some(Details {
                        compare: Some(c), ..
                    }) => (c.diff_args(&file), Request::CompareDiff(c.clone(), file)),
                    Some(d) => (
                        show::diff_args(&d.commit, &file),
                        Request::Diff(d.commit.clone(), file),
                    ),
                    None => return Vec::new(),
                };
                // With a pager (S12) the same git command is piped into it
                // instead of read for the built-in view.
                vec![match &self.pager {
                    Some(pager) => {
                        Command::External(external::pager_command(&self.repo, &args, pager))
                    }
                    None => Command::Git(request),
                }]
            }
            DetailsOutcome::ViewFile(file) => match &self.details {
                Some(d) => vec![Command::Git(Request::FileAt(d.commit.clone(), file.path))],
                None => Vec::new(),
            },
            DetailsOutcome::JumpTo(id) => {
                self.jump_target = Some(id);
                self.continue_jump().unwrap_or_default()
            }
            DetailsOutcome::OpenIssue(number) => match self.issues.url_for(&number) {
                Some(url) => self.open_url(url),
                // Read origin's URL to find its forge's issue page.
                None => vec![Command::Git(Request::IssueUrl { number })],
            },
        }
    }

    /// Opens issue `number` once origin's URL is read (None: no origin).
    pub(super) fn on_issue_remote_url(
        &mut self,
        number: &str,
        origin: Option<&str>,
    ) -> Vec<Command> {
        let origin = origin.map(str::trim);
        if let Some(url) = origin.and_then(|url| tgg_actions::issue_url(url, number)) {
            return self.open_url(url);
        }
        let reference = format!("{}{number}", self.issues.prefix());
        let why = if origin.is_some() {
            "origin isn't on GitHub, GitLab or Bitbucket"
        } else {
            "this repository has no origin remote"
        };
        self.message = Some(format!(
            "Can't open {reference}: {why}. Set [issues] url in the config file, for example url = \"https://jira.example.com/browse/PROJ-$1\"."
        ));
        Vec::new()
    }

    fn open_url(&mut self, url: String) -> Vec<Command> {
        self.message = Some(format!("Opening {url}"));
        vec![Command::OpenUrl(url)]
    }

    /// Moves a pending jump forward: selects the target and opens its
    /// details once it is loaded, otherwise asks for the next history page
    /// (S10: a parent can be older than everything loaded so far). Returns
    /// None when no jump is pending, so the caller does its usual paging.
    pub(super) fn continue_jump(&mut self) -> Option<Vec<Command>> {
        let id = self.jump_target.clone()?;
        if let Some(index) = self.commits.iter().position(|c| c.id == id) {
            self.jump_target = None;
            self.message = None;
            self.select(index + usize::from(!self.status.is_clean()));
            return Some(self.open_details(id));
        }
        if self.history_complete {
            self.jump_target = None;
            self.message = Some(format!("{} is not in the loaded history.", id.short(8)));
            return Some(Vec::new());
        }
        self.message = Some(format!("Loading older commits to find {}…", id.short(8)));
        let loading_log = self
            .pending
            .values()
            .any(|(job, _)| matches!(job, Job::Read(Request::Log { .. })));
        Some(if loading_log {
            Vec::new()
        } else {
            self.log_request(self.commits.len()).into_iter().collect()
        })
    }

    /// Applies a details read. Results for a panel that has since closed or
    /// moved to another commit are ignored.
    pub(super) fn on_details_read(&mut self, request: Request, stdout: &str) {
        let Some(details) = &mut self.details else {
            return;
        };
        match request {
            Request::Details(id) if details.shows_commit(&id) => {
                match show::parse_details(stdout) {
                    Ok(info) => details.info = Some(info),
                    Err(err) => self.message = Some(format!("Could not read the commit: {err}")),
                }
            }
            Request::Files(id) if details.shows_commit(&id) => match show::parse_files(stdout) {
                Ok(files) => details.files = Some(files),
                Err(err) => self.message = Some(format!("Could not read changed files: {err}")),
            },
            Request::Diff(id, file) if details.shows_commit(&id) => {
                self.diff = Some(DiffPanel {
                    view: DiffView::new(&file, show::parse_diff(stdout)),
                    hits: None,
                });
            }
            Request::FileAt(id, path) if details.shows_commit(&id) => {
                let title = format!("{path} @ {}", id.short(7));
                self.diff = Some(DiffPanel {
                    view: DiffView::text(title, stdout),
                    hits: None,
                });
            }
            _ => {}
        }
        if let Some(details) = &mut self.details
            && details.view.is_none()
            && let (Some(info), Some(files)) = (&details.info, &details.files)
        {
            details.view = Some(
                CommitDetailsView::new(info.clone(), files.clone())
                    .with_issue_prefix(self.issues.prefix()),
            );
        }
    }

    pub(super) fn on_diff_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(diff) = &mut self.diff else {
            return Vec::new();
        };
        let key = match key.code {
            KeyCode::Char('j') => Key::Down,
            KeyCode::Char('k') => Key::Up,
            KeyCode::Char('q') => Key::Esc,
            _ => match widget_key(key) {
                Some(key) => key,
                None => return Vec::new(),
            },
        };
        let page = diff.hits.as_ref().map_or(1, DiffHitMap::page);
        if diff.view.handle_key(key, page) == DiffOutcome::Close {
            self.diff = None;
        }
        Vec::new()
    }

    pub(super) fn on_diff_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        let Some(diff) = &mut self.diff else {
            return Vec::new();
        };
        let page = diff.hits.as_ref().map_or(1, DiffHitMap::page);
        let lines = isize::try_from(SCROLL_LINES).unwrap_or(1);
        match mouse.kind {
            MouseEventKind::ScrollDown => diff.view.scroll_by(lines, page),
            MouseEventKind::ScrollUp => diff.view.scroll_by(-lines, page),
            // The diff covers the screen; a right click closes it.
            MouseEventKind::Down(MouseButton::Right) => self.diff = None,
            _ => {}
        }
        Vec::new()
    }
}
