use crate::Oid;

/// A stash entry (`stash@{index}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stash {
    /// The stash commit.
    pub id: Oid,
    /// Position in the stash list: 0 is the newest (`stash@{0}`).
    pub index: usize,
    /// Stash message, e.g. `On main: WIP`.
    pub subject: String,
}

impl Stash {
    /// The selector git understands, e.g. `stash@{0}`.
    #[must_use]
    pub fn selector(&self) -> String {
        format!("stash@{{{}}}", self.index)
    }
}
