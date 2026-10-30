use std::collections::HashSet;

use ggt_model::{Commit, Oid};

/// A line segment in one half of a row, between two lane columns.
///
/// In [`Row::top`], `from` is a lane at the boundary above the row and `to` is
/// where it goes in this row (its own lane, or the commit's lane when it ends
/// here). In [`Row::bottom`], `from` is the lane in this row and `to` is the lane
/// at the boundary below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    /// Index into the lane color palette (the UI cycles through its palette).
    pub color: usize,
}

/// One commit's row in the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Lane (column) of the commit's node.
    pub lane: usize,
    /// Color of the commit's lane.
    pub color: usize,
    /// Edges from the boundary above into this row.
    pub top: Vec<Edge>,
    /// Edges from this row to the boundary below.
    pub bottom: Vec<Edge>,
    /// False for commits that aren't ancestors of HEAD (drawn muted).
    pub in_head_ancestry: bool,
}

impl Row {
    /// Number of lane columns this row needs.
    #[must_use]
    pub fn width(&self) -> usize {
        self.top
            .iter()
            .chain(&self.bottom)
            .flat_map(|e| [e.from, e.to])
            .chain([self.lane])
            .max()
            .map_or(0, |max| max + 1)
    }
}

/// A lane that is waiting for a specific commit further down.
#[derive(Debug, Clone)]
struct Lane {
    expects: Oid,
    color: usize,
}

/// Incremental lane layout.
///
/// Feed commits newest first (git's `--topo-order`), one page at a time, with
/// [`Graph::extend`]. Rows already produced never change when more commits are
/// added, so the view can load history lazily while scrolling.
#[derive(Debug, Clone)]
pub struct Graph {
    head: Option<Oid>,
    lanes: Vec<Option<Lane>>,
    next_color: usize,
    /// Commits known to be reachable from HEAD that haven't been laid out yet.
    head_ancestry: HashSet<Oid>,
    rows: Vec<Row>,
}

impl Graph {
    /// An empty layout. `head` is the commit HEAD points at (None when unborn).
    #[must_use]
    pub fn new(head: Option<Oid>) -> Self {
        Self {
            head_ancestry: head.iter().cloned().collect(),
            head,
            lanes: Vec::new(),
            next_color: 0,
            rows: Vec::new(),
        }
    }

    /// Lays out the next page of commits and returns the new rows.
    pub fn extend(&mut self, commits: &[Commit]) -> &[Row] {
        let start = self.rows.len();
        for commit in commits {
            let row = self.place(commit);
            self.rows.push(row);
        }
        &self.rows[start..]
    }

    /// All rows laid out so far, in commit order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    fn place(&mut self, commit: &Commit) -> Row {
        // Lanes from above that end at this commit.
        let incoming: Vec<usize> = self
            .lanes
            .iter()
            .enumerate()
            .filter_map(|(i, lane)| lane.as_ref().filter(|l| l.expects == commit.id).map(|_| i))
            .collect();

        // The leftmost lane waiting for this commit continues through it; a
        // branch tip (nothing waiting for it) opens a lane with a new color.
        let (lane, color) = if let Some(&first) = incoming.first() {
            (first, self.color_of(first))
        } else {
            let color = self.new_color();
            (self.free_slot(), color)
        };

        let top = self
            .lanes
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                l.as_ref().map(|l| Edge {
                    from: i,
                    to: if incoming.contains(&i) { lane } else { i },
                    color: l.color,
                })
            })
            .collect();

        // Lanes that joined into this commit's lane are free from here on.
        for &i in &incoming {
            self.lanes[i] = None;
        }
        let passing: Vec<usize> = (0..self.lanes.len())
            .filter(|&i| i != lane && self.lanes[i].is_some())
            .collect();

        let mut bottom: Vec<Edge> = passing
            .iter()
            .map(|&i| Edge {
                from: i,
                to: i,
                color: self.color_of(i),
            })
            .collect();

        let mut parents = commit.parents.iter();
        if let Some(first) = parents.next() {
            // The first parent continues straight down in this commit's lane.
            self.set_lane(
                lane,
                Lane {
                    expects: first.clone(),
                    color,
                },
            );
            bottom.push(Edge {
                from: lane,
                to: lane,
                color,
            });
        }
        for parent in parents {
            let target = self.lane_expecting(parent, lane).unwrap_or_else(|| {
                let slot = self.free_slot();
                let color = self.new_color();
                self.set_lane(
                    slot,
                    Lane {
                        expects: parent.clone(),
                        color,
                    },
                );
                slot
            });
            bottom.push(Edge {
                from: lane,
                to: target,
                color: self.color_of(target),
            });
        }
        bottom.sort_by_key(|e| (e.to, e.from));

        while matches!(self.lanes.last(), Some(None)) {
            self.lanes.pop();
        }

        Row {
            lane,
            color,
            top,
            bottom,
            in_head_ancestry: self.track_head_ancestry(commit),
        }
    }

    /// Marks the parents of HEAD's ancestors as ancestors too. Works across
    /// pages because commits arrive in topological order (children first).
    fn track_head_ancestry(&mut self, commit: &Commit) -> bool {
        if self.head.is_none() {
            return true;
        }
        let reachable = self.head_ancestry.remove(&commit.id);
        if reachable {
            self.head_ancestry.extend(commit.parents.iter().cloned());
        }
        reachable
    }

    fn lane_expecting(&self, oid: &Oid, except: usize) -> Option<usize> {
        self.lanes.iter().enumerate().find_map(|(i, lane)| {
            lane.as_ref()
                .filter(|l| i != except && &l.expects == oid)
                .map(|_| i)
        })
    }

    fn free_slot(&mut self) -> usize {
        if let Some(i) = self.lanes.iter().position(Option::is_none) {
            i
        } else {
            self.lanes.push(None);
            self.lanes.len() - 1
        }
    }

    fn set_lane(&mut self, index: usize, lane: Lane) {
        if index >= self.lanes.len() {
            self.lanes.resize(index + 1, None);
        }
        self.lanes[index] = Some(lane);
    }

    fn color_of(&self, index: usize) -> usize {
        self.lanes[index].as_ref().map_or(0, |l| l.color)
    }

    fn new_color(&mut self) -> usize {
        let color = self.next_color;
        self.next_color += 1;
        color
    }
}
