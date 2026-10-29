//! `git stash list`: stash entries, newest first.

use tgg_model::Stash;

use crate::ParseError;
use crate::error::{fields, oid};

/// Stash commit id and message.
pub const FORMAT: &str = "%H%x1f%s";

/// Arguments for `git stash list` (without `-C <repo>`).
#[must_use]
pub fn args() -> Vec<String> {
    vec![
        "stash".into(),
        "list".into(),
        "-z".into(),
        format!("--format={FORMAT}"),
    ]
}

/// Arguments for `git stash show --patch <selector>`: a stash's changes
/// against the commit it was made on, as one diff.
#[must_use]
pub fn show_args(selector: &str) -> Vec<String> {
    vec![
        "stash".into(),
        "show".into(),
        "--patch".into(),
        selector.into(),
    ]
}

/// Parses NUL-separated records produced with [`FORMAT`]. Position in the
/// output is the stash index (`stash@{0}` first).
pub fn parse(output: &str) -> Result<Vec<Stash>, ParseError> {
    output
        .split('\0')
        .map(|record| record.trim_start_matches('\n'))
        .filter(|record| !record.is_empty())
        .enumerate()
        .map(|(index, record)| {
            let f = fields(record, index, &["id", "subject"])?;
            Ok(Stash {
                id: oid(f[0], index, "id")?,
                index,
                subject: f[1].to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn args_use_nul_separated_format() {
        let args = args();
        assert_eq!(args[..3], ["stash", "list", "-z"]);
    }

    #[test]
    fn parses_stash_entries_in_order() {
        let out = format!("{A}\x1fOn main: WIP\0{B}\x1fOn dev: older\0");
        let stashes = parse(&out).unwrap();
        assert_eq!(stashes.len(), 2);
        assert_eq!(stashes[0].index, 0);
        assert_eq!(stashes[0].selector(), "stash@{0}");
        assert_eq!(stashes[0].subject, "On main: WIP");
        assert_eq!(stashes[1].id.as_str(), B);
        assert_eq!(stashes[1].index, 1);
    }

    #[test]
    fn empty_output_means_no_stashes() {
        assert_eq!(parse("").unwrap(), vec![]);
    }
}
