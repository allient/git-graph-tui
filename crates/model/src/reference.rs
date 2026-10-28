use crate::Oid;

/// What a ref points at, derived from its full name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// `refs/heads/*`
    LocalBranch,
    /// `refs/remotes/<remote>/*` (except `HEAD`)
    RemoteBranch,
    /// `refs/remotes/<remote>/HEAD`
    RemoteHead,
    /// `refs/tags/*`
    Tag,
    /// `refs/stash`
    Stash,
    /// Anything else (notes, replace refs, ...).
    Other,
}

impl RefKind {
    /// Classifies a full ref name such as `refs/heads/main`.
    #[must_use]
    pub fn from_full_name(full: &str) -> Self {
        if full.starts_with("refs/heads/") {
            Self::LocalBranch
        } else if let Some((_, branch)) = split_remote(full) {
            if branch == "HEAD" {
                Self::RemoteHead
            } else {
                Self::RemoteBranch
            }
        } else if full.starts_with("refs/tags/") {
            Self::Tag
        } else if full == "refs/stash" {
            Self::Stash
        } else {
            Self::Other
        }
    }
}

/// A branch, tag, remote branch or stash and the commit it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    /// Full name, e.g. `refs/heads/main`.
    pub full_name: String,
    pub kind: RefKind,
    /// The commit this ref points at (peeled for annotated tags).
    pub target: Oid,
    /// Full name of the upstream branch, for local branches that track one.
    pub upstream: Option<String>,
    /// True for the branch HEAD points at.
    pub is_head: bool,
}

impl Ref {
    /// The name shown to users: `main`, `origin/main`, `v1.0`, `stash`.
    #[must_use]
    pub fn short_name(&self) -> &str {
        short_name(&self.full_name)
    }
}

/// Strips the `refs/heads/`, `refs/remotes/`, `refs/tags/` or `refs/` prefix.
#[must_use]
pub fn short_name(full: &str) -> &str {
    ["refs/heads/", "refs/remotes/", "refs/tags/", "refs/"]
        .iter()
        .find_map(|prefix| full.strip_prefix(prefix))
        .unwrap_or(full)
}

/// Splits `refs/remotes/<remote>/<branch>` into `(remote, branch)`.
#[must_use]
pub fn split_remote(full: &str) -> Option<(&str, &str)> {
    full.strip_prefix("refs/remotes/")?.split_once('/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn classifies_full_ref_names() {
        assert_eq!(
            RefKind::from_full_name("refs/heads/main"),
            RefKind::LocalBranch
        );
        assert_eq!(
            RefKind::from_full_name("refs/heads/feature/login"),
            RefKind::LocalBranch
        );
        assert_eq!(
            RefKind::from_full_name("refs/remotes/origin/main"),
            RefKind::RemoteBranch
        );
        assert_eq!(
            RefKind::from_full_name("refs/remotes/origin/HEAD"),
            RefKind::RemoteHead
        );
        assert_eq!(RefKind::from_full_name("refs/tags/v1.0"), RefKind::Tag);
        assert_eq!(RefKind::from_full_name("refs/stash"), RefKind::Stash);
        assert_eq!(
            RefKind::from_full_name("refs/notes/commits"),
            RefKind::Other
        );
    }

    #[test]
    fn short_names_drop_the_namespace() {
        assert_eq!(short_name("refs/heads/feature/login"), "feature/login");
        assert_eq!(short_name("refs/remotes/origin/main"), "origin/main");
        assert_eq!(short_name("refs/tags/v1.0"), "v1.0");
        assert_eq!(short_name("refs/stash"), "stash");
    }

    #[test]
    fn remote_branch_splits_remote_and_branch() {
        assert_eq!(
            split_remote("refs/remotes/origin/feature/x"),
            Some(("origin", "feature/x"))
        );
        assert_eq!(split_remote("refs/heads/main"), None);
    }
}
