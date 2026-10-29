use crate::BadgeKind;

/// The characters toolbar buttons are drawn with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Icons {
    /// Plain Unicode symbols, which any font has.
    #[default]
    Unicode,
    /// VS Code's Codicons, from a Nerd Font the user installed.
    Nerd,
}

impl Icons {
    #[must_use]
    pub fn refresh(self) -> &'static str {
        match self {
            Self::Unicode => "[↻]",
            Self::Nerd => " \u{eb37} ",
        }
    }

    #[must_use]
    pub fn fetch(self) -> &'static str {
        match self {
            Self::Unicode => "[⇣]",
            Self::Nerd => " \u{eac2} ",
        }
    }

    #[must_use]
    pub fn more(self) -> &'static str {
        match self {
            Self::Unicode => "[...]",
            Self::Nerd => " \u{ea7c} ",
        }
    }
}

impl Icons {
    /// The icon at the start of a ref badge in the graph. Plain Unicode
    /// badges have none.
    #[must_use]
    pub fn ref_badge(self, kind: BadgeKind) -> Option<char> {
        match self {
            Self::Unicode => None,
            Self::Nerd => Some(match kind {
                // cod-tag and cod-git_stash
                BadgeKind::Tag => '\u{ea66}',
                BadgeKind::Stash => '\u{ec26}',
                // cod-git_branch
                _ => '\u{ec6f}',
            }),
        }
    }

    /// The rounded left and right ends of a ref badge (Powerline half
    /// circles). Plain Unicode badges are square.
    #[must_use]
    pub fn badge_ends(self) -> Option<(char, char)> {
        match self {
            Self::Unicode => None,
            Self::Nerd => Some(('\u{e0b6}', '\u{e0b4}')),
        }
    }
}

/// A small icon: a button in the Source Control pane, or a row of the
/// details panel's file tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Check,
    Refresh,
    More,
    Discard,
    Add,
    Remove,
    /// A folder row of a file tree.
    Folder,
    /// A file row of a file tree.
    File,
    /// Opens the diffs of a whole Source Control section ("Open Changes").
    OpenChanges,
    /// Opens a changed file in the editor ("Open File").
    OpenFile,
    /// Closes the details panel.
    Close,
    /// Filters the details panel's files by name.
    Find,
    /// Shows the details panel's files as a folder tree.
    TreeView,
    /// Shows the details panel's files as a flat list of paths.
    ListView,
}

impl Icons {
    /// One cell wide in either style.
    #[must_use]
    pub fn glyph(self, glyph: Glyph) -> char {
        match (self, glyph) {
            (Self::Unicode, Glyph::Check) => '✓',
            (Self::Unicode, Glyph::Refresh) => '↻',
            (Self::Unicode, Glyph::More) => '…',
            (Self::Unicode, Glyph::Discard) => '↶',
            (Self::Unicode, Glyph::Add) => '+',
            (Self::Unicode, Glyph::Remove) => '−',
            (Self::Unicode, Glyph::Folder) => '▾',
            (Self::Unicode, Glyph::File) => '·',
            (Self::Unicode, Glyph::OpenChanges) => '⧉',
            (Self::Unicode, Glyph::OpenFile) => '↗',
            (Self::Unicode, Glyph::Close) => '✕',
            (Self::Unicode, Glyph::Find) => '⌕',
            (Self::Unicode, Glyph::TreeView) => '├',
            (Self::Unicode, Glyph::ListView) => '≡',
            (Self::Nerd, Glyph::Check) => '\u{eab2}',
            (Self::Nerd, Glyph::Refresh) => '\u{eb37}',
            (Self::Nerd, Glyph::More) => '\u{ea7c}',
            (Self::Nerd, Glyph::Discard) => '\u{eae2}',
            (Self::Nerd, Glyph::Add) => '\u{ea60}',
            (Self::Nerd, Glyph::Remove) => '\u{eb3b}',
            // cod-folder, cod-file
            (Self::Nerd, Glyph::Folder) => '\u{ea83}',
            (Self::Nerd, Glyph::File) => '\u{ea7b}',
            // cod-diff_multiple and cod-go_to_file, as in VS Code.
            (Self::Nerd, Glyph::OpenChanges) => '\u{ec23}',
            (Self::Nerd, Glyph::OpenFile) => '\u{ea94}',
            // cod-close, cod-search, cod-list_tree and cod-list_flat, the
            // buttons beside VS Code Git Graph's commit details.
            (Self::Nerd, Glyph::Close) => '\u{ea76}',
            (Self::Nerd, Glyph::Find) => '\u{ea6d}',
            (Self::Nerd, Glyph::TreeView) => '\u{eb86}',
            (Self::Nerd, Glyph::ListView) => '\u{eb84}',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_changes_and_open_file_have_both_styles() {
        assert_eq!(Icons::Unicode.glyph(Glyph::OpenChanges), '⧉');
        assert_eq!(Icons::Unicode.glyph(Glyph::OpenFile), '↗');
        assert_eq!(Icons::Nerd.glyph(Glyph::OpenChanges), '\u{ec23}');
        assert_eq!(Icons::Nerd.glyph(Glyph::OpenFile), '\u{ea94}');
    }

    #[test]
    fn the_details_buttons_have_one_cell_unicode_fallbacks() {
        use unicode_width::UnicodeWidthChar;
        for glyph in [Glyph::Close, Glyph::Find, Glyph::TreeView, Glyph::ListView] {
            let ch = Icons::Unicode.glyph(glyph);
            assert_eq!(ch.width(), Some(1), "{glyph:?} {ch}");
        }
        assert_eq!(Icons::Nerd.glyph(Glyph::Close), '\u{ea76}');
        assert_eq!(Icons::Nerd.glyph(Glyph::Find), '\u{ea6d}');
        assert_eq!(Icons::Nerd.glyph(Glyph::TreeView), '\u{eb86}');
        assert_eq!(Icons::Nerd.glyph(Glyph::ListView), '\u{eb84}');
    }
}
