//! Read-only info dialogs: "View Details" on a tag.

use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ggt_git::tag::{self, TagDetails};

use super::{AppState, Command, Request};

/// A read-only dialog: a title and lines of text. Esc, Enter, `q` or any
/// click closes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub title: String,
    pub lines: Vec<String>,
}

impl AppState {
    /// Asks git for the tag; the dialog opens when the read arrives.
    pub(super) fn show_tag(&mut self, name: String) -> Vec<Command> {
        self.message = Some(format!("Loading tag {name}…"));
        vec![Command::Git(Request::Tag(name))]
    }

    pub(super) fn on_tag_read(&mut self, name: &str, stdout: &str) {
        match tag::parse(stdout, name) {
            Ok(Some(details)) => {
                self.message = None;
                self.info = Some(tag_info(&details));
            }
            Ok(None) => self.message = Some(format!("Tag {name} not found.")),
            Err(err) => self.message = Some(format!("Could not read tag {name}: {err}")),
        }
    }

    pub(super) fn on_info_key(&mut self, key: KeyEvent) -> Vec<Command> {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
            self.info = None;
        }
        Vec::new()
    }

    pub(super) fn on_info_mouse(&mut self, mouse: MouseEvent) -> Vec<Command> {
        if matches!(mouse.kind, MouseEventKind::Down(_)) {
            self.info = None;
        }
        Vec::new()
    }
}

fn tag_info(details: &TagDetails) -> Info {
    let mut lines = vec![format!("Commit:  {}", details.commit)];
    if let Some(annotation) = &details.annotation {
        let tagger = &annotation.tagger;
        lines.push(format!("Tagger:  {} <{}>", tagger.name, tagger.email));
        lines.push(format!("Date:    {}", ggt_ui::format_date(tagger.time)));
        lines.push(String::new());
        lines.extend(annotation.message.lines().map(str::to_owned));
    } else {
        lines.push(String::new());
        lines.push("Lightweight tag: no tagger or message.".to_owned());
    }
    Info {
        title: format!("Tag {}", details.name),
        lines,
    }
}
