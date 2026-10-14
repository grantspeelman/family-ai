# AGENTS.md

Entry point for coding agents (Claude Code and others). Read it in full before changing anything. `CLAUDE.md` points here.

family-ai is a public repo (AGPL-3.0). The one non-negotiable rule is in `CLAUDE.md`: anything others can see on GitHub is public (PR titles and descriptions, commit messages, issues, comments, code, docs). Keep security concerns about the local environment out of all of it. Examples: credentials found on disk, how the dev container is set up or exposed, weaknesses in local tooling or config. Report such concerns to the user in the session instead and let them decide what to publish. Read `CLAUDE.md` before writing any PR text or commit message.

## Where things are

| Path | What |
| --- | --- |
| `docs/plans/` | One plan per PR. `docs/plans/README.md` has the sequence and the format. |
| `first-build-pr-sequence.md` (repo root) | The overall PR sequence and the working notes behind it. |
| `.devcontainer/` | The dev container. `.devcontainer/README.md` explains it. |
| `.env.example` | Names of host environment variables the container reads, no values. Real `.env` files are gitignored and never committed. |
| `lefthook.yml` | Git hook configuration (secret scan and changed-files checks before each commit). |
| `.lefthook/` | Helper scripts the hook calls (currently the placeholder step). |
| `.github/workflows/` | CI. Workflows follow the [workflow safety rules](docs/conventions.md#workflow-safety); `.github/scripts/check-workflow-rules.py` checks the mechanical ones. |
| `docs/github-settings.md` | GitHub settings the owner applies by hand (branch protection). |

There is no application code yet. Language and framework rules arrive with the PRs that introduce those stacks.

## Entering the container

Work happens in the dev container, where the tools and the commit hook are installed. From a host terminal:

```
.devcontainer/start.sh             # build if needed, start, open a shell; rerun to get back in
.devcontainer/start.sh --rebuild   # rebuild the image from scratch
```

`--docker` adds nested Docker (privileged); use it only when a task needs Docker. Details in `.devcontainer/README.md`. If `command -v gitleaks lefthook` finds both, you are likely already inside it.

## Running the checks

Today the only real automated check is the secret scan. The hook also has placeholder steps for lint, tests, duplication and complexity, and `cargo-deny`; they run on matching changed files, print that they are placeholders and check nothing, until PRs 2 and 7 install the tools. Until PR 2 and PR 7 add their globs, these steps cover only `.rs` and `Cargo.*` files. Files of any other type, including the registry app's, are checked by the secret scan only, and are skipped by the other steps without any output. Inside the container:

```
lefthook run pre-commit                                          # what the commit hook runs
gitleaks git --pre-commit --staged --redact --no-banner --verbose   # the secret scan itself, on staged changes
```

The first command runs the whole hook (secret scan first, then each other step on the staged files that match its glob) through the hook configuration; the second is the secret scan alone. A step that exits non-zero blocks the commit. If the hook blocks a commit, fix or remove the finding. Never bypass it with `--no-verify`; any allowlist entry needs a written reason. How to add a language's checks is in [docs/conventions.md](docs/conventions.md#pre-commit-hook-what-passing-means-and-adding-a-language).

The hook is installed into `.git/hooks` by `.devcontainer/postStartCommand.sh`, so it runs on every commit made inside the container. Commits outside the container skip it; CI is the backstop.

CI runs the gate on every PR and on pushes to `main`: the job named `ci` in `.github/workflows/devcontainer-ci.yml` fails if any job it depends on fails, and is the one check meant to be required before merge (the owner sets that in GitHub, see [docs/github-settings.md](docs/github-settings.md)). Today it covers the container checks below, the workflow safety lint (`actionlint` plus `.github/scripts/check-workflow-rules.py`), the full-history secret scan, and the hook's checks run over the whole tree (`lefthook run pre-commit --all-files`; the placeholder steps skip until PRs 2 and 7, and the hook's gitleaks step scans nothing there because no files are staged, so the full-history scan is the secret check). Lint, tests, duplication, complexity and dependency license checks are not wired into CI yet.

Checks for the container image itself (`.devcontainer/smoke-test.sh`, `.devcontainer/container-structure-test.yaml`) are described in `.devcontainer/README.md` and run in CI.

There is no test suite yet. When a PR adds one, that PR adds its command to this section.

## Per-PR workflow

Plans are numbered. Each builds on the plans listed under its "Builds on", and each lands as its own PR, stacked on the previous one (branches look like `pr0/NN-slug`). Each plan also states a review depth, close read or skim, which tells the human reviewer how closely to read; `docs/plans/README.md` explains it.

1. Start from the plan in `docs/plans/`. One plan, one PR; do not build other plans' work.
2. Restate the plan (goal, scope, checklist) and confirm you understand it before touching anything.
3. Implement only what is in scope. Set the plan's `Status` to `in progress`; it stays that way while the PR is open.
4. Run the tests and checks and show them passing. Do not mark the PR done on memory.
5. Tick only the checklist items you verified; leave the rest unticked with a reason. The PR is done only when every item is ticked, or each unticked one has a stated reason (owner action, needs a real CI run, and so on).
6. Once the PR exists and is ready, flip `Status` to `merged` with the PR link in a final one-line push to that same PR.

## Conventions

Working principles, the sizing rule, the dependency license policy and commit and PR conventions are in [docs/conventions.md](docs/conventions.md). It also says which rules are enforced today and which wait on later plans.

## Claude Code settings

Shared project settings, if any, live in `.claude/settings.json` and are committed. Personal settings go in `.claude/settings.local.json`, which is gitignored. Commit nothing personal under `.claude/`.
