//! The Source Control pane: a header with commit, refresh and more-actions
//! buttons, a multi-line commit message, a Commit button, and the changed
//! files split into "Staged Changes" and "Changes", like VS Code.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use tgg_model::{Change, ChangedFile};
use unicode_width::UnicodeWidthChar;

use crate::text::fit;
use crate::{Glyph, Key, TextField, Theme};

/// Lines the commit message shows; longer messages scroll.
const MESSAGE_LINES: u16 = 3;
/// Lines above the file list: header, message, Commit button, blank.
const TOP_LINES: u16 = 1 + MESSAGE_LINES + 2;

/// What a key or click asks the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceControlOutcome {
    Stay,
    /// Give the keyboard back to the graph.
    Leave,
    Commit(String),
    Stage(Vec<String>),
    Unstage(Vec<String>),
    /// Throw away unstaged changes: restore tracked `paths`, delete `untracked`.
    Discard {
        paths: Vec<String>,
        untracked: Vec<String>,
    },
    Refresh,
    /// Open the repository menu ("..."), anchored below the header button.
    More(Position),
    /// Show one file's diff (Enter, or a click on its row).
    OpenDiff(PaneFile),
    /// Show the diffs of every file of a section, one after the other
    /// ("Open Changes": `o`, or the header's button).
    OpenChanges {
        staged: bool,
        files: Vec<PaneFile>,
    },
    /// Open a file in the editor ("Open File": `e`, or the row's button).
    OpenFile(PaneFile),
}

/// A file as listed in one section of the pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneFile {
    pub path: String,
    /// Path before a rename.
    pub orig_path: Option<String>,
    /// What changed, in the section the file is listed in.
    pub change: Change,
    /// Listed under "Staged Changes" (the index against HEAD), not under
    /// "Changes" (the working tree against the index).
    pub staged: bool,
    /// The file exists in the working tree (it wasn't deleted).
    pub on_disk: bool,
}

/// Where the keyboard is inside the pane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Focus {
    #[default]
    Message,
    Commit,
    Files,
}

/// One line of the file list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    StagedHeader,
    Staged(usize),
    /// A blank line between the two sections; the keyboard skips it.
    Gap,
    ChangesHeader,
    Unstaged(usize),
}

/// What is under a cell of the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    HeaderCommit,
    HeaderRefresh,
    HeaderMore,
    Message,
    Commit,
    /// An item of the list.
    Row(usize),
    /// The `+` or `−` button of an item.
    Toggle(usize),
    /// The discard button of an item.
    Discard(usize),
    /// The "Open Changes" button of a section header.
    OpenChanges(usize),
    /// The "Open File" button of a file row.
    OpenFile(usize),
}

/// The pane's regions from the last frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceControlHitMap {
    regions: Vec<(Rect, Hit)>,
    cursor: Option<Position>,
    area: Rect,
}

impl SourceControlHitMap {
    /// Where the terminal cursor goes while the message is being typed.
    #[must_use]
    pub fn cursor(&self) -> Option<Position> {
        self.cursor
    }

    /// Whether a cell is inside the pane.
    #[must_use]
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.area.contains(Position { x, y })
    }

    fn hit(&self, x: u16, y: u16) -> Option<Hit> {
        self.regions
            .iter()
            .rev()
            .find(|(area, _)| area.contains(Position { x, y }))
            .map(|(_, hit)| *hit)
    }
}

/// The Source Control pane's state.
#[derive(Debug, Clone, Default)]
pub struct SourceControl {
    message: TextField,
    changes: Vec<ChangedFile>,
    branch: Option<String>,
    focus: Focus,
    /// Selected item while the file list has the keyboard.
    selected: usize,
    /// First item shown.
    offset: usize,
    /// First message line shown.
    message_offset: usize,
    /// Width of the message box in the last frame, for moving between lines.
    message_width: usize,
    /// Below the header's "..." button in the last frame.
    more_at: Position,
    /// Sections whose files are hidden, by clicking their header.
    staged_collapsed: bool,
    changes_collapsed: bool,
}

impl SourceControl {
    /// Replaces the listed files (after every `git status`).
    pub fn set_changes(&mut self, changes: Vec<ChangedFile>, branch: Option<String>) {
        self.changes = changes;
        self.branch = branch;
        self.selected = self.selected.min(self.items().len().saturating_sub(1));
    }

    #[must_use]
    pub fn message(&self) -> &str {
        self.message.text()
    }

    /// Empties the message, after a commit.
    pub fn clear_message(&mut self) {
        self.message = TextField::default();
        self.message_offset = 0;
    }

    /// Puts the keyboard in the message box.
    pub fn focus_message(&mut self) {
        self.focus = Focus::Message;
    }

    /// Indexes into `changes` of the files in the staged or the changes
    /// section, whether or not the section is collapsed.
    fn section(&self, staged: bool) -> Vec<usize> {
        (0..self.changes.len())
            .filter(|&i| {
                let file = &self.changes[i];
                if staged {
                    file.staged.is_some()
                } else {
                    file.unstaged.is_some()
                }
            })
            .collect()
    }

    fn items(&self) -> Vec<Item> {
        let mut items = Vec::new();
        let staged = self.section(true);
        let unstaged = self.section(false);
        if !staged.is_empty() {
            items.push(Item::StagedHeader);
            if !self.staged_collapsed {
                items.extend(staged.into_iter().map(Item::Staged));
            }
        }
        if !unstaged.is_empty() {
            if !items.is_empty() {
                items.push(Item::Gap);
            }
            items.push(Item::ChangesHeader);
            if !self.changes_collapsed {
                items.extend(unstaged.into_iter().map(Item::Unstaged));
            }
        }
        items
    }

    /// Collapses (`Some(true)`), expands (`Some(false)`) or flips (`None`)
    /// the section a header item heads; false when `item` isn't a header.
    fn collapse(&mut self, item: usize, collapsed: Option<bool>) -> bool {
        let flag = match self.items().get(item) {
            Some(Item::StagedHeader) => &mut self.staged_collapsed,
            Some(Item::ChangesHeader) => &mut self.changes_collapsed,
            _ => return false,
        };
        *flag = collapsed.unwrap_or(!*flag);
        true
    }

    pub fn handle_key(&mut self, key: Key) -> SourceControlOutcome {
        match self.focus {
            Focus::Message => match key {
                Key::Enter => return self.commit(),
                Key::Up => {
                    self.move_line(false);
                }
                // Down moves within the message, then on to the Commit button.
                Key::Down if !self.move_line(true) => self.focus = Focus::Commit,
                Key::Tab => self.focus = Focus::Commit,
                Key::Esc => return SourceControlOutcome::Leave,
                Key::Char(ch) => self.message.insert(ch),
                Key::Backspace => self.message.backspace(),
                Key::Delete => self.message.delete(),
                Key::Left => self.message.left(),
                Key::Right => self.message.right(),
                Key::Home => self.message.home(),
                Key::End => self.message.end(),
                _ => {}
            },
            Focus::Commit => match key {
                Key::Enter | Key::Char(' ') => return self.commit(),
                Key::Char('m') => return SourceControlOutcome::More(self.more_at),
                Key::Up | Key::BackTab => self.focus = Focus::Message,
                Key::Down | Key::Tab if !self.items().is_empty() => self.focus = Focus::Files,
                Key::Esc => return SourceControlOutcome::Leave,
                _ => {}
            },
            Focus::Files => {
                let items = self.items();
                let last = items.len().saturating_sub(1);
                let gap = |i: usize| items.get(i) == Some(&Item::Gap);
                match key {
                    Key::Up if self.selected == 0 => self.focus = Focus::Commit,
                    Key::Up => {
                        self.selected -= 1;
                        if gap(self.selected) {
                            self.selected -= 1;
                        }
                    }
                    Key::Down => {
                        let next = self.selected + 1;
                        self.selected = (if gap(next) { next + 1 } else { next }).min(last);
                    }
                    Key::Home => self.selected = 0,
                    Key::End => self.selected = last,
                    Key::Left => {
                        self.collapse(self.selected, Some(true));
                    }
                    Key::Right => {
                        self.collapse(self.selected, Some(false));
                    }
                    Key::Enter => return self.open(self.selected),
                    Key::Char(' ') => return self.toggle(self.selected),
                    Key::Char('o') => return self.open_changes(self.selected),
                    Key::Char('e') => return self.open_file(self.selected),
                    Key::Char('+' | 's') => return self.only(self.selected, true),
                    Key::Char('-' | 'u') => return self.only(self.selected, false),
                    Key::Char('d') | Key::Delete => return self.discard(self.selected),
                    Key::Char('m') => return SourceControlOutcome::More(self.more_at),
                    Key::BackTab => self.focus = Focus::Commit,
                    Key::Esc | Key::Tab => return SourceControlOutcome::Leave,
                    _ => {}
                }
            }
        }
        SourceControlOutcome::Stay
    }

    pub fn handle_click(
        &mut self,
        hits: &SourceControlHitMap,
        x: u16,
        y: u16,
    ) -> SourceControlOutcome {
        match hits.hit(x, y) {
            Some(Hit::HeaderCommit) => return self.commit(),
            Some(Hit::HeaderRefresh) => return SourceControlOutcome::Refresh,
            Some(Hit::HeaderMore) => return SourceControlOutcome::More(self.more_at),
            Some(Hit::Message) => self.focus = Focus::Message,
            Some(Hit::Commit) => {
                self.focus = Focus::Commit;
                return self.commit();
            }
            Some(Hit::Row(item)) => {
                self.select(item);
                // A file row opens its diff, a header collapses or expands.
                if !self.collapse(item, None) {
                    return self.open(item);
                }
            }
            Some(Hit::OpenChanges(item)) => {
                self.select(item);
                return self.open_changes(item);
            }
            Some(Hit::OpenFile(item)) => {
                self.select(item);
                return self.open_file(item);
            }
            Some(Hit::Toggle(item)) => {
                self.select(item);
                return self.toggle(item);
            }
            Some(Hit::Discard(item)) => {
                self.select(item);
                return self.discard(item);
            }
            None => {}
        }
        SourceControlOutcome::Stay
    }

    fn select(&mut self, item: usize) {
        self.focus = Focus::Files;
        self.selected = item;
    }

    fn commit(&self) -> SourceControlOutcome {
        SourceControlOutcome::Commit(self.message.text().to_owned())
    }

    /// Moves the message cursor one line up or down, keeping its column.
    /// False when there is no line that way.
    fn move_line(&mut self, down: bool) -> bool {
        let chars: Vec<char> = self.message.text().chars().collect();
        // Before the first frame there is no width yet: newlines only.
        let room = if self.message_width == 0 {
            usize::MAX
        } else {
            self.message_width
        };
        let lines = wrap(&chars, room);
        let row = cursor_row(&lines, self.message.cursor());
        let target = if down { row + 1 } else { row.wrapping_sub(1) };
        let Some(&(start, end)) = lines.get(target) else {
            return false;
        };
        let column = width(&chars[lines[row].0..self.message.cursor()]);
        let mut cursor = start;
        while cursor < end && width(&chars[start..=cursor]) <= column {
            cursor += 1;
        }
        self.message.set_cursor(cursor);
        true
    }

    /// The file an item lists, as its section sees it; None for headers.
    fn pane_file(&self, item: usize) -> Option<PaneFile> {
        let (i, staged) = match self.items().get(item)? {
            Item::Staged(i) => (*i, true),
            Item::Unstaged(i) => (*i, false),
            _ => return None,
        };
        self.file_in(i, staged)
    }

    /// `changes[i]` as the staged or the changes section lists it.
    fn file_in(&self, i: usize, staged: bool) -> Option<PaneFile> {
        let file = &self.changes[i];
        let change = if staged { file.staged } else { file.unstaged }?;
        let deleted = file.unstaged == Some(Change::Deleted)
            || (file.staged == Some(Change::Deleted) && file.unstaged.is_none());
        Some(PaneFile {
            path: file.path.clone(),
            orig_path: file.orig_path.clone(),
            change,
            staged,
            on_disk: !deleted,
        })
    }

    /// Enter: a file's diff, or a header's whole section.
    fn open(&self, item: usize) -> SourceControlOutcome {
        match self.pane_file(item) {
            Some(file) => SourceControlOutcome::OpenDiff(file),
            None => self.open_changes(item),
        }
    }

    /// The diffs of the section an item (a header or a file) is in.
    fn open_changes(&self, item: usize) -> SourceControlOutcome {
        let items = self.items();
        let staged = match items.get(item) {
            Some(Item::StagedHeader | Item::Staged(_)) => true,
            Some(Item::ChangesHeader | Item::Unstaged(_)) => false,
            Some(Item::Gap) | None => return SourceControlOutcome::Stay,
        };
        let files = self
            .section(staged)
            .into_iter()
            .filter_map(|i| self.file_in(i, staged))
            .collect();
        SourceControlOutcome::OpenChanges { staged, files }
    }

    /// A file row's file, to open in the editor.
    fn open_file(&self, item: usize) -> SourceControlOutcome {
        self.pane_file(item)
            .map_or(SourceControlOutcome::Stay, SourceControlOutcome::OpenFile)
    }

    /// Stages an unstaged item, unstages a staged one.
    fn toggle(&self, item: usize) -> SourceControlOutcome {
        match self.items().get(item) {
            Some(Item::StagedHeader | Item::Staged(_)) => self.only(item, false),
            Some(Item::ChangesHeader | Item::Unstaged(_)) => self.only(item, true),
            Some(Item::Gap) | None => SourceControlOutcome::Stay,
        }
    }

    /// The changes (`Item::Unstaged`) or staged files (`Item::Staged`) an item
    /// stands for: itself, or its whole section for a header.
    fn files_of(&self, item: usize, staged: bool) -> Option<Vec<usize>> {
        match (self.items().get(item)?, staged) {
            (Item::Unstaged(i), false) | (Item::Staged(i), true) => Some(vec![*i]),
            (Item::ChangesHeader, false) => Some(self.section(false)),
            (Item::StagedHeader, true) => Some(self.section(true)),
            _ => None,
        }
    }

    /// Stages (`stage`) or unstages the files of an item; Stay when the item
    /// is in the other section.
    fn only(&self, item: usize, stage: bool) -> SourceControlOutcome {
        let Some(files) = self.files_of(item, !stage) else {
            return SourceControlOutcome::Stay;
        };
        if stage {
            let paths = files
                .iter()
                .map(|&i| self.changes[i].path.clone())
                .collect();
            SourceControlOutcome::Stage(paths)
        } else {
            // A staged rename unstages both its new and its old path.
            let paths = files
                .iter()
                .flat_map(|&i| {
                    let file = &self.changes[i];
                    std::iter::once(file.path.clone()).chain(file.orig_path.clone())
                })
                .collect();
            SourceControlOutcome::Unstage(paths)
        }
    }

    /// Discards the unstaged changes of an item; Stay for staged items.
    fn discard(&self, item: usize) -> SourceControlOutcome {
        let Some(files) = self.files_of(item, false) else {
            return SourceControlOutcome::Stay;
        };
        let (untracked, tracked): (Vec<usize>, Vec<usize>) = files
            .into_iter()
            .partition(|&i| self.changes[i].unstaged == Some(Change::Untracked));
        let paths_of =
            |list: Vec<usize>| list.iter().map(|&i| self.changes[i].path.clone()).collect();
        SourceControlOutcome::Discard {
            paths: paths_of(tracked),
            untracked: paths_of(untracked),
        }
    }

    /// Draws the pane; `focused` when it has the keyboard.
    pub fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: bool,
    ) -> SourceControlHitMap {
        let mut hits = SourceControlHitMap {
            area,
            ..SourceControlHitMap::default()
        };
        if area.width < 10 || area.height == 0 {
            return hits;
        }
        self.render_header(area, buf, theme, &mut hits);
        if area.height < TOP_LINES {
            return hits;
        }
        self.render_message(area, buf, theme, focused, &mut hits);

        // Commit button, across the pane but the margins.
        let button = Rect::new(area.x + 1, area.y + 1 + MESSAGE_LINES, area.width - 2, 1);
        let mut style = Style::new()
            .fg(theme.badge_text(theme.lane(0)))
            .bg(theme.lane(0));
        if focused && self.focus == Focus::Commit {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        buf.set_style(button, style);
        let label = format!("{} Commit", theme.icons.glyph(Glyph::Check));
        let x = button.x + (button.width.saturating_sub(8)) / 2;
        buf.set_stringn(x, button.y, label, usize::from(button.width), style);
        hits.regions.push((button, Hit::Commit));

        // File list, scrolled so the selection stays visible.
        let items = self.items();
        let rows = usize::from(area.height - TOP_LINES);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if rows > 0 && self.selected >= self.offset + rows {
            self.offset = self.selected + 1 - rows;
        }
        for (row, (index, item)) in items
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(rows)
            .enumerate()
        {
            let y = area.y + TOP_LINES + u16::try_from(row).unwrap_or(u16::MAX);
            let row_area = Rect::new(area.x, y, area.width, 1);
            if focused && self.focus == Focus::Files && index == self.selected {
                buf.set_style(row_area, Style::new().bg(theme.selected_bg));
            }
            if *item == Item::Gap {
                continue;
            }
            hits.regions.push((row_area, Hit::Row(index)));
            self.draw_item(buf, row_area, index, *item, theme, &mut hits);
        }
        hits
    }

    /// `SOURCE CONTROL` with the commit, refresh and "..." buttons.
    fn render_header(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        hits: &mut SourceControlHitMap,
    ) {
        let style = Style::new()
            .fg(theme.text)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD);
        buf.set_style(Rect::new(area.x, area.y, area.width, 1), style);
        buf.set_stringn(
            area.x,
            area.y,
            " SOURCE CONTROL",
            usize::from(area.width),
            style,
        );
        let right = area.right();
        for (offset, glyph, hit) in [
            (6, Glyph::Check, Hit::HeaderCommit),
            (4, Glyph::Refresh, Hit::HeaderRefresh),
            (2, Glyph::More, Hit::HeaderMore),
        ] {
            let x = right - offset;
            buf[(x, area.y)].set_char(theme.icons.glyph(glyph));
            hits.regions.push((Rect::new(x, area.y, 1, 1), hit));
        }
        self.more_at = Position::new(right - 2, area.y + 1);
    }

    /// The multi-line commit message, or its placeholder.
    fn render_message(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: bool,
        hits: &mut SourceControlHitMap,
    ) {
        let input = Rect::new(area.x, area.y + 1, area.width, MESSAGE_LINES);
        let text_x = area.x + 1;
        let room = usize::from(area.width - 2);
        self.message_width = room;
        buf.set_style(input, Style::new().bg(theme.selected_bg));
        hits.regions.push((input, Hit::Message));
        let typing = focused && self.focus == Focus::Message;

        if self.message.text().is_empty() {
            let branch = self
                .branch
                .as_deref()
                .map_or_else(String::new, |b| format!(" on \"{b}\""));
            let muted = Style::new().fg(theme.muted);
            let hint = fit(&format!("Message (Enter commits{branch})"), room);
            buf.set_stringn(text_x, input.y, hint.trim_end(), room, muted);
            buf.set_stringn(text_x, input.y + 1, "Alt+Enter: new line", room, muted);
            if typing {
                hits.cursor = Some(Position::new(text_x, input.y));
            }
            return;
        }

        let chars: Vec<char> = self.message.text().chars().collect();
        let lines = wrap(&chars, room);
        let row = cursor_row(&lines, self.message.cursor());
        let shown = usize::from(MESSAGE_LINES);
        if row < self.message_offset {
            self.message_offset = row;
        } else if row >= self.message_offset + shown {
            self.message_offset = row + 1 - shown;
        }
        let style = Style::new().fg(theme.text);
        for (i, &(start, end)) in lines
            .iter()
            .skip(self.message_offset)
            .take(shown)
            .enumerate()
        {
            let y = input.y + u16::try_from(i).unwrap_or(u16::MAX);
            let line: String = chars[start..end].iter().collect();
            buf.set_stringn(text_x, y, line, room, style);
        }
        if typing {
            let column = width(&chars[lines[row].0..self.message.cursor()]);
            let y = input.y + u16::try_from(row - self.message_offset).unwrap_or(0);
            hits.cursor = Some(Position::new(
                text_x + u16::try_from(column).unwrap_or(u16::MAX),
                y,
            ));
        }
    }

    fn draw_item(
        &self,
        buf: &mut Buffer,
        area: Rect,
        index: usize,
        item: Item,
        theme: &Theme,
        hits: &mut SourceControlHitMap,
    ) {
        let text = Style::new().fg(theme.text);
        let muted = Style::new().fg(theme.muted);
        let icons = theme.icons;
        // Right side, VS Code order: buttons, then the status letter or count.
        let (left, tail, buttons): RowParts<'_> = match item {
            Item::Gap => return,
            Item::StagedHeader | Item::ChangesHeader => {
                let staged = item == Item::StagedHeader;
                let count = self.section(staged).len();
                let collapsed = if staged {
                    self.staged_collapsed
                } else {
                    self.changes_collapsed
                };
                let chevron = if collapsed { '▸' } else { '▾' };
                let name = if staged { "Staged Changes" } else { "Changes" };
                buf.set_stringn(
                    area.x,
                    area.y,
                    format!("{chevron} {name}"),
                    usize::from(area.width),
                    text.add_modifier(Modifier::BOLD),
                );
                let mut buttons = vec![(Glyph::OpenChanges, Hit::OpenChanges(index))];
                buttons.extend(stage_buttons(staged, index));
                (None, (count.to_string(), muted), buttons)
            }
            Item::Staged(i) | Item::Unstaged(i) => {
                let file = &self.changes[i];
                let staged = matches!(item, Item::Staged(_));
                let change = if staged { file.staged } else { file.unstaged };
                let (name, dir) = match file.path.rsplit_once('/') {
                    Some((dir, name)) => (name, dir),
                    None => (file.path.as_str(), ""),
                };
                let letter = change.map_or_else(
                    || (String::new(), muted),
                    |c| (c.letter().to_string(), Style::new().fg(theme.change(c))),
                );
                let mut buttons = vec![(Glyph::OpenFile, Hit::OpenFile(index))];
                buttons.extend(stage_buttons(staged, index));
                (Some((name, dir)), letter, buttons)
            }
        };

        // Lay the right side out from the right edge, one blank column in;
        // a blank cell follows every button, room for a Nerd Font icon
        // drawn wider than its cell.
        let tail_width = u16::try_from(unicode_width::UnicodeWidthStr::width(tail.0.as_str()))
            .unwrap_or(1)
            .max(1);
        let mut x = area.right().saturating_sub(1 + tail_width);
        buf.set_string(x, area.y, &tail.0, tail.1);
        for (glyph, hit) in buttons.iter().rev() {
            x = x.saturating_sub(2);
            buf[(x, area.y)]
                .set_char(icons.glyph(*glyph))
                .set_fg(theme.text);
            hits.regions.push((Rect::new(x, area.y, 1, 1), *hit));
        }

        if let Some((name, dir)) = left {
            let room = usize::from(x.saturating_sub(area.x + 3));
            let mut cx = area.x + 2;
            let name_width = unicode_width::UnicodeWidthStr::width(name);
            let name_fit = fit(name, room.min(name_width));
            cx = buf.set_stringn(cx, area.y, &name_fit, room, text).0;
            let used = usize::from(cx - area.x - 2);
            if !dir.is_empty() && used + 1 < room {
                buf.set_stringn(cx + 1, area.y, dir, room - used - 1, muted);
            }
        }
    }
}

/// The `−` of a staged row, or the `↶ +` of an unstaged one.
fn stage_buttons(staged: bool, index: usize) -> Vec<(Glyph, Hit)> {
    if staged {
        vec![(Glyph::Remove, Hit::Toggle(index))]
    } else {
        vec![
            (Glyph::Discard, Hit::Discard(index)),
            (Glyph::Add, Hit::Toggle(index)),
        ]
    }
}

/// A list row's file name and folder (none for headers), its status letter
/// or count with its style, and its buttons.
type RowParts<'a> = (
    Option<(&'a str, &'a str)>,
    (String, Style),
    Vec<(Glyph, Hit)>,
);

/// Splits text into the lines a `width`-column box shows: at newlines, and
/// wherever a line runs out of room. Each line is a char range, newline excluded.
fn wrap(chars: &[char], width: usize) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let (mut start, mut column) = (0, 0);
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '\n' {
            lines.push((start, i));
            (start, column) = (i + 1, 0);
            continue;
        }
        let w = ch.width().unwrap_or(0);
        if column + w > width && column > 0 {
            lines.push((start, i));
            (start, column) = (i, 0);
        }
        column += w;
    }
    lines.push((start, chars.len()));
    lines
}

/// The line the cursor (a char index) is on.
fn cursor_row(lines: &[(usize, usize)], cursor: usize) -> usize {
    lines
        .iter()
        .rposition(|&(start, _)| start <= cursor)
        .unwrap_or(0)
}

fn width(chars: &[char]) -> usize {
    chars.iter().map(|c| c.width().unwrap_or(0)).sum()
}

#[cfg(test)]
mod tests;
