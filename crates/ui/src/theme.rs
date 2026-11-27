use ggt_model::Change;
use jiff::tz::TimeZone;
use ratatui::style::Color;

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

/// The same hues for a light terminal background: the bright ones (green,
/// orange, cyan, lime, yellow) are darkened so lines and dots stay visible
/// on white.
const LIGHT_LANES: [Color; 12] = [
    Color::Rgb(0x00, 0x85, 0xd9),
    Color::Rgb(0xd9, 0x00, 0x8f),
    Color::Rgb(0x00, 0xa6, 0x08),
    Color::Rgb(0xc2, 0x77, 0x00),
    Color::Rgb(0xa3, 0x00, 0xd9),
    Color::Rgb(0xe0, 0x00, 0x00),
    Color::Rgb(0x00, 0xa8, 0x9e),
    Color::Rgb(0xc9, 0x2f, 0xd0),
    Color::Rgb(0x5f, 0x96, 0x00),
    Color::Rgb(0xdc, 0x5b, 0x23),
    Color::Rgb(0x6f, 0x24, 0xd6),
    Color::Rgb(0xa6, 0x84, 0x00),
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
    /// Status-bar messages: green for what worked, orange for what needs
    /// attention (a refusal, a conflict), red for what failed.
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub icons: Icons,
    /// The frame of menus and dropdown lists.
    pub menu_border: Color,
    /// Where dates are shown: the user's local time zone in the app, UTC by
    /// default so tests and snapshots don't depend on the machine.
    pub time_zone: TimeZone,
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
        Self::dark()
    }
}

impl Theme {
    /// For terminals with a dark background: VS Code Git Graph's colors.
    #[must_use]
    pub fn dark() -> Self {
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
            success: Color::Rgb(0x00, 0xd9, 0x0a),
            warning: Color::Rgb(0xd9, 0x85, 0x00),
            error: Color::Rgb(0xff, 0x00, 0x00),
            icons: Icons::Unicode,
            menu_border: GIT_GRAPH_LANES[0],
            time_zone: TimeZone::UTC,
        }
    }

    /// For terminals with a light background: light gray selection and
    /// header, darker text colors.
    #[must_use]
    pub fn light() -> Self {
        // Dark gold: ANSI yellow is often unreadable on white.
        let gold = Color::Rgb(0xa6, 0x84, 0x00);
        Self {
            lanes: LIGHT_LANES.to_vec(),
            text: Color::Reset,
            muted: Color::Indexed(245),
            selected_bg: Color::Indexed(253),
            header_bg: Color::Indexed(255),
            badge_fg: Color::Black,
            badge_fg_on_dark: Color::White,
            badge_bg: Color::Indexed(252),
            tag: gold,
            stash: Color::Indexed(241),
            date: Color::Indexed(241),
            author: Color::Indexed(241),
            hash: Color::Indexed(244),
            // VS Code's light theme link color.
            link: Color::Rgb(0x00, 0x6a, 0xb1),
            added: Color::Green,
            modified: gold,
            deleted: Color::Red,
            renamed: Color::Blue,
            success: Color::Rgb(0x00, 0x8a, 0x07),
            warning: Color::Rgb(0xb3, 0x6b, 0x00),
            error: Color::Rgb(0xd7, 0x00, 0x00),
            icons: Icons::Unicode,
            menu_border: LIGHT_LANES[0],
            time_zone: TimeZone::UTC,
        }
    }
}

#[cfg(test)]
mod tests {
    use ggt_model::Change;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;

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

    /// Whether `color` is light enough that black text reads on it.
    fn is_light(theme: &Theme, color: Color) -> bool {
        theme.badge_text(color) == theme.badge_fg
    }

    #[test]
    fn the_default_theme_is_dark() {
        assert_eq!(Theme::default(), Theme::dark());
    }

    #[test]
    fn the_light_theme_uses_light_backgrounds() {
        let theme = Theme::light();
        assert!(is_light(&theme, theme.selected_bg));
        assert!(is_light(&theme, theme.header_bg));
        assert!(is_light(&theme, theme.badge_bg));
        let dark = Theme::dark();
        assert!(!is_light(&dark, dark.selected_bg));
        assert!(!is_light(&dark, dark.header_bg));
    }

    #[test]
    fn the_light_theme_has_no_text_colors_meant_for_dark_backgrounds() {
        let theme = Theme::light();
        let washed_out = [
            Color::Yellow,
            Color::LightGreen,
            Color::LightRed,
            Color::LightBlue,
            Color::Gray,
            Color::White,
        ];
        for color in [
            theme.muted,
            theme.tag,
            theme.stash,
            theme.date,
            theme.author,
            theme.hash,
            theme.added,
            theme.modified,
            theme.deleted,
            theme.renamed,
        ] {
            assert!(!washed_out.contains(&color), "{color:?}");
        }
        // Lane colors are lines and dots on a light background: none as
        // bright as the dark theme's yellow or lime.
        for lane in &theme.lanes {
            assert!(!is_light(&theme, *lane), "{lane:?}");
        }
        assert_eq!(theme.lanes.len(), Theme::dark().lanes.len());
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
