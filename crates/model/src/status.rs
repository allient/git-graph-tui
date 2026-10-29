/// Where HEAD points.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HeadState {
    /// On a branch (short name).
    Branch(String),
    /// Detached at a commit.
    Detached,
    /// A new repository with no commits yet.
    #[default]
    Unborn,
}

/// Summary of uncommitted changes, used for the "Uncommitted Changes (N)" row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingTreeStatus {
    pub head: HeadState,
    /// Distinct changed paths (staged, unstaged, untracked or conflicted).
    /// A file can be in several categories below but counts once here.
    pub files: usize,
    /// Files with staged changes.
    pub staged: usize,
    /// Tracked files with unstaged changes.
    pub unstaged: usize,
    pub untracked: usize,
    /// Files with merge conflicts.
    pub conflicted: usize,
    /// Each changed file, in `git status` order.
    pub changes: Vec<ChangedFile>,
}

/// A file with uncommitted changes, as the Source Control pane lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    /// The path before a rename or copy.
    pub orig_path: Option<String>,
    /// The change in the index (None when nothing is staged).
    pub staged: Option<Change>,
    /// The change in the working tree (None when it matches the index).
    pub unstaged: Option<Change>,
}

/// What happened to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}

impl Change {
    /// The status letter VS Code shows next to the file.
    #[must_use]
    pub fn letter(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Copied => 'C',
            Self::TypeChanged => 'T',
            Self::Untracked => 'U',
            Self::Conflicted => '!',
        }
    }
}

impl WorkingTreeStatus {
    /// Number of changes shown in the uncommitted row.
    #[must_use]
    pub fn changed_files(&self) -> usize {
        self.files
    }

    /// No uncommitted changes.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.changed_files() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_tree_has_no_changes() {
        let status = WorkingTreeStatus::default();
        assert_eq!(status.changed_files(), 0);
        assert!(status.is_clean());
    }

    #[test]
    fn counts_files_not_categories() {
        // One file staged and also modified again (`MM`) is one changed file.
        let status = WorkingTreeStatus {
            files: 1,
            staged: 1,
            unstaged: 1,
            ..Default::default()
        };
        assert_eq!(status.changed_files(), 1);
        assert!(!status.is_clean());
    }
}
