# AGENTS.md

This is the single entry point for coding agents, including Claude Code. Read it in full before changing anything.

Development should happen inside the dev container. `.devcontainer/README.md` explains how to start it.

family-ai is a public repo licensed under AGPL-3.0. There is no `CLAUDE.md`.

## Public repo: keep security concerns private

This rule is non-negotiable. Read it before writing any PR text or commit message.

PR titles and descriptions, commit messages, issues, comments, code, docs, and anything else others can see on GitHub are public.

Do not publish security concerns about the local environment. This includes credentials or tokens found on disk, how the dev container is set up or exposed, and weaknesses in local tooling or config. Tell the user in the session and let them decide what, if anything, to publish.

## Delegate implementation and review

The primary agent must coordinate all repository changes and delegate implementation to sub-agents. This includes code, documentation, and configuration.

A sub-agent who did not implement the changes must review them. Address all review findings before the work is complete.

Keep pull request titles and descriptions simple and high level. Add a Mermaid diagram where it helps explain the change.

## Where things are

| Path | What |
| --- | --- |
| `first-build-pr-sequence.md` in the repo root | The overall PR sequence and its working notes. |
| `Cargo.toml`, `Cargo.lock` | The Cargo workspace for the Rust code. Add new crates to `members`. |
| `registry/` | The registry app: a Rust GraphQL API with its own `Dockerfile`, `schema.graphql` and tests. `registry/README.md` explains it. |
| `.devcontainer/` | The dev container. `.devcontainer/README.md` explains it. |
| `.env.example` | Names of host environment variables the container reads, without values. Real `.env` files are gitignored and never committed. |
| `lefthook.yml` | Git hook configuration for secret scans and changed-files checks before each commit. |
| `.lefthook/` | Helper scripts the hook calls, currently for the placeholder step. |
| `.github/workflows/` | CI workflows. Follow the [workflow safety rules](docs/conventions.md#workflow-safety). actionlint and zizmor check workflows in CI and before commits. `.github/dependabot.yml` configures Dependabot version updates. |
| `docs/README.md` | What belongs in `docs/` and what may be committed there. |
| `docs/github-settings.md` | GitHub settings the owner applies by hand. Covers branch protection, fork PR approval, secret scanning and push protection, Dependabot alerts and security updates, and CodeQL default setup. |

The registry app is a scaffold so far: one query and a health check. Language and framework rules arrive with the PRs that introduce those stacks (the registry app's tooling in PR 2, the family app's in PR 7).

## Running the checks

Write tests first, following TDD, and run the tests for the code you change. Keep checks focused on the task. Do not run the whole test suite for a sanity check or lint every file.

### On commit

lefthook uses `lefthook.yml` to run gitleaks on staged changes and actionlint plus zizmor on staged `.github/workflows/*.{yml,yaml}` files. Other changed-files checks remain placeholders.

`.devcontainer/postStartCommand.sh` installs lefthook into `.git/hooks`. Commits made inside the dev container run the hook. Commits outside it skip the hook.

Never bypass the hook with `--no-verify`. If it blocks a commit, fix the finding. Every allowlist entry needs a written reason.

### On the PR

CI runs every test and lint job, including checks the hook does not cover.

The `ci` job in `.github/workflows/devcontainer-ci.yml` is the single check intended to be required before merge. When a PR adds a CI job, add it to `ci`'s `needs:`. See [docs/github-settings.md](docs/github-settings.md) for owner setup.
