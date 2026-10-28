//! `git for-each-ref` on one tag: what it points at and, for annotated
//! tags, the tagger and message (for "View Details" on a tag badge).

use tgg_model::{Oid, Signature};

use crate::ParseError;
use crate::error::{fields, oid};

/// refname, object type, object id, peeled id, tagger name/email/date,
/// contents; records end with NUL (`%00`) because messages span lines.
pub const FORMAT: &str = "%(refname)%1f%(objecttype)%1f%(objectname)%1f%(*objectname)%1f%(taggername)%1f%(taggeremail)%1f%(taggerdate:unix)%1f%(contents)%00";
const FIELDS: [&str; 8] = [
    "refname",
    "object type",
    "object id",
    "peeled id",
    "tagger name",
    "tagger email",
    "tagger date",
    "contents",
];

/// The tagger and message of an annotated tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub tagger: Signature,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDetails {
    pub name: String,
    /// The commit the tag points at (peeled for annotated tags).
    pub commit: Oid,
    /// `None` for a lightweight tag.
    pub annotation: Option<Annotation>,
}

/// Arguments for the details of tag `name` (without `-C <repo>`).
#[must_use]
pub fn args(name: &str) -> Vec<String> {
    vec![
        "for-each-ref".into(),
        format!("--format={FORMAT}"),
        "--".into(),
        format!("refs/tags/{name}"),
    ]
}

/// Parses the output of [`args`] and returns tag `name`, if present.
///
/// for-each-ref patterns also match by prefix, so only the record whose
/// refname is exactly `refs/tags/<name>` counts.
pub fn parse(output: &str, name: &str) -> Result<Option<TagDetails>, ParseError> {
    let wanted = format!("refs/tags/{name}");
    for (index, record) in output
        .split('\0')
        .map(|r| r.trim_start_matches('\n'))
        .filter(|r| !r.is_empty())
        .enumerate()
    {
        let f = fields(record, index, &FIELDS)?;
        if f[0] != wanted {
            continue;
        }
        let annotation = if f[1] == "tag" {
            Some(Annotation {
                tagger: Signature {
                    name: f[4].to_owned(),
                    email: f[5]
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_owned(),
                    time: f[6].parse().map_err(|_| ParseError::InvalidNumber {
                        record: index,
                        field: "tagger date",
                        value: f[6].to_owned(),
                    })?,
                },
                message: f[7].trim_end_matches('\n').to_owned(),
            })
        } else {
            None
        };
        let commit = if f[3].is_empty() { f[2] } else { f[3] };
        return Ok(Some(TagDetails {
            name: name.to_owned(),
            commit: oid(commit, index, "object id")?,
            annotation,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
