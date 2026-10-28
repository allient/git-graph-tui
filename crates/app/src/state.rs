//! Application state and the pure `update` function: input and git results
//! go in, git requests come out. Rendering reads the state; nothing else
//! changes it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;
use tgg_actions::{CommitTarget, MenuContext, MenuItem, Target, menu_for};
use tgg_git::{CancelToken, ExecError, GitCommand, log, refs, status};
use tgg_graph::Graph;
use tgg_model::{Commit, Oid, Ref, RefKind, WorkingTreeStatus, split_remote};
use tgg_ui::{HitMap, HitTarget, Key, MenuHitMap, MenuOutcome, PopupMenu};

use crate::git::{JobHandle, JobId, JobResult};

/// Commits loaded per `git log` call.
pub const DEFAULT_PAGE_SIZE: usize = 500;

/// Lines scrolled per mouse-wheel step.
pub const SCROLL_LINES: usize = 3;

/// Something that happened.
#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Git(JobResult),
    /// Reload refs, status and history from the start.
    Refresh,
}

/// Something for the outside world to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Git(Request),
}

/// A git read the app needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Head,
    Refs,
    Status,
    /// A page of history starting `skip` commits from the newest.
    Log {
        skip: usize,
        max_count: usize,
    },
}

impl Request {
    /// The git command for this request in `repo`.
    pub fn command(self, repo: &Path) -> GitCommand {
        match self {
            Self::Head => GitCommand::read(repo, ["rev-parse", "--verify", "--quiet", "HEAD"]),
            Self::Refs => GitCommand::read(repo, refs::args()),
            Self::Status => GitCommand::read(repo, status::args()),
            Self::Log { skip, max_count } => {
                GitCommand::read(repo, log::args(&log::LogQuery { max_count, skip }))
            }
        }
    }
}

/// Everything the UI shows and every in-flight git request.
#[derive(Debug)]
pub struct AppState {
    pub repo: PathBuf,
    pub commits: Vec<Commit>,
    pub graph: Graph,
    pub refs: Vec<Ref>,
    pub status: WorkingTreeStatus,
    pub head: Option<Oid>,
    /// Selected display row (0 is the uncommitted row when the tree is dirty).
    pub selected: usize,
    /// First visible display row.
    pub offset: usize,
    /// Rows the graph view can show; set by the renderer every frame.
    pub viewport_height: usize,
    pub page_size: usize,
    /// True once `git log` returned a short page (no older commits).
    pub history_complete: bool,
    /// Status-bar message (the last git error).
    pub message: Option<String>,
    pub should_quit: bool,
    /// The open context menu, if any.
    pub menu: Option<PopupMenu>,
    /// What the last frame drew where; mouse events resolve against these.
    pub graph_hits: HitMap,
    pub menu_hits: Option<MenuHitMap>,
    pending: HashMap<JobId, (Request, CancelToken)>,
    /// HEAD from the refresh in progress; applied together with its first log page.
    refreshing_head: Option<Oid>,
    /// Commit to keep selected across a refresh (None keeps the uncommitted row).
    keep_selected: Option<Oid>,
}

impl AppState {
    pub fn new(repo: PathBuf) -> Self {
        Self {
            repo,
            commits: Vec::new(),
            graph: Graph::new(None),
            refs: Vec::new(),
            status: WorkingTreeStatus::default(),
            head: None,
            selected: 0,
            offset: 0,
            viewport_height: 1,
            page_size: DEFAULT_PAGE_SIZE,
            history_complete: false,
            message: None,
            should_quit: false,
            menu: None,
            graph_hits: HitMap::default(),
            menu_hits: None,
            pending: HashMap::new(),
            refreshing_head: None,
            keep_selected: None,
        }
    }

    /// Records a submitted request so its result can be matched, and so a
    /// newer refresh can cancel it.
    pub fn track(&mut self, job: &JobHandle, request: Request) {
        self.pending.insert(job.id, (request, job.cancel.clone()));
    }

    pub fn is_loading(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Display rows: the uncommitted row (when dirty) plus the commits.
    pub fn display_len(&self) -> usize {
        self.commits.len() + usize::from(!self.status.is_clean())
    }

    /// Commit index of a display row; None for the uncommitted row.
    pub fn commit_index(&self, display: usize) -> Option<usize> {
        if self.status.is_clean() {
            Some(display)
        } else {
            display.checked_sub(1)
        }
    }

    pub fn selected_commit(&self) -> Option<&Commit> {
        self.commit_index(self.selected)
            .and_then(|i| self.commits.get(i))
    }

    pub fn update(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Refresh => self.refresh(),
            Event::Key(key) => self.on_key(key),
            Event::Mouse(mouse) => self.on_mouse(mouse),
            Event::Git(result) => self.on_git(result),
        }
    }

    fn refresh(&mut self) -> Vec<Command> {
        // Cancel in-flight requests; any result that still arrives is ignored as stale.
        for (_, (_, cancel)) in self.pending.drain() {
            cancel.cancel();
        }
        self.keep_selected = self.selected_commit().map(|c| c.id.clone());
        self.refreshing_head = None;
        [Request::Head, Request::Refs, Request::Status]
            .into_iter()
            .map(Command::Git)
            .collect()
    }

    fn on_key(&mut self, key: KeyEvent) -> Vec<Command> {
        if key.kind != KeyEventKind::Press {
            return Vec::new();
        }
        if self.menu.is_some() {
            if let Some(key) = menu_key(key) {
                self.on_menu_outcome(|menu| menu.handle_key(key));
            }
            return Vec::new();
        }
        let page = self.viewport_height.max(1);
        let last = self.display_len().saturating_sub(1);
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL)
            | (KeyCode::Char('q') | KeyCode::Esc, _) => {
                self.should_quit = true;
                return Vec::new();
            }
            (KeyCode::Char('r'), _) => return self.refresh(),
            (KeyCode::Char('j') | KeyCode::Down, _) => self.select(self.selected + 1),
            (KeyCode::Char('k') | KeyCode::Up, _) => self.select(self.selected.saturating_sub(1)),
            (KeyCode::PageDown, _) => self.select(self.selected + page),
            (KeyCode::PageUp, _) => self.select(self.selected.saturating_sub(page)),
            (KeyCode::Char('g') | KeyCode::Home, _) => self.select(0),
            (KeyCode::Char('G') | KeyCode::End, _) => self.select(last),
            (KeyCode::Char('m') | KeyCode::Menu, _) | (KeyCode::F(10), KeyModifiers::SHIFT) => {
                self.open_menu_for_selected();
            }
            (KeyCode::Char('H'), _) => {
                if let Some(index) = self.head_display_index() {
                    self.select(index);
                }
            }
            _ => {}
        }
        self.load_more_if_needed()
    }

    fn on_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        let (x, y) = (mouse.column, mouse.row);
        if self.menu.is_some() {
            let Some(hits) = self.menu_hits.clone() else {
                return Vec::new();
            };
            match mouse.kind {
                MouseEventKind::Moved => {
                    if let Some(menu) = &mut self.menu {
                        menu.handle_hover(&hits, x, y);
                    }
                }
                MouseEventKind::Down(_) => {
                    self.on_menu_outcome(|menu| menu.handle_click(&hits, x, y));
                }
                _ => {}
            }
            return Vec::new();
        }
        match mouse.kind {
            MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Right)) => {
                let Some(target) = self.graph_hits.hit(x, y).cloned() else {
                    return Vec::new();
                };
                self.select(self.display_index(&target));
                if button == MouseButton::Right {
                    self.open_menu(&target, Position { x, y });
                }
                Vec::new()
            }
            MouseEventKind::ScrollDown => {
                let max = self.display_len().saturating_sub(self.viewport_height);
                self.offset = (self.offset + SCROLL_LINES).min(max);
                self.load_more_if_needed()
            }
            MouseEventKind::ScrollUp => {
                self.offset = self.offset.saturating_sub(SCROLL_LINES);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Display row a hit target belongs to.
    fn display_index(&self, target: &HitTarget) -> usize {
        let dirty = usize::from(!self.status.is_clean());
        match target {
            HitTarget::Uncommitted => 0,
            HitTarget::Commit(index) | HitTarget::Ref { commit: index, .. } => index + dirty,
        }
    }

    fn open_menu_for_selected(&mut self) {
        let target = match self.commit_index(self.selected) {
            Some(index) => HitTarget::Commit(index),
            None => HitTarget::Uncommitted,
        };
        // Anchor just below the selected row, in from the left edge.
        let row = self.selected.saturating_sub(self.offset) + 1;
        let anchor = Position {
            x: 4,
            y: u16::try_from(row).unwrap_or(u16::MAX),
        };
        self.open_menu(&target, anchor);
    }

    fn open_menu(&mut self, target: &HitTarget, anchor: Position) {
        let Some(target) = self.menu_target(target) else {
            return;
        };
        let items = menu_for(&target, &self.menu_context());
        if items.iter().all(|i| matches!(i, MenuItem::Separator)) {
            self.message = Some("No actions here yet.".into());
            return;
        }
        self.menu = Some(PopupMenu::new(items, anchor));
        self.menu_hits = None;
    }

    fn menu_target(&self, target: &HitTarget) -> Option<Target> {
        match target {
            HitTarget::Uncommitted => Some(Target::Uncommitted),
            HitTarget::Commit(index) => {
                let commit = self.commits.get(*index)?;
                let in_head_ancestry = self
                    .graph
                    .rows()
                    .get(*index)
                    .is_some_and(|r| r.in_head_ancestry);
                Some(Target::Commit(CommitTarget {
                    id: commit.id.clone(),
                    subject: commit.subject.clone(),
                    parent_count: commit.parents.len(),
                    is_head_ancestor: in_head_ancestry,
                }))
            }
            HitTarget::Ref { full_name, .. } => {
                let r = self.refs.iter().find(|r| &r.full_name == full_name)?;
                Target::from_ref(r)
            }
        }
    }

    fn menu_context(&self) -> MenuContext {
        let mut remotes: Vec<String> = self
            .refs
            .iter()
            .filter(|r| matches!(r.kind, RefKind::RemoteBranch | RefKind::RemoteHead))
            .filter_map(|r| split_remote(&r.full_name).map(|(remote, _)| remote.to_owned()))
            .collect();
        remotes.sort();
        remotes.dedup();
        MenuContext {
            head: self.status.head.clone(),
            head_commit: self.head.clone(),
            remotes,
            branch_filter: Vec::new(),
        }
    }

    /// Applies a menu outcome: closing, reporting a disabled entry, or a choice.
    fn on_menu_outcome(&mut self, handle: impl FnOnce(&mut PopupMenu) -> MenuOutcome) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        match handle(menu) {
            MenuOutcome::Open => {}
            MenuOutcome::Disabled(reason) => self.message = Some(reason),
            MenuOutcome::Closed => self.close_menu(),
            MenuOutcome::Chosen(_action) => {
                let label = menu
                    .selected()
                    .and_then(|i| match menu.items().get(i) {
                        Some(MenuItem::Entry(entry)) => Some(entry.label.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                // Running actions (dialogs, git, refresh) is the next task (T10).
                self.message = Some(format!(
                    "{} — runs in the next step",
                    label.trim_end_matches("...")
                ));
                self.close_menu();
            }
        }
    }

    fn close_menu(&mut self) {
        self.menu = None;
        self.menu_hits = None;
    }

    fn on_git(&mut self, result: JobResult) -> Vec<Command> {
        let Some((request, _)) = self.pending.remove(&result.id) else {
            return Vec::new(); // Superseded by a newer refresh.
        };
        let JobResult {
            command, result, ..
        } = result;
        match (request, result) {
            (Request::Head, Ok(out)) => self.on_head(Oid::parse(out.stdout.trim()).ok()),
            // `rev-parse --verify --quiet HEAD` fails silently in a repo with no commits.
            (Request::Head, Err(ExecError::Failed { .. })) => self.on_head(None),
            (Request::Refs, Ok(out)) => {
                self.parsed(refs::parse(&out.stdout), |s, refs| s.refs = refs);
                Vec::new()
            }
            (Request::Status, Ok(out)) => {
                self.parsed(status::parse(&out.stdout), Self::apply_status);
                Vec::new()
            }
            (Request::Log { skip, .. }, Ok(out)) => {
                match log::parse(&out.stdout) {
                    Ok(page) => self.on_log_page(skip, page),
                    Err(err) => self.message = Some(format!("Could not read git log: {err}")),
                }
                self.load_more_if_needed()
            }
            (_, Err(err)) => {
                self.message = Some(format!("{} ({command})", err.user_message()));
                Vec::new()
            }
        }
    }

    /// Replaces the working-tree status. The uncommitted row may appear or
    /// disappear above the commits: a selected commit stays selected, and a
    /// selection on the top row stays on the (new) top row. Status and log
    /// results arrive in either order, so this must not depend on which came first.
    fn apply_status(&mut self, status: WorkingTreeStatus) {
        let keep = (self.selected > 0)
            .then(|| self.selected_commit().map(|c| c.id.clone()))
            .flatten();
        self.status = status;
        if keep.is_some() {
            self.restore_selection(keep.as_ref());
        } else {
            self.select(0);
        }
    }

    fn on_head(&mut self, head: Option<Oid>) -> Vec<Command> {
        let has_commits = head.is_some();
        self.refreshing_head = head;
        if has_commits {
            vec![self.log_request(0)]
        } else {
            // No commits yet: nothing to load.
            self.on_log_page(0, Vec::new());
            Vec::new()
        }
    }

    fn on_log_page(&mut self, skip: usize, page: Vec<Commit>) {
        let complete = page.len() < self.page_size;
        if skip == 0 {
            let head = self.refreshing_head.take();
            self.graph = Graph::new(head.clone());
            self.head = head;
            self.commits = page;
            self.graph.extend(&self.commits);
            let keep = self.keep_selected.take();
            self.restore_selection(keep.as_ref());
        } else if skip == self.commits.len() {
            self.graph.extend(&page);
            self.commits.extend(page);
        } else {
            return; // A page for a history that has since been reloaded.
        }
        self.history_complete = complete;
        self.message = None;
    }

    fn parsed<T>(
        &mut self,
        parsed: Result<T, tgg_git::ParseError>,
        apply: impl FnOnce(&mut Self, T),
    ) {
        match parsed {
            Ok(value) => apply(self, value),
            Err(err) => self.message = Some(format!("Could not read git output: {err}")),
        }
    }

    fn log_request(&self, skip: usize) -> Command {
        Command::Git(Request::Log {
            skip,
            max_count: self.page_size,
        })
    }

    /// Asks for the next page when the selection is within half a page of the end.
    fn load_more_if_needed(&mut self) -> Vec<Command> {
        let loading_log = self
            .pending
            .values()
            .any(|(r, _)| matches!(r, Request::Log { .. }));
        let margin = (self.page_size / 2).max(1);
        let bottom = (self.offset + self.viewport_height).max(self.selected);
        let near_end = self
            .commit_index(bottom)
            .is_some_and(|i| i + margin >= self.commits.len());
        if self.history_complete || loading_log || self.commits.is_empty() || !near_end {
            return Vec::new();
        }
        vec![self.log_request(self.commits.len())]
    }

    fn head_display_index(&self) -> Option<usize> {
        let head = self.head.as_ref()?;
        let index = self.commits.iter().position(|c| &c.id == head)?;
        Some(index + usize::from(!self.status.is_clean()))
    }

    fn restore_selection(&mut self, keep: Option<&Oid>) {
        let found = keep.and_then(|id| self.commits.iter().position(|c| &c.id == id));
        let display = match found {
            Some(index) => index + usize::from(!self.status.is_clean()),
            None => self.selected,
        };
        self.select(display);
    }

    /// Selects a display row (clamped) and scrolls it into view.
    fn select(&mut self, display: usize) {
        self.selected = display.min(self.display_len().saturating_sub(1));
        let height = self.viewport_height.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
    }
}

/// Maps a terminal key to the widget key set; vim keys move inside menus too.
fn menu_key(key: KeyEvent) -> Option<Key> {
    Some(match key.code {
        KeyCode::Up | KeyCode::Char('k') => Key::Up,
        KeyCode::Down | KeyCode::Char('j') => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Char(c) => Key::Char(c),
        _ => return None,
    })
}

#[cfg(test)]
mod mouse_tests;
#[cfg(test)]
mod tests;
