# Conventions

Standing rules for every PR in this build. `AGENTS.md` is the entry point and covers the mechanics (container, checks, per-PR steps). This document covers the principles behind them. Where something is not decided yet, it says so.

"PR N" means the Nth PR in the build sequence in `first-build-pr-sequence.md`. "Plan NN" means the numbered plan in `docs/plans/`. They are different numberings: for example PR 7 is the family app tooling, while plan 07 is the CI gate.

## What is enforced today

Only the secret scan (gitleaks). The pre-commit hook runs it, but only inside the dev container; CI scans the full history on every PR and push to main. Everything else here is a convention that people and agents follow by hand until the tooling lands:

| Rule | Enforced by | Status |
| --- | --- | --- |
| No secrets in commits | gitleaks pre-commit hook | Enforced today |
| Lint, tests, duplication and complexity on changed files | Pre-commit hook | Plan 06, not yet |
| Full test suite as the merge gate | CI | Plan 07, not yet |
| Dependency license check, registry app | CI | PR 2, not yet |
| Dependency license check, Rust (`cargo-deny`) | CI, and the hook when dependencies change | PR 7, not yet |

## Working principles

These apply to every PR, not just the foundation ones.

1. **Start from a written spec.** Each PR starts from its plan (a short spec and checklist). The agent restates the goal, scope and checklist and confirms it understands before touching code.
2. **Decide review depth per PR, and size the PR by it.** Each plan says whether it needs a close read or a skim (see sizing below).
3. **The agent runs the tests itself and shows them passing** before marking a PR done. The pre-commit hook will back this up automatically once plan 06 lands; until then the agent does not rely on memory.
4. **Keep PRs small enough for a quick loop.** Rust compile times are slow, so large PRs make the test-fix-retest loop painful for the agent.

## Sizing rule

Size a PR by how much human attention it needs, not by a fixed line count.

- Boilerplate-heavy, low-judgement work (scaffolding, generated tooling config) may be larger. It is skimmed.
- Work with real judgement calls stays small and is read line by line: API contracts, data handling, security-adjacent code.
- This rule applies to every PR. The two PRs flagged as likely bundles (PR 2, registry app tooling, and PR 7, family app tooling) are broken down live before starting, not pre-cut.

## Licensing

The `LICENSE` file is AGPL-3.0 and committed. `first-build-pr-sequence.md` still calls AGPL-3.0 a leaning, not final. Treat it as in place but not confirmed.

**Dependency license policy.** An agent will add whatever dependency makes the tests pass, so the check is mechanical, not remembered.

Allowed:

- Permissive licenses: MIT, Apache-2.0, BSD, ISC, and public domain (such as SQLite).

Disallowed:

- GPL-2.0-only
- Proprietary libraries
- Source-available licenses with usage restrictions: SSPL, BSL, Elastic, FSL, O'Saasy

Licenses on neither list (LGPL, MPL, GPL-3.0 and so on): not yet decided. Ask the user before adding such a dependency.

Scope: dependencies linked into shipped code. Dev tooling (linters, coverage, CI) is not linked into shipped code and is out of scope.

Planned checks (none exist yet):

- Rust: `cargo-deny` with a license allowlist, in CI (PR 7) and in the pre-commit hook when dependencies change.
- Registry app: the equivalent license-check tool for its stack, added in PR 2.

Open items:

- Whether to require a CLA (or similar) before accepting outside contributions. Undecided.
- The license check for the front end's dependencies. It waits on the front end's stack being chosen.

## Commits and PRs

- **One concept per PR.** If a PR needs "and" to describe it, split it.
- **Stacked branches.** Foundation PRs are stacked, one on the previous, on branches named `pr0/NN-slug` matching the plan number.
- **Commit messages.** A short imperative summary line (for example "Add gitleaks pre-commit hook"), then a body only if the why is not obvious. Commits made by an agent end with a `Co-Authored-By:` line naming the agent.
- **PR description.** Say what the PR does, link its plan, state the checklist status (each item ticked, or unticked with the reason), and state the review depth that applies (close read or skim).
- **Plan status.** Set the plan's `Status` to `in progress` while the PR is open, and to `merged` with the PR link at the end (see `AGENTS.md`).
- **Never bypass the hook** with `--no-verify`.
- **Public text stays clean.** Commit messages, PR text and docs are public. Follow the rule in `CLAUDE.md` about security concerns.
- **Pricing and positioning stay out of the repo.** Architecture docs may be committed; which docs are, is decided in plan 12.
