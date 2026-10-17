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
| Workflow safety rules (below) | CI (`workflows` job: `actionlint` and `.github/scripts/check-workflow-rules.py`) | Workflow in place (plan 08). Checks syntax, untrusted input in `run:`, SHA pins, `persist-credentials`, `secrets.`, `pull_request_target` / `workflow_run` and read-only top-level permissions. Judgement rules (per-job write permissions, a reason for each) are for the reviewer. The fork-PR approval setting is the owner's action: [docs/github-settings.md](github-settings.md) |
| Bash script tests (`*.bats`) | CI (`image` job, step "Bats tests", inside the `ci` gate) | Workflow in place (plan 15). Finds every tracked `*.bats` file with `git ls-files` and runs them in the built image; fails if none are found. Not run by the pre-commit hook |
| Dependabot version updates | `.github/dependabot.yml` | Config in place (plan 10) for GitHub Actions and the dev container's `FROM` image. Alerts and security updates are repo settings, the owner's action: [docs/github-settings.md](github-settings.md) |
| CodeQL analysis (GitHub Actions workflows) | `.github/workflows/codeql.yml` | Workflow in place (plan 11), analysing the `actions` language on PRs, pushes to `main` and weekly. Not a required check and not in the `ci` gate's `needs:` yet. Results in the code scanning tab need the owner's settings: [docs/github-settings.md](github-settings.md) |
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

## Testing bash scripts

Bash scripts are tested with [bats-core](https://github.com/bats-core/bats-core), installed in the dev container. Tests live in `*.bats` files next to the script they cover and run with `bats <file>`. CI runs every tracked `*.bats` file automatically (the `image` job finds them with `git ls-files` and runs them in the built image, so the same pinned bats-core), so a new test needs no workflow change. The file must be committed (tracked) to be found, and CI fails if it finds none. See [plan 14](plans/14-bats-core.md), [plan 15](plans/15-bats-ci.md) and [plan 16](plans/16-bats-script-tests.md).

Covered today: `.devcontainer/start.sh`, `postStartCommand.sh`, `entrypoint.sh`, the token-file check, and `.lefthook/placeholder.sh`. Tests shim external commands (`docker`, `ssh-agent` and so on) and use temp dirs, never the real `HOME`. `.devcontainer/smoke-test.sh` is the deliberate exception: it builds and starts a real container, so it needs Docker and a network and CI runs it as its own step.

## Workflow safety

Anyone can open a PR against this public repo, so no workflow may run untrusted code with secrets or write access available. Every workflow in `.github/workflows/` follows these rules. A new or changed workflow is checked against them in review, and the `workflows` CI job checks the mechanical ones.

1. **Read-only by default.** Set `permissions: contents: read` at the top of the workflow. Anything broader is set on the one job that needs it, with a comment giving the reason.
2. **No `pull_request_target` or `workflow_run`** that checks out or runs fork code. The check script is stricter and bans both triggers outright, so adding one needs a deliberate change to the script and a written reason. Today no workflow uses either; use plain `pull_request`, which gives fork PRs a read-only token and no secrets.
3. **Fork PRs get no secrets, so workflows must not need any.** No `secrets.` reference in a workflow unless a reviewer has agreed it is safe and the reason is written next to it.
4. **No real Fly token or API key in GitHub Actions.** CI runs against the Fly mock.
5. **Untrusted input never goes straight into `run:`.** PR titles, branch names, issue text and anything else under a contributor's control (`github.head_ref`, `github.event.*`) is passed through `env:` and read as a shell variable. No `${{ }}` expression inside a `run:` script.
6. **Third-party actions are pinned by full commit SHA**, with the version in a trailing comment (`uses: owner/action@<40 hex> # v1.2.3`). Tags and branches move; a SHA does not. Downloaded binaries are pinned by version and verified against a SHA-256 taken from the project's published checksums.
7. **`persist-credentials: false` on checkout** unless the job must push.
8. **Every job has `timeout-minutes`.**

The `workflows` job checks part of rules 1 to 3 and 5 to 8, plus `actionlint`'s own syntax and expression checks: top-level permissions are present and not `write`; no `pull_request_target`, `workflow_run` or `secrets` reference; no `${{ }}` in a `run:` block; every `uses:` is SHA-pinned with a version comment; every checkout sets `persist-credentials: false`; every job has `timeout-minutes`. Reviewers still check: per-job permissions and their reasons, rule 4 (an API key can hide in a plain value, not just `secrets.`), the checksums of downloaded binaries, and anything the script cannot see. The script is a line-based text check, not a YAML parser, so a pass is not proof. Known limits: flow-style YAML (other than `permissions: {}`), anchors and aliases, untrusted input outside `run:` (such as an `actions/github-script` script), and `uses:` text inside `run:` blocks are not covered. The full list is in the script's docstring. Run both checks locally with `actionlint` and `python3 -I .github/scripts/check-workflow-rules.py` from the repo root. The audit of each existing workflow against these rules is in [docs/plans/08-workflow-safety.md](plans/08-workflow-safety.md).

Repo settings for fork PRs are the owner's to apply: see [docs/github-settings.md](github-settings.md).

## Dependency updates (Dependabot)

`.github/dependabot.yml` opens a weekly PR (Mondays, UTC) for each ecosystem that exists today. Nothing is auto-merged: every update is a normal PR, reviewed, and it must pass the `ci` gate like any other.

| Ecosystem | Directory | What it updates | Open PR limit |
| --- | --- | --- | --- |
| `github-actions` | `/` | `uses:` lines in `.github/workflows/` | 5 |
| `docker` | `/.devcontainer` | The `FROM` image in `.devcontainer/Dockerfile` | 3 |

**New ecosystem rule.** A PR that introduces a new package ecosystem (Cargo with PR 1 or PR 6, the registry app's package manager with PR 2, and so on) adds that ecosystem's entry to `.github/dependabot.yml` in the same PR, with the same weekly schedule and an open-PR limit. Otherwise its dependencies go stale unnoticed. Reviewers check for it. The same applies to anything else the two entries above do not read, none of which exists today: local composite actions under `.github/actions/` (the `github-actions` entry at `/` does not cover them), a compose file with an `image:` line (the `docker` ecosystem does not read compose files; add a `docker-compose` entry), and devcontainer features or images in `devcontainer.json` (add a `devcontainers` entry). A PR adding any of these adds the matching entry in the same PR.

**How it fits this repo's conventions**

- Actions are pinned by full commit SHA with a trailing `# vX.Y.Z` comment (workflow safety rule 6). Dependabot reads that form and updates the SHA and the comment together, as long as the comment is on the same line. Keep that format or updates stop applying cleanly.
- Dependabot PRs run the `ci` workflow as fork-like runs: a read-only `GITHUB_TOKEN` and no Actions secrets (GitHub's docs: [troubleshooting Dependabot on GitHub Actions](https://docs.github.com/en/code-security/dependabot/troubleshooting-dependabot/troubleshooting-dependabot-on-github-actions), checked 2026-10-14). No workflow here uses a secret (rule 3; the `workflows` job fails on any `secrets` reference) and the top-level permission is `contents: read`, so they run unchanged. This holds for every Dependabot-triggered `push`, `pull_request` and `schedule` run, not just PRs. Dependabot secrets are a separate store that this repo does not use, so nothing here should reach for `secrets.` in a Dependabot-triggered job.
- The `docker` entry only covers the `FROM` line. Dependabot does not touch the version and SHA-256 pins in `ARG` and `ENV` lines, or in workflow `env:`. These stay **manually maintained**, so nobody should assume they are covered:
  - `GH_VERSION` and its checksums, `GITLEAKS_VERSION` and its checksums, `LEFTHOOK_VERSION` and its checksums, `BATS_VERSION` and `BATS_SHA256`, in `.devcontainer/Dockerfile`.
  - `CST_VERSION` and `CST_SHA256` in the `image` job of `.github/workflows/devcontainer-ci.yml`.
  - `ACTIONLINT_VERSION` and `ACTIONLINT_SHA256` in the `workflows` job of the same file.
  - Packages installed from the Ubuntu archive follow the base image and are not pinned.

  When bumping one by hand, take the new SHA-256 from the project's published checksums (workflow safety rule 6). There is no automation for this yet.
- A base image bump can propose a tag the container does not suit (for example a non-LTS Ubuntu). A base-image bump is proposed by Dependabot but judged by a human: review it, and close it if it is not wanted. If the same unwanted tag would be proposed every week, add an `ignore` entry for it to the `docker` entry in `dependabot.yml`, with a comment giving the reason.

## CodeQL

`.github/workflows/codeql.yml` runs CodeQL on pull requests to `main`, on pushes to `main`, and weekly (Wednesdays, 03:17 UTC). It analyses what exists today: the workflows in `.github/workflows/`, CodeQL language `actions` (checked against GitHub's [advanced setup docs](https://docs.github.com/en/code-security/code-scanning/creating-an-advanced-setup-for-code-scanning/customizing-your-advanced-setup-for-code-scanning) on 2026-10-14).

- **Permissions.** Top-level `contents: read` like every workflow. The one `analyze` job adds `security-events: write`, which CodeQL needs to upload results, with the reason in a comment. `contents: read` is restated on the job because job-level permissions replace the top-level block. GitHub's template also lists `actions: read` and `packages: read`; the first is for private repos and the second for private CodeQL packs, and neither applies here.
- **Schedule trigger.** `schedule` is the one trigger here beyond `push` and `pull_request`. It is allowed: it runs only the workflow file on the default branch, never fork code, and the check script does not restrict it. GitHub disables scheduled workflows in a public repo after 60 days without repo activity, so a quiet repo can stop getting the weekly run until someone re-enables it.
- **Not a gate yet.** CodeQL is not a required check and is not in the `ci` job's `needs:`. Decide that once there is real code and a sense of its noise.
- **Dependabot** already covers the new action pins: the `github-actions` entry at `/` reads `github/codeql-action/init` and `analyze` like any other `uses:` line.

**New language rule.** A PR that introduces a language (Rust with PR 1 or PR 6, the registry app's language with PR 2, and so on) adds it to the `matrix` in `codeql.yml` in the same PR, using CodeQL's language identifier from the docs above (for example `rust`) and the `build-mode` that language needs. Otherwise the new code is never analysed. Reviewers check for it. Do not add a language before its code exists.

## What goes in the repo

The repo is public and history is permanent, so what is committed is a deliberate choice (plan 12). The rule:

- **Yes:** architecture docs, engineering docs, plans, conventions, runbooks and settings guides for the repo itself.
- **No:** pricing, positioning, go-to-market or other commercial strategy, named customers or competitors, and personal data about people. Anything commercially sensitive, including drafts of it and notes that quote it.
- **Unsure:** leave it out and ask the owner. Adding a document later is easy; removing one from history is not.

A PR that adds or changes a doc is read against this rule. Mentioning that pricing and positioning stay out (as here) is fine; the content itself is not. `docs/README.md` says what lives where.

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
- **Commit messages.** A short imperative summary line (for example "Add gitleaks pre-commit hook"), then a body only if the why is not obvious. Commits made by an agent end with a `Co-Authored-By:` line naming the agent. Dependabot-authored titles are exempt: they use the prefixes set in `dependabot.yml` (`ci: Bump ...`, `devcontainer: Bump ...`).
- **PR description.** Say what the PR does, link its plan, state the checklist status (each item ticked, or unticked with the reason), and state the review depth that applies (close read or skim).
- **Plan status.** Set the plan's `Status` to `in progress` while the PR is open, and to `merged` with the PR link at the end (see `AGENTS.md`).
- **Never bypass the hook** with `--no-verify`.
- **Public text stays clean.** Commit messages, PR text and docs are public. Follow the rule in `CLAUDE.md` about security concerns.
- **Pricing and positioning stay out of the repo.** See [What goes in the repo](#what-goes-in-the-repo).
