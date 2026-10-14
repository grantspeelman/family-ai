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
| `lefthook.yml` | Git hook configuration (secret scan before each commit). |
| `.github/workflows/` | CI. |

There is no application code yet. Language and framework rules arrive with the PRs that introduce those stacks.

## Entering the container

Work happens in the dev container, where the tools and the commit hook are installed. From a host terminal:

```
.devcontainer/start.sh             # build if needed, start, open a shell; rerun to get back in
.devcontainer/start.sh --rebuild   # rebuild the image from scratch
```

`--docker` adds nested Docker (privileged); use it only when a task needs Docker. Details in `.devcontainer/README.md`. If `command -v gitleaks lefthook` finds both, you are likely already inside it.

## Running the checks

Today the only automated check is the secret scan. Inside the container:

```
lefthook run pre-commit                                          # what the commit hook runs
gitleaks git --pre-commit --staged --redact --no-banner --verbose   # the scan itself, on staged changes
```

The two commands are the same check; the first runs the second through the hook configuration. If the hook blocks a commit, fix or remove the finding. Never bypass it with `--no-verify`; any allowlist entry needs a written reason.

The hook is installed into `.git/hooks` by `.devcontainer/postStartCommand.sh`, so it runs on every commit made inside the container. Commits outside the container skip it; CI is the backstop.

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
