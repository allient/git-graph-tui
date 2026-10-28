//! Lane layout tests. Commits are given newest first (git's topological order).
//!
//! Snapshots use one line per row:
//! `<name> lane=<n> color=<c> top=[from>to:color ...] bottom=[...]` plus `muted`
//! when the commit isn't an ancestor of HEAD.

// Test helpers panic on bad fixtures, like the tests themselves.
#![expect(clippy::unwrap_used, reason = "test helpers")]

use std::collections::HashMap;

use pretty_assertions::assert_eq;
use tgg_graph::{Edge, Graph, Row};
use tgg_model::{Commit, Oid, Signature};

/// Builds commits from `(name, parents)` pairs; names are single hex digits.
fn commits(spec: &[(&str, &[&str])]) -> Vec<Commit> {
    spec.iter()
        .map(|(name, parents)| Commit {
            id: oid(name),
            parents: parents.iter().map(|p| oid(p)).collect(),
            author: sig(),
            committer: sig(),
            subject: (*name).to_owned(),
        })
        .collect()
}

fn oid(name: &str) -> Oid {
    Oid::parse(&name.repeat(40)).unwrap()
}

fn sig() -> Signature {
    Signature {
        name: "a".into(),
        email: "a@x".into(),
        time: 0,
    }
}

fn describe(commits: &[Commit], rows: &[Row]) -> String {
    let names: HashMap<&Oid, &str> = commits
        .iter()
        .map(|c| (&c.id, c.subject.as_str()))
        .collect();
    let edges = |edges: &[Edge]| {
        edges
            .iter()
            .map(|e| format!("{}>{}:{}", e.from, e.to, e.color))
            .collect::<Vec<_>>()
            .join(" ")
    };
    commits
        .iter()
        .zip(rows)
        .map(|(c, r)| {
            format!(
                "{} lane={} color={} top=[{}] bottom=[{}]{}",
                names[&c.id],
                r.lane,
                r.color,
                edges(&r.top),
                edges(&r.bottom),
                if r.in_head_ancestry { "" } else { " muted" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn layout(spec: &[(&str, &[&str])], head: &str) -> (Vec<Commit>, Vec<Row>) {
    let commits = commits(spec);
    let mut graph = Graph::new(Some(oid(head)));
    graph.extend(&commits);
    let rows = graph.rows().to_vec();
    (commits, rows)
}

#[test]
fn linear_history_uses_one_lane() {
    let (_, rows) = layout(&[("3", &["2"]), ("2", &["1"]), ("1", &[])], "3");
    assert!(rows.iter().all(|r| r.lane == 0 && r.color == rows[0].color));
    assert_eq!(
        rows[0].top,
        vec![],
        "the newest commit has nothing above it"
    );
    assert_eq!(rows[2].bottom, vec![], "the root has nothing below it");
    assert_eq!(rows.iter().map(Row::width).max(), Some(1));
}

#[test]
fn merge_opens_a_lane_for_the_second_parent() {
    // m merges b (from a side branch) into a.
    let (c, rows) = layout(&[("f", &["a", "b"]), ("b", &["a"]), ("a", &[])], "f");
    insta::assert_snapshot!(describe(&c, &rows));
    assert_eq!(rows[0].bottom.len(), 2, "merge has two outgoing edges");
    assert_eq!(rows[1].lane, 1, "the merged branch gets its own lane");
    assert_ne!(
        rows[0].color, rows[1].color,
        "branches get different colors"
    );
}

#[test]
fn two_tips_join_at_their_common_parent() {
    // Two branches (e, d) both branch off a; d is not reachable from HEAD (e).
    let (c, rows) = layout(&[("e", &["a"]), ("d", &["a"]), ("a", &[])], "e");
    insta::assert_snapshot!(describe(&c, &rows));
    assert_eq!(
        rows[2].top.iter().filter(|e| e.to == rows[2].lane).count(),
        2,
        "both lanes end at a"
    );
    assert!(!rows[1].in_head_ancestry, "d is muted");
    assert!(rows[2].in_head_ancestry, "a is an ancestor of HEAD");
}

#[test]
fn freed_lanes_are_reused_with_a_new_color() {
    let (c, rows) = layout(
        &[
            ("9", &["8", "7"]),
            ("7", &["6"]),
            ("8", &["6"]),
            ("6", &["5"]),
            ("5", &["4"]),
            ("4", &["3"]),
            ("3", &[]),
        ],
        "9",
    );
    // A second, unrelated side branch after the first one closed.
    let (c2, rows2) = layout(
        &[
            ("9", &["8", "7"]),
            ("7", &["6"]),
            ("8", &["6"]),
            ("6", &["5", "4"]),
            ("4", &["3"]),
            ("5", &["3"]),
            ("3", &[]),
        ],
        "9",
    );
    insta::assert_snapshot!("reuse_first_branch_only", describe(&c, &rows));
    insta::assert_snapshot!("reuse_two_side_branches", describe(&c2, &rows2));
    assert_eq!(rows2[1].lane, 1);
    assert_eq!(
        rows2[4].lane, 1,
        "lane 1 is reused for the second side branch"
    );
    assert_ne!(
        rows2[1].color, rows2[4].color,
        "a reused lane gets a new color"
    );
}

#[test]
fn octopus_merge_opens_a_lane_per_extra_parent() {
    let (c, rows) = layout(
        &[
            ("f", &["a", "b", "c"]),
            ("c", &["a"]),
            ("b", &["a"]),
            ("a", &[]),
        ],
        "f",
    );
    insta::assert_snapshot!(describe(&c, &rows));
    assert_eq!(rows[0].bottom.len(), 3);
    assert_eq!(rows[0].width(), 3);
}

#[test]
fn merging_into_an_existing_lane_does_not_open_a_new_one() {
    // Both b and f need a; f merges b, which is already on its own lane via tip d.
    let (c, rows) = layout(
        &[("d", &["b"]), ("f", &["a", "b"]), ("b", &["a"]), ("a", &[])],
        "f",
    );
    insta::assert_snapshot!(describe(&c, &rows));
    assert_eq!(rows.iter().map(Row::width).max(), Some(2), "no third lane");
}

#[test]
fn parents_outside_the_loaded_page_keep_their_lane_open() {
    let (_, rows) = layout(&[("3", &["2"])], "3");
    assert_eq!(
        rows[0].bottom,
        vec![Edge {
            from: 0,
            to: 0,
            color: rows[0].color
        }]
    );
}

#[test]
fn loading_in_pages_gives_the_same_rows_as_loading_everything() {
    let spec: &[(&str, &[&str])] = &[
        ("f", &["e", "d"]),
        ("d", &["c"]),
        ("e", &["c"]),
        ("c", &["b", "a"]),
        ("a", &["9"]),
        ("b", &["9"]),
        ("9", &[]),
    ];
    let all = commits(spec);
    let mut whole = Graph::new(Some(oid("f")));
    whole.extend(&all);

    for split in 1..all.len() {
        let mut paged = Graph::new(Some(oid("f")));
        let first = paged.extend(&all[..split]).to_vec();
        let second = paged.extend(&all[split..]).to_vec();
        assert_eq!(first.len() + second.len(), all.len());
        assert_eq!(paged.rows(), whole.rows(), "split at {split}");
    }
}

#[test]
fn no_head_means_nothing_is_muted() {
    let commits = commits(&[("e", &["a"]), ("d", &["a"]), ("a", &[])]);
    let mut graph = Graph::new(None);
    graph.extend(&commits);
    assert!(graph.rows().iter().all(|r| r.in_head_ancestry));
}
