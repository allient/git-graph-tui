//! Comparing two commits, or a commit with the working tree (S12), in the
//! inline details panel.

use ggt_git::show::{self, FileChange};
use ggt_model::Oid;
use ggt_ui::{CommitDetailsView, DiffView};

use super::details::Details;
use super::{AppState, Command, Request};

/// Characters of a hash in comparison titles and messages.
const SHORT: usize = 7;

/// The two sides of a comparison: from `base` to `other`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// The older side.
    pub base: Oid,
    /// The newer side; None is the working tree.
    pub other: Option<Oid>,
}

impl Comparison {
    /// `Comparing abc1234..def5678` or `Comparing abc1234..working tree`.
    fn title(&self) -> String {
        let other = self
            .other
            .as_ref()
            .map_or_else(|| "working tree".to_owned(), |o| o.short(SHORT).to_owned());
        format!("Comparing {}..{other}", self.base.short(SHORT))
    }

    /// Arguments listing the files that differ.
    pub fn files_args(&self) -> Vec<String> {
        show::compare_files_args(&self.base, self.other.as_ref())
    }

    /// Arguments for one file's diff.
    pub fn diff_args(&self, file: &FileChange) -> Vec<String> {
        show::compare_diff_args(&self.base, self.other.as_ref(), file)
    }
}

impl AppState {
    /// "Select for Compare": remembers `commit` for "Compare with Selected".
    pub(super) fn select_for_compare(&mut self, commit: Oid) {
        self.message = Some(format!(
            "Selected {} for compare. Right-click another commit and choose \"Compare with Selected\".",
            commit.short(SHORT)
        ));
        self.compare_base = Some(commit);
    }

    /// Opens a comparison in the details panel, below `anchor`'s row. It
    /// replaces whatever the panel showed.
    pub(super) fn open_comparison(&mut self, anchor: Oid, comparison: Comparison) -> Vec<Command> {
        self.details = Some(Details {
            commit: anchor,
            info: None,
            files: None,
            view: None,
            hits: None,
            compare: Some(comparison.clone()),
        });
        vec![Command::Git(Request::CompareFiles(comparison))]
    }

    /// Applies a comparison read. Results for a comparison that has since
    /// closed or been replaced are ignored.
    pub(super) fn on_compare_read(&mut self, request: Request, stdout: &str) {
        let Some(details) = &mut self.details else {
            return;
        };
        if let Some(comparison) = &details.compare {
            match request {
                Request::CompareFiles(c) if c == *comparison => match show::parse_files(stdout) {
                    Ok(files) => {
                        details.view = Some(CommitDetailsView::comparison(c.title(), files));
                    }
                    Err(err) => {
                        self.message = Some(format!("Could not read changed files: {err}"));
                    }
                },
                Request::CompareDiff(c, file) if c == *comparison => {
                    self.show_diff(DiffView::new(&file, show::parse_diff(stdout)));
                }
                _ => {}
            }
        }
    }
}
