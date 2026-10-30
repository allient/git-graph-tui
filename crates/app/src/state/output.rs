//! "Show Git Output": a log of the git commands actions ran, with their
//! status and output, and the scrollable view that shows it.

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ggt_git::{ExecError, GitCommand, Output};

use super::{AppState, Command, SCROLL_LINES};

/// Entries kept; older ones are dropped.
pub const OUTPUT_CAP: usize = 200;

/// One git command an action ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputEntry {
    /// `git <args>`, as shown in the log.
    pub command: String,
    /// `ok`, `exit <code>`, or why it didn't finish (timeout, cancel, ...).
    pub status: String,
    pub stdout: String,
    pub stderr: String,
}

/// The newest [`OUTPUT_CAP`] entries, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitOutput {
    entries: VecDeque<OutputEntry>,
}

impl GitOutput {
    pub fn push(&mut self, entry: OutputEntry) {
        if self.entries.len() == OUTPUT_CAP {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    #[cfg(test)]
    pub fn entries(&self) -> &VecDeque<OutputEntry> {
        &self.entries
    }

    /// The lines the view shows, newest command first.
    pub fn lines(&self) -> Vec<OutputLine> {
        if self.entries.is_empty() {
            return vec![OutputLine::Note("No git commands have run yet.".into())];
        }
        let mut lines = Vec::new();
        for entry in self.entries.iter().rev() {
            lines.push(OutputLine::Command {
                command: entry.command.clone(),
                status: entry.status.clone(),
            });
            lines.extend(
                entry
                    .stdout
                    .lines()
                    .map(|l| OutputLine::Stdout(l.to_owned())),
            );
            lines.extend(
                entry
                    .stderr
                    .lines()
                    .map(|l| OutputLine::Stderr(l.to_owned())),
            );
            lines.push(OutputLine::Note(String::new()));
        }
        lines
    }
}

/// A line of the git output view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputLine {
    Command { command: String, status: String },
    Stdout(String),
    Stderr(String),
    Note(String),
}

/// The open git output view: its scroll offset and the page height of the
/// last frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputView {
    pub offset: usize,
    pub page: usize,
}

impl AppState {
    /// Records one step of the running action for the git output view.
    pub(super) fn record_output(
        &mut self,
        command: &GitCommand,
        result: &Result<Output, ExecError>,
    ) {
        let (status, stdout, stderr) = match result {
            Ok(out) => ("ok".to_owned(), out.stdout.clone(), out.stderr.clone()),
            Err(ExecError::Failed { code, stderr, .. }) => {
                let status = code.map_or_else(|| "killed".to_owned(), |c| format!("exit {c}"));
                (status, String::new(), stderr.clone())
            }
            Err(other) => (other.to_string(), String::new(), String::new()),
        };
        self.git_output.push(OutputEntry {
            command: command.to_string(),
            status,
            stdout,
            stderr,
        });
    }

    pub(super) fn show_git_output(&mut self) -> Vec<Command> {
        self.output_view = Some(OutputView::default());
        Vec::new()
    }

    pub(super) fn on_output_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let page = self.output_view.as_ref().map_or(1, |v| v.page.max(1));
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.output_view = None,
            KeyCode::Down | KeyCode::Char('j') => self.scroll_output(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_output(-1),
            KeyCode::PageDown => self.scroll_output(signed(page)),
            KeyCode::PageUp => self.scroll_output(-signed(page)),
            KeyCode::Home | KeyCode::Char('g') => self.scroll_output(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.scroll_output(isize::MAX),
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn on_output_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_output(signed(SCROLL_LINES)),
            MouseEventKind::ScrollUp => self.scroll_output(-signed(SCROLL_LINES)),
            // The view covers the graph; a right click closes it, like the diff.
            MouseEventKind::Down(MouseButton::Right) => self.output_view = None,
            _ => {}
        }
        Vec::new()
    }

    /// Scrolls by `delta` lines, keeping the last line at or below the bottom.
    fn scroll_output(&mut self, delta: isize) {
        let total = self.git_output.lines().len();
        let Some(view) = &mut self.output_view else {
            return;
        };
        let max = total.saturating_sub(view.page.max(1));
        view.offset = view.offset.saturating_add_signed(delta).min(max);
    }
}

fn signed(n: usize) -> isize {
    isize::try_from(n).unwrap_or(isize::MAX)
}
