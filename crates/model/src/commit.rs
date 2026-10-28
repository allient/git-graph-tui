use crate::Oid;

/// Who made a commit and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub email: String,
    /// Seconds since the Unix epoch.
    pub time: i64,
}

/// A commit as shown in the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: Oid,
    /// Parents in git order: the first parent is the branch the commit was made on.
    pub parents: Vec<Oid>,
    pub author: Signature,
    pub committer: Signature,
    /// First line of the commit message.
    pub subject: String,
}

impl Commit {
    /// A commit with no parents.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.parents.is_empty()
    }

    /// A commit with two or more parents.
    #[must_use]
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Oid;

    fn oid(c: char) -> Oid {
        Oid::parse(&c.to_string().repeat(40)).unwrap()
    }

    fn commit(parents: Vec<Oid>) -> Commit {
        let sig = Signature {
            name: "A".into(),
            email: "a@x".into(),
            time: 0,
        };
        Commit {
            id: oid('a'),
            parents,
            author: sig.clone(),
            committer: sig,
            subject: "s".into(),
        }
    }

    #[test]
    fn root_merge_and_regular_commits() {
        assert!(commit(vec![]).is_root());
        assert!(!commit(vec![oid('b')]).is_merge());
        assert!(commit(vec![oid('b'), oid('c')]).is_merge());
    }
}
