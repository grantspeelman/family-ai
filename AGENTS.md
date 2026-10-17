# AGENTS.md

Entry point for coding agents (Claude Code and others). Read it in full before changing anything.

Development is expected to happen inside the dev container; `.devcontainer/README.md` explains how to start it.

family-ai is a public repo (AGPL-3.0). There is no `CLAUDE.md`; this file is the single entry point.

## Public repo: keep security concerns private

This is the one non-negotiable rule. Read it before writing any PR text or commit message.

Anything in a PR title or description, commit message, issue, comment, code, docs or any other place on GitHub that others can see is public.

Do not put security concerns about the local environment there. That includes credentials or tokens found on disk, how the dev container is set up or exposed, and weaknesses in local tooling or config. Tell the user in the session instead and let them decide what, if anything, to publish.

## Where things are

| Path | What |
| --- | --- |
| `docs/plans/` | One plan per PR. `docs/plans/README.md` has the sequence and the format. |
| `first-build-pr-sequence.md` (repo root) | The overall PR sequence and the working notes behind it. |
| `.devcontainer/` | The dev container. `.devcontainer/README.md` explains it. |
| `.env.example` | Names of host environment variables the container reads, no values. Real `.env` files are gitignored and never committed. |
| `lefthook.yml` | Git hook configuration (secret scan before each commit). |
| `.github/workflows/` | CI. |

There is no application code yet. Language and framework rules arrive with the PRs that introduce those stacks.

## Running the checks

Write tests first (TDD) and run the tests for the code you are changing. Do not run sanity passes of the whole test suite or lint every file; focus on the task at hand.

- **On commit:** lefthook (`lefthook.yml`) runs the minimum required checks on the files you changed. It is installed into `.git/hooks` by `.devcontainer/postStartCommand.sh`, so it runs on commits made inside the dev container; commits outside it skip the hook. Never bypass it with `--no-verify`. If it blocks a commit, fix the finding; any allowlist entry needs a written reason.
- **On the PR:** CI runs the full checks, so whatever the hook does not cover is caught there.

Today the hook runs only the secret scan, and there is no test suite yet. Each PR that adds a check or a test suite says how to run it in its own plan.

## Per-PR workflow

Plans are numbered. Each builds on the plans listed under its "Builds on", and each lands as its own PR, stacked on the previous one (branches look like `pr0/NN-slug`). Each plan also states a review depth, close read or skim, which tells the human reviewer how closely to read; `docs/plans/README.md` explains it.

1. Start from the plan in `docs/plans/`. One plan, one PR; do not build other plans' work.
2. Restate the plan (goal, scope, checklist) and confirm you understand it before touching anything.
3. Implement only what is in scope. Set the plan's `Status` to `in progress`; it stays that way while the PR is open.
4. Write the tests first, then the code, and run the tests for what you changed and show them passing. Do not mark the PR done on memory.
5. Tick only the checklist items you verified; leave the rest unticked with a reason. The PR is done only when every item is ticked, or each unticked one has a stated reason (owner action, needs a real CI run, and so on).
6. Once the PR exists and is ready, flip `Status` to `merged` with the PR link in a final one-line push to that same PR.

## Conventions

The long-form conventions and working principles arrive with [plan 05](docs/plans/05-conventions.md). The conventions document itself does not exist yet and will be linked from here when it does.

## Claude Code settings

Shared project settings, if any, live in `.claude/settings.json` and are committed. Personal settings go in `.claude/settings.local.json`, which is gitignored. Commit nothing personal under `.claude/`.
