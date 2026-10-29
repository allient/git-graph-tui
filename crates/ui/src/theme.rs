use ratatui::style::Color;

/// Colors used by the widgets. Lane colors cycle through `lanes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub lanes: Vec<Color>,
    pub text: Color,
    /// Commits that aren't ancestors of HEAD.
    pub muted: Color,
    pub selected_bg: Color,
    /// Background of the graph's column header.
    pub header_bg: Color,
    /// Text on colored badges.
    pub badge_fg: Color,
    pub tag: Color,
    pub stash: Color,
    pub date: Color,
    pub author: Color,
    pub hash: Color,
}

impl Theme {
    /// Color for lane color index `index`.
    #[must_use]
    pub fn lane(&self, index: usize) -> Color {
        self.lanes
            .get(index % self.lanes.len().max(1))
            .copied()
            .unwrap_or(self.text)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            lanes: vec![
                Color::LightBlue,
                Color::LightMagenta,
                Color::LightGreen,
                Color::LightYellow,
                Color::LightCyan,
                Color::LightRed,
            ],
            text: Color::Reset,
            muted: Color::DarkGray,
            selected_bg: Color::Indexed(237),
            header_bg: Color::Indexed(235),
            badge_fg: Color::Black,
            tag: Color::Yellow,
            stash: Color::Gray,
            date: Color::Gray,
            author: Color::Gray,
            hash: Color::DarkGray,
        }
    }
}
