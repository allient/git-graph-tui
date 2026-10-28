//! ratatui widgets for tui-git-graph: the graph view, ref badges, commit
//! details, popup menus and dialogs.
//!
//! Rendering is a pure function of application state. Every frame also
//! produces a hit-map (screen cell to target), which the app uses to resolve
//! mouse clicks and right-clicks against exactly what was drawn.
