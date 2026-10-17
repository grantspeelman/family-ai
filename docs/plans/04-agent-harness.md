# 04 — Agent harness

**Status:** in progress

## Goal

Claude (and any other coding agent) opens the repo and finds out how to work in it without being told: where things are, how to run checks, and which rules are not negotiable.

## Builds on

- 01 — Dev container (the commands and paths it documents must exist)

## In scope

- `AGENTS.md` at the repo root as the single entry point, short enough to read in full.
- No separate `CLAUDE.md`: `AGENTS.md` is the only entry point, and the public-repo rule that `CLAUDE.md` used to carry lives in it.
- Project facts an agent cannot get from the code: that development happens inside the dev container (one line, pointing to its README), how to run checks, where plans live.
- Personal Claude Code settings (`.claude/settings.local.json`) kept out of the repo by `.gitignore`.

## Out of scope

- The long-form conventions and principles themselves (plan 05); `AGENTS.md` links to them.
- The per-PR workflow, the conventions link and notes on shared Claude Code settings: the owner will write these later. They were in an earlier draft of `AGENTS.md` and were removed on purpose.
- Any language- or framework-specific rules, which come with the PRs that introduce those stacks.

## Acceptance checklist

- [x] `AGENTS.md` exists, links to `docs/plans/`, and is under about 100 lines. (Exists, links `docs/plans/`, 35 lines. The link to the conventions document is added when plan 05 delivers it.)
- [x] Every command it gives has been run or checked against the scripts. (It now gives no commands: it describes the commit hook and CI in words and points to `lefthook.yml` and `.devcontainer/README.md`. The hook itself, `lefthook run pre-commit`, was run in the container and passes.)
- [x] It says to write tests first, run the tests for what changed, and not to sanity-run the whole suite or lint every file, because the commit hook and CI cover that.
- [x] `.claude/settings.local.json` is gitignored and nothing under `.claude/` is committed. (`git check-ignore` confirms the repo `.gitignore` rule.)
- [ ] A fresh agent session, asked "how do I run the checks?", answers correctly from the repo alone. (Left for the reviewer: re-reading `AGENTS.md` myself is not the check described. Self-read found the "Running the checks" section answers the question, but no fresh session was run.)

## Review depth

**Close read.** This file steers every later agent session, so a wrong line here repeats in every PR.
