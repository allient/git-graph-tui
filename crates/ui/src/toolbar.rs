use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;

use crate::{Key, Theme, fit};

/// Toolbar buttons, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarButton {
    BranchFilter,
    ShowRemotes,
    Refresh,
    Fetch,
    /// The repository "..." menu (S11).
    RepoMenu,
}

const ORDER: [ToolbarButton; 5] = [
    ToolbarButton::BranchFilter,
    ToolbarButton::ShowRemotes,
    ToolbarButton::Fetch,
    ToolbarButton::Refresh,
    ToolbarButton::RepoMenu,
];

/// What a key press or click means for the app. Popups open below their
/// button: pass the position to the picker or [`crate::PopupMenu::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarOutcome {
    Stay,
    /// Open the branch filter picker (multi-select).
    OpenBranchFilter(Position),
    ToggleRemotes,
    Refresh,
    Fetch,
    /// Open `menu_for(&Target::Repository, ..)`.
    OpenRepoMenu(Position),
    /// Keyboard focus leaves the toolbar (back to the graph).
    Leave,
}

/// Where each button was drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolbarHitMap {
    buttons: Vec<(Rect, ToolbarButton)>,
}

impl ToolbarHitMap {
    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> Option<ToolbarButton> {
        let point = Position { x, y };
        self.buttons
            .iter()
            .find(|(r, _)| r.contains(point))
            .map(|(_, b)| *b)
    }

    #[must_use]
    pub fn button(&self, button: ToolbarButton) -> Option<Rect> {
        self.buttons
            .iter()
            .find(|(_, b)| *b == button)
            .map(|(r, _)| *r)
    }
}

/// The one-line toolbar (S2): `Branches: [<filter> ▾]` and the
/// `[x] Show Remote Branches` toggle in the center, then Fetch, Refresh and
/// `...` on the right. On narrow terminals the labels shorten.
///
/// The filter and toggle state belong to the app; the toolbar only shows
/// them and reports what the user pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolbar {
    summary: String,
    show_remotes: bool,
    focus: Option<ToolbarButton>,
    /// Whether the "..." repository menu button is here (the Source Control
    /// pane shows it when visible).
    more: bool,
}

/// The dropdown text for the selected branches: `Show All` when none are,
/// the names when one or two are, otherwise a count.
#[must_use]
pub fn filter_summary(branches: &[String]) -> String {
    match branches {
        [] => "Show All".to_owned(),
        [_] | [_, _] => branches.join(", "),
        _ => format!("{} selected", branches.len()),
    }
}

impl Toolbar {
    #[must_use]
    pub fn new(branch_filter: &[String], show_remotes: bool) -> Self {
        Self {
            summary: filter_summary(branch_filter),
            show_remotes,
            focus: None,
            more: true,
        }
    }

    /// Leaves out the "..." button.
    #[must_use]
    pub fn without_more(mut self) -> Self {
        self.more = false;
        self
    }

    /// The buttons in keyboard order.
    fn buttons(&self) -> Vec<ToolbarButton> {
        ORDER
            .into_iter()
            .filter(|b| self.more || *b != ToolbarButton::RepoMenu)
            .collect()
    }

    #[must_use]
    pub fn focus(&self) -> Option<ToolbarButton> {
        self.focus
    }

    /// Gives the toolbar keyboard focus (`Some`) or takes it away (`None`).
    pub fn set_focus(&mut self, focus: Option<ToolbarButton>) {
        self.focus = focus;
    }

    /// Left/Right/Tab/Shift-Tab move between buttons, Enter or Space
    /// presses the focused one, Esc or Down leaves the toolbar. `hits` is
    /// the last render, so menus open below their button.
    pub fn handle_key(&mut self, key: Key, hits: &ToolbarHitMap) -> ToolbarOutcome {
        match key {
            Key::Right | Key::Tab => self.step(1),
            Key::Left | Key::BackTab => self.step(-1),
            Key::Enter | Key::Char(' ') => return self.activate(hits),
            Key::Esc | Key::Down => {
                self.focus = None;
                return ToolbarOutcome::Leave;
            }
            _ => {}
        }
        ToolbarOutcome::Stay
    }

    pub fn handle_click(&mut self, hits: &ToolbarHitMap, x: u16, y: u16) -> ToolbarOutcome {
        match hits.hit(x, y) {
            Some(button) => {
                self.focus = Some(button);
                self.activate(hits)
            }
            None => ToolbarOutcome::Stay,
        }
    }

    /// Presses the focused button.
    #[must_use]
    pub fn activate(&self, hits: &ToolbarHitMap) -> ToolbarOutcome {
        let below = |button| {
            hits.button(button)
                .map_or_else(Position::default, |r| Position::new(r.x, r.y + 1))
        };
        match self.focus {
            Some(ToolbarButton::BranchFilter) => {
                ToolbarOutcome::OpenBranchFilter(below(ToolbarButton::BranchFilter))
            }
            Some(ToolbarButton::ShowRemotes) => ToolbarOutcome::ToggleRemotes,
            Some(ToolbarButton::Refresh) => ToolbarOutcome::Refresh,
            Some(ToolbarButton::Fetch) => ToolbarOutcome::Fetch,
            Some(ToolbarButton::RepoMenu) => {
                ToolbarOutcome::OpenRepoMenu(below(ToolbarButton::RepoMenu))
            }
            None => ToolbarOutcome::Stay,
        }
    }

    fn step(&mut self, delta: isize) {
        let order = self.buttons();
        let current = self
            .focus
            .and_then(|f| order.iter().position(|b| *b == f))
            .unwrap_or(0);
        let len = order.len().cast_signed();
        let next = (current.cast_signed() + delta)
            .rem_euclid(len)
            .cast_unsigned();
        self.focus = Some(order[next]);
    }

    /// Draws the toolbar on the first row of `area`.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) -> ToolbarHitMap {
        let mut hits = ToolbarHitMap::default();
        if area.width == 0 || area.height == 0 {
            return hits;
        }
        let y = area.y;
        let right: Vec<(ToolbarButton, &str)> = [
            (ToolbarButton::Fetch, theme.icons.fetch()),
            (ToolbarButton::Refresh, theme.icons.refresh()),
            (ToolbarButton::RepoMenu, theme.icons.more()),
        ]
        .into_iter()
        .filter(|(b, _)| self.more || *b != ToolbarButton::RepoMenu)
        .collect();
        let right_width: usize = right.iter().map(|(_, l)| l.width() + 1).sum::<usize>() - 1;
        let width = usize::from(area.width);
        // One column of margin on each side.
        let right_start = width.saturating_sub(1 + right_width);
        let mark = if self.show_remotes { "[x]" } else { "[ ]" };

        let full = [
            format!("[{} ▾]", self.summary),
            format!("{mark} Show Remote Branches"),
        ];
        let full_width = 1 + "Branches: ".width() + full[0].width() + 2 + full[1].width();
        let (label, left) = if full_width + 2 <= right_start {
            ("Branches: ", full)
        } else {
            // Compact: no label, a short toggle, and the summary cut to fit.
            let toggle = format!("{mark} Remotes");
            let room = right_start.saturating_sub(1 + 2 + toggle.width() + 2 + "[ ▾]".width());
            (
                "",
                [
                    format!("[{} ▾]", fit(&self.summary, room).trim_end()),
                    toggle,
                ],
            )
        };

        // Centered like VS Code's, but never under the buttons on the right.
        let group = label.width() + left[0].width() + 2 + left[1].width();
        let start = (width.saturating_sub(group) / 2)
            .min(right_start.saturating_sub(group + 2))
            .max(1);

        let muted = Style::default().fg(theme.muted);
        let mut x = area.x + to_u16(start);
        buf.set_string(x, y, label, muted);
        x += to_u16(label.width());
        for ((button, text), gap) in [ToolbarButton::BranchFilter, ToolbarButton::ShowRemotes]
            .into_iter()
            .zip(left.iter())
            .zip([2, 0])
        {
            x = self.button(buf, &mut hits, (x, y), button, text, theme) + gap;
        }
        let mut x = area.x + to_u16(right_start);
        for (button, text) in right {
            x = self.button(buf, &mut hits, (x, y), button, text, theme) + 1;
        }
        hits
    }

    /// Draws one button and returns the column after it.
    fn button(
        &self,
        buf: &mut Buffer,
        hits: &mut ToolbarHitMap,
        (x, y): (u16, u16),
        button: ToolbarButton,
        text: &str,
        theme: &Theme,
    ) -> u16 {
        let style = if self.focus == Some(button) {
            Style::default().fg(theme.text).bg(theme.selected_bg)
        } else {
            Style::default().fg(theme.text)
        };
        let width = to_u16(text.width());
        buf.set_string(x, y, text, style);
        hits.buttons.push((Rect::new(x, y, width, 1), button));
        x + width
    }
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests;
