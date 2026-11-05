use crate::RefNameError;

/// Identifies one field of an action's dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputId {
    /// A branch or tag name.
    Name,
    /// Annotated tag message.
    Message,
    /// Tag type: `Annotated` or `Lightweight`.
    TagType,
    /// Push a new tag to the remote.
    PushToRemote,
    /// Archive output path.
    Output,
    Remote,
    /// Check out the branch after creating it.
    Checkout,
    NoFastForward,
    /// Merge with `--squash`.
    Squash,
    /// Merge with `--no-commit`.
    NoCommit,
    SetUpstream,
    ForceWithLease,
    /// Force delete (`branch -D`).
    Force,
    /// Reset mode: `soft`, `mixed` or `hard`.
    Mode,
    /// Also delete a tag on the remote.
    DeleteOnRemote,
    /// Restore staged changes when applying a stash (`--index`).
    ReinstateIndex,
    /// Stash untracked files too.
    IncludeUntracked,
    /// A remote's URL.
    Url,
    /// A worktree's folder.
    Path,
    /// Create the worktree's branch instead of checking out an existing one.
    NewBranch,
    /// Pull after checking out.
    Pull,
}

/// How a dialog should edit a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// A branch or tag name, checked with [`crate::validate_ref_name`].
    RefName,
    /// Free text, may be empty.
    Text,
    /// A file path, required.
    Path,
    /// One of the repository's remotes (the app lists them).
    Remote,
    Toggle,
    /// One of a fixed set of values.
    Choice(&'static [Choice]),
}

/// One option of a choice field: the value git gets, and what the dialog's
/// dropdown says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub value: &'static str,
    pub description: &'static str,
    /// Loses work (a hard reset): drawn in the warning color.
    pub destructive: bool,
}

impl Choice {
    pub(crate) const fn new(value: &'static str, description: &'static str) -> Self {
        Self {
            value,
            description,
            destructive: false,
        }
    }

    pub(crate) const fn destructive(value: &'static str, description: &'static str) -> Self {
        Self {
            value,
            description,
            destructive: true,
        }
    }
}

/// A field value: text for names, paths and choices, bool for toggles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValue {
    Text(String),
    Bool(bool),
}

/// One field of an action's dialog, with its current value as the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSpec {
    pub id: InputId,
    pub label: &'static str,
    pub kind: InputKind,
    pub value: InputValue,
}

impl InputSpec {
    pub(crate) fn text(id: InputId, label: &'static str, kind: InputKind, value: &str) -> Self {
        Self {
            id,
            label,
            kind,
            value: InputValue::Text(value.to_owned()),
        }
    }

    pub(crate) fn toggle(id: InputId, label: &'static str, value: bool) -> Self {
        Self {
            id,
            label,
            kind: InputKind::Toggle,
            value: InputValue::Bool(value),
        }
    }
}

/// Why a dialog value was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("{0}")]
    InvalidName(#[from] RefNameError),
    #[error("This field is required.")]
    Required(InputId),
    #[error("Wrong kind of value for {0:?}.")]
    WrongType(InputId),
    #[error("'{0}' is not one of the options.")]
    NotAChoice(String),
    #[error("This action has no {0:?} field.")]
    NotAnInput(InputId),
}

pub(crate) fn text(id: InputId, value: InputValue) -> Result<String, InputError> {
    match value {
        InputValue::Text(text) => Ok(text),
        InputValue::Bool(_) => Err(InputError::WrongType(id)),
    }
}

pub(crate) fn toggle(id: InputId, value: &InputValue) -> Result<bool, InputError> {
    match value {
        InputValue::Bool(on) => Ok(*on),
        InputValue::Text(_) => Err(InputError::WrongType(id)),
    }
}

pub(crate) fn ref_name(id: InputId, value: InputValue) -> Result<String, InputError> {
    let name = text(id, value)?;
    crate::validate_ref_name(&name)?;
    Ok(name)
}

pub(crate) fn required(id: InputId, value: InputValue) -> Result<String, InputError> {
    let value = text(id, value)?;
    if value.trim().is_empty() {
        return Err(InputError::Required(id));
    }
    Ok(value)
}
