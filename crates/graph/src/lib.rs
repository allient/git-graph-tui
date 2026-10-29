//! Commit graph lane layout for git-graph-tui.
//!
//! Turns an ordered list of commits into rows of lanes, nodes and edges. The
//! layout is incremental: appending more commits never changes rows that were
//! already laid out. Pure functions only, so the output can be snapshot-tested.

mod layout;

pub use layout::{Edge, Graph, Row};
