# Release setup: installable tui-git-graph

Objective: tui-git-graph can be installed from prebuilt binaries (GitHub Releases, shell and PowerShell installers, Homebrew tap).
Branch: build/release-setup, PR against main. Delivery: ask-on-risk. Tests: `cargo t` (nextest), `dist plan`, `dist build` locally.

## Specs

- **S1** "ok continue do setup ti be able to have appp availble to be installed". Prebuilt binaries for macOS (Apple Silicon and Intel), Linux (x86_64 and arm64) and Windows, published as a GitHub Release with archives and checksums when a `v*` tag is pushed. Built with cargo-dist (`dist`).
- **S2** Installers: a shell installer (`curl ... | sh`), a PowerShell installer, and a Homebrew formula published to the tap repository `allient/homebrew-tap` (roadmap step 4 items 6 and 7).
- **S3** macOS runners are allowed only for the release workflow on tag pushes, never on pull requests or the regular CI (user: "ok continue", after being asked whether release builds may use macOS runners; standing constraint "No macOS CI" stays for CI).
- **S4** "do i need to create an acount on anyhting??": no new account. The tap needs the GitHub repository `allient/homebrew-tap` and a token secret `HOMEBREW_TAP_TOKEN` in allient/tui-git-graph, which the user creates (assumption: the user creates the token; Claude never handles secret values).
- **S5** README: install instructions for each installer; roadmap step 4 updated.
- **S6** Before the first tag: an independent verifier reviews the release setup (Risk item 5: release, CI).
- **S7** "before deplot first version make sure all features are working as expected and use argent or any other mcp to test all features work as extected". End-to-end QA of every README feature on the release binary before tagging v0.1.0; failures are fixed first. Argent does not apply (mobile/Chromium only), so the binary is driven in a pseudo-terminal (pyte) with real keys and SGR mouse events, checking screen and git state.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2, S3 | inline | done | build(release): cargo-dist |
| T2 | S5 | inline | done | docs: install section |
| T3 | S6 | delegated (verifier) | done | ci: type-check release targets |
| T5 | S7 | delegated (3 QA testers in parallel: graph/views, context menus, Source Control + repo menu), fixes inline | in progress | |
| T6 | L9 | inline, after T5 | done | refactor: ggt-* crates |
| T4 | S4 | user (tap repo + token), then first tag (after T5, T6) | done | v0.1.0 released |

Route evidence: one feature; the release config and docs are small and sequential, so the parent writes inline; T3 delegates because the Risk line names item 5.

## Log

- **L1** (2026-10-29) "ok continue do setup ti be able to have appp availble to be installed", after the plan: cargo-dist, targets macOS/Linux/Windows, shell + PowerShell installers, Homebrew tap `allient/homebrew-tap`, macOS runners only on release tags.
- **L2** (2026-10-29) "do i need to create an acount on anyhting??" Answer: no new account; GitHub covers releases, runners and the tap; crates.io and Apple notarization are optional.
- **L3** (2026-10-29) cargo-dist 0.33.1 installed with Homebrew on the dev machine.
- **L4** (2026-10-29) T1 done: `dist init` (targets aarch64/x86_64 macOS, x86_64/aarch64 Linux gnu, x86_64 Windows msvc; installers shell, powershell, homebrew), `[package.metadata.dist] dist = true` on the app (it has `publish = false`), tap `allient/homebrew-tap`, `publish-jobs = ["homebrew"]`, `pr-run-mode = "plan"`. Evidence: `dist plan` lists all 5 archives, both installers, `tui-git-graph.rb` and checksums; `dist build --target aarch64-apple-darwin` built a 660 KB archive whose sha256 matches and whose binary runs (`--version` 0.1.0). macOS runners only in `build-local-artifacts`, which runs only when publishing (tag push), not on PRs. Gate green.
- **L5** (2026-10-29) T3 independent verifier: no blockers. S1, S2, S3, S5 PASS (only the ubuntu `plan` job runs on PRs; macOS only in `build-local-artifacts`, gated on publishing; `dist generate --check` clean; installer URLs and `brew install allient/tap/tui-git-graph` correct). S4 prerequisite open: `allient/homebrew-tap` does not exist yet. Advisories: without the tap repo/token a tag still publishes the GitHub Release but the Homebrew job fails (re-run it after setup); aarch64 Linux builds on free `ubuntu-22.04-arm` (public repo only); Windows had never been compiled. Fix: `cargo check --target x86_64-pc-windows-msvc` passes; two Windows-only test warnings fixed (`run_raw` is `cfg(all(test, unix))`, duplicate time import removed); new CI job type-checks Windows and macOS on Linux with `-D warnings`.
- **L6** (2026-10-29) "before deplot first version make sure all features are working as expected and use argent or any other mcp to test all features work as extected . how I should do 2 create toke". Three QA agents launched on the release binary (commit eb962c3), copied to scratchpad/qa/tui-git-graph.
- **L7** (2026-10-29) User created `allient/homebrew-tap` (public) and the `HOMEBREW_TAP_TOKEN` secret (confirmed present via `gh secret list`, value not readable). The tap was empty, which would break the release's tap checkout; user: "do this The fix is to give the tap one first commit (a short README) on main". Added README.md on `main` (commit ab0ba07). T4 prerequisites done; the first tag waits for T5 (QA).
- **L8** (2026-10-29) "before release I wan tto rename project to git-graph-tui" and "including repo name"; scope chosen: all user-facing (GitHub repo, command and crate, env vars `GIT_GRAPH_TUI_*`, config folder `~/.config/git-graph-tui`, herdr plugin id, Homebrew formula, docs); internal `tgg-*` crates unchanged; `odd/` history notes keep the old name. QA F2 (README graph line) fixed in the same commit.
- **L9** (2026-10-29) "aftr test rename to ggt The internal crates (tgg-model, tgg-ui and so on) ... betetr". After QA: rename the internal crates `tgg-*` to `ggt-*` (T6). Local checkout folder rename stays for later (user: "ok keep thsi for later").
- **L10** (2026-10-29) T5 (QA) done: recheck 9/9 PASS plus smoke (see odd/tasks/qa-fixes.md). T6 done: internal crates renamed `tgg-*` to `ggt-*` (117 files; Cargo.lock only reorders). Gate green: 811 tests, clippy, fmt, MSRV 1.88, deny, machete, herdr script tests, `dist plan`.
- **L11** (2026-10-30) PR #47 merged (b5d738e); CI on main green (7 jobs). User: "yes do it by yourslef". Tagged `v0.1.0` (annotated) on b5d738e and pushed. Release run 40980645944: all 11 jobs success (5 builds, host, publish-homebrew-formula). GitHub Release v0.1.0 published with 5 archives + checksums, installers, `git-graph-tui.rb`; `allient/homebrew-tap` has `Formula/git-graph-tui.rb`. Verified: the shell installer from `releases/latest` installs `git-graph-tui 0.1.0` into a scratch CARGO_HOME; the aarch64 archive's sha256 matches its checksum file and the formula.
- **L12** (2026-10-30) User: "myes do 1-3. no need ttt plugin remove from todos". GitHub repo description, homepage (latest release) and topics set; merged branch `build/release-setup` deleted locally and on GitHub; the planned TTT plugin removed from the README (roadmap step 4, layout, integrations); TTT stays a host where it runs in a terminal tab.
