//! Find (`/`, S12): search the loaded commits by message, author, date, hash
//! and branch or tag name, and step through the matches.

use crossterm::event::KeyEvent;
use tgg_model::{Commit, Ref};
use tgg_ui::{Key, TextField, format_date};

use super::{AppState, Command, actions::widget_key};

/// The open find bar.
#[derive(Debug, Default)]
pub struct Find {
    pub query: TextField,
    /// Indices of the matching commits, newest first.
    pub matches: Vec<usize>,
    /// The match that is selected.
    pub current: usize,
}

impl AppState {
    pub(super) fn open_find(&mut self) {
        self.find = Some(Find::default());
    }

    pub(super) fn on_find_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let (Some(key), Some(find)) = (widget_key(key), &mut self.find) else {
            return Vec::new();
        };
        match key {
            Key::Esc => {
                self.find = None;
                return Vec::new();
            }
            Key::Enter | Key::Down | Key::Tab => self.step_find(true),
            Key::Up | Key::BackTab => self.step_find(false),
            Key::Char(ch) => {
                find.query.insert(ch);
                self.search();
            }
            Key::Backspace => {
                find.query.backspace();
                self.search();
            }
            Key::Delete => {
                find.query.delete();
                self.search();
            }
            Key::Left => find.query.left(),
            Key::Right => find.query.right(),
            Key::Home => find.query.home(),
            Key::End => find.query.end(),
            Key::PageUp | Key::PageDown => {}
        }
        self.load_more_if_needed()
    }

    /// Recomputes the matches and selects the first one.
    fn search(&mut self) {
        self.refind();
        self.select_match();
    }

    /// Recomputes the matches, after typing or when history changed.
    pub(super) fn refind(&mut self) {
        let Some(find) = &mut self.find else {
            return;
        };
        let query = find.query.text().to_lowercase();
        find.matches = if query.is_empty() {
            Vec::new()
        } else {
            (0..self.commits.len())
                .filter(|&i| matches(&self.commits[i], &self.refs, &query))
                .collect()
        };
        find.current = 0;
    }

    fn step_find(&mut self, forward: bool) {
        let Some(find) = &mut self.find else {
            return;
        };
        let count = find.matches.len();
        if count == 0 {
            return;
        }
        find.current = if forward {
            (find.current + 1) % count
        } else {
            (find.current + count - 1) % count
        };
        self.select_match();
    }

    fn select_match(&mut self) {
        let Some(&index) = self.find.as_ref().and_then(|f| f.matches.get(f.current)) else {
            return;
        };
        let uncommitted = usize::from(self.status.changed_files() > 0);
        self.select(index + uncommitted);
    }
}

/// Whether `commit` matches a lowercase `query`: its subject, author, date,
/// hash prefix, or the name of a branch or tag on it.
fn matches(commit: &Commit, refs: &[Ref], query: &str) -> bool {
    let contains = |text: &str| text.to_lowercase().contains(query);
    contains(&commit.subject)
        || contains(&commit.author.name)
        || contains(&commit.author.email)
        || contains(&format_date(commit.author.time))
        || commit.id.as_str().starts_with(query)
        || refs
            .iter()
            .any(|r| r.target == commit.id && contains(r.short_name()))
}
