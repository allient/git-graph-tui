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
            (Self::Nerd, Glyph::Check) => '\u{eab2}',
            (Self::Nerd, Glyph::Refresh) => '\u{eb37}',
            (Self::Nerd, Glyph::More) => '\u{ea7c}',
            (Self::Nerd, Glyph::Discard) => '\u{eae2}',
            (Self::Nerd, Glyph::Add) => '\u{ea60}',
            (Self::Nerd, Glyph::Remove) => '\u{eb3b}',
            // cod-folder, cod-file
            (Self::Nerd, Glyph::Folder) => '\u{ea83}',
            (Self::Nerd, Glyph::File) => '\u{ea7b}',
        }
    }
}
