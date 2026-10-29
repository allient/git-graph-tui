//! Diffs of uncommitted changes, opened from the Source Control pane: one
//! file (Enter), a whole section ("Open Changes"), and "Open File" in the
//! user's editor.

use tgg_git::show::{self, Diff};
use tgg_model::{Change, Stash};
use tgg_ui::{DiffView, PaneFile};

use super::{AppState, Command, Request};
use crate::external;

/// What the reads are put together into.
#[derive(Debug)]
enum Shape {
    /// One file's diff.
    File { title: String },
    /// Every file of a section, one after the other.
    Section { title: String, files: Vec<PaneFile> },
}

/// A Source Control diff whose reads are in flight.
#[derive(Debug)]
pub struct WorkingDiffLoad {
    shape: Shape,
    /// Every read the view needs, with its output once it arrived.
    parts: Vec<(Request, Option<String>)>,
}

/// Which side of the changes a section shows, for titles.
fn side(staged: bool) -> &'static str {
    if staged { "staged" } else { "working tree" }
}

/// A file's path, with where it came from for a rename.
fn display_path(file: &PaneFile) -> String {
    match &file.orig_path {
        Some(old) => format!("{old} → {}", file.path),
        None => file.path.clone(),
    }
}

/// The read for one file's diff: an untracked file against nothing, others
/// in their section's side (both paths of a rename).
fn file_request(file: &PaneFile) -> Request {
    if file.change == Change::Untracked {
        return Request::UntrackedDiff(file.path.clone());
    }
    let paths = file
        .orig_path
        .iter()
        .chain(std::iter::once(&file.path))
        .cloned()
        .collect();
    Request::WorkingDiff {
        staged: file.staged,
        paths,
    }
}

/// The git arguments of a diff read, for a pager.
fn diff_args(request: &Request) -> Vec<String> {
    match request {
        Request::UntrackedDiff(path) => show::untracked_diff_args(path),
        Request::WorkingDiff { staged, paths } => show::working_diff_args(*staged, paths),
        Request::StashDiff(selector) => tgg_git::stash::show_args(selector),
        _ => Vec::new(),
    }
}

impl AppState {
    /// Enter on a file of the pane: its diff, in the pager when there is one.
    pub(super) fn open_working_diff(&mut self, file: &PaneFile) -> Vec<Command> {
        let request = file_request(file);
        if let Some(pager) = &self.pager {
            let args = diff_args(&request);
            return vec![Command::External(external::pager_command(
                &self.repo, &args, pager,
            ))];
        }
        let title = format!("{} ({})", display_path(file), side(file.staged));
        self.load_working_diff(Shape::File { title }, vec![request])
    }

    /// Stash ▸ "View Stash...": the stash's changes, in the pager when
    /// there is one.
    pub(super) fn open_stash_diff(&mut self, stash: &Stash) -> Vec<Command> {
        let request = Request::StashDiff(stash.selector());
        if let Some(pager) = &self.pager {
            let args = diff_args(&request);
            return vec![Command::External(external::pager_command(
                &self.repo, &args, pager,
            ))];
        }
        let title = format!("{}: {}", stash.selector(), stash.subject);
        self.load_working_diff(Shape::File { title }, vec![request])
    }

    /// "Open Changes": every file of a section in one view. Tracked files
    /// come from one diff of the whole section, untracked ones one by one.
    pub(super) fn open_section_diff(&mut self, staged: bool, files: Vec<PaneFile>) -> Vec<Command> {
        if files.is_empty() {
            return Vec::new();
        }
        let mut requests = Vec::new();
        if files.iter().any(|f| f.change != Change::Untracked) {
            requests.push(Request::WorkingDiff {
                staged,
                paths: Vec::new(),
            });
        }
        requests.extend(
            files
                .iter()
                .filter(|f| f.change == Change::Untracked)
                .map(file_request),
        );
        let name = if staged { "Staged Changes" } else { "Changes" };
        let title = format!("{name} ({})", side(staged));
        self.load_working_diff(Shape::Section { title, files }, requests)
    }

    fn load_working_diff(&mut self, shape: Shape, requests: Vec<Request>) -> Vec<Command> {
        self.working_diff = Some(WorkingDiffLoad {
            shape,
            parts: requests.iter().cloned().map(|r| (r, None)).collect(),
        });
        requests.into_iter().map(Command::Git).collect()
    }

    /// One read of the diff being loaded; the view opens with the last one.
    /// Reads for a diff that has since been replaced are ignored.
    pub(super) fn on_working_diff_read(&mut self, request: &Request, stdout: String) {
        let Some(load) = &mut self.working_diff else {
            return;
        };
        let Some(slot) = load
            .parts
            .iter_mut()
            .find(|(r, out)| r == request && out.is_none())
        else {
            return;
        };
        slot.1 = Some(stdout);
        if load.parts.iter().any(|(_, out)| out.is_none()) {
            return;
        }
        let Some(load) = self.working_diff.take() else {
            return;
        };
        let output: String = load.parts.into_iter().filter_map(|(_, out)| out).collect();
        let view = match load.shape {
            Shape::File { title } => DiffView::titled(title, show::parse_diff(&output)),
            Shape::Section { title, files } => {
                let mut blocks = show::split_diff(&output);
                let entries: Vec<(String, Diff)> = files
                    .iter()
                    .filter_map(|file| {
                        let at = blocks.iter().position(|(path, _)| *path == file.path)?;
                        let (_, diff) = blocks.remove(at);
                        let label = format!("{} {}", display_path(file), file.change.letter());
                        Some((label, diff))
                    })
                    .collect();
                DiffView::multi(title, entries)
            }
        };
        self.show_diff(view);
    }

    /// "Open File": the file in the editor, which gets the terminal; the
    /// repository is reloaded once it exits.
    pub(super) fn open_in_editor(&mut self, file: &PaneFile) -> Vec<Command> {
        if !file.on_disk {
            self.message = Some(format!("Can't open {}: it was deleted.", file.path));
            return Vec::new();
        }
        vec![Command::External(external::editor_command(
            &self.repo,
            &self.editor,
            &file.path,
        ))]
    }
}
