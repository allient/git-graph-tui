//! The inline commit details panel (S10) and the diff view it opens.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use tgg_git::show::{self, CommitDetails, FileChange};
use tgg_model::Oid;
use tgg_ui::{
    CommitDetailsView, DetailsHit, DetailsHitMap, DetailsOutcome, DiffHitMap, DiffOutcome,
    DiffView, Key,
};

use super::{AppState, Command, Request, SCROLL_LINES, actions::widget_key};

/// The details of one commit, filled in as its two reads arrive.
#[derive(Debug)]
pub struct Details {
    pub commit: Oid,
    info: Option<CommitDetails>,
    files: Option<Vec<FileChange>>,
    /// Built once both reads arrived.
    pub view: Option<CommitDetailsView>,
    /// Where the last frame drew the panel.
    pub hits: Option<DetailsHitMap>,
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

    fn on_details_outcome(&mut self, outcome: DetailsOutcome) -> Vec<Command> {
        match outcome {
            DetailsOutcome::Stay => Vec::new(),
            DetailsOutcome::Close => {
                self.details = None;
                Vec::new()
            }
            DetailsOutcome::OpenFile(file) => match &self.details {
                Some(d) => vec![Command::Git(Request::Diff(d.commit.clone(), file))],
                None => Vec::new(),
            },
            DetailsOutcome::JumpTo(id) => {
                if let Some(index) = self.commits.iter().position(|c| c.id == id) {
                    self.select(index + usize::from(!self.status.is_clean()));
                    return self.open_details(id);
                }
                self.message = Some(format!(
                    "{} is not loaded yet; scroll down to load older commits.",
                    id.short(8)
                ));
                Vec::new()
            }
        }
    }

    /// Applies a details read. Results for a panel that has since closed or
    /// moved to another commit are ignored.
    pub(super) fn on_details_read(&mut self, request: Request, stdout: &str) {
        let Some(details) = &mut self.details else {
            return;
        };
        match request {
            Request::Details(id) if id == details.commit => match show::parse_details(stdout) {
                Ok(info) => details.info = Some(info),
                Err(err) => self.message = Some(format!("Could not read the commit: {err}")),
            },
            Request::Files(id) if id == details.commit => match show::parse_files(stdout) {
                Ok(files) => details.files = Some(files),
                Err(err) => self.message = Some(format!("Could not read changed files: {err}")),
            },
            Request::Diff(id, file) if id == details.commit => {
                self.diff = Some(DiffPanel {
                    view: DiffView::new(&file, show::parse_diff(stdout)),
                    hits: None,
                });
            }
            _ => {}
        }
        if let Some(details) = &mut self.details
            && details.view.is_none()
            && let (Some(info), Some(files)) = (&details.info, &details.files)
        {
            details.view = Some(CommitDetailsView::new(info.clone(), files.clone()));
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
