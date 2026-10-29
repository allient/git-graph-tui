//! Application state and the pure `update` function: input and git results
//! go in, git requests come out. Rendering reads the state; nothing else
//! changes it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use tgg_actions::{CommitTarget, MenuContext, MenuItem, Target, menu_for};
use tgg_git::show::{self, FileChange};
use tgg_git::{CancelToken, ExecError, GitCommand, log, refs, status};
use tgg_graph::Graph;
use tgg_model::{Commit, Oid, Ref, RefKind, WorkingTreeStatus, split_remote};
use tgg_ui::{HitMap, HitTarget, Key, MenuHitMap, MenuOutcome, PopupMenu};

use crate::git::{JobHandle, JobId, JobResult};

mod actions;
mod details;
mod info;
mod output;
mod source_control;
pub use source_control::{MIN_WIDTH_FOR_PANE, SourceControlState};
mod toolbar;

use actions::Running;
pub use actions::{Dialog, DialogHits};
pub use details::{Details, DiffPanel};
pub use info::Info;
pub use output::{GitOutput, OutputLine, OutputView};
pub use toolbar::{OpenPicker, ToolbarState};

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
    /// A background read (refresh, history page).
    Git(Request),
    /// One git step of the running action.
    Run(GitCommand),
    /// Text to put on the system clipboard.
    Clipboard(String),
}

/// What a tracked job is for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Job {
    Read(Request),
    Action,
}

/// A git read the app needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Head,
    Refs,
    Status,
    /// A page of history starting `skip` commits from the newest.
    Log {
        skip: usize,
        max_count: usize,
        revisions: log::Revisions,
    },
    /// Metadata and message of a commit, for the details panel.
    Details(Oid),
    /// Files a commit changed, for the details panel.
    Files(Oid),
    /// The diff of one file in a commit.
    Diff(Oid, FileChange),
    /// A tag's target and annotation, for its info dialog.
    Tag(String),
}

impl Request {
    /// The git command for this request in `repo`.
    pub fn command(&self, repo: &Path) -> GitCommand {
        match self {
            Self::Head => GitCommand::read(repo, ["rev-parse", "--verify", "--quiet", "HEAD"]),
            Self::Refs => GitCommand::read(repo, refs::args()),
            Self::Status => GitCommand::read(repo, status::args()),
            Self::Log {
                skip,
                max_count,
                revisions,
            } => GitCommand::read(
                repo,
                log::args(&log::LogQuery {
                    max_count: *max_count,
                    skip: *skip,
                    revisions: revisions.clone(),
                }),
            ),
            Self::Details(id) => GitCommand::read(repo, show::details_args(id)),
            Self::Files(id) => GitCommand::read(repo, show::files_args(id)),
            Self::Diff(id, file) => GitCommand::read(repo, show::diff_args(id, file)),
            Self::Tag(name) => GitCommand::read(repo, tgg_git::tag::args(name)),
        }
    }

    /// Part of a refresh (history and status), which a newer refresh replaces.
    fn is_refresh(&self) -> bool {
        matches!(
            self,
            Self::Head | Self::Refs | Self::Status | Self::Log { .. }
        )
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
    /// The open modal dialog (inputs or confirmation), above any menu.
    pub dialog: Option<Dialog>,
    pub dialog_hits: Option<DialogHits>,
    /// The commit details panel, open below its commit's row.
    pub details: Option<Details>,
    /// The diff view, drawn over the whole screen.
    pub diff: Option<DiffPanel>,
    /// A read-only info dialog (tag details), above everything but dialogs.
    pub info: Option<Info>,
    /// Git commands actions ran, for "Show Git Output".
    pub git_output: GitOutput,
    /// The open git output view, drawn over the graph.
    pub output_view: Option<OutputView>,
    /// The toolbar's branch filter, remote toggle and keyboard focus.
    pub toolbar: ToolbarState,
    /// The open ref picker (branch filter, "Checkout to...", ...), above any menu.
    pub picker: Option<OpenPicker>,
    /// The Source Control pane beside the graph.
    pub source_control: SourceControlState,
    running: Option<Running>,
    pending: HashMap<JobId, (Job, CancelToken)>,
    /// HEAD from the refresh in progress; applied together with its first log page.
    refreshing_head: Option<Oid>,
    /// Commit to keep selected across a refresh (None keeps the uncommitted row).
    keep_selected: Option<Oid>,
    /// A commit to select once history paging reaches it (a parent jump).
    jump_target: Option<Oid>,
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
            dialog: None,
            dialog_hits: None,
            details: None,
            diff: None,
            info: None,
            git_output: GitOutput::default(),
            output_view: None,
            toolbar: ToolbarState::default(),
            picker: None,
            source_control: SourceControlState::default(),
            running: None,
            pending: HashMap::new(),
            refreshing_head: None,
            keep_selected: None,
            jump_target: None,
        }
    }

    /// Records a submitted request so its result can be matched, and so a
    /// newer refresh can cancel it.
    pub fn track(&mut self, job: &JobHandle, request: Request) {
        self.pending
            .insert(job.id, (Job::Read(request), job.cancel.clone()));
    }

    /// Records a submitted step of the running action.
    pub fn track_action(&mut self, job: &JobHandle) {
        self.pending
            .insert(job.id, (Job::Action, job.cancel.clone()));
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
        // Cancel in-flight reads; any result that still arrives is ignored as
        // stale. A running action is never cancelled: stopping git halfway
        // through a merge or rebase would leave the repository in a mess.
        // Details reads are not part of a refresh and keep going.
        self.pending.retain(|_, (job, cancel)| {
            let read = matches!(job, Job::Read(r) if r.is_refresh());
            if read {
                cancel.cancel();
            }
            !read
        });
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
        if self.dialog.is_some() {
            return self.on_dialog_key(key);
        }
        if self.info.is_some() {
            return self.on_info_key(key);
        }
        if self.picker.is_some() {
            return self.on_picker_key(key);
        }
        if self.menu.is_some() {
            return match menu_key(key) {
                Some(key) => self.on_menu_outcome(|menu| menu.handle_key(key)),
                None => Vec::new(),
            };
        }
        if self.output_view.is_some() {
            return self.on_output_key(key);
        }
        if self.diff.is_some() {
            return self.on_diff_key(key);
        }
        if self.toolbar.focus.is_some() {
            return self.on_toolbar_key(key);
        }
        if self.source_control.focus {
            return self.on_source_control_key(key);
        }
        if let Some(commands) = self.on_details_key(key) {
            return commands;
        }
        let page = self.viewport_height.max(1);
        let last = self.display_len().saturating_sub(1);
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL)
            | (KeyCode::Char('q') | KeyCode::Esc, _) => {
                self.should_quit = true;
                return Vec::new();
            }
            (KeyCode::Char('r'), _) => {
                // A manual refresh clears old messages; automatic ones keep the
                // result of the action that triggered them.
                self.message = None;
                return self.refresh();
            }
            (KeyCode::Char('j') | KeyCode::Down, _) => self.select(self.selected + 1),
            (KeyCode::Char('k') | KeyCode::Up, _) => self.select(self.selected.saturating_sub(1)),
            (KeyCode::PageDown, _) => self.select(self.selected + page),
            (KeyCode::PageUp, _) => self.select(self.selected.saturating_sub(page)),
            (KeyCode::Enter, _) => return self.toggle_details(),
            (KeyCode::Tab, _) => self.focus_toolbar(),
            (KeyCode::Char('c'), _) => self.focus_source_control(),
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
        if self.dialog.is_some() {
            return match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => self.on_dialog_click(x, y),
                _ => Vec::new(),
            };
        }
        if self.info.is_some() {
            return self.on_info_mouse(mouse);
        }
        if self.picker.is_some() {
            return match mouse.kind {
                MouseEventKind::Down(_) => self.on_picker_click(x, y),
                _ => Vec::new(),
            };
        }
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
                MouseEventKind::Down(_) if !hits.contains(x, y) => self.close_menu(),
                MouseEventKind::Down(_) => {
                    return self.on_menu_outcome(|menu| menu.handle_click(&hits, x, y));
                }
                _ => {}
            }
            return Vec::new();
        }
        if self.output_view.is_some() {
            return self.on_output_mouse(mouse);
        }
        if self.diff.is_some() {
            return self.on_diff_mouse(mouse);
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && let Some(commands) = self
                .on_source_control_click(x, y)
                .or_else(|| self.on_toolbar_click(x, y))
                .or_else(|| self.on_details_click(x, y))
        {
            return commands;
        }
        match mouse.kind {
            MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Right)) => {
                let Some(target) = self.graph_hits.hit(x, y).cloned() else {
                    return Vec::new();
                };
                self.select(self.display_index(&target));
                match button {
                    MouseButton::Right => self.open_menu(&target, Position { x, y }),
                    _ if target != HitTarget::Uncommitted => return self.toggle_details(),
                    _ => {}
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
        // Anchor just below the selected row as last drawn, in from the left edge.
        let y = self.graph_hits.area_of(&target).map_or(0, Rect::bottom);
        let anchor = Position { x: 4, y };
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
            branch_filter: self.toolbar.branch_filter.clone(),
        }
    }

    /// Applies a menu outcome: closing, reporting a disabled entry, or a choice.
    fn on_menu_outcome(
        &mut self,
        handle: impl FnOnce(&mut PopupMenu) -> MenuOutcome,
    ) -> Vec<Command> {
        let Some(menu) = &mut self.menu else {
            return Vec::new();
        };
        match handle(menu) {
            MenuOutcome::Open => Vec::new(),
            MenuOutcome::Disabled(reason) => {
                self.message = Some(reason);
                Vec::new()
            }
            MenuOutcome::Closed => {
                self.close_menu();
                Vec::new()
            }
            MenuOutcome::Chosen(action) => {
                // The chosen entry at the deepest level (submenus included).
                let label = menu
                    .selected_label()
                    .unwrap_or_default()
                    .trim_end_matches("...")
                    .to_owned();
                self.close_menu();
                self.start_action(action, label)
            }
        }
    }

    fn close_menu(&mut self) {
        self.menu = None;
        self.menu_hits = None;
    }

    fn on_git(&mut self, result: JobResult) -> Vec<Command> {
        let Some((job, _)) = self.pending.remove(&result.id) else {
            return Vec::new(); // Superseded by a newer refresh.
        };
        let request = match job {
            Job::Read(request) => request,
            Job::Action => return self.on_action_result(&result.command, result.result),
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
                // A pending parent jump does its own paging.
                self.continue_jump()
                    .unwrap_or_else(|| self.load_more_if_needed())
            }
            (Request::Tag(name), Ok(out)) => {
                self.on_tag_read(&name, &out.stdout);
                Vec::new()
            }
            (request @ (Request::Details(_) | Request::Files(_) | Request::Diff(..)), Ok(out)) => {
                self.on_details_read(request, &out.stdout);
                Vec::new()
            }
            (request, Err(err)) => {
                if matches!(request, Request::Log { .. }) {
                    // Paging stopped: don't jump later, out of the blue.
                    self.jump_target = None;
                }
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
        self.update_source_control();
        if keep.is_some() {
            self.restore_selection(keep.as_ref());
        } else {
            self.select(0);
        }
    }

    fn on_head(&mut self, head: Option<Oid>) -> Vec<Command> {
        let has_commits = head.is_some();
        self.refreshing_head = head;
        match self.log_request(0) {
            Some(request) if has_commits => vec![request],
            // No commits yet, or a branch filter that matches nothing: nothing to load.
            _ => {
                self.on_log_page(0, Vec::new());
                Vec::new()
            }
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
            self.drop_details_if_gone();
        } else if skip == self.commits.len() {
            self.graph.extend(&page);
            self.commits.extend(page);
        } else {
            return; // A page for a history that has since been reloaded.
        }
        self.history_complete = complete;
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

    fn log_request(&self, skip: usize) -> Option<Command> {
        Some(Command::Git(Request::Log {
            skip,
            max_count: self.page_size,
            revisions: self.log_revisions()?,
        }))
    }

    /// Asks for the next page when the selection is within half a page of the end.
    fn load_more_if_needed(&mut self) -> Vec<Command> {
        let loading_log = self
            .pending
            .values()
            .any(|(job, _)| matches!(job, Job::Read(Request::Log { .. })));
        let margin = (self.page_size / 2).max(1);
        let bottom = (self.offset + self.viewport_height).max(self.selected);
        let near_end = self
            .commit_index(bottom)
            .is_some_and(|i| i + margin >= self.commits.len());
        if self.history_complete || loading_log || self.commits.is_empty() || !near_end {
            return Vec::new();
        }
        self.log_request(self.commits.len()).into_iter().collect()
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

/// Maps a terminal key for menus: like dialogs, plus vim keys to move.
fn menu_key(key: KeyEvent) -> Option<Key> {
    match key.code {
        KeyCode::Char('k') => Some(Key::Up),
        KeyCode::Char('j') => Some(Key::Down),
        _ => actions::widget_key(key),
    }
}

#[cfg(test)]
mod action_tests;
#[cfg(test)]
mod details_tests;
#[cfg(test)]
mod info_tests;
#[cfg(test)]
mod jump_tests;
#[cfg(test)]
mod mouse_tests;
#[cfg(test)]
mod output_tests;
#[cfg(test)]
mod source_control_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod toolbar_tests;
