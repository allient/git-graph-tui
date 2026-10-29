//! Links to a hosting site's "new pull request" page, from a remote's URL.

/// The page that opens a pull request (merge request on GitLab) from
/// `branch`, for a remote at `remote_url`. None when the remote isn't on
/// GitHub, GitLab or Bitbucket, or its URL can't be read.
#[must_use]
pub fn pull_request_url(remote_url: &str, branch: &str) -> Option<String> {
    let (host, path) = host_and_path(remote_url.trim())?;
    let branch = encode(branch);
    let base = format!("https://{host}/{path}");
    let lower = host.to_lowercase();
    if lower.contains("github") {
        Some(format!("{base}/compare/{branch}?expand=1"))
    } else if lower.contains("gitlab") {
        Some(format!(
            "{base}/-/merge_requests/new?merge_request%5Bsource_branch%5D={branch}"
        ))
    } else if lower.contains("bitbucket") {
        Some(format!("{base}/pull-requests/new?source={branch}"))
    } else {
        None
    }
}

/// The host and repository path of `https://`, `ssh://` and `git@host:path` URLs.
fn host_and_path(url: &str) -> Option<(String, String)> {
    let (host, path) = if let Some((_, rest)) = url.split_once("://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        (host.split(':').next()?, path)
    } else {
        // scp-like: [user@]host:path
        let (authority, path) = url.split_once(':')?;
        (authority.rsplit('@').next()?, path)
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    (!host.is_empty() && !path.is_empty()).then(|| (host.to_owned(), path.to_owned()))
}

/// Percent-encodes everything but unreserved characters and `/`.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn github_https_and_ssh_remotes() {
        let expected = "https://github.com/allient/tui-git-graph/compare/feature/login?expand=1";
        for url in [
            "https://github.com/allient/tui-git-graph.git",
            "https://github.com/allient/tui-git-graph",
            "git@github.com:allient/tui-git-graph.git",
            "ssh://git@github.com/allient/tui-git-graph.git",
            "https://user:token@github.com/allient/tui-git-graph.git\n",
        ] {
            assert_eq!(
                pull_request_url(url, "feature/login").as_deref(),
                Some(expected),
                "{url}"
            );
        }
    }

    #[test]
    fn gitlab_merge_requests_keep_subgroups() {
        assert_eq!(
            pull_request_url("git@gitlab.com:group/sub/project.git", "fix-1").as_deref(),
            Some(
                "https://gitlab.com/group/sub/project/-/merge_requests/new?merge_request%5Bsource_branch%5D=fix-1"
            )
        );
    }

    #[test]
    fn bitbucket_pull_requests() {
        assert_eq!(
            pull_request_url("https://bitbucket.org/team/repo.git", "dev").as_deref(),
            Some("https://bitbucket.org/team/repo/pull-requests/new?source=dev")
        );
    }

    #[test]
    fn self_hosted_forges_are_recognized_by_name_and_ports_are_dropped() {
        assert_eq!(
            pull_request_url("ssh://git@gitlab.example.com:2222/a/b.git", "x").as_deref(),
            Some(
                "https://gitlab.example.com/a/b/-/merge_requests/new?merge_request%5Bsource_branch%5D=x"
            )
        );
    }

    #[test]
    fn branch_names_are_encoded() {
        assert_eq!(
            pull_request_url("git@github.com:a/b.git", "fix #1 ü").as_deref(),
            Some("https://github.com/a/b/compare/fix%20%231%20%C3%BC?expand=1")
        );
    }

    #[test]
    fn other_hosts_and_local_paths_have_no_link() {
        assert_eq!(pull_request_url("https://example.com/a/b.git", "x"), None);
        assert_eq!(pull_request_url("/srv/git/repo.git", "x"), None);
        assert_eq!(pull_request_url("", "x"), None);
    }
}
