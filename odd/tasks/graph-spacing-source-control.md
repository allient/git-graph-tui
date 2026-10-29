# Graph spacing and Source Control panel

Objective: a roomier graph with lines between commits, and a Source Control section beside the graph, like VS Code.
Branches: one feature branch per task, each PR against main (no stacked PRs). Delivery: ask-on-risk. Tests: `cargo t` (nextest).

## Specs

- **S1** Graph spacing: "on grapg make spacing bigger and rounded of line liek thsi one". A connector line between commit lines carries the lanes: `│` for straight lanes and rounded corners (`╭╮╰╯`) for forks and merges, so branch curves look like the VS Code screenshot.
- **S2** Lines between dots: "here miss lines between dots". Consecutive commits on one lane are joined by a vertical line.
- **S3** Source Control section: "where is this panel of chnages" and "i want gitgraph have these 2 sections source controla nd gitgraph". A left panel shows the changed files (staged and unstaged), a commit message input and a Commit button, beside the graph, as in the VS Code screenshot.
- **S4** Merged branches: "some branches are nto merged yer not see merged". Cause: PRs were rebase-merged, so old branch tips stay as separate lines (VS Code shows the same). Remedy: delete merged branches, which needs the user's consent (not a code change).
- **S5** Column header: "also add columsn names liek thsi". The graph's first line names the columns: Graph, Description, Date, Author, Commit (as in the VS Code screenshot).
- **S6** Resizable widths: "if psisbel we can be able to resize width". Assumption: drag the column dividers in the header with the mouse, and the divider between the Source Control section and the graph.
- **S7** Toolbar icons: "as thes e two icons" (cloud-download for Fetch, circular arrow for Refresh). Plain Unicode so it works without a Nerd Font: `↻` Refresh, `⇣` Fetch.

## Tasks

| ID | Specs | Route | Status | Commit |
|----|-------|-------|--------|--------|
| T1 | S1, S2, S5, S7 | inline | done (gate green, 421 tests) | feat/graph-spacing |
| T2 | S3 | inline | pending | |
| T4 | S6 | inline | pending | |
| T3 | S4 | user decision | done: 9 branches deleted locally and on origin after checking PRs #16-#24 merged | - |

## Log

- **L1** (2026-10-28) User: "I se this [Image #10] some branches are nto merged yer not see merged [Image #11] [Image #12]. where is this panel of chnages [Image #13]. also on grapg make spacing bigger and rounded of line liek thsi one [Image #14]. here miss lines between dots [Image #15]. i want gitgraph have these 2 sections source controla nd gitgraph [Image #16]"
- **L2** Findings: corners already use `╭╮╰╯` (crates/ui/src/glyphs.rs:99); lanes are 2 cells apart and each commit takes one terminal line, so adjacent commits have no connector. `feat/app-info-output` tip 73d79b6 vs its rebased copy 638bfa9 on main confirms rebase merges. "Open Source Control" only shows a hint (state/actions.rs).
- **L3** (2026-10-28) User: "yes deete them" (T3 consent).
- **L4** (2026-10-28) User: "also add columsn names liek thsi [Image #17]"
- **L5** (2026-10-28) User: "if psisbel we can be able to resize width"
- **L6** (2026-10-28) User: "as thes e two icons [Image #18]"
- **L7** T1: commit lines carry only verticals and the node; a connector line below each row draws lane changes with rounded corners (glyphs.rs connector_glyphs). Header line with Graph/Description/Date/Author/Commit; toolbar `[↻]` `[⇣]`. Rows take 2 lines (ROW_HEIGHT), so the viewport shows about half as many commits. Glyph tests were written with the code, not RED first.
