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
}
