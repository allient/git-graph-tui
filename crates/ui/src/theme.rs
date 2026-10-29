use ratatui::style::Color;
use tgg_model::Change;

use crate::Icons;

/// VS Code Git Graph's default `git-graph.graph.colours`, in order: blue,
/// magenta, green, orange, purple, red, cyan, pink, lime, rust, violet,
/// yellow.
const GIT_GRAPH_LANES: [Color; 12] = [
    Color::Rgb(0x00, 0x85, 0xd9),
    Color::Rgb(0xd9, 0x00, 0x8f),
    Color::Rgb(0x00, 0xd9, 0x0a),
    Color::Rgb(0xd9, 0x85, 0x00),
    Color::Rgb(0xa3, 0x00, 0xd9),
    Color::Rgb(0xff, 0x00, 0x00),
    Color::Rgb(0x00, 0xd9, 0xcc),
    Color::Rgb(0xe1, 0x38, 0xe8),
    Color::Rgb(0x85, 0xd9, 0x00),
    Color::Rgb(0xdc, 0x5b, 0x23),
    Color::Rgb(0x6f, 0x24, 0xd6),
    Color::Rgb(0xff, 0xcc, 0x00),
];

/// Colors and icons used by the widgets. Lane colors cycle through `lanes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub lanes: Vec<Color>,
    pub text: Color,
    /// Commits that aren't ancestors of HEAD.
    pub muted: Color,
    pub selected_bg: Color,
    /// Background of the graph's column header.
    pub header_bg: Color,
    /// Text on colored badges (and on light RGB backgrounds).
    pub badge_fg: Color,
    /// Text on dark RGB badge backgrounds, such as most lane colors.
    pub badge_fg_on_dark: Color,
    /// Behind a ref badge's name when badges have icons; the icon sits on
    /// the lane color.
    pub badge_bg: Color,
    pub tag: Color,
    pub stash: Color,
    pub date: Color,
    pub author: Color,
    pub hash: Color,
    /// Clickable or address-like text: parent hashes, email addresses.
    pub link: Color,
    /// File names and status letters, by what happened to the file.
    pub added: Color,
    pub modified: Color,
    pub deleted: Color,
    pub renamed: Color,
    pub icons: Icons,
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

    /// Readable text on a badge colored `bg`: `badge_fg_on_dark` on a dark
    /// RGB color or a dark gray from the 256-color ramp, `badge_fg` on
    /// anything else.
    #[must_use]
    pub fn badge_text(&self, bg: Color) -> Color {
        match bg {
            Color::Rgb(r, g, b) => {
                // Perceived brightness (ITU-R BT.601 weights), 0..=255 000.
                let luma = 299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b);
                if luma < 128_000 {
                    self.badge_fg_on_dark
                } else {
                    self.badge_fg
                }
            }
            // 232..=255 runs from near black to near white.
            Color::Indexed(232..=243) => self.badge_fg_on_dark,
            _ => self.badge_fg,
        }
    }

    /// The color of a file `change`d this way: added green, modified
    /// yellow, deleted red, renamed or copied blue (with the default theme).
    #[must_use]
    pub fn change(&self, change: Change) -> Color {
        match change {
            Change::Modified | Change::TypeChanged => self.modified,
            Change::Added | Change::Untracked => self.added,
            Change::Deleted | Change::Conflicted => self.deleted,
            Change::Renamed | Change::Copied => self.renamed,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            lanes: GIT_GRAPH_LANES.to_vec(),
            text: Color::Reset,
            muted: Color::DarkGray,
            selected_bg: Color::Indexed(237),
            header_bg: Color::Indexed(235),
            badge_fg: Color::Black,
            badge_fg_on_dark: Color::White,
            badge_bg: Color::Indexed(238),
            tag: Color::Yellow,
            stash: Color::Gray,
            date: Color::Gray,
            author: Color::Gray,
            hash: Color::DarkGray,
            // VS Code's link color.
            link: Color::Rgb(0x37, 0x94, 0xff),
            added: Color::LightGreen,
            modified: Color::Yellow,
            deleted: Color::LightRed,
            renamed: Color::LightBlue,
            icons: Icons::Unicode,
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;
    use tgg_model::Change;

    use super::Theme;

    #[test]
    fn lanes_use_git_graphs_colors_in_order() {
        let expected = [
            Color::Rgb(0x00, 0x85, 0xd9),
            Color::Rgb(0xd9, 0x00, 0x8f),
            Color::Rgb(0x00, 0xd9, 0x0a),
            Color::Rgb(0xd9, 0x85, 0x00),
            Color::Rgb(0xa3, 0x00, 0xd9),
            Color::Rgb(0xff, 0x00, 0x00),
            Color::Rgb(0x00, 0xd9, 0xcc),
            Color::Rgb(0xe1, 0x38, 0xe8),
            Color::Rgb(0x85, 0xd9, 0x00),
            Color::Rgb(0xdc, 0x5b, 0x23),
            Color::Rgb(0x6f, 0x24, 0xd6),
            Color::Rgb(0xff, 0xcc, 0x00),
        ];
        assert_eq!(Theme::default().lanes, expected);
    }

    #[test]
    fn lane_colors_cycle() {
        let theme = Theme::default();
        assert_eq!(theme.lane(12), theme.lane(0));
        assert_eq!(theme.lane(13), theme.lane(1));
    }

    #[test]
    fn badge_text_is_dark_on_light_colors_and_light_on_dark_ones() {
        let theme = Theme::default();
        assert_eq!(theme.badge_text(Color::Rgb(0xff, 0xcc, 0x00)), Color::Black);
        assert_eq!(theme.badge_text(Color::Rgb(0x00, 0xd9, 0xcc)), Color::Black);
        assert_eq!(theme.badge_text(Color::Rgb(0x00, 0x85, 0xd9)), Color::White);
        assert_eq!(theme.badge_text(Color::Rgb(0x6f, 0x24, 0xd6)), Color::White);
        // Named colors keep the configured badge text.
        assert_eq!(theme.badge_text(Color::Yellow), theme.badge_fg);
    }

    #[test]
    fn badge_names_are_white_on_the_dark_badge_background() {
        let theme = Theme::default();
        assert_eq!(theme.badge_text(theme.badge_bg), Color::White);
        // The grayscale ramp: dark half white, light half black.
        assert_eq!(theme.badge_text(Color::Indexed(232)), Color::White);
        assert_eq!(theme.badge_text(Color::Indexed(250)), Color::Black);
    }

    #[test]
    fn changes_have_their_own_colors() {
        let theme = Theme::default();
        assert_eq!(theme.change(Change::Added), theme.added);
        assert_eq!(theme.change(Change::Untracked), theme.added);
        assert_eq!(theme.change(Change::Modified), theme.modified);
        assert_eq!(theme.change(Change::TypeChanged), theme.modified);
        assert_eq!(theme.change(Change::Deleted), theme.deleted);
        assert_eq!(theme.change(Change::Conflicted), theme.deleted);
        assert_eq!(theme.change(Change::Renamed), theme.renamed);
        assert_eq!(theme.change(Change::Copied), theme.renamed);
        assert_eq!(
            [theme.added, theme.modified, theme.deleted, theme.renamed],
            [
                Color::LightGreen,
                Color::Yellow,
                Color::LightRed,
                Color::LightBlue
            ]
        );
    }
}
