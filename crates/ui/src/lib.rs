//! ratatui widgets for tui-git-graph: the graph view, ref badges, commit
//! details, popup menus and dialogs.
//!
//! Rendering is a pure function of application state. Every frame also
//! produces a hit-map (screen cell to target), which the app uses to resolve
//! mouse clicks and right-clicks against exactly what was drawn.

mod badges;
mod glyphs;
mod graph_view;
mod text;
mod theme;

pub use badges::{Badge, BadgeKind, badges_for};
pub use glyphs::{GlyphCell, lane_glyphs};
pub use graph_view::{GraphView, HitMap, HitTarget};
pub use text::{fit, format_date};
pub use theme::Theme;
mod key;
mod menu;

pub use key::Key;
pub use menu::{MenuHit, MenuHitMap, MenuOutcome, PopupMenu};
mod details;
pub use details::{
    CommitDetailsView, DetailsHit, DetailsHitMap, DetailsOutcome, TreeRow, file_label,
};
