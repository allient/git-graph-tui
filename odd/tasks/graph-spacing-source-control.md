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
- **S8** Nerd Font icons, opt-in: "ok use those icons and add of readme isntructions to isntall forst icons nerd onews" and "do thsi same That's why tools like lazygit and yazi make icons opt-in: they print plain text by default,  if icons use tehm in other case use what we have now". `--icons nerd` or `TUI_GIT_GRAPH_ICONS=nerd` shows Codicons (refresh U+EB37, cloud-download U+EAC2, ellipsis U+EA7C); the default stays the Unicode `[↻] [⇣] [...]`. README explains installing a Nerd Font.
- **S9** Graph padding: "add more oadding at left of vlue dots". The lanes start a couple of columns in from the graph column's left edge.
- **S10** Auto icons: "coplete teh two open item". Default `auto`: Nerd Font icons when the terminal is known to ship the Nerd Font symbols (Ghostty, WezTerm, Kitty), plain Unicode otherwise; `--icons nerd|unicode` and `TUI_GIT_GRAPH_ICONS` still force either.
- **S11** README checklist: tick the features that work (second open item).
- **S12** "continue with they feature not built yet", in this order: Find (search by message, author, hash, branch or tag); create a pull request from a branch (opens the browser); view a file at a revision and compare two commits or a commit with the working tree; an external diff pager such as `delta`; repository settings and a config file (persists pane and column widths).

## Tasks

| ID | Specs | Route | Status | Commit |
|----|-------|-------|--------|--------|
| T1 | S1, S2, S5, S7 | inline | done (gate green, 421 tests) | feat/graph-spacing |
| T5 | S8 | inline | done (429 tests) | feat/graph-spacing |
| T2 | S3 | inline | done (454 tests, pty smoke committed a file) | feat/source-control: 88f9e46, fcac605, slice 4 |
| T4 | S6 | inline | done (462 tests, pty drag moved the divider 30→45) | feat/resize-panes |
| T6 | S9, S10, S11 | inline | done (463 tests) | feat/graph-padding-auto-icons |
| T7 | S12 Find | inline | pending | |
| T8 | S12 Create PR | inline | pending | |
| T9 | S12 file at revision, compare | inline | pending | |
| T10 | S12 external pager | inline | pending | |
| T11 | S12 settings and config | inline | pending | |
| T3 | S4 | user decision | done: 9 branches deleted locally and on origin after checking PRs #16-#24 merged | - |

## Log

- **L1** (2026-10-28) User: "I se this [Image #10] some branches are nto merged yer not see merged [Image #11] [Image #12]. where is this panel of chnages [Image #13]. also on grapg make spacing bigger and rounded of line liek thsi one [Image #14]. here miss lines between dots [Image #15]. i want gitgraph have these 2 sections source controla nd gitgraph [Image #16]"
- **L2** Findings: corners already use `╭╮╰╯` (crates/ui/src/glyphs.rs:99); lanes are 2 cells apart and each commit takes one terminal line, so adjacent commits have no connector. `feat/app-info-output` tip 73d79b6 vs its rebased copy 638bfa9 on main confirms rebase merges. "Open Source Control" only shows a hint (state/actions.rs).
- **L3** (2026-10-28) User: "yes deete them" (T3 consent).
- **L4** (2026-10-28) User: "also add columsn names liek thsi [Image #17]"
- **L5** (2026-10-28) User: "if psisbel we can be able to resize width"
- **L6** (2026-10-28) User: "as thes e two icons [Image #18]"
- **L7** T1: commit lines carry only verticals and the node; a connector line below each row draws lane changes with rounded corners (glyphs.rs connector_glyphs). Header line with Graph/Description/Date/Author/Commit; toolbar `[↻]` `[⇣]`. Rows take 2 lines (ROW_HEIGHT), so the viewport shows about half as many commits. Glyph tests were written with the code, not RED first.
- **L8** (2026-10-28) User: "ok use those icons and add of readme isntructions to isntall forst icons nerd onews"
- **L9** (2026-10-28) User: "do thsi same That's why tools like lazygit and yazi make icons opt-in: they print plain text by default,  if icons use tehm in other case use what we have now"
- **L10** (2026-10-28) User: "Do not forget to create 2 panes one is source controls and teho other is gitgraph [Image #20]" (T2, S3).
- **L11** T2 plan, 4 slices on feat/source-control: (1) per-file status (`ChangedFile`, `Change`), (2) `WorkingTreeAction::{Stage, Unstage, Commit}` (`add --all`, `reset --quiet` (works unborn), `commit --message=`), (3) `SourceControlPane` widget in tgg-ui, (4) left pane plus graph pane layout, focus and wiring in the app. Slices 1 and 2 are done (434 tests).
- **L12** T2 slice 4: the pane is on the left (a quarter of the width, 30-44 columns, hidden below 90 columns) with a divider; toolbar and graph are on the right. `c` or "Open Source Control" focuses the message; a click in the pane takes the keyboard and a click elsewhere gives it back. Commit is refused with "Type a commit message first." or "Nothing staged: stage files with + first.", and the message clears after a successful commit. At 110 columns the graph drops Date/Author (they need 100 graph columns).
- **L13** T4: drag the pane divider (pane 20 columns to total-50, kept for the session) or the header dividers `│` (AfterGraph resizes the graph column; AfterDescription widens/narrows Date; AfterDate and AfterAuthor trade width between neighbours; 4-80 columns). A drag never selects a row. Widths are not saved between runs. Built on feat/source-control; PR after #26 merges.
- **L14** (2026-10-28) User: "add more oadding at left of vlue dots [Image #21]. coplete teh two open item and continue with they feature not built yet"
- **L15** T6: lanes start 2 columns in (GRAPH_PADDING). `--icons auto` is the new default: Nerd icons when TERM_PROGRAM is ghostty/wezterm, TERM is xterm-ghostty/xterm-kitty, or GHOSTTY_RESOURCES_DIR, WEZTERM_EXECUTABLE or KITTY_WINDOW_ID is set (these survive inside herdr/tmux). README: 23 checklist items ticked; still open: Find, fetch into local branch, column show/hide, create PR, external pager, file at revision, compare, issue linking, stretch goals.
