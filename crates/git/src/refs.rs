//! `git for-each-ref`: branches, remote branches, tags and the stash ref.

use tgg_model::{Ref, RefKind};

use crate::ParseError;
use crate::error::{fields, oid};

/// refname, objectname, peeled objectname (annotated tags), upstream, HEAD marker.
pub const FORMAT: &str = "%(refname)%1f%(objectname)%1f%(*objectname)%1f%(upstream)%1f%(HEAD)";
const FIELDS: [&str; 5] = ["refname", "objectname", "peeled", "upstream", "head"];

/// Arguments for `git for-each-ref` (without `-C <repo>`).
#[must_use]
pub fn args() -> Vec<String> {
    vec![
        "for-each-ref".into(),
        format!("--format={FORMAT}"),
        "refs/heads".into(),
        "refs/remotes".into(),
        "refs/tags".into(),
        "refs/stash".into(),
    ]
}

/// Parses newline-separated records produced with [`FORMAT`].
/// (Ref names can't contain newlines, so `-z` isn't needed here.)
pub fn parse(output: &str) -> Result<Vec<Ref>, ParseError> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(index, line)| parse_record(line, index))
        .collect()
}

fn parse_record(record: &str, index: usize) -> Result<Ref, ParseError> {
    let f = fields(record, index, &FIELDS)?;
    let full_name = f[0].to_owned();
    // Annotated tags point at a tag object; the graph needs the commit behind it.
    let target = if f[2].is_empty() {
        oid(f[1], index, "objectname")?
    } else {
        oid(f[2], index, "peeled")?
    };
    Ok(Ref {
        kind: RefKind::from_full_name(&full_name),
        full_name,
        target,
        upstream: (!f[3].is_empty()).then(|| f[3].to_owned()),
        is_head: f[4] == "*",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tgg_model::RefKind;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const T: &str = "tttttttttttttttttttttttttttttttttttttttt";

    #[test]
    fn args_list_branches_remotes_tags_and_stash() {
        let args = args();
        assert_eq!(args[0], "for-each-ref");
        for pattern in ["refs/heads", "refs/remotes", "refs/tags", "refs/stash"] {
            assert!(args.contains(&pattern.to_owned()), "missing {pattern}");
        }
    }

    #[test]
    fn parses_branches_with_upstream_and_head_marker() {
        let out = format!(
            "refs/heads/main\x1f{A}\x1f\x1frefs/remotes/origin/main\x1f*\n\
             refs/heads/dev\x1f{A}\x1f\x1f\x1f \n\
             refs/remotes/origin/main\x1f{A}\x1f\x1f\x1f \n"
        );
        let refs = parse(&out).unwrap();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].short_name(), "main");
        assert_eq!(refs[0].kind, RefKind::LocalBranch);
        assert_eq!(
            refs[0].upstream.as_deref(),
            Some("refs/remotes/origin/main")
        );
        assert!(refs[0].is_head);
        assert!(!refs[1].is_head);
        assert_eq!(refs[1].upstream, None);
        assert_eq!(refs[2].kind, RefKind::RemoteBranch);
    }

    #[test]
    fn annotated_tags_point_at_the_peeled_commit() {
        // For an annotated tag, objectname is the tag object and *objectname the commit.
        let out = format!("refs/tags/v1\x1f{T}\x1f{A}\x1f\x1f \n");
        let refs = parse(&out).unwrap();
        assert_eq!(refs[0].target.as_str(), A);
        assert_eq!(refs[0].kind, RefKind::Tag);
    }

    #[test]
    fn empty_output_means_no_refs() {
        assert_eq!(parse("").unwrap(), vec![]);
    }
}
