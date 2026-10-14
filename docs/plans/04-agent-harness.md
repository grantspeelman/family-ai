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

- [ ] `AGENTS.md` exists, links to `docs/plans/` and the conventions document, and is under about 100 lines. (Exists, links `docs/plans/`, 60 lines. Unticked: the conventions document is delivered by plan 05 and does not exist yet. `AGENTS.md` links the plan, `docs/plans/05-conventions.md`, and says the document arrives with it.)
- [x] Every command that runs inside the container has been run; host-side commands (`start.sh`) are checked against the scripts. (`lefthook run pre-commit` and the `gitleaks` command were run in the container and pass. `start.sh` and its flags were checked against `.devcontainer/start.sh` and `.devcontainer/README.md`, not run.)
- [x] It states the per-PR workflow: start from the plan, restate it, implement, run tests and show them passing.
- [x] `.claude/settings.local.json` is gitignored; any committed `.claude/` file is deliberate and contains nothing personal. (`git check-ignore` confirms the repo `.gitignore` rule; nothing under `.claude/` is committed.)
- [ ] A fresh agent session, asked "how do I run the checks?", answers correctly from the repo alone. (Left for the reviewer: re-reading `AGENTS.md` myself is not the check described. Self-read found the "Running the checks" section answers the question, but no fresh session was run.)

## Review depth

**Close read.** This file steers every later agent session, so a wrong line here repeats in every PR.
