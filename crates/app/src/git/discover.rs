use std::path::{Path, PathBuf};

use tgg_git::{CancelToken, ExecError, Executor, GitCommand};

/// Finds the top-level directory of the repository containing `path`.
pub fn discover_repo(executor: &impl Executor, path: &Path) -> Result<PathBuf, ExecError> {
    let command = GitCommand::read(path, ["rev-parse", "--show-toplevel"]);
    let output = executor.run(&command, &CancelToken::new())?;
    Ok(PathBuf::from(output.stdout.trim_end_matches(['\n', '\r'])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::ProcessExecutor;
    use tgg_git::{ExecError, Failure};
    use tgg_testkit::FixtureRepo;

    #[test]
    fn finds_the_top_level_from_a_subdirectory() {
        let repo = FixtureRepo::new();
        repo.commit_file("src/deep/a.txt", "1\n", "Base");
        let top = discover_repo(&ProcessExecutor::new(), &repo.path().join("src/deep")).unwrap();
        assert_eq!(
            top.canonicalize().unwrap(),
            repo.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn outside_a_repository_is_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = discover_repo(&ProcessExecutor::new(), dir.path()).unwrap_err();
        assert!(
            matches!(
                err,
                ExecError::Failed {
                    failure: Failure::NotARepository,
                    ..
                }
            ),
            "{err:?}"
        );
    }
}
