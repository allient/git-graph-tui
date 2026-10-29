use std::collections::BTreeMap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Clear, Widget};
use tgg_git::show::{CommitDetails, FileChange, Numstat};
use tgg_model::Oid;
use unicode_width::UnicodeWidthStr;

use crate::{Glyph, Key, Theme, fit, format_date};

/// Characters of a parent hash shown (and clickable) in the details.
const PARENT_HASH: usize = 8;
/// Width of the `Committer: ` label column.
const LABEL: usize = 11;
/// Keys for the selected file, beside the changed-files title.
const KEY_HINTS: &str = "enter diff  v view file  t difftool";
/// The same for a comparison, which has no single commit to view a file at.
const COMPARISON_HINTS: &str = "enter diff  t difftool  esc close";

/// A row of the changed-files tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeRow {
    /// A folder (one path component), `depth` levels deep.
    Dir { depth: usize, name: String },
    /// A file: index into [`CommitDetailsView::files`].
    File { depth: usize, index: usize },
}

/// What a key press or click means for the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailsOutcome {
    /// Nothing for the app to do (the selection may have moved).
    Stay,
    /// Show this file's diff (`show::diff_args`, then a [`crate::DiffView`]).
    OpenFile(FileChange),
    /// Show this file's whole content at the commit (`show::file_at_args`).
    ViewFile(FileChange),
    /// Select this parent commit in the graph (S10).
    JumpTo(Oid),
    /// Open the page of the issue with this number (`#123` in the message).
    OpenIssue(String),
    /// Collapse the details panel.
    Close,
}

/// What is under a screen cell, as of the last render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailsHit {
    /// A file row (index into [`CommitDetailsView::rows`]).
    File(usize),
    Parent(Oid),
    /// An issue reference in the message: its number.
    Issue(String),
    Frame,
    Outside,
}

/// Where the panel, its parent hashes and file rows were drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailsHitMap {
    area: Rect,
    files: Vec<(Rect, usize)>,
    parents: Vec<(Rect, Oid)>,
    issues: Vec<(Rect, String)>,
}

impl DetailsHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> DetailsHit {
        let point = Position { x, y };
        if let Some((_, row)) = self.files.iter().find(|(r, _)| r.contains(point)) {
            DetailsHit::File(*row)
        } else if let Some((_, id)) = self.parents.iter().find(|(r, _)| r.contains(point)) {
            DetailsHit::Parent(id.clone())
        } else if let Some((_, number)) = self.issues.iter().find(|(r, _)| r.contains(point)) {
            DetailsHit::Issue(number.clone())
        } else if self.area.contains(point) {
            DetailsHit::Frame
        } else {
            DetailsHit::Outside
        }
    }

    /// Where tree row `row` was drawn, if it is a visible file.
    #[must_use]
    pub fn file_row(&self, row: usize) -> Option<Rect> {
        self.files.iter().find(|(_, r)| *r == row).map(|(r, _)| *r)
    }

    /// Where the parent hash `id` was drawn.
    #[must_use]
    pub fn parent(&self, id: &Oid) -> Option<Rect> {
        self.parents.iter().find(|(_, p)| p == id).map(|(r, _)| *r)
    }
}

/// What the panel shows above or beside its file tree.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Header {
    /// One commit's metadata and message, on the left half.
    Commit(CommitDetails),
    /// A comparison of two revisions: only this title, over the files.
    Comparison(String),
}

/// The inline commit details panel (S10): metadata and message on the left,
/// the changed files as a folder tree with `(+a -d)` counts on the right.
/// A comparison (S12) shows only the files, under its title.
///
/// The app draws it in the rows under the selected commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetailsView {
    header: Header,
    files: Vec<FileChange>,
    rows: Vec<TreeRow>,
    /// Index into `rows`, always a file row.
    selected: Option<usize>,
    /// What starts an issue reference in the message (`#` in `#123`).
    issue_prefix: String,
}

impl CommitDetailsView {
    #[must_use]
    pub fn new(details: CommitDetails, files: Vec<FileChange>) -> Self {
        Self::with_header(Header::Commit(details), files)
    }

    /// The files that differ between two revisions, under `title`
    /// (`Comparing abc1234..def5678`). `v` does nothing: the files belong to
    /// no single commit.
    #[must_use]
    pub fn comparison(title: String, files: Vec<FileChange>) -> Self {
        Self::with_header(Header::Comparison(title), files)
    }

    fn with_header(header: Header, files: Vec<FileChange>) -> Self {
        let rows = file_tree(&files);
        let selected = rows.iter().position(|r| matches!(r, TreeRow::File { .. }));
        Self {
            header,
            files,
            rows,
            selected,
            issue_prefix: "#".into(),
        }
    }

    /// Links issue references that start with `prefix` (such as `PROJ-`)
    /// instead of `#`.
    #[must_use]
    pub fn with_issue_prefix(mut self, prefix: &str) -> Self {
        prefix.clone_into(&mut self.issue_prefix);
        self
    }

    /// The commit shown, or None for a comparison.
    #[must_use]
    pub fn details(&self) -> Option<&CommitDetails> {
        match &self.header {
            Header::Commit(details) => Some(details),
            Header::Comparison(_) => None,
        }
    }

    #[must_use]
    pub fn files(&self) -> &[FileChange] {
        &self.files
    }

    #[must_use]
    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    #[must_use]
    pub fn selected_file(&self) -> Option<&FileChange> {
        match self.rows.get(self.selected?)? {
            TreeRow::File { index, .. } => self.files.get(*index),
            TreeRow::Dir { .. } => None,
        }
    }

    /// Up/Down/Home/End/PageUp/PageDown move between files, Enter opens the
    /// selected one's diff, `v` shows its content, Esc closes the panel.
    pub fn handle_key(&mut self, key: Key) -> DetailsOutcome {
        match key {
            Key::Down => self.step(true),
            Key::Up => self.step(false),
            Key::Home | Key::PageUp => {
                let first = self.file_rows().next();
                self.selected = first;
            }
            Key::End | Key::PageDown => {
                let last = self.file_rows().next_back();
                self.selected = last;
            }
            Key::Enter => {
                if let Some(file) = self.selected_file() {
                    return DetailsOutcome::OpenFile(file.clone());
                }
            }
            Key::Char('v') if self.details().is_some() => {
                if let Some(file) = self.selected_file() {
                    return DetailsOutcome::ViewFile(file.clone());
                }
            }
            Key::Esc => return DetailsOutcome::Close,
            _ => {}
        }
        DetailsOutcome::Stay
    }

    /// A click on a file selects and opens it; a click on a parent hash
    /// jumps to that commit; a click on an issue reference opens it.
    pub fn handle_click(&mut self, hits: &DetailsHitMap, x: u16, y: u16) -> DetailsOutcome {
        match hits.hit(x, y) {
            DetailsHit::File(row) => {
                self.selected = Some(row);
                self.selected_file()
                    .cloned()
                    .map_or(DetailsOutcome::Stay, DetailsOutcome::OpenFile)
            }
            DetailsHit::Parent(id) => DetailsOutcome::JumpTo(id),
            DetailsHit::Issue(number) => DetailsOutcome::OpenIssue(number),
            DetailsHit::Frame | DetailsHit::Outside => DetailsOutcome::Stay,
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) -> DetailsHitMap {
        let mut hits = DetailsHitMap {
            area,
            ..DetailsHitMap::default()
        };
        if area.width < 3 || area.height == 0 {
            return hits;
        }
        Clear.render(area, buf);
        let Header::Commit(details) = &self.header else {
            self.render_files(area, buf, theme, &mut hits);
            return hits;
        };
        let left = Rect {
            width: area.width / 2,
            ..area
        };
        let right = Rect {
            x: left.right() + 1,
            width: area.width - left.width - 1,
            ..area
        };
        for y in area.top()..area.bottom() {
            buf.set_string(left.right(), y, "│", Style::default().fg(theme.muted));
        }
        let message_y = Self::render_metadata(details, left, buf, theme, &mut hits);
        let message = Rect {
            y: message_y,
            height: left.bottom().saturating_sub(message_y),
            ..left
        };
        self.render_message(&details.message, message, buf, theme, &mut hits);
        self.render_files(right, buf, theme, &mut hits);
        hits
    }

    /// Draws the commit's fields; returns the line its message starts on.
    fn render_metadata(
        details: &CommitDetails,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        hits: &mut DetailsHitMap,
    ) -> u16 {
        let commit = &details.commit;
        let width = usize::from(area.width.saturating_sub(1));
        let text = Style::default().fg(theme.text);
        let label = text.add_modifier(Modifier::BOLD);
        let link = Style::default()
            .fg(theme.link)
            .add_modifier(Modifier::UNDERLINED);
        let x = area.x + 1;
        let value_width = width.saturating_sub(LABEL);
        let field = |buf: &mut Buffer, y: u16, name: &str, value: &str, style: Style| {
            buf.set_string(x, y, fit(name, LABEL.min(width)), label);
            buf.set_string(x + to_u16(LABEL), y, fit(value, value_width), style);
        };
        let signature = |s: &tgg_model::Signature| format!("{} <{}>", s.name, s.email);
        let mut lines: Vec<(&str, String, Style)> = vec![
            ("Commit:", commit.id.to_string(), text),
            ("Parents:", String::new(), text),
            ("Author:", signature(&commit.author), text),
            ("Committer:", signature(&commit.committer), text),
            ("Date:", format_date(commit.author.time), text),
        ];
        if commit.parents.is_empty() {
            lines[1].1 = "(none)".into();
        }
        let mut y = area.y;
        for (name, value, style) in &lines {
            if y >= area.bottom() {
                return y;
            }
            field(buf, y, name, value, *style);
            let email = match *name {
                "Author:" => Some(&commit.author),
                "Committer:" => Some(&commit.committer),
                _ => None,
            };
            if let Some(person) = email {
                // The address between `<` and `>`, as far as it is visible
                // (a cut value ends in an ellipsis).
                let start = person.name.width() + 2;
                let visible = if value.width() <= value_width {
                    value_width
                } else {
                    value_width.saturating_sub(1)
                };
                let end = (start + person.email.width()).min(visible);
                if end > start {
                    let at = x + to_u16(LABEL + start);
                    buf.set_style(Rect::new(at, y, to_u16(end - start), 1), link);
                }
            }
            if *name == "Parents:" {
                // One clickable hash per parent, left to right.
                let mut px = x + to_u16(LABEL);
                for parent in &commit.parents {
                    let short = parent.short(PARENT_HASH);
                    let w = to_u16(short.width());
                    if px + w > area.right() {
                        break;
                    }
                    buf.set_string(px, y, short, link);
                    hits.parents.push((Rect::new(px, y, w, 1), parent.clone()));
                    px += w + 1;
                }
            }
            y += 1;
        }
        // A blank line before the message.
        y + 1
    }

    /// The message in `area`: the subject bold, then the body. Issue
    /// references are underlined and clickable where they fit.
    fn render_message(
        &self,
        message: &str,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        hits: &mut DetailsHitMap,
    ) {
        let width = usize::from(area.width.saturating_sub(1));
        let text = Style::default().fg(theme.text);
        let link_style = Style::default()
            .fg(theme.tag)
            .add_modifier(Modifier::UNDERLINED);
        let x = area.x + 1;
        for (y, (i, line)) in (area.top()..area.bottom()).zip(message.lines().enumerate()) {
            let style = if i == 0 {
                text.add_modifier(Modifier::BOLD)
            } else {
                text
            };
            let line = line.replace('\t', "    ");
            buf.set_string(x, y, fit(&line, width), style);
            // A cut line ends in an ellipsis.
            let room = if line.width() <= width {
                width
            } else {
                width.saturating_sub(1)
            };
            for issue in tgg_actions::issue_refs(&line, &self.issue_prefix) {
                let start = line[..issue.range.start].width();
                let end = line[..issue.range.end].width();
                if end > room {
                    break;
                }
                let at = x + to_u16(start);
                buf.set_string(at, y, &line[issue.range], style.patch(link_style));
                hits.issues
                    .push((Rect::new(at, y, to_u16(end - start), 1), issue.number));
            }
        }
    }

    fn render_files(&self, area: Rect, buf: &mut Buffer, theme: &Theme, hits: &mut DetailsHitMap) {
        let width = usize::from(area.width.saturating_sub(1));
        let count = self.files.len();
        let (title, keys) = match &self.header {
            Header::Commit(_) => (format!("Changed files ({count})"), KEY_HINTS),
            Header::Comparison(title) => {
                let plural = if count == 1 { "" } else { "s" };
                let title = format!("{title} ({count} changed file{plural})");
                (title, COMPARISON_HINTS)
            }
        };
        let muted = Style::default().fg(theme.muted);
        buf.set_string(area.x + 1, area.y, fit(&title, width), muted);
        // Key hints at the right of the title row, when they fit.
        if count > 0 && title.width() + 2 + keys.width() <= width {
            let x = area.right().saturating_sub(to_u16(keys.width()) + 1);
            buf.set_string(x, area.y, keys, muted);
        }
        let visible = usize::from(area.height.saturating_sub(1));
        // Scroll so the selected file stays visible.
        let offset = self
            .selected
            .map_or(0, |s| s.saturating_sub(visible.saturating_sub(1)));
        for (n, (row_index, row)) in self
            .rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .enumerate()
        {
            let y = area.y + 1 + to_u16(n);
            let rect = Rect::new(area.x, y, area.width, 1);
            match row {
                TreeRow::Dir { depth, name } => {
                    let folder = theme.icons.glyph(Glyph::Folder);
                    let label = format!("{}{folder} {name}/", "  ".repeat(*depth));
                    buf.set_string(
                        area.x + 1,
                        y,
                        fit(&label, width),
                        Style::default().fg(theme.muted),
                    );
                }
                TreeRow::File { depth, index } => {
                    let Some(file) = self.files.get(*index) else {
                        continue;
                    };
                    let selected = self.selected == Some(row_index);
                    let style = if selected {
                        Style::default().fg(theme.text).bg(theme.selected_bg)
                    } else {
                        Style::default().fg(theme.text)
                    };
                    let (name, counts) = file_label(file);
                    let prefix =
                        format!("{}{} ", "  ".repeat(*depth), theme.icons.glyph(Glyph::File));
                    let room = width.saturating_sub(counts.width() + 1);
                    let line = format!("{}{counts}", fit(&format!("{prefix}{name}"), room + 1));
                    buf.set_string(area.x + 1, y, fit(&line, width), style);
                    // The name in the color of what happened to the file.
                    let start = prefix.width();
                    let end = (start + name.width()).min(room).min(width);
                    if end > start {
                        let at = area.x + 1 + to_u16(start);
                        buf.set_style(
                            Rect::new(at, y, to_u16(end - start), 1),
                            Style::default().fg(theme.change(file.status)),
                        );
                    }
                    hits.files.push((rect, row_index));
                }
            }
        }
    }

    fn file_rows(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        (0..self.rows.len()).filter(|&i| matches!(self.rows[i], TreeRow::File { .. }))
    }

    fn step(&mut self, down: bool) {
        let Some(current) = self.selected else {
            return;
        };
        let next = if down {
            self.file_rows().find(|&i| i > current)
        } else {
            self.file_rows().rev().find(|&i| i < current)
        };
        if let Some(next) = next {
            self.selected = Some(next);
        }
    }
}

/// A file's tree label and its counts: `name` (with rename info) and `(+a -d)`.
#[must_use]
pub fn file_label(file: &FileChange) -> (String, String) {
    let (dir, name) = split_path(&file.path);
    let label = match file.old_path.as_deref() {
        None => name.to_owned(),
        Some(old) => {
            let (old_dir, old_name) = split_path(old);
            if old_dir == dir {
                format!("{old_name} → {name}")
            } else {
                format!("{name} ← {old}")
            }
        }
    };
    let counts = match file.stat {
        Numstat::Lines { added, deleted } => format!("(+{added} -{deleted})"),
        Numstat::Binary => "(binary)".to_owned(),
    };
    (label, counts)
}

/// Terminal columns are u16; anything wider is clipped by the buffer anyway.
fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

fn split_path(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

#[derive(Default)]
struct Node {
    dirs: BTreeMap<String, Node>,
    /// (file name, index into files)
    files: Vec<(String, usize)>,
}

/// Groups `files` by folder: folders first (alphabetical), then files.
fn file_tree(files: &[FileChange]) -> Vec<TreeRow> {
    let mut root = Node::default();
    for (index, file) in files.iter().enumerate() {
        let mut node = &mut root;
        let mut parts: Vec<&str> = file.path.split('/').collect();
        let name = parts.pop().unwrap_or_default();
        for dir in parts {
            node = node.dirs.entry(dir.to_owned()).or_default();
        }
        node.files.push((name.to_owned(), index));
    }
    let mut rows = Vec::new();
    flatten(&mut root, 0, &mut rows);
    rows
}

fn flatten(node: &mut Node, depth: usize, rows: &mut Vec<TreeRow>) {
    for (name, child) in &mut node.dirs {
        rows.push(TreeRow::Dir {
            depth,
            name: name.clone(),
        });
        flatten(child, depth + 1, rows);
    }
    node.files.sort();
    rows.extend(node.files.iter().map(|(_, index)| TreeRow::File {
        depth,
        index: *index,
    }));
}

#[cfg(test)]
mod tests;
