use std::path::Path;

use tgg_git::GitCommand;
use tgg_model::Oid;

use crate::input::{self, InputError, InputId, InputKind, InputSpec, InputValue};
use crate::{
    CommitScope, InteractiveRebase, OperationAction, PickPurpose, RepoAction, StashAction,
    TagAction, WorkingTreeAction, WorktreeAction,
};

/// Abbreviated hash length in confirmation text (git's default).
const SHORT: usize = 7;

/// A revision an action works against, kept typed so messages can say what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rev {
    Local(String),
    Remote { remote: String, branch: String },
    Tag(String),
    Commit(Oid),
}

impl Rev {
    /// The argument git gets.
    #[must_use]
    pub fn arg(&self) -> String {
        match self {
            Self::Local(name) | Self::Tag(name) => name.clone(),
            Self::Remote { remote, branch } => format!("{remote}/{branch}"),
            Self::Commit(id) => id.to_string(),
        }
    }

    /// How messages name it: `branch 'develop'`, `commit 0123456`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Local(name) => format!("branch '{name}'"),
            Self::Remote { .. } => format!("remote branch '{}'", self.arg()),
            Self::Tag(name) => format!("tag '{name}'"),
            Self::Commit(id) => format!("commit {}", id.short(SHORT)),
        }
    }
}

/// `git reset` mode. Mixed is the default, as in git.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ResetMode {
    /// Keep the changes staged.
    Soft,
    /// Keep the changes in the working tree, unstaged.
    #[default]
    Mixed,
    /// Discard every change.
    Hard,
}

impl ResetMode {
    pub const CHOICES: &'static [&'static str] = &["soft", "mixed", "hard"];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Mixed => "mixed",
            Self::Hard => "hard",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "soft" => Some(Self::Soft),
            "mixed" => Some(Self::Mixed),
            "hard" => Some(Self::Hard),
            _ => None,
        }
    }
}

/// The Add Tag dialog's types; the first is the default, as in VS Code.
const TAG_TYPES: &[&str] = &["Annotated", "Lightweight"];

/// Something the app does in response to a menu entry, as data.
///
/// Fields hold both what git needs and what the confirmation text names
/// (subjects, the current branch), so planning and messages need no lookup.
/// `current: None` means HEAD is detached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    CheckoutBranch {
        name: String,
    },
    /// Checks out a remote branch by creating a local branch that tracks it.
    CheckoutRemoteBranch {
        remote: String,
        branch: String,
        local_name: String,
    },
    CreateBranch {
        at: Oid,
        name: String,
        checkout: bool,
    },
    RenameBranch {
        from: String,
        to: String,
    },
    /// `force: false` is `branch -d`, which refuses unmerged branches.
    DeleteBranch {
        name: String,
        force: bool,
    },
    MergeIntoCurrent {
        source: Rev,
        current: Option<String>,
        no_ff: bool,
    },
    RebaseCurrentOnto {
        onto: Rev,
        current: Option<String>,
    },
    PushBranch {
        branch: String,
        remote: String,
        set_upstream: bool,
        force_with_lease: bool,
    },
    /// Updates (or creates) the local branch `local_name` from `branch` on
    /// `remote` without checking it out. Without `force`, git only
    /// fast-forwards it.
    FetchIntoLocal {
        remote: String,
        branch: String,
        local_name: String,
        force: bool,
    },
    /// `output` is relative to the repository root; git picks the format
    /// (zip, tar, tar.gz, ...) from its extension.
    CreateArchive {
        rev: Rev,
        output: String,
    },
    /// Tags `at`; an annotated tag's `message` may be empty, a lightweight
    /// tag has none. With `push`, also pushes the tag to `remote` (the
    /// default remote; `None` when the repository has none).
    AddTag {
        at: Oid,
        name: String,
        annotated: bool,
        message: String,
        remote: Option<String>,
        push: bool,
    },
    CheckoutCommit {
        commit: Oid,
        subject: String,
    },
    CherryPick {
        commit: Oid,
        subject: String,
        is_merge: bool,
    },
    Revert {
        commit: Oid,
        subject: String,
        is_merge: bool,
    },
    /// Removes a commit from the current branch (the menus only offer it for
    /// non-merge, non-root ancestors of HEAD).
    Drop {
        commit: Oid,
        subject: String,
        current: Option<String>,
    },
    ResetCurrentTo {
        commit: Oid,
        subject: String,
        current: String,
        mode: ResetMode,
    },
    /// Adds or removes a branch in the "Branches:" dropdown. `selected` is
    /// its state when the menu opened.
    ToggleBranchFilter {
        branch: String,
        selected: bool,
    },
    CopyBranchName {
        name: String,
    },
    CopyCommitHash {
        commit: Oid,
    },
    CopyCommitSubject {
        subject: String,
    },
    Tag(TagAction),
    Stash(StashAction),
    WorkingTree(WorkingTreeAction),
    /// The worktrees view (S20 4).
    Worktree(WorktreeAction),
    Repo(RepoAction),
    /// Continue or abort the operation in progress (the conflict banner).
    Operation(OperationAction),
    /// Rewrites the commits after a base on the current branch with an
    /// edited todo list (the interactive rebase panel).
    InteractiveRebase(InteractiveRebase),
    /// An app-only effect from a menu (pickers, the git output view).
    Ui(UiEffect),
}

/// What running an action means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Run these in order, stopping at the first failure.
    Git(Vec<GitCommand>),
    Clipboard(String),
    Ui(UiEffect),
}

/// Changes to app state that don't involve git.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEffect {
    ToggleBranchFilter {
        branch: String,
    },
    /// Show a tag (and an annotated tag's message).
    ShowTag {
        name: String,
    },
    /// The VS Code "Open Source Control View" entry; the app shows a hint.
    OpenSourceControl,
    /// Open a ref picker; [`PickPurpose::action_for`] turns the pick into an action.
    PickRef {
        purpose: PickPurpose,
    },
    /// Commit with the Source Control pane's message (the "..." menu's
    /// Commit entries); the app checks there is something to commit.
    Commit {
        scope: CommitScope,
        amend: bool,
        signoff: bool,
    },
    /// Show the log of git commands and their output.
    ShowGitOutput,
    /// Show where HEAD has been (the reflog), to recover lost commits.
    ShowReflog,
    /// Show the repository's worktrees, to add, remove or prune them.
    ShowWorktrees,
    /// Open the interactive rebase panel for the commits after `base`.
    OpenInteractiveRebase {
        base: Oid,
    },
    /// Open the hosting site's "new pull request" page for `branch` on `remote`.
    CreatePullRequest {
        remote: String,
        branch: String,
    },
    /// Mark `commit` as the older side of a later "Compare with Selected".
    SelectForCompare {
        commit: Oid,
    },
    /// List what changed from the marked commit `base` to `commit`.
    CompareWithSelected {
        base: Oid,
        commit: Oid,
    },
    /// List what changed from `commit` to the working tree.
    CompareWithWorkingTree {
        commit: Oid,
    },
    /// Show or hide a graph column (the column header's menu).
    ToggleColumn {
        column: GraphColumn,
    },
}

/// A graph column the header's menu can hide; the graph and the
/// description always show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GraphColumn {
    Date,
    Author,
    Commit,
}

impl GraphColumn {
    /// In the order they appear, left to right.
    pub const ALL: [Self; 3] = [Self::Date, Self::Author, Self::Commit];

    /// The header's name for it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Author => "Author",
            Self::Commit => "Commit",
        }
    }

    /// Its name in the config file (`hidden_columns`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Author => "author",
            Self::Commit => "commit",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// The dialog shown before an action runs (S13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmation {
    pub title: String,
    /// Names exactly what will happen.
    pub message: String,
    pub confirm_label: String,
}

impl Confirmation {
    pub(crate) fn new(title: &str, message: String, confirm_label: &str) -> Self {
        Self {
            title: title.to_owned(),
            message,
            confirm_label: confirm_label.to_owned(),
        }
    }
}

fn commit_label(commit: &Oid, subject: &str) -> String {
    format!("commit {} \"{subject}\"", commit.short(SHORT))
}

fn reset_confirmation(commit: &Oid, subject: &str, current: &str, mode: ResetMode) -> Confirmation {
    let target = commit_label(commit, subject);
    let message = match mode {
        ResetMode::Hard => format!(
            "Hard reset '{current}' to {target}? Later commits leave '{current}' and ALL uncommitted changes are discarded."
        ),
        ResetMode::Soft => format!(
            "Reset '{current}' to {target} (soft)? Later commits leave '{current}'; their changes stay staged."
        ),
        ResetMode::Mixed => format!(
            "Reset '{current}' to {target} (mixed)? Later commits leave '{current}'; their changes stay in the working tree, unstaged."
        ),
    };
    Confirmation::new("Reset Current Branch", message, "Reset")
}

fn branch_or_head(current: Option<&String>) -> String {
    current.map_or_else(|| "HEAD".to_owned(), |name| format!("'{name}'"))
}

impl Action {
    /// The git commands (or other effect) that carry out the action.
    ///
    /// User-supplied names always come after `--` so they can't be read as
    /// options. Call [`Action::validate`] first for input from a dialog.
    #[must_use]
    pub fn plan(&self, repo: &Path) -> Plan {
        match self {
            Self::ToggleBranchFilter { branch, .. } => Plan::Ui(UiEffect::ToggleBranchFilter {
                branch: branch.clone(),
            }),
            Self::CopyBranchName { name } => Plan::Clipboard(name.clone()),
            Self::CopyCommitHash { commit } => Plan::Clipboard(commit.to_string()),
            Self::CopyCommitSubject { subject } => Plan::Clipboard(subject.clone()),
            Self::PushBranch { .. } | Self::FetchIntoLocal { .. } => {
                Plan::Git(vec![GitCommand::network(repo, self.git_args())])
            }
            Self::AddTag {
                name,
                remote: Some(remote),
                push: true,
                ..
            } => Plan::Git(vec![
                GitCommand::write(repo, self.git_args()),
                GitCommand::network(
                    repo,
                    strings(&["push", "--", remote, &format!("refs/tags/{name}")]),
                ),
            ]),
            Self::Tag(action) => action.plan(repo),
            Self::Stash(action) => action.plan(repo),
            Self::WorkingTree(action) => action.plan(repo),
            Self::Repo(action) => action.plan(repo),
            Self::Worktree(action) => action.plan(repo),
            Self::Operation(action) => action.plan(repo),
            Self::InteractiveRebase(rebase) => rebase.plan(repo),
            Self::Ui(effect) => Plan::Ui(effect.clone()),
            _ => Plan::Git(vec![GitCommand::write(repo, self.git_args())]),
        }
    }

    /// `push [--set-upstream] [--force-with-lease] -- <remote> <refspec>`.
    fn push_args(&self) -> Vec<String> {
        let Self::PushBranch {
            branch,
            remote,
            set_upstream,
            force_with_lease,
        } = self
        else {
            return Vec::new();
        };
        let mut argv = strings(&["push"]);
        if *set_upstream {
            argv.push("--set-upstream".into());
        }
        if *force_with_lease {
            argv.push("--force-with-lease".into());
        }
        // An explicit refspec pushes exactly this branch, whatever
        // push.default says.
        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        argv.extend(strings(&["--", remote, &refspec]));
        argv
    }

    /// `fetch [--force] -- <remote> refs/heads/<branch>:refs/heads/<local>`:
    /// full ref names, so neither side can be read as something else.
    fn fetch_into_args(&self) -> Vec<String> {
        let Self::FetchIntoLocal {
            remote,
            branch,
            local_name,
            force,
        } = self
        else {
            return Vec::new();
        };
        let mut argv = strings(&["fetch"]);
        if *force {
            argv.push("--force".into());
        }
        let refspec = format!("refs/heads/{branch}:refs/heads/{local_name}");
        argv.extend(strings(&["--", remote, &refspec]));
        argv
    }

    /// The arguments after `git` for actions that run git (empty otherwise).
    fn git_args(&self) -> Vec<String> {
        match self {
            Self::CheckoutBranch { name } => strings(&["switch", "--", name]),
            Self::CheckoutRemoteBranch {
                remote,
                branch,
                local_name,
            } => strings(&[
                "switch",
                "--create",
                local_name,
                "--track",
                "--",
                &format!("{remote}/{branch}"),
            ]),
            Self::CreateBranch {
                at,
                name,
                checkout: true,
            } => strings(&["switch", "--create", name, "--", at.as_str()]),
            Self::CreateBranch { at, name, .. } => strings(&["branch", "--", name, at.as_str()]),
            Self::RenameBranch { from, to } => strings(&["branch", "--move", "--", from, to]),
            Self::DeleteBranch { name, force } => {
                let mut argv = strings(&["branch", "--delete"]);
                if *force {
                    argv.push("--force".into());
                }
                argv.extend(strings(&["--", name]));
                argv
            }
            Self::MergeIntoCurrent { source, no_ff, .. } => {
                let mut argv = strings(&["merge", "--no-edit"]);
                if *no_ff {
                    argv.push("--no-ff".into());
                }
                argv.extend(["--".into(), source.arg()]);
                argv
            }
            Self::RebaseCurrentOnto { onto, .. } => strings(&["rebase", "--", &onto.arg()]),
            Self::PushBranch { .. } => self.push_args(),
            Self::FetchIntoLocal { .. } => self.fetch_into_args(),
            Self::CreateArchive { rev, output } => {
                strings(&["archive", &format!("--output={output}"), "--", &rev.arg()])
            }
            Self::AddTag {
                at,
                name,
                annotated,
                message,
                ..
            } => {
                let mut argv = strings(&["tag"]);
                if *annotated {
                    argv.extend(strings(&["--annotate", "--message", message]));
                }
                argv.extend(strings(&["--", name, at.as_str()]));
                argv
            }
            Self::CheckoutCommit { commit, .. } => {
                strings(&["switch", "--detach", "--", commit.as_str()])
            }
            Self::CherryPick {
                commit, is_merge, ..
            } => with_mainline(strings(&["cherry-pick"]), *is_merge, commit),
            Self::Revert {
                commit, is_merge, ..
            } => with_mainline(strings(&["revert", "--no-edit"]), *is_merge, commit),
            // The interactive-rebase "drop" without an editor: replay the
            // commits after `commit` onto its parent.
            Self::Drop { commit, .. } => strings(&[
                "rebase",
                "--onto",
                &format!("{commit}^"),
                "--",
                commit.as_str(),
            ]),
            // `--` goes after the commit: for reset, what follows it are paths.
            Self::ResetCurrentTo { commit, mode, .. } => strings(&[
                "reset",
                &format!("--{}", mode.name()),
                commit.as_str(),
                "--",
            ]),
            Self::ToggleBranchFilter { .. }
            | Self::CopyBranchName { .. }
            | Self::CopyCommitHash { .. }
            | Self::CopyCommitSubject { .. }
            | Self::Tag(_)
            | Self::Stash(_)
            | Self::WorkingTree(_)
            | Self::Repo(_)
            | Self::Worktree(_)
            | Self::Operation(_)
            | Self::InteractiveRebase(_)
            | Self::Ui(_) => Vec::new(),
        }
    }

    /// Would start another merge, rebase, cherry-pick or revert, or move
    /// HEAD or the index (checkout, reset, pull): refused while git is in the
    /// middle of one. Staging, committing and discarding stay allowed, since
    /// they are how conflicts get resolved.
    #[must_use]
    pub fn conflicts_with_operation(&self) -> bool {
        matches!(
            self,
            Self::CheckoutBranch { .. }
                | Self::CheckoutRemoteBranch { .. }
                | Self::CheckoutCommit { .. }
                | Self::MergeIntoCurrent { .. }
                | Self::RebaseCurrentOnto { .. }
                | Self::CherryPick { .. }
                | Self::Revert { .. }
                | Self::Drop { .. }
                | Self::InteractiveRebase(_)
                | Self::Ui(UiEffect::OpenInteractiveRebase { .. })
                | Self::ResetCurrentTo { .. }
                | Self::CreateBranch { checkout: true, .. }
                | Self::WorkingTree(WorkingTreeAction::Reset { .. })
                | Self::Stash(
                    StashAction::Apply { .. }
                        | StashAction::Pop { .. }
                        | StashAction::CreateBranch { .. }
                )
                | Self::Repo(
                    RepoAction::Sync
                        | RepoAction::PullUpstream { .. }
                        | RepoAction::PullFrom { .. }
                        | RepoAction::ApplyLatestStash
                        | RepoAction::PopLatestStash
                )
        )
    }

    /// Can lose work or rewrite history.
    #[must_use]
    pub fn is_destructive(&self) -> bool {
        match self {
            Self::DeleteBranch { .. }
            | Self::Drop { .. }
            | Self::RebaseCurrentOnto { .. }
            | Self::InteractiveRebase(_) => true,
            Self::ResetCurrentTo { mode, .. } => *mode == ResetMode::Hard,
            Self::PushBranch {
                force_with_lease, ..
            } => *force_with_lease,
            Self::FetchIntoLocal { force, .. } => *force,
            Self::Tag(action) => action.is_destructive(),
            Self::Stash(action) => action.is_destructive(),
            Self::WorkingTree(action) => action.is_destructive(),
            Self::Repo(action) => action.is_destructive(),
            Self::Worktree(action) => action.is_destructive(),
            Self::Operation(action) => action.is_destructive(),
            _ => false,
        }
    }

    /// The confirmation dialog to show first, if any. Every destructive
    /// action has one, and so do resets and detached checkouts.
    #[must_use]
    pub fn confirmation(&self) -> Option<Confirmation> {
        let confirmation = match self {
            Self::DeleteBranch { name, force: false } => Confirmation::new(
                "Delete Branch",
                format!("Delete the local branch '{name}'? git refuses if it isn't fully merged."),
                "Delete",
            ),
            Self::DeleteBranch { name, force: true } => Confirmation::new(
                "Delete Branch",
                format!(
                    "Force delete the local branch '{name}'? Commits that are only on this branch will be lost."
                ),
                "Force Delete",
            ),
            Self::ResetCurrentTo {
                commit,
                subject,
                current,
                mode,
            } => reset_confirmation(commit, subject, current, *mode),
            Self::PushBranch {
                branch,
                remote,
                force_with_lease: true,
                ..
            } => Confirmation::new(
                "Force Push",
                format!(
                    "Force push '{branch}' to '{remote}'? Commits on {remote}/{branch} that aren't in your '{branch}' will be overwritten. The push is refused if {remote}/{branch} changed since your last fetch (--force-with-lease)."
                ),
                "Force Push",
            ),
            Self::FetchIntoLocal {
                remote,
                branch,
                local_name,
                force: true,
            } => Confirmation::new(
                "Force Fetch",
                format!(
                    "Force fetch {remote}/{branch} into the local branch '{local_name}'? Commits on '{local_name}' that aren't on {remote}/{branch} will be lost."
                ),
                "Force Fetch",
            ),
            Self::CheckoutCommit { commit, subject } => Confirmation::new(
                "Checkout Commit",
                format!(
                    "Check out {}? HEAD will be detached: new commits won't belong to any branch until you create one.",
                    commit_label(commit, subject)
                ),
                "Checkout",
            ),
            Self::Drop {
                commit,
                subject,
                current,
            } => {
                let short = commit.short(SHORT);
                let branch = branch_or_head(current.as_ref());
                Confirmation::new(
                    "Drop Commit",
                    format!(
                        "Drop {} from {branch}? The commits after it are rebased onto its parent (git rebase --onto {short}^ {short}), which rewrites the history of {branch}.",
                        commit_label(commit, subject)
                    ),
                    "Drop",
                )
            }
            Self::RebaseCurrentOnto { onto, current } => {
                let branch = branch_or_head(current.as_ref());
                let onto = onto.describe();
                Confirmation::new(
                    "Rebase Current Branch",
                    format!(
                        "Rebase {branch} on {onto}? The commits of {branch} that aren't in {onto} are rewritten on top of it."
                    ),
                    "Rebase",
                )
            }
            Self::Tag(action) => return action.confirmation(),
            Self::Stash(action) => return action.confirmation(),
            Self::WorkingTree(action) => return action.confirmation(),
            Self::Repo(action) => return action.confirmation(),
            Self::Worktree(action) => return action.confirmation(),
            Self::Operation(action) => return action.confirmation(),
            Self::InteractiveRebase(rebase) => rebase.confirmation(),
            _ => return None,
        };
        Some(confirmation)
    }

    /// Name, type, message and, with a remote, "Push to remote", as in
    /// VS Code's Add Tag dialog.
    fn add_tag_inputs(&self) -> Vec<InputSpec> {
        let Self::AddTag {
            name,
            annotated,
            message,
            remote,
            push,
            ..
        } = self
        else {
            return Vec::new();
        };
        let mut inputs = vec![
            InputSpec::text(InputId::Name, "Tag name", InputKind::RefName, name),
            InputSpec::text(
                InputId::TagType,
                "Type",
                InputKind::Choice(TAG_TYPES),
                TAG_TYPES[usize::from(!annotated)],
            ),
            InputSpec::text(
                InputId::Message,
                "Message (optional, annotated tags only)",
                InputKind::Text,
                message,
            ),
        ];
        if remote.is_some() {
            inputs.push(InputSpec::toggle(
                InputId::PushToRemote,
                "Push to remote",
                *push,
            ));
        }
        inputs
    }

    /// The fields its dialog asks for, prefilled with the current values.
    #[must_use]
    pub fn inputs(&self) -> Vec<InputSpec> {
        use InputKind::{Path, RefName, Remote};
        match self {
            Self::CheckoutRemoteBranch { local_name, .. } => vec![InputSpec::text(
                InputId::Name,
                "Name of the new local branch",
                RefName,
                local_name,
            )],
            Self::FetchIntoLocal {
                local_name, force, ..
            } => vec![
                InputSpec::text(
                    InputId::Name,
                    "Name of the local branch",
                    RefName,
                    local_name,
                ),
                InputSpec::toggle(InputId::Force, "Force fetch", *force),
            ],
            Self::CreateBranch { name, checkout, .. } => vec![
                InputSpec::text(InputId::Name, "Branch name", RefName, name),
                InputSpec::toggle(InputId::Checkout, "Check out the new branch", *checkout),
            ],
            Self::RenameBranch { to, .. } => {
                vec![InputSpec::text(InputId::Name, "New name", RefName, to)]
            }
            Self::DeleteBranch { force, .. } => vec![InputSpec::toggle(
                InputId::Force,
                "Force delete, even if not fully merged",
                *force,
            )],
            Self::MergeIntoCurrent { no_ff, .. } => vec![InputSpec::toggle(
                InputId::NoFastForward,
                "Create a merge commit even if fast-forward is possible",
                *no_ff,
            )],
            Self::PushBranch {
                remote,
                set_upstream,
                force_with_lease,
                ..
            } => vec![
                InputSpec::text(InputId::Remote, "Remote", Remote, remote),
                InputSpec::toggle(InputId::SetUpstream, "Set upstream", *set_upstream),
                InputSpec::toggle(
                    InputId::ForceWithLease,
                    "Force push (with lease)",
                    *force_with_lease,
                ),
            ],
            Self::CreateArchive { output, .. } => {
                vec![InputSpec::text(
                    InputId::Output,
                    "Save archive as",
                    Path,
                    output,
                )]
            }
            Self::AddTag { .. } => self.add_tag_inputs(),
            Self::ResetCurrentTo { mode, .. } => vec![InputSpec::text(
                InputId::Mode,
                "Mode",
                InputKind::Choice(ResetMode::CHOICES),
                mode.name(),
            )],
            Self::Tag(action) => action.inputs(),
            Self::Stash(action) => action.inputs(),
            Self::WorkingTree(action) => action.inputs(),
            Self::Repo(action) => action.inputs(),
            Self::Worktree(action) => action.inputs(),
            _ => Vec::new(),
        }
    }

    /// Sets one dialog field, validating it.
    pub fn set_input(&mut self, id: InputId, value: InputValue) -> Result<(), InputError> {
        match (self, id) {
            (
                Self::CheckoutRemoteBranch {
                    local_name: field, ..
                }
                | Self::FetchIntoLocal {
                    local_name: field, ..
                }
                | Self::CreateBranch { name: field, .. }
                | Self::RenameBranch { to: field, .. }
                | Self::AddTag { name: field, .. },
                InputId::Name,
            ) => *field = input::ref_name(id, value)?,
            (Self::PushBranch { remote, .. }, InputId::Remote) => {
                *remote = input::ref_name(id, value)?;
            }
            (Self::AddTag { message, .. }, InputId::Message) => *message = input::text(id, value)?,
            (Self::AddTag { annotated, .. }, InputId::TagType) => {
                let name = input::text(id, value)?;
                *annotated = match TAG_TYPES.iter().position(|t| *t == name) {
                    Some(index) => index == 0,
                    None => return Err(InputError::NotAChoice(name)),
                };
            }
            (
                Self::AddTag {
                    remote: Some(_),
                    push,
                    ..
                },
                InputId::PushToRemote,
            ) => *push = input::toggle(id, &value)?,
            (Self::CreateArchive { output, .. }, InputId::Output) => {
                *output = input::required(id, value)?;
            }
            (
                Self::CreateBranch {
                    checkout: field, ..
                },
                InputId::Checkout,
            )
            | (
                Self::DeleteBranch { force: field, .. } | Self::FetchIntoLocal { force: field, .. },
                InputId::Force,
            )
            | (Self::MergeIntoCurrent { no_ff: field, .. }, InputId::NoFastForward)
            | (
                Self::PushBranch {
                    set_upstream: field,
                    ..
                },
                InputId::SetUpstream,
            )
            | (
                Self::PushBranch {
                    force_with_lease: field,
                    ..
                },
                InputId::ForceWithLease,
            ) => *field = input::toggle(id, &value)?,
            (Self::ResetCurrentTo { mode, .. }, InputId::Mode) => {
                let name = input::text(id, value)?;
                *mode = ResetMode::from_name(&name).ok_or(InputError::NotAChoice(name))?;
            }
            (Self::Tag(action), _) => return action.set_input(id, value),
            (Self::Stash(action), _) => return action.set_input(id, value),
            (Self::WorkingTree(action), _) => return action.set_input(id, value),
            (Self::Repo(action), _) => return action.set_input(id, value),
            (Self::Worktree(action), _) => return action.set_input(id, value),
            _ => return Err(InputError::NotAnInput(id)),
        }
        Ok(())
    }

    /// Checks every field, e.g. before running an action built from a dialog.
    pub fn validate(&self) -> Result<(), InputError> {
        for spec in self.inputs() {
            match (spec.kind, &spec.value) {
                (InputKind::RefName | InputKind::Remote, InputValue::Text(name)) => {
                    crate::validate_ref_name(name)?;
                }
                (InputKind::Path, InputValue::Text(path)) if path.trim().is_empty() => {
                    return Err(InputError::Required(spec.id));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub(crate) fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn with_mainline(mut argv: Vec<String>, is_merge: bool, commit: &Oid) -> Vec<String> {
    // A merge has several parents; replaying it relative to the first one
    // (the branch it was made on) is what users mean.
    if is_merge {
        argv.extend(["--mainline".into(), "1".into()]);
    }
    argv.extend(["--".into(), commit.to_string()]);
    argv
}

#[cfg(test)]
mod tests;
