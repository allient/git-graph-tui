use ggt_model::OidError;

/// Git printed something the parser didn't expect.
///
/// `record` is the zero-based index of the record (commit, ref, entry) that
/// failed, so the message points at the exact place in the output.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("record {record}: missing field `{field}`")]
    MissingField { record: usize, field: &'static str },
    #[error("record {record}: invalid object id in `{field}`: {source}")]
    InvalidOid {
        record: usize,
        field: &'static str,
        source: OidError,
    },
    #[error("record {record}: `{field}` is not a number: {value:?}")]
    InvalidNumber {
        record: usize,
        field: &'static str,
        value: String,
    },
    #[error("record {record}: unexpected record {text:?}")]
    UnexpectedRecord { record: usize, text: String },
}

/// Splits `record` on the field separator, failing if fewer than `names.len()` fields exist.
pub(crate) fn fields<'a>(
    record: &'a str,
    index: usize,
    names: &[&'static str],
) -> Result<Vec<&'a str>, ParseError> {
    let parts: Vec<&str> = record.splitn(names.len(), crate::FIELD_SEP).collect();
    if let Some(&missing) = names.get(parts.len()) {
        return Err(ParseError::MissingField {
            record: index,
            field: missing,
        });
    }
    Ok(parts)
}

pub(crate) fn oid(
    value: &str,
    record: usize,
    field: &'static str,
) -> Result<ggt_model::Oid, ParseError> {
    ggt_model::Oid::parse(value).map_err(|source| ParseError::InvalidOid {
        record,
        field,
        source,
    })
}
