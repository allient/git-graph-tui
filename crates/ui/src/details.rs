use std::collections::BTreeMap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Clear, Widget};
use tgg_git::show::{CommitDetails, FileChange, Numstat};
use tgg_model::{Change, Oid};
use unicode_width::UnicodeWidthStr;

use crate::{Glyph, Key, Theme, fit, format_date};

/// Characters of a parent hash shown (and clickable) in the details.
const PARENT_HASH: usize = 8;
/// Width of the `Committer: ` label column.
const LABEL: usize = 11;
/// Lines of commit fields above the message (Commit to Date).
const FIELDS: usize = 5;
/// Keys for the selected file, beside the changed-files title.
const KEY_HINTS: &str = "enter diff  v view file  t difftool";
/// The same for a comparison, which has no single commit to view a file at.
const COMPARISON_HINTS: &str = "enter diff  t difftool  esc close";
/// Columns the side buttons take at the right edge: a blank, the icon and
/// the blank after it (a Codicon can draw wider than its cell).
const BUTTONS_WIDTH: u16 = 3;
/// Narrowest panel that still gets the side buttons.
const MIN_WIDTH_FOR_BUTTONS: u16 = 20;

/// How the changed files are listed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileLayout {
    /// Grouped into folders.
    #[default]
    Tree,
    /// One row per file, with its full path.
    List,
}

/// A button in the column at the panel's right edge, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsButton {
    Close,
    /// Filters the files by name (`f`).
    Find,
    Tree,
    List,
}

impl DetailsButton {
    const ALL: [Self; 4] = [Self::Close, Self::Find, Self::Tree, Self::List];

    fn glyph(self) -> Glyph {
        match self {
            Self::Close => Glyph::Close,
            Self::Find => Glyph::Find,
            Self::Tree => Glyph::TreeView,
            Self::List => Glyph::ListView,
        }
    }
}

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
    /// One of the side buttons.
    Button(DetailsButton),
    Frame,
    Outside,
}

/// Where the panel, its parent hashes, file rows and buttons were drawn, and
/// how far its two sides were scrolled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailsHitMap {
    area: Rect,
    files: Vec<(Rect, usize)>,
    parents: Vec<(Rect, Oid)>,
    issues: Vec<(Rect, String)>,
    buttons: Vec<(Rect, DetailsButton)>,
    /// The commit's fields and message, which scroll together.
    message: Scrolled,
    /// The file rows below the title.
    file_list: Scrolled,
}

/// A part of the panel that scrolls on its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Scrolled {
    area: Rect,
    offset: usize,
    max: usize,
}

impl Scrolled {
    /// The offset after scrolling by `lines` (negative is up), within bounds.
    fn moved(self, lines: isize) -> usize {
        self.offset.saturating_add_signed(lines).min(self.max)
    }
}

impl DetailsHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> DetailsHit {
        let point = Position { x, y };
        if let Some((_, button)) = self.buttons.iter().find(|(r, _)| r.contains(point)) {
            DetailsHit::Button(*button)
        } else if let Some((_, row)) = self.files.iter().find(|(r, _)| r.contains(point)) {
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

    /// Where `button` was drawn.
    #[must_use]
    pub fn button(&self, button: DetailsButton) -> Option<Rect> {
        self.buttons
            .iter()
            .find(|(_, b)| *b == button)
            .map(|(r, _)| *r)
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
/// the changed files as a folder tree (or a flat list) with `(+a | -d)`
/// counts on the right, and a column of buttons at the right edge (close,
/// find, tree, list). Both sides scroll on their own (S24).
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
    layout: FileLayout,
    /// Only files whose path contains this (ignoring case) are listed.
    filter: String,
    /// Keys type into the filter.
    filtering: bool,
    /// First file row shown; None keeps the selection in view.
    files_scroll: Option<usize>,
    /// First line of the fields and message shown.
    message_scroll: usize,
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
        let mut view = Self {
            header,
            files,
            rows: Vec::new(),
            selected: None,
            issue_prefix: "#".into(),
            layout: FileLayout::Tree,
            filter: String::new(),
            filtering: false,
            files_scroll: None,
            message_scroll: 0,
        };
        view.rebuild_rows();
        view
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

    /// The rows listed: folders and files in the tree layout, files in the
    /// list layout; only those matching the filter.
    #[must_use]
    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    #[must_use]
    pub fn layout(&self) -> FileLayout {
        self.layout
    }

    /// The text the files are filtered by ("" lists them all).
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Whether keys type into the filter (after `f` or the find button).
    #[must_use]
    pub fn is_filtering(&self) -> bool {
        self.filtering
    }

    #[must_use]
    pub fn selected_file(&self) -> Option<&FileChange> {
        match self.rows.get(self.selected?)? {
            TreeRow::File { index, .. } => self.files.get(*index),
            TreeRow::Dir { .. } => None,
        }
    }

    /// Up/Down/Home/End/PageUp/PageDown move between files, Enter opens the
    /// selected one's diff, `v` shows its content, `f` filters the files by
    /// name, `l` switches between the tree and the list, Esc clears the
    /// filter or closes the panel. While filtering, keys type into the
    /// filter; Enter keeps it and Esc clears it.
    pub fn handle_key(&mut self, key: Key) -> DetailsOutcome {
        if self.filtering {
            return self.filter_key(key);
        }
        match key {
            Key::Down => self.step(true),
            Key::Up => self.step(false),
            Key::Home | Key::PageUp => {
                let first = self.file_rows().next();
                self.select_row(first);
            }
            Key::End | Key::PageDown => {
                let last = self.file_rows().next_back();
                self.select_row(last);
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
            Key::Char('f') => self.filtering = true,
            Key::Char('l') => {
                let other = match self.layout {
                    FileLayout::Tree => FileLayout::List,
                    FileLayout::List => FileLayout::Tree,
                };
                self.set_layout(other);
            }
            Key::Esc if !self.filter.is_empty() => self.clear_filter(),
            Key::Esc => return DetailsOutcome::Close,
            _ => {}
        }
        DetailsOutcome::Stay
    }

    fn filter_key(&mut self, key: Key) -> DetailsOutcome {
        match key {
            Key::Char(c) => {
                self.filter.push(c);
                self.rebuild_rows();
            }
            Key::Backspace if self.filter.pop().is_some() => self.rebuild_rows(),
            Key::Enter => self.filtering = false,
            Key::Esc => self.clear_filter(),
            Key::Down => self.step(true),
            Key::Up => self.step(false),
            _ => {}
        }
        DetailsOutcome::Stay
    }

    fn clear_filter(&mut self) {
        self.filtering = false;
        if !self.filter.is_empty() {
            self.filter.clear();
            self.rebuild_rows();
        }
    }

    fn set_layout(&mut self, layout: FileLayout) {
        if self.layout != layout {
            self.layout = layout;
            self.rebuild_rows();
        }
    }

    /// A click on a file selects and opens it; a click on a parent hash
    /// jumps to that commit; a click on an issue reference opens it; the
    /// side buttons close the panel, start the filter, or switch the layout.
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
            DetailsHit::Button(DetailsButton::Close) => DetailsOutcome::Close,
            DetailsHit::Button(DetailsButton::Find) => {
                self.filtering = true;
                DetailsOutcome::Stay
            }
            DetailsHit::Button(DetailsButton::Tree) => {
                self.set_layout(FileLayout::Tree);
                DetailsOutcome::Stay
            }
            DetailsHit::Button(DetailsButton::List) => {
                self.set_layout(FileLayout::List);
                DetailsOutcome::Stay
            }
            DetailsHit::Frame | DetailsHit::Outside => DetailsOutcome::Stay,
        }
    }

    /// The mouse wheel at a cell: scrolls the side under it by `lines`
    /// (negative is up), the graph stays put. False when the cell is outside
    /// the panel.
    pub fn scroll(&mut self, hits: &DetailsHitMap, x: u16, y: u16, lines: isize) -> bool {
        let point = Position { x, y };
        if hits.message.area.contains(point) {
            self.scroll_message(hits, lines);
        } else if hits.file_list.area.contains(point) {
            self.files_scroll = Some(hits.file_list.moved(lines));
        }
        hits.area.contains(point)
    }

    /// Scrolls the commit's fields and message by `lines` (negative is up).
    pub fn scroll_message(&mut self, hits: &DetailsHitMap, lines: isize) {
        self.message_scroll = hits.message.moved(lines);
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
        let content = if area.width >= MIN_WIDTH_FOR_BUTTONS {
            self.render_buttons(area, buf, theme, &mut hits);
            Rect {
                width: area.width - BUTTONS_WIDTH,
                ..area
            }
        } else {
            area
        };
        let Header::Commit(details) = &self.header else {
            self.render_files(content, buf, theme, &mut hits);
            return hits;
        };
        let left = Rect {
            width: content.width / 2,
            ..content
        };
        let right = Rect {
            x: left.right() + 1,
            width: content.width - left.width - 1,
            ..content
        };
        for y in area.top()..area.bottom() {
            buf.set_string(left.right(), y, "│", Style::default().fg(theme.muted));
        }
        self.render_left(details, left, buf, theme, &mut hits);
        self.render_files(right, buf, theme, &mut hits);
        hits
    }

    /// The side buttons, one per line from the top, at the right edge. The
    /// layout shown and an active filter are highlighted.
    fn render_buttons(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        hits: &mut DetailsHitMap,
    ) {
        let x = area.right() - 2;
        let normal = Style::default().fg(theme.text);
        let active = normal.bg(theme.selected_bg);
        for (y, button) in (area.top()..area.bottom()).zip(DetailsButton::ALL) {
            let on = match button {
                DetailsButton::Close => false,
                DetailsButton::Find => self.filtering || !self.filter.is_empty(),
                DetailsButton::Tree => self.layout == FileLayout::Tree,
                DetailsButton::List => self.layout == FileLayout::List,
            };
            let style = if on { active } else { normal };
            let icon = theme.icons.glyph(button.glyph());
            // The blank cell after the icon, so a wide Codicon doesn't
            // overlap what follows.
            buf.set_string(x, y, format!("{icon} "), style);
            hits.buttons.push((Rect::new(x, y, 2, 1), button));
        }
    }

    /// The commit's fields and message in `area`, scrolled as one text.
    fn render_left(
        &self,
        details: &CommitDetails,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        hits: &mut DetailsHitMap,
    ) {
        // Fields, a blank line, then the message: drawn whole off screen,
        // then the visible lines are copied, so drawing needn't know about
        // scrolling.
        let total = to_u16(FIELDS + 1 + details.message.lines().count());
        let full = Rect::new(area.x, 0, area.width, total);
        let mut scratch = Buffer::empty(full);
        let mut drawn = DetailsHitMap::default();
        let message_y = Self::render_metadata(details, full, &mut scratch, theme, &mut drawn);
        let message = Rect {
            y: message_y,
            height: full.bottom().saturating_sub(message_y),
            ..full
        };
        self.render_message(&details.message, message, &mut scratch, theme, &mut drawn);

        let max = usize::from(total.saturating_sub(area.height));
        let offset = self.message_scroll.min(max);
        hits.message = Scrolled { area, offset, max };
        let first = to_u16(offset);
        for (y, from) in (area.top()..area.bottom()).zip(first..total) {
            for x in area.left()..area.right() {
                buf[(x, y)] = scratch[(x, from)].clone();
            }
        }
        let shown = |r: Rect| -> Option<Rect> {
            let y = r.y.checked_sub(first).filter(|&dy| dy < area.height)?;
            Some(Rect { y: area.y + y, ..r })
        };
        hits.parents.extend(
            drawn
                .parents
                .into_iter()
                .filter_map(|(r, id)| Some((shown(r)?, id))),
        );
        hits.issues.extend(
            drawn
                .issues
                .into_iter()
                .filter_map(|(r, number)| Some((shown(r)?, number))),
        );
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
        let filtered = self.filtering || !self.filter.is_empty();
        let shown = self.file_rows().count();
        let counted = if filtered {
            format!("{shown} of {count}")
        } else {
            count.to_string()
        };
        let (mut title, keys) = match &self.header {
            Header::Commit(_) => (format!("Changed files ({counted})"), KEY_HINTS),
            Header::Comparison(title) => {
                let plural = if count == 1 { "" } else { "s" };
                let title = format!("{title} ({counted} changed file{plural})");
                (title, COMPARISON_HINTS)
            }
        };
        if filtered {
            let cursor = if self.filtering { "▏" } else { "" };
            title = format!("{title}  find: {}{cursor}", self.filter);
        }
        let muted = Style::default().fg(theme.muted);
        buf.set_string(area.x + 1, area.y, fit(&title, width), muted);
        // Key hints at the right of the title row, when they fit.
        if count > 0 && !filtered && title.width() + 2 + keys.width() <= width {
            let x = area.right().saturating_sub(to_u16(keys.width()) + 1);
            buf.set_string(x, area.y, keys, muted);
        }
        let visible = usize::from(area.height.saturating_sub(1));
        let max = self.rows.len().saturating_sub(visible);
        // Scrolled by the wheel, or so the selected file stays visible.
        let offset = self
            .files_scroll
            .unwrap_or_else(|| {
                self.selected
                    .map_or(0, |s| s.saturating_sub(visible.saturating_sub(1)))
            })
            .min(max);
        hits.file_list = Scrolled {
            area: Rect {
                y: area.y + 1,
                height: area.height.saturating_sub(1),
                ..area
            },
            offset,
            max,
        };
        for (n, (row_index, row)) in self
            .rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .enumerate()
        {
            let y = area.y + 1 + to_u16(n);
            match row {
                TreeRow::Dir { depth, name } => {
                    let folder = theme.icons.glyph(Glyph::Folder);
                    let label = format!("{}{folder} {name}/", "  ".repeat(*depth));
                    buf.set_string(area.x + 1, y, fit(&label, width), muted);
                }
                TreeRow::File { depth, index } => {
                    let Some(file) = self.files.get(*index) else {
                        continue;
                    };
                    let selected = self.selected == Some(row_index);
                    self.render_file(file, *depth, selected, area, y, buf, theme);
                    hits.files
                        .push((Rect::new(area.x, y, area.width, 1), row_index));
                }
            }
        }
    }

    /// One file row: its icon, its name in the color of what happened to it
    /// (S21), then its counts, added lines green and deleted ones red (S25).
    #[expect(
        clippy::too_many_arguments,
        reason = "one row of render_files, which owns the layout"
    )]
    fn render_file(
        &self,
        file: &FileChange,
        depth: usize,
        selected: bool,
        area: Rect,
        y: u16,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        let width = usize::from(area.width.saturating_sub(1));
        let x = area.x + 1;
        let right = x + to_u16(width);
        let mut base = Style::default().fg(theme.text);
        if selected {
            base = base.bg(theme.selected_bg);
        }
        buf.set_string(x, y, " ".repeat(width), base);
        let name = match self.layout {
            FileLayout::Tree => file_label(file).0,
            FileLayout::List => list_label(file),
        };
        let prefix = format!("{}{} ", "  ".repeat(depth), theme.icons.glyph(Glyph::File));
        let counts = count_parts(file, theme);
        let counts_width: usize = counts.iter().map(|(text, _)| text.width()).sum();
        let room = width
            .saturating_sub(prefix.width())
            .saturating_sub(if counts_width > 0 {
                counts_width + 1
            } else {
                0
            });
        let name = if name.width() > room {
            fit(&name, room)
        } else {
            name
        };
        let mut at = put(buf, x, y, right, &prefix, base);
        at = put(buf, at, y, right, &name, base.fg(theme.change(file.status)));
        if !counts.is_empty() {
            at = put(buf, at, y, right, " ", base);
            for (text, color) in counts {
                at = put(buf, at, y, right, &text, base.fg(color));
            }
        }
    }

    fn file_rows(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        (0..self.rows.len()).filter(|&i| matches!(self.rows[i], TreeRow::File { .. }))
    }

    /// Selects tree row `row` and scrolls the file list back to it.
    fn select_row(&mut self, row: Option<usize>) {
        self.selected = row;
        self.files_scroll = None;
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
        if next.is_some() {
            self.select_row(next);
        }
    }

    /// Lists the files matching the filter in the current layout, keeping
    /// the selected file selected when it is still listed.
    fn rebuild_rows(&mut self) {
        let kept = self.selected_index();
        let needle = self.filter.to_lowercase();
        let shown = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.path.to_lowercase().contains(&needle));
        self.rows = match self.layout {
            FileLayout::Tree => file_tree(shown),
            FileLayout::List => {
                let mut files: Vec<_> = shown.collect();
                files.sort_by(|a, b| a.1.path.cmp(&b.1.path));
                files
                    .into_iter()
                    .map(|(index, _)| TreeRow::File { depth: 0, index })
                    .collect()
            }
        };
        let row_of = |index: usize| {
            self.rows
                .iter()
                .position(|r| matches!(r, TreeRow::File { index: i, .. } if *i == index))
        };
        let row = kept.and_then(row_of).or_else(|| self.file_rows().next());
        self.select_row(row);
    }

    /// Index into `files` of the selected file.
    fn selected_index(&self) -> Option<usize> {
        match self.rows.get(self.selected?)? {
            TreeRow::File { index, .. } => Some(*index),
            TreeRow::Dir { .. } => None,
        }
    }
}

/// A file's tree label and its counts: `name` (with rename info) and
/// `(+a -d)`, as the diff header shows them. The details panel draws its
/// counts in color instead (see `count_parts`).
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

/// A file's label in the list layout: its full path.
fn list_label(file: &FileChange) -> String {
    match file.old_path.as_deref() {
        None => file.path.clone(),
        Some(old) => format!("{old} → {}", file.path),
    }
}

/// The parts of a file's counts and their colors: `(`, `+a` green, ` | `,
/// `-d` red, `)`. None for a new file.
fn count_parts(file: &FileChange, theme: &Theme) -> Vec<(String, Color)> {
    if file.status == Change::Added {
        return Vec::new();
    }
    match file.stat {
        Numstat::Lines { added, deleted } => vec![
            ("(".to_owned(), theme.text),
            (format!("+{added}"), theme.added),
            (" | ".to_owned(), theme.text),
            (format!("-{deleted}"), theme.deleted),
            (")".to_owned(), theme.text),
        ],
        Numstat::Binary => vec![("(binary)".to_owned(), theme.text)],
    }
}

/// Writes `text` from `x` up to (not including) `right`; returns the next x.
fn put(buf: &mut Buffer, x: u16, y: u16, right: u16, text: &str, style: Style) -> u16 {
    if x >= right {
        return x;
    }
    buf.set_stringn(x, y, text, usize::from(right - x), style).0
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

/// Groups `files` (with their indices) by folder: folders first
/// (alphabetical), then files.
fn file_tree<'a>(files: impl Iterator<Item = (usize, &'a FileChange)>) -> Vec<TreeRow> {
    let mut root = Node::default();
    for (index, file) in files {
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
