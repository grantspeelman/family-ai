# Conventions

Standing rules for every PR in this build. `AGENTS.md` is the entry point and covers the mechanics (container, checks, per-PR steps). This document covers the principles behind them. Where something is not decided yet, it says so.

"PR N" means the Nth PR in the build sequence in `first-build-pr-sequence.md`. "Plan NN" means the numbered plan in `docs/plans/`. They are different numberings: for example PR 7 is the family app tooling, while plan 07 is the CI gate.

## What is enforced today

Only the secret scan (gitleaks) is a real check today. The pre-commit hook runs it, but only inside the dev container; CI scans the full history on every PR and push to main. The hook also carries placeholder steps for the checks below, which run on matching changed files and check nothing until the tools are installed. Everything else here is a convention that people and agents follow by hand until the tooling lands:

Coverage gap: Until PR 2 and PR 7 add their globs, these steps cover only `.rs` and `Cargo.*` files. Files of any other type, including the registry app's, are checked by the secret scan only, and are skipped by the other steps without any output.

| Rule | Enforced by | Status |
| --- | --- | --- |
| No secrets in commits | gitleaks pre-commit hook | Enforced today |
| Lint, tests, duplication and complexity on changed files | Pre-commit hook | Hook wiring in place (plan 06); steps are placeholders until PRs 2 and 7 |
| Merge gate: one required `ci` check over all CI jobs | CI (`ci` job in `devcontainer-ci.yml`) | Workflow in place (plan 07). It runs the container checks, the full-history secret scan and the hook over the whole tree. The hook's gitleaks step scans nothing there (it only looks at staged files, and none are staged), so the full-history scan is the secret check. Lint, tests, duplication, complexity and license checks are not wired yet (PRs 2 and 7). Requiring it in branch protection is the owner's action: [docs/github-settings.md](github-settings.md) |
| Dependency license check, registry app | CI | PR 2, not yet |
| Dependency license check, Rust (`cargo-deny`) | CI, and the hook when dependencies change | Hook step is a placeholder (plan 06); real check in PR 7, not yet |

## Pre-commit hook: what passing means, and adding a language

The hook is configured in `lefthook.yml`. It is a fast local check on the files in the commit. CI is the real gate: today it runs the container checks and the full-history secret scan, and it will run the full suite (lint, tests, duplication, complexity, licenses) once PRs 2 and 7 land.

**Passing** means every step that applies to the staged files exits zero. A step applies when at least one staged file matches its glob; otherwise it is skipped. Any non-zero exit blocks the commit. Placeholder steps exit zero but are not checks run: they print `PLACEHOLDER` and what they would have received, and lefthook's summary still shows them as done, so read the output, not the tick. A hook pass is not evidence that lint, tests, duplication, complexity or dependency licenses were checked until the real steps replace the placeholders.

Steps run in order and stop at the first failure. Each step gets only the staged files matching its glob, through `{staged_files}`, not the whole tree. `{staged_files}` covers added, copied, modified and renamed files; deleted files are not passed.

To add a language's checks (PRs 2 and 7), edit the matching step in `lefthook.yml`:

1. Set the step's `glob` to the language's files (for example `"*.rs"`), if the placeholder's glob is not right.
2. Replace `.lefthook/placeholder.sh ...` in `run` with the real command, passing `{staged_files}` when the tool accepts file arguments. Tools that must see the whole project (a test run, say) can ignore the file list but should still be scoped by the glob so they only run when that language changed.
3. Keep the step fast; the hook only helps if it is not skipped. Anything slow belongs in CI.
4. Delete the `# PLACEHOLDER` comment above the step, and update the table above and the "Running the checks" section of `AGENTS.md`.

For `cargo-deny`, the step's glob matches `Cargo.toml` and `Cargo.lock`, so it only runs when dependencies change.

## Working principles

These apply to every PR, not just the foundation ones.

1. **Start from a written spec.** Each PR starts from its plan (a short spec and checklist). The agent restates the goal, scope and checklist and confirms it understands before touching code.
2. **Decide review depth per PR, and size the PR by it.** Each plan says whether it needs a close read or a skim (see sizing below).
3. **The agent runs the tests itself and shows them passing** before marking a PR done. The pre-commit hook backs this up on changed files; it does not replace running the tests, and CI is the real gate (it will run the full suite once PRs 2 and 7 land; today it runs the container checks and the full-history secret scan).
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
- **Public text stays clean.** Commit messages, PR text and docs are public. Follow the public-repo rule in `AGENTS.md` about security concerns.
- **Pricing and positioning stay out of the repo.** Architecture docs may be committed; which docs are, is decided in plan 12.
