//! Dragging the divider between the panes and the graph header's column
//! dividers (S6).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use tgg_ui::{ColumnDivider, ColumnWidths};

use super::{AppState, Command};

/// A divider being dragged with the left button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drag {
    /// The line between Source Control and the graph.
    Pane,
    /// A header divider, from where the drag started.
    Column {
        divider: ColumnDivider,
        start_x: u16,
        start: ColumnWidths,
    },
}

impl AppState {
    /// Starts, continues or ends a divider drag. None when the event isn't
    /// part of one, so it goes on to the panes below.
    pub(super) fn on_resize_mouse(&mut self, mouse: MouseEvent) -> Option<Vec<Command>> {
        let (x, y) = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.source_control.divider == Some(x) {
                    self.drag = Some(Drag::Pane);
                } else if let Some(divider) = self.graph_hits.divider_at(x, y) {
                    let start = ColumnWidths {
                        graph: Some(self.graph_hits.graph_width()),
                        ..self.columns
                    };
                    self.drag = Some(Drag::Column {
                        divider,
                        start_x: x,
                        start,
                    });
                } else {
                    return None;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => match self.drag? {
                Drag::Pane => self.source_control.width = Some(x),
                Drag::Column {
                    divider,
                    start_x,
                    start,
                } => {
                    let delta = i32::from(x) - i32::from(start_x);
                    self.columns = start.dragged(divider, delta);
                }
            },
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag.take()?;
            }
            _ => return None,
        }
        Some(Vec::new())
    }
}
