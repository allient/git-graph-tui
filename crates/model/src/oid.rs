use std::fmt;

/// A git object id (commit hash): 40 hex characters (SHA-1) or 64 (SHA-256),
/// stored lowercase.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Oid(Box<str>);

/// Why a string isn't a valid [`Oid`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OidError {
    #[error("object id must be 40 or 64 hex characters, got {0}")]
    Length(usize),
    #[error("object id must contain only hex characters")]
    NotHex,
}

impl Oid {
    /// Parses a full hex object id.
    pub fn parse(hex: &str) -> Result<Self, OidError> {
        if hex.len() != 40 && hex.len() != 64 {
            return Err(OidError::Length(hex.len()));
        }
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(OidError::NotHex);
        }
        Ok(Self(hex.to_ascii_lowercase().into_boxed_str()))
    }

    /// The full hex id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first `len` characters (the whole id if `len` is larger).
    #[must_use]
    pub fn short(&self, len: usize) -> &str {
        &self.0[..len.min(self.0.len())]
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Oid({})", self.short(10))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn parses_a_sha1_hex_id() {
        let oid = Oid::parse(SHA1).unwrap();
        assert_eq!(oid.as_str(), SHA1);
    }

    #[test]
    fn parses_a_sha256_hex_id() {
        let hex = "a".repeat(64);
        assert_eq!(Oid::parse(&hex).unwrap().as_str(), hex);
    }

    #[test]
    fn normalizes_to_lowercase() {
        assert_eq!(Oid::parse(&SHA1.to_uppercase()).unwrap().as_str(), SHA1);
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!(Oid::parse("abc"), Err(OidError::Length(3)));
    }

    #[test]
    fn rejects_non_hex() {
        let bad = format!("{}z", &SHA1[..39]);
        assert_eq!(Oid::parse(&bad), Err(OidError::NotHex));
    }

    #[test]
    fn short_form_is_a_prefix() {
        let oid = Oid::parse(SHA1).unwrap();
        assert_eq!(oid.short(7), "0123456");
        assert_eq!(oid.short(100), SHA1);
    }

    #[test]
    fn displays_full_hex() {
        assert_eq!(Oid::parse(SHA1).unwrap().to_string(), SHA1);
    }
}
