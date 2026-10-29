//! Links to a hosting site's "new pull request" and issue pages, from a
//! remote's URL, and the issue references in commit messages.

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

/// The page of issue `number` for a remote at `remote_url`. None when the
/// remote isn't on GitHub, GitLab or Bitbucket, or its URL can't be read.
#[must_use]
pub fn issue_url(remote_url: &str, number: &str) -> Option<String> {
    let (host, path) = host_and_path(remote_url.trim())?;
    let base = format!("https://{host}/{path}");
    let lower = host.to_lowercase();
    if lower.contains("gitlab") {
        Some(format!("{base}/-/issues/{number}"))
    } else if lower.contains("github") || lower.contains("bitbucket") {
        Some(format!("{base}/issues/{number}"))
    } else {
        None
    }
}

/// An issue reference such as `#123` in a commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueRef {
    /// Byte range of the whole reference, prefix included.
    pub range: std::ops::Range<usize>,
    /// The digits after the prefix.
    pub number: String,
}

/// Finds `<prefix><digits>` references in `text`, such as `#123` (prefix
/// `#`) or `PROJ-123` (prefix `PROJ-`). A reference starts and ends at a
/// word boundary, so `abc#12` and `#12abc` are not references.
#[must_use]
pub fn issue_refs(text: &str, prefix: &str) -> Vec<IssueRef> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut found = Vec::new();
    if prefix.is_empty() {
        return found;
    }
    for (start, _) in text.match_indices(prefix) {
        let after_word = text[..start].chars().next_back().is_some_and(is_word);
        let rest = &text[start + prefix.len()..];
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let end = start + prefix.len() + digits;
        let before_word = text[end..].chars().next().is_some_and(is_word);
        if !after_word && digits > 0 && !before_word {
            found.push(IssueRef {
                range: start..end,
                number: rest[..digits].to_owned(),
            });
        }
    }
    found
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
    fn issue_pages_on_each_forge() {
        assert_eq!(
            issue_url("git@github.com:allient/demo.git", "12").as_deref(),
            Some("https://github.com/allient/demo/issues/12")
        );
        assert_eq!(
            issue_url("https://gitlab.com/group/sub/project.git", "7").as_deref(),
            Some("https://gitlab.com/group/sub/project/-/issues/7")
        );
        assert_eq!(
            issue_url("https://bitbucket.org/team/repo.git", "3").as_deref(),
            Some("https://bitbucket.org/team/repo/issues/3")
        );
        assert_eq!(issue_url("https://example.com/a/b.git", "1"), None);
        assert_eq!(issue_url("/srv/git/repo.git", "1"), None);
    }

    fn found(text: &str, prefix: &str) -> Vec<(String, String)> {
        issue_refs(text, prefix)
            .into_iter()
            .map(|issue| (text[issue.range].to_owned(), issue.number))
            .collect()
    }

    #[test]
    fn issue_references_need_a_word_boundary_and_digits() {
        assert_eq!(
            found("Fix login (#12), see #3.", "#"),
            [("#12".into(), "12".into()), ("#3".into(), "3".into())]
        );
        assert_eq!(found("abc#12 a_#4 #12abc # #x", "#"), []);
        assert_eq!(found("#5 at the start", "#").len(), 1);
        assert_eq!(
            found("PROJ-42: done, not XPROJ-1 or #9", "PROJ-"),
            [("PROJ-42".into(), "42".into())]
        );
        assert_eq!(found("ü#8", "#"), [], "letters beyond ASCII count too");
        assert_eq!(found("#12", ""), [], "an empty prefix links nothing");
    }

    #[test]
    fn other_hosts_and_local_paths_have_no_link() {
        assert_eq!(pull_request_url("https://example.com/a/b.git", "x"), None);
        assert_eq!(pull_request_url("/srv/git/repo.git", "x"), None);
        assert_eq!(pull_request_url("", "x"), None);
    }
}
