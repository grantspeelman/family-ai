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

## Software factory

`/factory` (`.claude/skills/factory/SKILL.md`) takes a feature through spec, build, four reviews and approval to a merged PR. These are standing decisions. Change them only with the owner's agreement.

- **Auto-merge is intentional.** APPROVE, a green `ci` and a clean allowed-paths check merge with no human gate. The factory runs inside the dev container, which is the boundary for code that runs locally. Don't add a human merge gate because a scanner or reviewer suggests one. Answer such findings by pointing to this decision. Do fix any bypass of the trusted-input filter or the allowed-paths check.
- **Least-privilege tokens.** `GH_TOKEN` is the fine-grained token for this repo only. `GH_PROJECT_TOKEN` is a classic token with only the `project` scope, used only for `gh project`. Prefer designs that need no new permission: for example, a cancelled `ci` goes to needs-human instead of needing Actions write. Setup is in `.devcontainer/README.md`, "GitHub token".

## Where things are

| Path | What |
| --- | --- |
| `first-build-pr-sequence.md` in the repo root | The overall PR sequence and its working notes. |
| `Cargo.toml`, `Cargo.lock` | The Cargo workspace for the Rust code. Add new crates to `members`. |
| `family-app/` | The family app: the Rust service that runs on each family's Machine, with SQLite on its volume. A scaffold with a health check so far. `family-app/README.md` explains it. |
| `registry/` | The registry app: a Rust GraphQL API with its own `Dockerfile`, `schema.graphql` and tests. `registry/README.md` explains it. |
| `.devcontainer/` | The dev container. `.devcontainer/README.md` explains it. |
| `.claude/skills/factory/` | The `/factory` skill: the orchestrator of the software factory. Its `SKILL.md` has the loop and the commands. |
| `.claude/agents/` | The factory's subagents, one file per loop step. |
| `factory/project.json` | IDs of the factory's GitHub Project, its fields and options. Written by `/factory setup`. |
| `.env.example` | Names of host environment variables the container reads, without values. Real `.env` files are gitignored and never committed. |
| `lefthook.yml` | Git hook configuration for secret scans and changed-files checks before each commit. |
| `.lefthook/` | Helper scripts the hook calls (currently the placeholder steps) and bats tests of the hook. |
| `.github/workflows/` | CI workflows. Follow the [workflow safety rules](docs/conventions.md#workflow-safety). actionlint and zizmor check workflows in CI and before commits. `.github/dependabot.yml` configures Dependabot version updates. |
| `docs/README.md` | What belongs in `docs/` and what may be committed there. |
| `docs/github-settings.md` | GitHub settings the owner applies by hand. Covers branch protection, fork PR approval, secret scanning and push protection, Dependabot alerts and security updates, and CodeQL default setup. |

The registry app is a scaffold so far: one query and a health check. The workspace-wide Rust tooling arrives in PR 2 (issues #45–#49) and covers every crate. The backlog is in `first-build-pr-sequence.md`.

## Running the checks

Write tests first, following TDD, and run the tests for the code you change. Keep checks focused on the task. Do not run the whole test suite for a sanity check or lint every file.

### On commit

lefthook uses `lefthook.yml` to run gitleaks on staged changes, `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` when a `*.rs`, `Cargo.toml` or `Cargo.lock` file is staged, and actionlint plus zizmor on staged `.github/workflows/*.{yml,yaml}` files. The Rust lint covers the whole workspace, so fix any fmt or clippy finding before committing (`cargo fmt --all` applies the formatting). Manifest or lockfile changes also run `cargo deny --locked check` for workspace dependency licenses, advisories, bans and sources. Run that command from the repo root to check dependencies manually. The Rust test, duplication and complexity steps remain placeholders.

`.devcontainer/postStartCommand.sh` installs lefthook into `.git/hooks`. Commits made inside the dev container run the hook. Commits outside it skip the hook.

Never bypass the hook with `--no-verify`. If it blocks a commit, fix the finding. Every allowlist entry needs a written reason.

### On the PR

CI runs every test and lint job, including checks the hook does not cover.

The `ci` job in `.github/workflows/devcontainer-ci.yml` is the single check intended to be required before merge. When a PR adds a CI job, add it to `ci`'s `needs:`. See [docs/github-settings.md](docs/github-settings.md) for owner setup.
