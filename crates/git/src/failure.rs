/// Why a git command failed, classified from its stderr so the UI can show a
/// clear message. The full stderr is always kept for the git log view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    NotARepository,
    LocalChangesWouldBeOverwritten,
    Conflict,
    NonFastForward,
    AuthenticationFailed,
    AlreadyExists,
    NotFullyMerged,
    /// A rebase todo list leaves out commits and `missingCommitsCheck=error`
    /// stopped the rebase before it replayed anything.
    CommitsMissingFromTodo,
    /// Anything else: the first `fatal:` / `error:` line (or first non-hint line).
    Other(String),
}

impl Failure {
    /// Classifies git's stderr.
    #[must_use]
    pub fn classify(stderr: &str) -> Self {
        let has = |needle: &str| stderr.contains(needle);
        if has("not a git repository") {
            Self::NotARepository
        } else if has("would be overwritten by") {
            Self::LocalChangesWouldBeOverwritten
        } else if has("some commits may have been dropped accidentally") {
            Self::CommitsMissingFromTodo
        } else if has("CONFLICT (") || has("fix conflicts and then commit") {
            Self::Conflict
        } else if has("non-fast-forward") || has("[rejected]") {
            Self::NonFastForward
        } else if has("Authentication failed")
            || has("could not read Username")
            || has("Permission denied (publickey")
        {
            Self::AuthenticationFailed
        } else if has("already exists") {
            Self::AlreadyExists
        } else if has("is not fully merged") {
            Self::NotFullyMerged
        } else {
            Self::Other(first_meaningful_line(stderr))
        }
    }

    /// A short message for the status bar.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::NotARepository => "This folder is not inside a git repository.".into(),
            Self::LocalChangesWouldBeOverwritten => {
                "Your uncommitted changes would be overwritten. Commit or stash them first.".into()
            }
            Self::Conflict => "Conflicts found. Resolve them, then continue or abort.".into(),
            Self::NonFastForward => {
                "The remote has commits you don't have. Pull first, or force push with lease."
                    .into()
            }
            Self::AuthenticationFailed => {
                "Authentication failed. Check your credentials or SSH key.".into()
            }
            Self::AlreadyExists => "A ref with that name already exists.".into(),
            Self::NotFullyMerged => {
                "The branch is not fully merged. Delete it anyway with force delete.".into()
            }
            Self::CommitsMissingFromTodo => {
                "The rebase todo list leaves out commits, so git stopped before rewriting \
                 anything. Abort the rebase and try again."
                    .into()
            }
            Self::Other(line) => line.clone(),
        }
    }
}

fn first_meaningful_line(stderr: &str) -> String {
    let lines = || stderr.lines().map(str::trim).filter(|l| !l.is_empty());
    lines()
        .find(|l| l.starts_with("fatal:") || l.starts_with("error:"))
        .or_else(|| lines().find(|l| !l.starts_with("hint:")))
        .unwrap_or("git failed")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn recognizes_common_git_failures() {
        let cases = [
            (
                "fatal: not a git repository (or any of the parent directories): .git",
                Failure::NotARepository,
            ),
            (
                "error: Your local changes to the following files would be overwritten by checkout:\n\ta.txt\nPlease commit your changes or stash them before you switch branches.\nAborting",
                Failure::LocalChangesWouldBeOverwritten,
            ),
            (
                "CONFLICT (content): Merge conflict in a.txt\nAutomatic merge failed; fix conflicts and then commit the result.",
                Failure::Conflict,
            ),
            (
                " ! [rejected]        main -> main (non-fast-forward)\nerror: failed to push some refs",
                Failure::NonFastForward,
            ),
            (
                "fatal: Authentication failed for 'https://example.com/r.git/'",
                Failure::AuthenticationFailed,
            ),
            (
                "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
                Failure::AuthenticationFailed,
            ),
            (
                "fatal: a branch named 'dev' already exists",
                Failure::AlreadyExists,
            ),
            (
                "error: The branch 'dev' is not fully merged.",
                Failure::NotFullyMerged,
            ),
            (
                "Warning: some commits may have been dropped accidentally.\nDropped commits (newer to older):\n - 1a2b3c4 Add echo\nTo avoid this message, use \"drop\" to explicitly remove a commit.\n",
                Failure::CommitsMissingFromTodo,
            ),
        ];
        for (stderr, expected) in cases {
            assert_eq!(Failure::classify(stderr), expected, "{stderr}");
        }
    }

    #[test]
    fn unknown_errors_keep_the_first_meaningful_line() {
        assert_eq!(
            Failure::classify("hint: something\nfatal: weird thing happened\nmore"),
            Failure::Other("fatal: weird thing happened".into())
        );
    }

    #[test]
    fn messages_are_written_for_users() {
        assert_eq!(
            Failure::LocalChangesWouldBeOverwritten.message(),
            "Your uncommitted changes would be overwritten. Commit or stash them first."
        );
    }
}
