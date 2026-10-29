//! ratatui widgets for git-graph-tui: the graph view, ref badges, commit
//! details, popup menus and dialogs.
//!
//! Rendering is a pure function of application state. Every frame also
//! produces a hit-map (screen cell to target), which the app uses to resolve
//! mouse clicks and right-clicks against exactly what was drawn.

mod badges;
mod banner;
pub use banner::{Banner, BannerButton, BannerHitMap};
mod glyphs;
mod graph_view;
mod icons;
mod source_control;
mod text;
mod theme;

pub use badges::{Badge, BadgeKind, badges_for};
pub use glyphs::{GlyphCell, connector_glyphs, lane_glyphs};
pub use graph_view::{ColumnDivider, ColumnWidths, Gap, GraphView, HitMap, HitTarget, ROW_HEIGHT};
pub use icons::{Glyph, Icons};
pub use source_control::{PaneFile, SourceControl, SourceControlHitMap, SourceControlOutcome};
pub use text::{fit, format_date};
pub use theme::Theme;
mod key;
mod menu;

pub use key::Key;
pub use menu::{MenuHit, MenuHitMap, MenuOutcome, PopupMenu};
mod details;
pub use details::{
    CommitDetailsView, DetailsButton, DetailsHit, DetailsHitMap, DetailsOutcome, FileLayout,
    TreeRow, file_label,
};
mod diff_view;
pub use diff_view::{DiffHit, DiffHitMap, DiffOutcome, DiffStyle, DiffView, SIDE_BY_SIDE_WIDTH};
mod confirm;
pub use confirm::{
    Button, ConfirmDialog, DialogHit, DialogHitMap, DialogOutcome, DialogStyle, MAX_WIDTH,
};
mod input_dialog;
pub use input_dialog::{Focus, InputDialog, InputHit, InputHitMap, InputOutcome, TextField};
mod toolbar;
pub use toolbar::{Toolbar, ToolbarButton, ToolbarHitMap, ToolbarOutcome, filter_summary};
mod picker;
pub use picker::{Picker, PickerHit, PickerHitMap, PickerItem, PickerOutcome, Row, glob_match};
