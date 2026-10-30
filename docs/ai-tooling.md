# AI tooling

How AI coding agents are set up for this repo. Agents read `AGENTS.md` for the project rules. This page covers the tooling around it: what's installed, why, and how to reproduce it on another machine.

## Overview

| Tool | What it does here | Scope |
| --- | --- | --- |
| [Gentle AI](https://github.com/Gentleman-Programming/gentle-ai) `4.3.1` | Configures the agent with the **ODD** workflow, **RDD** review, Engram memory and a skill registry | Global (`~/.claude/`) + `.atl/` in this repo |
| [Engram](https://github.com/Gentleman-Programming/engram) | Persistent memory across sessions: decisions, discoveries, ODD feature mirrors | Global, project key `git-graph-tui` |
| [skills.sh](https://skills.sh) skills | Task-specific instructions the agent loads on demand | This repo (`.claude/skills/`, `.agents/skills/`, `skills-lock.json`) |

## Gentle AI

Gentle AI doesn't replace the agent. It writes instructions, skills, sub-agents and settings into the agent's existing config.

### Setup (once per machine)

```sh
brew install gentleman-programming/tap/gentle-ai
gentle-ai sync --agent claude-code      # backs up existing config before writing
gentle-ai doctor                        # health check
```

After upgrading (`brew upgrade gentle-ai`), run `gentle-ai sync` again so the installed prompts match the binary.

### ODD: Organic Driven Development

ODD is the default workflow on every request. It's installed in `~/.claude/CLAUDE.md`, not in this repo.

1. **Authorize.** Questions, reviews and proposals stay read-only until a change is requested.
2. **Explore** the existing code first.
3. **Resolve uncertainty.** One focused question per real product decision.
4. **Classify.** Work is *small* if it could be resumed from the request plus `git diff`. Small work creates no task files.
5. **Track** substantial work before the first write, in `odd/tasks/<feature-name>.md` (`## Specs` with `S#` items, `## Tasks`, `## Log`). It's mirrored to Engram topic `odd/<feature-name>/tasks`.
6. **Implement task by task, test-first** (RED → GREEN → REFACTOR). Commit in reviewable slices of about 400 changed lines, using Conventional Commits, on a feature branch.
7. **Close.** Report what was verified, what failed or is pending, and the next step.

**How it maps to this repo:**

- Each roadmap stage in `docs/features.md` (MVP, Daily driver, Parity, ...) or large checklist item becomes one `odd/tasks/<feature>.md`, e.g. `odd/tasks/git-graph-mvp.md`. Commit these files. They're the spec and progress record.
- Test runner for ODD's test-first step: **`cargo nextest run`**, with `insta` snapshots and fixture repos. See `AGENTS.md` §6.
- Delivery skills used by ODD: `chained-pr` (split changes over 400 lines into stacked PRs) and `work-unit-commits`.

### RDD: Receipt-Driven Development

An optional review step that runs after each work-unit commit. The `gentle-ai` binary scores the change's risk: low-risk changes get a silent structural check, while medium or high risk triggers reviewer agents (`review-risk`, `review-readability`, `review-reliability`, `review-resilience`). Approval never commits, pushes or opens a PR on its own.

Currently **on** (the default). Control it with:

```sh
gentle-ai review mode status
gentle-ai review mode disable   # opt out (global or per clone)
gentle-ai review mode enable
```

### Skill registry (`.atl/`)

`.atl/skill-registry.md` indexes every skill the agent can see, so the orchestrator can hand exact `SKILL.md` paths to sub-agents. It's generated, not hand-edited:

```sh
gentle-ai skill-registry refresh --force
```

Regenerate it after installing or removing skills. It contains absolute paths from the local machine, so `.atl/` is in `.gitignore`.

## Project skills (skills.sh)

Installed with `npx skills add <owner/repo@skill>`. Each one is recorded in `skills-lock.json` (source and content hash) and stored in `.agents/skills/`, with symlinks in `.claude/skills/` for Claude Code.

| Skill | Source | Why it's here |
| --- | --- | --- |
| `tdd` | `mattpocock/skills` | Test-first loop for parsers, lane layout and filters. Matches ODD's RED → GREEN → REFACTOR. |
| `git-guardrails-claude-code` | `mattpocock/skills` | Hooks that block destructive git commands (`push`, `reset --hard`, `clean`, `branch -D`) while agents build a git tool. **Inactive until its hooks are set up**: ask the agent to "set up git guardrails". |
| `git-advanced-workflows` | `wshobson/agents` | Reference for rebase, cherry-pick, bisect, worktrees and reflog: the actions the app implements. |
| `tui-design` | `hyperb1iss/hyperskills` | Terminal UI layout, keyboard navigation, box-drawing and color guidance for the graph view. |
| `rust-best-practices` | `apollographql/skills` | Ownership, error handling, clippy and workspace lint setup, API design. |
| `rust-testing` | `affaan-m/ecc` | Rust TDD, unit/integration test layout, insta snapshots, property tests. |
| `ratatui-tui` | `blacktop/dotfiles` | ratatui 0.30 patterns: TEA loop, widgets, layout, popups, styling. |

To add a skill, run `npx skills add <owner/repo@skill>`, then `gentle-ai skill-registry refresh --force`, then add a row to this table.

## Health check

```sh
gentle-ai doctor
gentle-ai review mode status
```

Known `doctor` warnings on the current setup (2026-10-28):

- `tool:gga not found`: optional Gentle AI companion, not needed here.
- `state:json ... no installed agents` and `engram:reachable not probed`: the state file doesn't record Claude Code as installed. ODD is present in `~/.claude/CLAUDE.md` and Engram works in sessions, so this is cosmetic. Running `gentle-ai install` (choose Claude Code) should clear it.
