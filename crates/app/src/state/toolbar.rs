//! The toolbar (S2), the branch filter and the ref pickers it and the
//! repository menu open (S11).

use crossterm::event::KeyEvent;
use ggt_actions::{
    Action, MenuItem, PickPurpose, RepoAction, Rev, StashAction, TagAction, Target, menu_for,
};
use ggt_git::log::Revisions;
use ggt_model::{Ref, RefKind, split_remote};
use ggt_ui::{
    Picker, PickerHitMap, PickerItem, PickerOutcome, PopupMenu, Toolbar, ToolbarButton,
    ToolbarHitMap, ToolbarOutcome, glob_match,
};
use ratatui::layout::Position;

use super::{AppState, Command, actions::widget_key};

/// What an open picker chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerFor {
    /// The toolbar's branch filter (multi-select).
    BranchFilter,
    /// A ref for a repository menu action.
    Ref(PickPurpose),
}

/// An open picker and where the last frame drew it.
#[derive(Debug)]
pub struct OpenPicker {
    pub view: Picker,
    pub purpose: PickerFor,
    pub hits: Option<PickerHitMap>,
}

/// The toolbar's state; the widget is rebuilt from it every frame.
#[derive(Debug)]
pub struct ToolbarState {
    /// Branches (short names or globs) the graph is limited to; empty shows all.
    pub branch_filter: Vec<String>,
    pub show_remotes: bool,
    /// The focused button while the toolbar has the keyboard.
    pub focus: Option<ToolbarButton>,
    pub hits: ToolbarHitMap,
    /// Whether the toolbar shows "..." (not while the Source Control pane does).
    pub more: bool,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            branch_filter: Vec::new(),
            show_remotes: true,
            focus: None,
            hits: ToolbarHitMap::default(),
            more: true,
        }
    }
}

impl ToolbarState {
    pub fn widget(&self) -> Toolbar {
        let mut toolbar = Toolbar::new(&self.branch_filter, self.show_remotes);
        if !self.more {
            toolbar = toolbar.without_more();
        }
        toolbar.set_focus(self.focus);
        toolbar
    }
}

impl AppState {
    /// The history `git log` loads: everything, or the filtered branches.
    /// None when the filter matches no branch, so there is nothing to load.
    pub(super) fn log_revisions(&self) -> Option<Revisions> {
        let filter = &self.toolbar.branch_filter;
        if filter.is_empty() {
            return Some(Revisions::All {
                remotes: self.toolbar.show_remotes,
            });
        }
        let refs: Vec<String> = self
            .refs
            .iter()
            .filter(|r| self.is_branch_shown(r))
            .filter(|r| filter.iter().any(|p| glob_match(p, r.short_name())))
            .map(|r| r.full_name.clone())
            .collect();
        (!refs.is_empty()).then_some(Revisions::Only(refs))
    }

    fn is_branch_shown(&self, r: &Ref) -> bool {
        match r.kind {
            RefKind::LocalBranch => true,
            RefKind::RemoteBranch => self.toolbar.show_remotes,
            _ => false,
        }
    }

    /// Refs the graph shows as badges (remote ones only when shown).
    pub fn visible_refs(&self) -> Vec<Ref> {
        self.refs
            .iter()
            .filter(|r| {
                // Each stash gets its own badge below, not just the newest.
                r.kind != RefKind::Stash
                    && (self.toolbar.show_remotes
                        || !matches!(r.kind, RefKind::RemoteBranch | RefKind::RemoteHead))
            })
            .cloned()
            .chain(self.stashes.iter().map(stash_ref))
            .collect()
    }

    /// The stash a stash badge (`refs/stash@{n}`) stands for.
    pub(super) fn stash_named(&self, full_name: &str) -> Option<&ggt_model::Stash> {
        self.stashes
            .iter()
            .find(|stash| stash_ref(stash).full_name == full_name)
    }

    /// Tab gives the keyboard to the toolbar.
    pub(super) fn focus_toolbar(&mut self) {
        self.toolbar.focus = Some(ToolbarButton::BranchFilter);
    }

    /// Keys while the toolbar has the keyboard.
    pub(super) fn on_toolbar_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(key) = widget_key(key) else {
            return Vec::new();
        };
        let mut toolbar = self.toolbar.widget();
        let outcome = toolbar.handle_key(key, &self.toolbar.hits);
        self.toolbar.focus = toolbar.focus();
        self.on_toolbar_outcome(outcome)
    }

    /// A left click on a toolbar button; None when the click missed the toolbar.
    pub(super) fn on_toolbar_click(&mut self, x: u16, y: u16) -> Option<Vec<Command>> {
        self.toolbar.hits.hit(x, y)?;
        let mut toolbar = self.toolbar.widget();
        let outcome = toolbar.handle_click(&self.toolbar.hits, x, y);
        // A click presses the button without moving the keyboard to the toolbar.
        self.toolbar.focus = None;
        Some(self.on_toolbar_outcome(outcome))
    }

    fn on_toolbar_outcome(&mut self, outcome: ToolbarOutcome) -> Vec<Command> {
        match outcome {
            ToolbarOutcome::Stay => Vec::new(),
            ToolbarOutcome::Leave => {
                self.toolbar.focus = None;
                Vec::new()
            }
            ToolbarOutcome::OpenBranchFilter(anchor) => {
                let items = self.picker_items(PickerFor::BranchFilter);
                let view = Picker::multi("Branches", items, &self.toolbar.branch_filter)
                    .with_anchor(anchor);
                self.open_picker(view, PickerFor::BranchFilter);
                Vec::new()
            }
            ToolbarOutcome::ToggleRemotes => {
                self.toolbar.show_remotes = !self.toolbar.show_remotes;
                self.refresh()
            }
            ToolbarOutcome::Refresh => {
                self.message = None;
                self.refresh()
            }
            ToolbarOutcome::Fetch => self.start_action(
                Action::Repo(RepoAction::Fetch { prune: false }),
                "Fetch".into(),
            ),
            ToolbarOutcome::OpenRepoMenu(anchor) => {
                self.open_repo_menu(anchor);
                Vec::new()
            }
        }
    }

    pub(super) fn open_repo_menu(&mut self, anchor: Position) {
        let items: Vec<MenuItem> = menu_for(&Target::Repository, &self.menu_context());
        self.menu = Some(PopupMenu::new(items, anchor));
        self.menu_hits = None;
    }

    /// Adds `branch` to the filter, or removes it when it is already there.
    pub(super) fn toggle_branch_filter(&mut self, branch: String) -> Vec<Command> {
        let filter = &mut self.toolbar.branch_filter;
        if let Some(index) = filter.iter().position(|b| *b == branch) {
            filter.remove(index);
        } else {
            filter.push(branch);
            filter.sort();
        }
        self.refresh()
    }

    /// Opens the picker for a repository menu entry, centered.
    pub(super) fn pick_ref(&mut self, purpose: PickPurpose) {
        let title = picker_title(purpose);
        let items = self.picker_items(PickerFor::Ref(purpose));
        if items.is_empty() {
            self.message = Some(format!("{title}: nothing to choose from."));
            return;
        }
        self.open_picker(Picker::single(title, items), PickerFor::Ref(purpose));
    }

    fn open_picker(&mut self, view: Picker, purpose: PickerFor) {
        self.picker = Some(OpenPicker {
            view,
            purpose,
            hits: None,
        });
    }

    fn picker_items(&self, purpose: PickerFor) -> Vec<PickerItem> {
        let item = |value: &str, kind: &str| PickerItem {
            value: value.to_owned(),
            kind: Some(kind.to_owned()),
            search_kind: false,
        };
        let current = match &self.status.head {
            ggt_model::HeadState::Branch(name) => Some(name.as_str()),
            _ => None,
        };
        let kind = |r: &Ref| match r.kind {
            RefKind::LocalBranch => Some("branch"),
            RefKind::RemoteBranch => Some("remote"),
            RefKind::Tag => Some("tag"),
            _ => None,
        };
        match purpose {
            PickerFor::BranchFilter => self
                .refs
                .iter()
                .filter(|r| self.is_branch_shown(r))
                .filter_map(|r| Some(item(r.short_name(), kind(r)?)))
                .collect(),
            PickerFor::Ref(PickPurpose::Checkout) => self
                .refs
                .iter()
                .filter(|r| Some(r.short_name()) != current || r.kind != RefKind::LocalBranch)
                .filter_map(|r| Some(item(r.short_name(), kind(r)?)))
                .collect(),
            PickerFor::Ref(PickPurpose::DeleteBranch) => self
                .refs
                .iter()
                .filter(|r| r.kind == RefKind::LocalBranch && Some(r.short_name()) != current)
                .map(|r| item(r.short_name(), "branch"))
                .collect(),
            // Branches other than the current one, local then remote.
            PickerFor::Ref(PickPurpose::Merge | PickPurpose::Rebase) => self
                .refs
                .iter()
                .filter(|r| {
                    r.kind == RefKind::RemoteBranch
                        || (r.kind == RefKind::LocalBranch && Some(r.short_name()) != current)
                })
                .filter_map(|r| Some(item(r.short_name(), kind(r)?)))
                .collect(),
            PickerFor::Ref(PickPurpose::CreateBranchFrom) => self
                .refs
                .iter()
                .filter_map(|r| Some(item(r.short_name(), kind(r)?)))
                .collect(),
            PickerFor::Ref(PickPurpose::DeleteRemoteBranch) => self
                .refs
                .iter()
                .filter(|r| r.kind == RefKind::RemoteBranch)
                .map(|r| item(r.short_name(), "remote"))
                .collect(),
            PickerFor::Ref(
                PickPurpose::ApplyStash
                | PickPurpose::PopStash
                | PickPurpose::DropStash
                | PickPurpose::ViewStash,
            ) => self
                .stashes
                .iter()
                .map(|stash| PickerItem {
                    search_kind: true,
                    ..item(&stash.selector(), &stash.subject)
                })
                .collect(),
            PickerFor::Ref(PickPurpose::DeleteTag | PickPurpose::DeleteRemoteTag) => self
                .refs
                .iter()
                .filter(|r| r.kind == RefKind::Tag)
                .map(|r| item(r.short_name(), "tag"))
                .collect(),
            PickerFor::Ref(PickPurpose::RemoveRemote) => self
                .menu_context()
                .remotes
                .iter()
                .map(|name| item(name, "remote"))
                .collect(),
        }
    }

    pub(super) fn on_picker_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let (Some(key), Some(picker)) = (widget_key(key), &mut self.picker) else {
            return Vec::new();
        };
        let outcome = picker.view.handle_key(key);
        self.on_picker_outcome(outcome)
    }

    pub(super) fn on_picker_click(&mut self, x: u16, y: u16) -> Vec<Command> {
        let Some(picker) = &mut self.picker else {
            return Vec::new();
        };
        let Some(hits) = picker.hits.clone() else {
            return Vec::new();
        };
        let outcome = picker.view.handle_click(&hits, x, y);
        self.on_picker_outcome(outcome)
    }

    fn on_picker_outcome(&mut self, outcome: PickerOutcome) -> Vec<Command> {
        if outcome == PickerOutcome::Pending {
            return Vec::new();
        }
        let Some(picker) = self.picker.take() else {
            return Vec::new();
        };
        match (picker.purpose, outcome) {
            (PickerFor::BranchFilter, PickerOutcome::PickedMany(branches)) => {
                self.toolbar.branch_filter = branches;
                self.refresh()
            }
            (PickerFor::Ref(purpose), PickerOutcome::Picked(name)) => {
                let action = match purpose {
                    PickPurpose::Checkout => self.checkout_action(&name),
                    PickPurpose::Merge
                    | PickPurpose::Rebase
                    | PickPurpose::CreateBranchFrom
                    | PickPurpose::DeleteRemoteBranch => self.picked_ref_action(purpose, &name),
                    PickPurpose::ViewStash => {
                        let Some(stash) = self.stashes.iter().find(|s| s.selector() == name) else {
                            return Vec::new();
                        };
                        let stash = stash.clone();
                        return self.open_stash_diff(&stash);
                    }
                    PickPurpose::ApplyStash
                    | PickPurpose::PopStash
                    | PickPurpose::DropStash
                    | PickPurpose::DeleteTag
                    | PickPurpose::DeleteRemoteTag => self.picked_action(purpose, &name),
                    PickPurpose::DeleteBranch | PickPurpose::RemoveRemote => {
                        purpose.action_for(&name)
                    }
                };
                if let Some(action) = action {
                    let label = format!("{} {name}", picker_title(purpose));
                    return self.start_action(action, label);
                }
                self.message = Some(format!("{name} is not a branch or tag."));
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// What the Stash and Tags submenus' pickers do with the picked stash
    /// (`stash@{n}`) or tag.
    fn picked_action(&self, purpose: PickPurpose, name: &str) -> Option<Action> {
        let stash = || self.stashes.iter().find(|s| s.selector() == name).cloned();
        let remote = self.menu_context().default_remote().map(str::to_owned);
        Some(match purpose {
            PickPurpose::ApplyStash => Action::Stash(StashAction::Apply {
                stash: stash()?,
                reinstate_index: false,
            }),
            PickPurpose::PopStash => Action::Stash(StashAction::Pop {
                stash: stash()?,
                reinstate_index: false,
            }),
            PickPurpose::DropStash => Action::Stash(StashAction::Drop { stash: stash()? }),
            PickPurpose::DeleteTag => Action::Tag(TagAction::Delete {
                name: name.to_owned(),
                remote,
                on_remote: false,
            }),
            PickPurpose::DeleteRemoteTag => Action::Tag(TagAction::DeleteRemote {
                name: name.to_owned(),
                remote: remote?,
            }),
            _ => return None,
        })
    }

    /// What Branch ▸ Merge, Rebase Branch, Create Branch From and Delete
    /// Remote Branch do with the picked branch or tag.
    fn picked_ref_action(&self, purpose: PickPurpose, name: &str) -> Option<Action> {
        let r = self.refs.iter().find(|r| {
            r.short_name() == name
                && matches!(
                    r.kind,
                    RefKind::LocalBranch | RefKind::RemoteBranch | RefKind::Tag
                )
        })?;
        let rev = match r.kind {
            RefKind::LocalBranch => Rev::Local(name.to_owned()),
            RefKind::RemoteBranch => {
                let (remote, branch) = split_remote(&r.full_name)?;
                Rev::Remote {
                    remote: remote.to_owned(),
                    branch: branch.to_owned(),
                }
            }
            _ => Rev::Tag(name.to_owned()),
        };
        let current = self.menu_context().current_branch().map(str::to_owned);
        Some(match (purpose, rev) {
            (PickPurpose::Merge, source) => Action::MergeIntoCurrent {
                source,
                current,
                no_ff: false,
                squash: false,
                no_commit: false,
            },
            (PickPurpose::Rebase, onto) => Action::RebaseCurrentOnto { onto, current },
            (PickPurpose::CreateBranchFrom, _) => Action::CreateBranch {
                at: r.target.clone(),
                name: String::new(),
                checkout: true,
            },
            (PickPurpose::DeleteRemoteBranch, Rev::Remote { remote, branch }) => {
                Action::Repo(RepoAction::DeleteRemoteBranch { remote, branch })
            }
            _ => return None,
        })
    }

    /// The checkout for a picked short name: a local branch, a remote branch
    /// (creating a local branch that tracks it) or a tag (detached).
    pub(super) fn checkout_action(&self, name: &str) -> Option<Action> {
        let r = self.refs.iter().find(|r| {
            r.short_name() == name
                && matches!(
                    r.kind,
                    RefKind::LocalBranch | RefKind::RemoteBranch | RefKind::Tag
                )
        })?;
        Some(match r.kind {
            RefKind::LocalBranch => Action::CheckoutBranch {
                name: name.to_owned(),
            },
            RefKind::RemoteBranch => {
                let (remote, branch) = split_remote(&r.full_name)?;
                Action::CheckoutRemoteBranch {
                    remote: remote.to_owned(),
                    branch: branch.to_owned(),
                    local_name: branch.to_owned(),
                }
            }
            _ => Action::CheckoutCommit {
                commit: r.target.clone(),
                subject: format!("tag {name}"),
            },
        })
    }
}

fn picker_title(purpose: PickPurpose) -> &'static str {
    match purpose {
        PickPurpose::Checkout => "Checkout",
        PickPurpose::DeleteBranch => "Delete Branch",
        PickPurpose::Merge => "Merge",
        PickPurpose::Rebase => "Rebase onto",
        PickPurpose::CreateBranchFrom => "Create Branch From",
        PickPurpose::DeleteRemoteBranch => "Delete Remote Branch",
        PickPurpose::ApplyStash => "Apply Stash",
        PickPurpose::PopStash => "Pop Stash",
        PickPurpose::DropStash => "Drop Stash",
        PickPurpose::ViewStash => "View Stash",
        PickPurpose::DeleteTag => "Delete Tag",
        PickPurpose::DeleteRemoteTag => "Delete Remote Tag",
        PickPurpose::RemoveRemote => "Remove Remote",
    }
}

/// A badge for `stash`: `stash@{n}` on the stash's row.
fn stash_ref(stash: &ggt_model::Stash) -> Ref {
    Ref {
        full_name: format!("refs/{}", stash.selector()),
        kind: RefKind::Stash,
        target: stash.id.clone(),
        upstream: None,
        is_head: false,
    }
}
