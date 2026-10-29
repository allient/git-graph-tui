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

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2, S3 | inline | done | build(release): cargo-dist |
| T2 | S5 | inline | done | docs: install section |
| T3 | S6 | delegated (verifier) | todo | |
| T4 | S4 | user (tap repo + token), then first tag | todo | |

Route evidence: one feature; the release config and docs are small and sequential, so the parent writes inline; T3 delegates because the Risk line names item 5.

## Log

- **L1** (2026-10-29) "ok continue do setup ti be able to have appp availble to be installed", after the plan: cargo-dist, targets macOS/Linux/Windows, shell + PowerShell installers, Homebrew tap `allient/homebrew-tap`, macOS runners only on release tags.
- **L2** (2026-10-29) "do i need to create an acount on anyhting??" Answer: no new account; GitHub covers releases, runners and the tap; crates.io and Apple notarization are optional.
- **L3** (2026-10-29) cargo-dist 0.33.1 installed with Homebrew on the dev machine.
- **L4** (2026-10-29) T1 done: `dist init` (targets aarch64/x86_64 macOS, x86_64/aarch64 Linux gnu, x86_64 Windows msvc; installers shell, powershell, homebrew), `[package.metadata.dist] dist = true` on the app (it has `publish = false`), tap `allient/homebrew-tap`, `publish-jobs = ["homebrew"]`, `pr-run-mode = "plan"`. Evidence: `dist plan` lists all 5 archives, both installers, `tui-git-graph.rb` and checksums; `dist build --target aarch64-apple-darwin` built a 660 KB archive whose sha256 matches and whose binary runs (`--version` 0.1.0). macOS runners only in `build-local-artifacts`, which runs only when publishing (tag push), not on PRs. Gate green.
