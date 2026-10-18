# Plans

One plan per PR. Each plan is a single idea that lands as its own PR, and each builds on the plans before it. These cover PR 0 (Foundation) from `first-build-pr-sequence.md`; plans for later PRs are written when their turn comes.

## How plans are used

- A PR starts from its plan. The agent restates the plan and confirms it understands before touching code.
- A plan says what must be true when the PR lands, not how to build it. How is the agent's call.
- The acceptance checklist is the definition of done. Each item can be checked by running something or looking at the PR.
- **Review depth** tells the human reviewer how closely to read: *close read* (line by line, judgement-heavy or security-adjacent) or *skim* (boilerplate, generated config).
- The implementing PR flips the plan's `Status` line to `merged` and adds the PR link.

## Sections every plan has

| Section | Content |
| --- | --- |
| Status | `planned`, `in progress`, or `merged` with the PR link |
| Goal | The one idea this PR lands, and why it matters |
| Builds on | The plans that must be merged first |
| In scope | What this PR contains |
| Out of scope | What it deliberately does not, and which plan owns it if any |
| Acceptance checklist | Verifiable items; the PR is done when all are ticked |
| Review depth | Close read or skim, with the reason |

## PR 0 sequence

| # | Plan | Review |
| --- | --- | --- |
| 01 | [Dev container](01-dev-container.md) | Close read of secrets handling, skim the rest |
| 02 | [Dev container CI](02-dev-container-ci.md) | Close read |
| 03 | [Secret scanning pre-commit hook](03-secret-scanning-hook.md) | Skim |
| 04 | [Agent harness](04-agent-harness.md) | Close read |
| 05 | [Conventions and working principles](05-conventions.md) | Close read |
| 06 | [Changed-files pre-commit checks](06-changed-files-checks.md) | Close read |
| 07 | [CI gate](07-ci-gate.md) | Close read |
| 08 | [Workflow safety](08-workflow-safety.md) | Close read |
| 09 | [GitHub secret scanning and push protection](09-github-secret-scanning.md) | Skim |
| 10 | [Dependabot](10-dependabot.md) | Skim |
| 11 | [CodeQL](11-codeql.md) | Skim |
| 12 | [Which docs are committed](12-docs-in-repo.md) | Close read |
| 13 | [Host-side token file](13-host-token-store.md) | Close read |
| 14 | [bats-core](14-bats-core.md) | Skim |
| 15 | [bats in CI](15-bats-ci.md) | Skim |
| 16 | [bats tests for bash scripts](16-bats-script-tests.md) | Skim |
| 17 | [Workflow linters in CI and pre-commit](17-workflow-linters.md) | Close read |
| 18 | [start.sh launcher tests in bats, and --help](18-start-launcher-bats.md) | Close read of the `--help` parsing, skim the tests |
| 19 | [Pull before --rebuild](19-rebuild-pull.md) | Close read |

The AGPL-3.0 `LICENSE` is already committed and has no plan.
