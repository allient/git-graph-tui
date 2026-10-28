/// Why a name can't be used as a branch or tag name.
///
/// This is a local pre-check of `git check-ref-format --branch` so dialogs
/// can flag a bad name while the user types. git stays the final judge.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RefNameError {
    #[error("The name can't be empty.")]
    Empty,
    #[error("The name can't start with '-'.")]
    LeadingDash,
    #[error("The name can't contain '{0}'.")]
    Forbidden(String),
    #[error("The name can't contain spaces or control characters.")]
    Whitespace,
    #[error("No part of the name can start with '.' or end with '.lock'.")]
    BadComponent,
    #[error("The name can't start or end with '/', or contain '//'.")]
    BadSlash,
    #[error("The name can't end with '.'.")]
    TrailingDot,
    #[error("'{0}' is reserved by git.")]
    Reserved(String),
}

/// Checks `name` against git's rules for branch and tag names.
pub fn validate_ref_name(name: &str) -> Result<(), RefNameError> {
    if name.is_empty() {
        return Err(RefNameError::Empty);
    }
    // `@` alone is git's shorthand for HEAD, and a branch named HEAD makes
    // every `HEAD` argument ambiguous (`check-ref-format --branch` rejects it).
    if name == "@" || name == "HEAD" {
        return Err(RefNameError::Reserved(name.into()));
    }
    // Would be parsed as an option by most git commands.
    if name.starts_with('-') {
        return Err(RefNameError::LeadingDash);
    }
    if name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(RefNameError::Whitespace);
    }
    if let Some(bad) = ["..", "@{", "~", "^", ":", "?", "*", "[", "\\"]
        .into_iter()
        .find(|bad| name.contains(bad))
    {
        return Err(RefNameError::Forbidden(bad.into()));
    }
    if name.starts_with('/') || name.ends_with('/') || name.contains("//") {
        return Err(RefNameError::BadSlash);
    }
    #[expect(
        clippy::case_sensitive_file_extension_comparisons,
        reason = "git rejects `.lock` exactly, not `.LOCK`"
    )]
    if name
        .split('/')
        .any(|part| part.starts_with('.') || part.ends_with(".lock"))
    {
        return Err(RefNameError::BadComponent);
    }
    if name.ends_with('.') {
        return Err(RefNameError::TrailingDot);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn accepts_ordinary_names() {
        for name in [
            "main",
            "feature/login",
            "release-1.2",
            "v1.0.0",
            "fix_ünïcode",
        ] {
            assert_eq!(validate_ref_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn rejects_empty_and_option_like_names() {
        assert_eq!(validate_ref_name(""), Err(RefNameError::Empty));
        assert_eq!(validate_ref_name("-f"), Err(RefNameError::LeadingDash));
    }

    #[test]
    fn rejects_characters_git_forbids() {
        for (name, bad) in [
            ("a..b", ".."),
            ("a~1", "~"),
            ("a^", "^"),
            ("a:b", ":"),
            ("a?", "?"),
            ("a*", "*"),
            ("a[b", "["),
            ("a\\b", "\\"),
            ("a@{1}", "@{"),
        ] {
            assert_eq!(
                validate_ref_name(name),
                Err(RefNameError::Forbidden(bad.into())),
                "{name}"
            );
        }
    }

    #[test]
    fn rejects_whitespace_and_control_characters() {
        assert_eq!(validate_ref_name("a b"), Err(RefNameError::Whitespace));
        assert_eq!(validate_ref_name("a\tb"), Err(RefNameError::Whitespace));
        assert_eq!(validate_ref_name("a\u{7f}"), Err(RefNameError::Whitespace));
    }

    #[test]
    fn rejects_bad_components_and_slashes() {
        assert_eq!(
            validate_ref_name(".hidden"),
            Err(RefNameError::BadComponent)
        );
        assert_eq!(validate_ref_name("a/.b"), Err(RefNameError::BadComponent));
        assert_eq!(validate_ref_name("a.lock"), Err(RefNameError::BadComponent));
        assert_eq!(
            validate_ref_name("a.lock/b"),
            Err(RefNameError::BadComponent)
        );
        assert_eq!(validate_ref_name("/a"), Err(RefNameError::BadSlash));
        assert_eq!(validate_ref_name("a/"), Err(RefNameError::BadSlash));
        assert_eq!(validate_ref_name("a//b"), Err(RefNameError::BadSlash));
        assert_eq!(validate_ref_name("a."), Err(RefNameError::TrailingDot));
    }

    #[test]
    fn rejects_names_git_reserves() {
        assert_eq!(
            validate_ref_name("@"),
            Err(RefNameError::Reserved("@".into()))
        );
        assert_eq!(
            validate_ref_name("HEAD"),
            Err(RefNameError::Reserved("HEAD".into()))
        );
    }
}
