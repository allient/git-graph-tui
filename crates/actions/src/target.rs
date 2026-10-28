use tgg_model::{HeadState, Oid, Ref, RefKind, Stash, split_remote};

/// A commit row the user opened a menu on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitTarget {
    pub id: Oid,
    pub subject: String,
    /// Number of parents: 0 for a root commit, 2+ for a merge.
    pub parent_count: usize,
    /// True when the commit is HEAD or reachable from it (the graph draws the
    /// others muted, S7).
    pub is_head_ancestor: bool,
}

impl CommitTarget {
    #[must_use]
    pub fn is_merge(&self) -> bool {
        self.parent_count > 1
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.parent_count == 0
    }
}

/// What a context menu was opened on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Commit(CommitTarget),
    /// A local branch, by short name (`feature/login`).
    LocalBranch {
        name: String,
        target: Oid,
    },
    /// A remote-tracking branch, split into remote and branch (`origin`, `main`).
    RemoteBranch {
        remote: String,
        branch: String,
        target: Oid,
    },
    Tag {
        name: String,
        target: Oid,
    },
    Stash(Stash),
    /// The virtual "Uncommitted Changes" row.
    Uncommitted,
}

impl Target {
    /// The target for a ref badge. `None` for refs without a menu
    /// (`origin/HEAD`, notes, the stash ref, which is listed via [`Stash`]).
    #[must_use]
    pub fn from_ref(r: &Ref) -> Option<Self> {
        match r.kind {
            RefKind::LocalBranch => Some(Self::LocalBranch {
                name: r.short_name().to_owned(),
                target: r.target.clone(),
            }),
            RefKind::RemoteBranch => {
                let (remote, branch) = split_remote(&r.full_name)?;
                Some(Self::RemoteBranch {
                    remote: remote.to_owned(),
                    branch: branch.to_owned(),
                    target: r.target.clone(),
                })
            }
            RefKind::Tag => Some(Self::Tag {
                name: r.short_name().to_owned(),
                target: r.target.clone(),
            }),
            RefKind::RemoteHead | RefKind::Stash | RefKind::Other => None,
        }
    }
}

/// Repository state that decides which menu entries apply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuContext {
    pub head: HeadState,
    /// The commit HEAD points at (`None` when unborn).
    pub head_commit: Option<Oid>,
    /// Configured remotes, in `git remote` order.
    pub remotes: Vec<String>,
    /// Branches currently selected in the "Branches:" dropdown (short names,
    /// `origin/x` for remote branches). Empty means "Show All".
    pub branch_filter: Vec<String>,
}

impl MenuContext {
    /// The branch HEAD is on, if any.
    #[must_use]
    pub fn current_branch(&self) -> Option<&str> {
        match &self.head {
            HeadState::Branch(name) => Some(name),
            HeadState::Detached | HeadState::Unborn => None,
        }
    }

    #[must_use]
    pub fn is_current_branch(&self, name: &str) -> bool {
        self.current_branch() == Some(name)
    }

    #[must_use]
    pub fn is_head_commit(&self, id: &Oid) -> bool {
        self.head_commit.as_ref() == Some(id)
    }

    /// The remote a push goes to by default: `origin` when it exists,
    /// otherwise the first remote.
    #[must_use]
    pub fn default_remote(&self) -> Option<&str> {
        self.remotes
            .iter()
            .find(|r| *r == "origin")
            .or_else(|| self.remotes.first())
            .map(String::as_str)
    }

    #[must_use]
    pub fn is_filtered(&self, branch: &str) -> bool {
        self.branch_filter.iter().any(|b| b == branch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn oid(c: char) -> Oid {
        Oid::parse(&c.to_string().repeat(40)).unwrap()
    }

    fn reference(full_name: &str) -> Ref {
        Ref {
            full_name: full_name.into(),
            kind: RefKind::from_full_name(full_name),
            target: oid('a'),
            upstream: None,
            is_head: false,
        }
    }

    #[test]
    fn targets_from_ref_badges() {
        assert_eq!(
            Target::from_ref(&reference("refs/heads/feature/login")),
            Some(Target::LocalBranch {
                name: "feature/login".into(),
                target: oid('a')
            })
        );
        assert_eq!(
            Target::from_ref(&reference("refs/remotes/origin/feature/x")),
            Some(Target::RemoteBranch {
                remote: "origin".into(),
                branch: "feature/x".into(),
                target: oid('a')
            })
        );
        assert_eq!(
            Target::from_ref(&reference("refs/tags/v1.0")),
            Some(Target::Tag {
                name: "v1.0".into(),
                target: oid('a')
            })
        );
        assert_eq!(
            Target::from_ref(&reference("refs/remotes/origin/HEAD")),
            None
        );
        assert_eq!(Target::from_ref(&reference("refs/stash")), None);
    }

    #[test]
    fn context_knows_the_current_branch_and_head_commit() {
        let ctx = MenuContext {
            head: HeadState::Branch("main".into()),
            head_commit: Some(oid('a')),
            ..MenuContext::default()
        };
        assert_eq!(ctx.current_branch(), Some("main"));
        assert!(ctx.is_current_branch("main"));
        assert!(!ctx.is_current_branch("develop"));
        assert!(ctx.is_head_commit(&oid('a')));
        assert!(!ctx.is_head_commit(&oid('b')));

        let detached = MenuContext {
            head: HeadState::Detached,
            ..ctx
        };
        assert_eq!(detached.current_branch(), None);
    }

    #[test]
    fn default_remote_prefers_origin() {
        let ctx = |remotes: &[&str]| MenuContext {
            remotes: remotes.iter().map(|r| (*r).to_owned()).collect(),
            ..MenuContext::default()
        };
        assert_eq!(
            ctx(&["upstream", "origin"]).default_remote(),
            Some("origin")
        );
        assert_eq!(ctx(&["upstream"]).default_remote(), Some("upstream"));
        assert_eq!(ctx(&[]).default_remote(), None);
    }

    #[test]
    fn commit_shapes() {
        let commit = |parent_count| CommitTarget {
            id: oid('a'),
            subject: "s".into(),
            parent_count,
            is_head_ancestor: true,
        };
        assert!(commit(0).is_root());
        assert!(!commit(1).is_merge());
        assert!(commit(2).is_merge());
    }
}
