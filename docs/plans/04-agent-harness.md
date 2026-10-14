# 04 — Agent harness

**Status:** in progress

## Goal

Claude (and any other coding agent) opens the repo and finds out how to work in it without being told: where things are, how to run checks, and which rules are not negotiable.

## Builds on

- 01 — Dev container (the commands and paths it documents must exist)

## In scope

- `AGENTS.md` at the repo root as the single entry point, short enough to read in full.
- `CLAUDE.md` pointing to it, if the tooling needs that file name.
- Project facts an agent cannot get from the code: how to enter the container, how to run checks, where plans live, what a PR must show before it is marked done.
- Shared project settings in `.claude/` that belong in the repo (committed), kept apart from personal local settings (ignored).

## Out of scope

- The long-form conventions and principles themselves (plan 05); `AGENTS.md` links to them.
- Any language- or framework-specific rules, which come with the PRs that introduce those stacks.

## Acceptance checklist

- [ ] `AGENTS.md` exists, links to `docs/plans/` and the conventions document, and is under about 100 lines. (Exists, links `docs/plans/`, 60 lines. Unticked: the conventions document is plan 05 and does not exist yet; `AGENTS.md` says it is added later.)
- [ ] Every command in it has been run and works inside the container. (`lefthook run pre-commit` and the `gitleaks` command were run in the container and pass. Unticked: `.devcontainer/start.sh` is a host-side command and was not run from inside the container.)
- [x] It states the per-PR workflow: start from the plan, restate it, implement, run tests and show them passing.
- [x] `.claude/settings.local.json` is gitignored; any committed `.claude/` file is deliberate and contains nothing personal. (`git check-ignore` confirms the repo `.gitignore` rule; nothing under `.claude/` is committed.)
- [x] A fresh agent session, asked "how do I run the checks?", answers correctly from the repo alone. (Verified by re-reading `AGENTS.md` cold: its "Running the checks" section gives the commands and says there is no test suite yet. No separate session was spawned.)

## Review depth

**Close read.** This file steers every later agent session, so a wrong line here repeats in every PR.
