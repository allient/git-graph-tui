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
