use std::collections::BTreeSet;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::confirm::{MAX_WIDTH, centered, draw_frame};
use crate::{Key, TextField, Theme, fit};

/// One choice: a branch, remote branch, tag or remote, with an optional
/// kind badge (`branch`, `remote`, `tag`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    pub value: String,
    pub kind: Option<String>,
}

/// A row of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// Multi mode only: clears the selection (the "Show All" filter).
    ShowAll,
    /// Multi mode only: a glob (typed, or already selected) like `feature/*`.
    Pattern(String),
    /// Index into the items.
    Item(usize),
}

/// What a key press or click means for the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerOutcome {
    Pending,
    /// Single mode: the chosen value.
    Picked(String),
    /// Multi mode: the whole selection, sorted (names and globs). Empty
    /// means "Show All".
    PickedMany(Vec<String>),
    Cancelled,
}

/// What is under a screen cell, as of the last render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerHit {
    /// Index into [`Picker::rows`].
    Row(usize),
    Query,
    Frame,
    Outside,
}

/// Where the picker, its query line and rows were drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickerHitMap {
    area: Rect,
    query: Rect,
    rows: Vec<(Rect, usize)>,
    cursor: Option<Position>,
}

impl PickerHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> PickerHit {
        let point = Position { x, y };
        if let Some((_, row)) = self.rows.iter().find(|(r, _)| r.contains(point)) {
            PickerHit::Row(*row)
        } else if self.query.contains(point) {
            PickerHit::Query
        } else if self.area.contains(point) {
            PickerHit::Frame
        } else {
            PickerHit::Outside
        }
    }

    #[must_use]
    pub fn area(&self) -> Rect {
        self.area
    }

    #[must_use]
    pub fn query(&self) -> Rect {
        self.query
    }

    /// Where row `row` of [`Picker::rows`] was drawn, if visible.
    #[must_use]
    pub fn row(&self, row: usize) -> Option<Rect> {
        self.rows.iter().find(|(_, r)| *r == row).map(|(r, _)| *r)
    }

    /// The text cursor in the query line.
    #[must_use]
    pub fn cursor(&self) -> Option<Position> {
        self.cursor
    }
}

/// A filterable list popup (S2, S11): type to filter (substring, or a glob
/// with `*`/`?`), Up/Down to move, Enter to choose.
///
/// - Single mode (`Checkout to...`): Enter or a click picks one value.
/// - Multi mode (the branch filter): Space or a click toggles a row,
///   Enter applies the selection, "Show All" clears it, and a typed glob
///   can be selected as a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    title: String,
    items: Vec<PickerItem>,
    query: TextField,
    /// Index into the visible rows.
    cursor: usize,
    /// `Some` in multi mode.
    selected: Option<BTreeSet<String>>,
    anchor: Option<Position>,
}

impl Picker {
    #[must_use]
    pub fn single(title: &str, items: Vec<PickerItem>) -> Self {
        Self {
            title: title.to_owned(),
            items,
            query: TextField::new(""),
            cursor: 0,
            selected: None,
            anchor: None,
        }
    }

    /// Multi-select, starting from the current `selected` names and globs.
    #[must_use]
    pub fn multi(title: &str, items: Vec<PickerItem>, selected: &[String]) -> Self {
        Self {
            selected: Some(selected.iter().cloned().collect()),
            ..Self::single(title, items)
        }
    }

    /// Opens with the top-left corner at `anchor` (e.g. below a toolbar
    /// button) instead of centered.
    #[must_use]
    pub fn with_anchor(mut self, anchor: Position) -> Self {
        self.anchor = Some(anchor);
        self
    }

    #[must_use]
    pub fn query(&self) -> &str {
        self.query.text()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The current multi-mode selection, sorted (empty in single mode).
    #[must_use]
    pub fn selected(&self) -> Vec<String> {
        self.selected
            .as_ref()
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// The rows the query leaves visible.
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        let query = self.query.text();
        let is_glob = query.contains(['*', '?']);
        let mut rows = Vec::new();
        if let Some(selected) = &self.selected {
            rows.push(Row::ShowAll);
            // Selected globs and names that aren't items, so they can be
            // unselected, then the glob being typed.
            let known = |v: &String| self.items.iter().any(|i| &i.value == v);
            rows.extend(
                selected
                    .iter()
                    .filter(|v| !known(v))
                    .map(|v| Row::Pattern(v.clone())),
            );
            if is_glob && !selected.contains(query) {
                rows.push(Row::Pattern(query.to_owned()));
            }
        }
        let lower = query.to_lowercase();
        rows.extend(
            self.items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    if is_glob {
                        glob_match(query, &item.value)
                    } else {
                        item.value.to_lowercase().contains(&lower)
                    }
                })
                .map(|(i, _)| Row::Item(i)),
        );
        rows
    }

    /// Values of the visible item rows (not Show All or patterns).
    #[must_use]
    pub fn visible_values(&self) -> Vec<&str> {
        self.rows()
            .into_iter()
            .filter_map(|row| match row {
                Row::Item(i) => self.items.get(i).map(|item| item.value.as_str()),
                Row::ShowAll | Row::Pattern(_) => None,
            })
            .collect()
    }

    /// Up/Down move (PageUp/PageDown to the ends), typing edits the query,
    /// Enter chooses, Esc cancels. In multi mode Space toggles the row.
    pub fn handle_key(&mut self, key: Key) -> PickerOutcome {
        let last = self.rows().len().saturating_sub(1);
        match key {
            Key::Esc => return PickerOutcome::Cancelled,
            Key::Enter => return self.choose(),
            Key::Char(' ') if self.selected.is_some() => return self.toggle(self.cursor),
            Key::Up | Key::BackTab => self.cursor = self.cursor.saturating_sub(1),
            Key::Down | Key::Tab => self.cursor = (self.cursor + 1).min(last),
            Key::PageUp => self.cursor = 0,
            Key::PageDown => self.cursor = last,
            Key::Char(ch) => self.edit(|q| q.insert(ch)),
            Key::Backspace => self.edit(TextField::backspace),
            Key::Delete => self.edit(TextField::delete),
            Key::Left => self.query.left(),
            Key::Right => self.query.right(),
            Key::Home => self.query.home(),
            Key::End => self.query.end(),
        }
        PickerOutcome::Pending
    }

    /// A click on a row picks it (single) or toggles it (multi); a click
    /// outside cancels, like closing a dropdown.
    pub fn handle_click(&mut self, hits: &PickerHitMap, x: u16, y: u16) -> PickerOutcome {
        match hits.hit(x, y) {
            PickerHit::Row(row) => {
                self.cursor = row;
                if self.selected.is_some() {
                    self.toggle(row)
                } else {
                    self.choose()
                }
            }
            PickerHit::Query | PickerHit::Frame => PickerOutcome::Pending,
            PickerHit::Outside => PickerOutcome::Cancelled,
        }
    }

    fn edit(&mut self, change: impl FnOnce(&mut TextField)) {
        change(&mut self.query);
        self.cursor = 0;
    }

    fn choose(&self) -> PickerOutcome {
        let rows = self.rows();
        match (&self.selected, rows.get(self.cursor)) {
            (Some(_), Some(Row::ShowAll)) => PickerOutcome::PickedMany(Vec::new()),
            (Some(_), _) => PickerOutcome::PickedMany(self.selected()),
            (None, Some(Row::Item(i))) => {
                self.items.get(*i).map_or(PickerOutcome::Pending, |item| {
                    PickerOutcome::Picked(item.value.clone())
                })
            }
            (None, _) => PickerOutcome::Pending,
        }
    }

    /// Multi mode: flips a row; "Show All" clears and applies at once.
    fn toggle(&mut self, row: usize) -> PickerOutcome {
        let value = match self.rows().get(row) {
            Some(Row::ShowAll) => return PickerOutcome::PickedMany(Vec::new()),
            Some(Row::Pattern(pattern)) => pattern.clone(),
            Some(Row::Item(i)) => match self.items.get(*i) {
                Some(item) => item.value.clone(),
                None => return PickerOutcome::Pending,
            },
            None => return PickerOutcome::Pending,
        };
        if let Some(selected) = self.selected.as_mut()
            && !selected.remove(&value)
        {
            selected.insert(value);
        }
        PickerOutcome::Pending
    }

    /// At the anchor (clamped) or centered, tall enough for the rows.
    #[must_use]
    pub fn area(&self, screen: Rect) -> Rect {
        let width = MAX_WIDTH.min(screen.width);
        let rows = u16::try_from(self.rows().len()).unwrap_or(u16::MAX).max(1);
        // Border+title, query, separator, rows, footer, border.
        let height = rows.saturating_add(5).min(screen.height);
        match self.anchor {
            None => centered(screen, width, height),
            Some(anchor) => {
                let clamp = |v: u16, size: u16, low: u16, high: u16| {
                    v.clamp(low, high.saturating_sub(size).max(low))
                };
                Rect::new(
                    clamp(anchor.x, width, screen.x, screen.right()),
                    clamp(anchor.y, height, screen.y, screen.bottom()),
                    width,
                    height,
                )
            }
        }
    }

    pub fn render(&self, screen: Rect, buf: &mut Buffer, theme: &Theme) -> PickerHitMap {
        let area = self.area(screen);
        let mut hits = PickerHitMap {
            area,
            ..PickerHitMap::default()
        };
        if area.width < 6 || area.height < 5 {
            return hits;
        }
        let muted = Style::default().fg(theme.muted);
        let text = Style::default().fg(theme.text);
        draw_frame(buf, area, &self.title, muted);
        let inner = usize::from(area.width - 4);
        let x = area.x + 2;

        // Query line with the cursor after `> `.
        hits.query = Rect::new(x, area.y + 1, area.width - 4, 1);
        let (visible, column) = query_window(&self.query, inner.saturating_sub(2));
        buf.set_string(x, area.y + 1, "> ", muted);
        buf.set_string(
            x + 2,
            area.y + 1,
            fit(&visible, inner.saturating_sub(2)),
            text,
        );
        hits.cursor = Some(Position::new(x + 2 + column, area.y + 1));
        buf.set_string(
            area.x,
            area.y + 2,
            format!("├{}┤", "─".repeat(usize::from(area.width - 2))),
            muted,
        );

        let rows = self.rows();
        let visible_rows = usize::from(area.height - 5);
        let offset = self.cursor.saturating_sub(visible_rows.saturating_sub(1));
        for (n, (index, row)) in rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_rows)
            .enumerate()
        {
            let y = area.y + 3 + u16::try_from(n).unwrap_or(u16::MAX);
            let style = if index == self.cursor {
                text.bg(theme.selected_bg)
            } else {
                text
            };
            let (label, badge) = self.row_label(row);
            let badge_width = badge.width();
            let room = inner.saturating_sub(badge_width + usize::from(badge_width > 0));
            let line = format!(
                "{}{}{badge}",
                fit(&label, room),
                if badge_width > 0 { " " } else { "" }
            );
            buf.set_string(x, y, fit(&line, inner), style);
            if badge_width > 0 {
                let bx = x + u16::try_from(inner - badge_width).unwrap_or(0);
                buf.set_string(
                    bx,
                    y,
                    &badge,
                    muted.patch(Style::default().bg(style.bg.unwrap_or_default())),
                );
            }
            hits.rows.push((Rect::new(x, y, area.width - 4, 1), index));
        }

        let hint = if self.selected.is_some() {
            "Space: select · Enter: apply · Esc: cancel"
        } else {
            "Enter: choose · Esc: cancel"
        };
        buf.set_string(x, area.bottom() - 2, fit(hint, inner), muted);
        hits
    }

    fn row_label(&self, row: &Row) -> (String, String) {
        let check = |on: bool| if on { "[x] " } else { "[ ] " };
        let selected = self.selected.as_ref();
        match row {
            Row::ShowAll => {
                let on = selected.is_some_and(BTreeSet::is_empty);
                (format!("{}Show All", check(on)), String::new())
            }
            Row::Pattern(pattern) => {
                let on = selected.is_some_and(|s| s.contains(pattern));
                (format!("{}{pattern}", check(on)), "pattern".to_owned())
            }
            Row::Item(i) => {
                let Some(item) = self.items.get(*i) else {
                    return (String::new(), String::new());
                };
                let prefix = selected.map_or("", |s| check(s.contains(&item.value)));
                (
                    format!("{prefix}{}", item.value),
                    item.kind.clone().unwrap_or_default(),
                )
            }
        }
    }
}

/// `*` matches any run of characters (slashes too), `?` exactly one.
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    // Where the last `*` was, and how much text it has absorbed.
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// The part of the query that fits with the cursor visible, and the
/// cursor's column in it.
fn query_window(field: &TextField, columns: usize) -> (String, u16) {
    let chars: Vec<char> = field.text().chars().collect();
    let room = columns.saturating_sub(1);
    // Drop characters from the left until the cursor fits.
    let mut start = 0;
    let mut used = field.width_before_cursor();
    while used > room && start < field.cursor() {
        used -= UnicodeWidthChar::width(chars[start]).unwrap_or(0);
        start += 1;
    }
    let visible: String = chars[start..].iter().collect();
    (visible, u16::try_from(used).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests;
