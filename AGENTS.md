# AGENTS.md

Entry point for coding agents (Claude Code and others). Read it in full before changing anything.

Development is expected to happen inside the dev container; `.devcontainer/README.md` explains how to start it.

family-ai is a public repo (AGPL-3.0). There is no `CLAUDE.md`; this file is the single entry point.

## Public repo: keep security concerns private

This is the one non-negotiable rule. Read it before writing any PR text or commit message.

Anything in a PR title or description, commit message, issue, comment, code, docs or any other place on GitHub that others can see is public.

Do not put security concerns about the local environment there. That includes credentials or tokens found on disk, how the dev container is set up or exposed, and weaknesses in local tooling or config. Tell the user in the session instead and let them decide what, if anything, to publish.

## Delegate implementation and review

The primary agent must coordinate all repository changes and delegate implementation to sub-agents. This applies to code, documentation, configuration, and all other repository changes.

A separate sub-agent who did not implement the changes must review them. Address all review findings before considering the work complete.

Keep pull request titles and descriptions simple and high level, with a mermaid diagram where it helps explain the change.

## Where things are

| Path | What |
| --- | --- |
| `docs/plans/` | One plan per PR. `docs/plans/README.md` has the sequence and the format. |
| `first-build-pr-sequence.md` (repo root) | The overall PR sequence and the working notes behind it. |
| `.devcontainer/` | The dev container. `.devcontainer/README.md` explains it. |
| `.env.example` | Names of host environment variables the container reads, no values. Real `.env` files are gitignored and never committed. |
| `lefthook.yml` | Git hook configuration (secret scan and changed-files checks before each commit). |
| `.lefthook/` | Helper scripts the hook calls (currently the placeholder step). |
| `.github/workflows/` | CI. Workflows follow the [workflow safety rules](docs/conventions.md#workflow-safety); actionlint and zizmor check workflows in CI and before commits. `.github/dependabot.yml` configures Dependabot version updates. |
| `docs/README.md` | What lives in `docs/`, and the rule for what may be committed there. |
| `docs/github-settings.md` | GitHub settings the owner applies by hand (branch protection, fork PR approval, secret scanning and push protection, Dependabot alerts and security updates, CodeQL default setup). |

There is no application code yet. Language and framework rules arrive with the PRs that introduce those stacks.

## Running the checks

Write tests first (TDD) and run the tests for the code you are changing. Do not run sanity passes of the whole test suite or lint every file; focus on the task at hand.

- **On commit:** lefthook (`lefthook.yml`) runs gitleaks on staged changes and actionlint plus zizmor on staged `.github/workflows/*.{yml,yaml}` files. Other changed-files checks remain placeholders. It is installed into `.git/hooks` by `.devcontainer/postStartCommand.sh`, so it runs on commits made inside the dev container; commits outside it skip the hook. Never bypass it with `--no-verify`. If it blocks a commit, fix the finding; any allowlist entry needs a written reason.
- **On the PR:** CI runs every test and lint job that exists, so whatever the hook does not cover is caught there. The `ci` job in `.github/workflows/devcontainer-ci.yml` is the single check meant to be required before merge; a PR that adds a CI job adds it to `ci`'s `needs:` (owner setup in [docs/github-settings.md](docs/github-settings.md)).
