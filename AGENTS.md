# AGENTS.md

Entry point for coding agents (Claude Code and others). Read it in full before changing anything. `CLAUDE.md` points here.

family-ai is a public repo (AGPL-3.0). The one non-negotiable rule is in `CLAUDE.md`: keep security concerns about the local environment out of anything that lands on GitHub, and tell the user in the session instead.

## Where things are

| Path | What |
| --- | --- |
| `docs/plans/` | One plan per PR. `docs/plans/README.md` has the sequence and the format. |
| `first-build-pr-sequence.md` | The overall PR sequence and the working notes behind it. |
| `.devcontainer/` | The dev container. `.devcontainer/README.md` explains it. |
| `lefthook.yml` | Git hook configuration (secret scan before each commit). |
| `.github/workflows/` | CI. |

There is no application code yet. Language and framework rules arrive with the PRs that introduce those stacks.

## Entering the container

Work happens in the dev container, where the tools and the commit hook are installed. From a host terminal:

```
.devcontainer/start.sh             # build if needed, start, open a shell; rerun to get back in
.devcontainer/start.sh --rebuild   # rebuild the image from scratch
```

`--docker` adds nested Docker (privileged); use it only when a task needs Docker. Details in `.devcontainer/README.md`. If you are already in a shell at `/workspace` with `gitleaks` and `lefthook` on the path, you are inside it.

## Running the checks

Today the only automated check is the secret scan. Inside the container:

```
lefthook run pre-commit                                          # what the commit hook runs
gitleaks git --pre-commit --staged --redact --no-banner --verbose   # the scan itself, on staged changes
```

The hook is installed into `.git/hooks` by `.devcontainer/postStartCommand.sh`, so it runs on every commit made inside the container. Commits outside the container skip it; CI is the backstop.

Checks for the container image itself (`.devcontainer/smoke-test.sh`, `.devcontainer/container-structure-test.yaml`) are described in `.devcontainer/README.md` and run in CI.

There is no test suite yet. When a PR adds one, that PR adds its command to this section.

## Per-PR workflow

1. Start from the plan in `docs/plans/`. One plan, one PR; do not build other plans' work.
2. Restate the plan (goal, scope, checklist) and confirm you understand it before touching anything.
3. Implement only what is in scope. Set the plan's `Status` to `in progress`.
4. Run the tests and checks and show them passing. Do not mark the PR done on memory.
5. Tick only the checklist items you verified; leave the rest unticked with a reason.
6. On merge the plan's `Status` becomes `merged` with the PR link.

## Conventions

The long-form conventions and working principles are added in a later plan (05) and will be linked from here then.

## Claude Code settings

Shared project settings, if any, live in `.claude/settings.json` and are committed. Personal settings go in `.claude/settings.local.json`, which is gitignored. Commit nothing personal under `.claude/`.
