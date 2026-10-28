use tgg_model::{Oid, Ref, RefKind, split_remote};

/// What a badge stands for; decides its style and order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BadgeKind {
    /// The branch HEAD is on.
    Head,
    Branch,
    Remote,
    RemoteHead,
    Tag,
    Stash,
}

/// A label drawn before a commit's subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    pub kind: BadgeKind,
    /// Text shown, e.g. `main`, `origin/feature`, `v1.0`.
    pub label: String,
    /// Remotes whose same-named branch is on this commit (`main | origin`).
    pub remotes: Vec<String>,
    /// Full ref name, used to build the right context menu.
    pub full_name: String,
}

/// Badges for the refs pointing at `commit`, in display order: the HEAD
/// branch, other local branches, remote branches, remote HEADs, tags, stash.
#[must_use]
pub fn badges_for(commit: &Oid, refs: &[Ref]) -> Vec<Badge> {
    let here: Vec<&Ref> = refs.iter().filter(|r| &r.target == commit).collect();
    let locals: Vec<&str> = here
        .iter()
        .filter(|r| r.kind == RefKind::LocalBranch)
        .map(|r| r.short_name())
        .collect();

    let mut badges: Vec<Badge> = here
        .iter()
        .filter_map(|r| {
            let kind = match r.kind {
                RefKind::LocalBranch if r.is_head => BadgeKind::Head,
                RefKind::LocalBranch => BadgeKind::Branch,
                RefKind::RemoteBranch => {
                    // Shown on the local badge instead (`main | origin`).
                    let (_, branch) = split_remote(&r.full_name)?;
                    if locals.contains(&branch) {
                        return None;
                    }
                    BadgeKind::Remote
                }
                RefKind::RemoteHead => BadgeKind::RemoteHead,
                RefKind::Tag => BadgeKind::Tag,
                RefKind::Stash => BadgeKind::Stash,
                RefKind::Other => return None,
            };
            Some(Badge {
                kind,
                label: r.short_name().to_owned(),
                remotes: if r.kind == RefKind::LocalBranch {
                    remotes_with_branch(&here, r.short_name())
                } else {
                    Vec::new()
                },
                full_name: r.full_name.clone(),
            })
        })
        .collect();
    badges.sort_by(|a, b| (a.kind, &a.label).cmp(&(b.kind, &b.label)));
    badges
}

fn remotes_with_branch(refs: &[&Ref], branch: &str) -> Vec<String> {
    let mut remotes: Vec<String> = refs
        .iter()
        .filter(|r| r.kind == RefKind::RemoteBranch)
        .filter_map(|r| split_remote(&r.full_name))
        .filter(|(_, b)| *b == branch)
        .map(|(remote, _)| remote.to_owned())
        .collect();
    remotes.sort();
    remotes
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tgg_model::{Oid, Ref, RefKind};

    fn oid(c: char) -> Oid {
        Oid::parse(&c.to_string().repeat(40)).unwrap()
    }

    fn r(full: &str, target: char, is_head: bool) -> Ref {
        Ref {
            full_name: full.into(),
            kind: RefKind::from_full_name(full),
            target: oid(target),
            upstream: None,
            is_head,
        }
    }

    fn labels(badges: &[Badge]) -> Vec<String> {
        badges
            .iter()
            .map(|b| match &b.remotes[..] {
                [] => format!("{:?} {}", b.kind, b.label),
                remotes => format!("{:?} {} | {}", b.kind, b.label, remotes.join(" ")),
            })
            .collect()
    }

    #[test]
    fn local_and_remote_on_the_same_commit_share_one_badge() {
        let refs = [
            r("refs/heads/main", 'a', true),
            r("refs/remotes/origin/main", 'a', false),
        ];
        assert_eq!(
            labels(&badges_for(&oid('a'), &refs)),
            ["Head main | origin"]
        );
    }

    #[test]
    fn remote_on_another_commit_gets_its_own_badge() {
        let refs = [
            r("refs/heads/main", 'a', true),
            r("refs/remotes/origin/main", 'b', false),
        ];
        assert_eq!(labels(&badges_for(&oid('a'), &refs)), ["Head main"]);
        assert_eq!(
            labels(&badges_for(&oid('b'), &refs)),
            ["Remote origin/main"]
        );
    }

    #[test]
    fn several_remotes_are_listed_on_the_local_badge() {
        let refs = [
            r("refs/heads/dev", 'a', false),
            r("refs/remotes/upstream/dev", 'a', false),
            r("refs/remotes/origin/dev", 'a', false),
        ];
        assert_eq!(
            labels(&badges_for(&oid('a'), &refs)),
            ["Branch dev | origin upstream"]
        );
    }

    #[test]
    fn order_is_head_branches_remotes_tags_stash() {
        let refs = [
            r("refs/stash", 'a', false),
            r("refs/tags/v1", 'a', false),
            r("refs/remotes/origin/HEAD", 'a', false),
            r("refs/remotes/origin/feature", 'a', false),
            r("refs/heads/zeta", 'a', false),
            r("refs/heads/alpha", 'a', false),
            r("refs/heads/main", 'a', true),
        ];
        assert_eq!(
            labels(&badges_for(&oid('a'), &refs)),
            [
                "Head main",
                "Branch alpha",
                "Branch zeta",
                "Remote origin/feature",
                "RemoteHead origin/HEAD",
                "Tag v1",
                "Stash stash",
            ]
        );
    }

    #[test]
    fn badges_keep_the_full_ref_name_for_menus() {
        let refs = [r("refs/heads/feature/login", 'a', false)];
        let badges = badges_for(&oid('a'), &refs);
        assert_eq!(badges[0].full_name, "refs/heads/feature/login");
        assert_eq!(badges[0].label, "feature/login");
    }

    #[test]
    fn commits_without_refs_have_no_badges() {
        assert_eq!(
            badges_for(&oid('c'), &[r("refs/heads/main", 'a', true)]),
            vec![]
        );
    }
}
