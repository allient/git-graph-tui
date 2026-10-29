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
