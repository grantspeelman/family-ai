# Conventions

Standing rules for every PR in this build. `AGENTS.md` is the entry point and covers the mechanics (container, checks, per-PR steps). This document covers the principles behind them. Where something is not decided yet, it says so.

"PR N" means the Nth PR in the build sequence in `first-build-pr-sequence.md`.

## What is enforced today

The pre-commit hook runs gitleaks on staged changes, cargo fmt and clippy on the Rust workspace when Rust or Cargo files are staged, actionlint plus zizmor on staged workflow files, and cargo-deny when a manifest or lockfile changes. CI scans the full history for secrets, checks all workflows and runs cargo fmt, clippy and cargo-deny on the whole workspace on every PR and push to main. CI also measures the workspace's line coverage and fails below a generous floor, reports smaller Rust duplicates, rejects large duplicated blocks, and rejects Rust functions above a loose cognitive complexity ceiling. The hook also carries placeholder application checks (Rust tests, duplication and complexity) that check nothing until their tools are installed.

Coverage gap: Rust (`*.rs`, `Cargo.toml`, `Cargo.lock`) and workflow files have real lint checks in the hook; Rust tests, duplication and complexity are still placeholders until the later PR 2 jobs and PR 7 install them. Other files are checked by the secret scan only.

| Rule | Enforced by | Status |
| --- | --- | --- |
| No secrets in commits | gitleaks pre-commit hook | Enforced today |
| Rust lint: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pre-commit hook (`2-lint`, when a `*.rs`, `Cargo.toml` or `Cargo.lock` file is staged) and CI (`image` job, steps "cargo fmt" and "cargo clippy", inside the `ci` gate) | Enforced today on the whole Cargo workspace, so a crate added to `members` is covered with no change. CI runs both commands in the dev container image the `image` job builds, which has Rust 1.93 with clippy and rustfmt. Default lints and formatting, no `rustfmt.toml`. The root `clippy.toml` sets only the complexity threshold, which this lint does not use because `cognitive_complexity` is allowed by default |
| Tests, duplication and complexity on changed files | Pre-commit hook | Hook wiring in place; steps are placeholders until the later PR 2 jobs and PR 7 |
| Merge gate: one required `ci` check, green only if every CI job is green | CI (`ci` job in `devcontainer-ci.yml`) | Workflow in place. It covers the container checks, the workflow safety lint, the full-history secret scan, the Rust workspace lint, the Rust workspace tests (registry and family app), the registry image, workspace dependency checks, workspace line coverage, Rust duplication and Rust function complexity. Requiring `ci` in branch protection is the owner's action: [docs/github-settings.md](github-settings.md) |
| Workflow safety rules (below) | Pre-commit hook and CI (`workflows` job): actionlint and zizmor | Enforced for staged workflows locally and all workflows in CI. Automated coverage and remaining reviewer checks are listed below. The fork-PR approval setting is the owner's action: [docs/github-settings.md](github-settings.md) |
| Bash script tests (`*.bats`) | CI (`image` job, step "Bats tests", inside the `ci` gate) | Workflow in place. Finds every tracked `*.bats` file with `git ls-files` and runs them in the built image; fails if none are found. Not run by the pre-commit hook |
| Rust workspace tests (registry and family app) and registry image | CI (`registry` job, inside the `ci` gate) | Workflow in place. Runs `cargo test --locked` for the whole workspace (today `registry` and `family-app`, so a crate added to `members` is tested with no change) in the Dockerfile's Rust `toolchain` stage, builds `registry/Dockerfile`, starts the image and queries `/healthz` and `/graphql`. Not run by the pre-commit hook |
| Dependabot version updates | `.github/dependabot.yml` | Config in place for GitHub Actions, Cargo, the dev container's `FROM` image and the registry app's `FROM` images. Alerts and security updates are repo settings, the owner's action: [docs/github-settings.md](github-settings.md) |
| CodeQL analysis | GitHub code scanning, default setup (a repo setting, no workflow file) | On, analysing `actions` and `python` on PRs, pushes to `main` and weekly. Not a required check and not in the `ci` gate's `needs:`. Owner's setting: [docs/github-settings.md](github-settings.md#code-scanning-codeql) |
| Workspace dependencies: licenses, advisories, bans and sources (`cargo-deny`) | CI (`cargo-deny` job, inside the `ci` gate), and the hook when a manifest or lockfile changes | Enforced for every workspace member, including future crates, by root `deny.toml` |
| Workspace line coverage at or above a floor (`cargo-llvm-cov`) | CI (`coverage` job, inside the `ci` gate) | Enforced today. Runs `cargo llvm-cov --workspace --locked` in the registry Dockerfile's Rust `toolchain` stage, so a crate added to `members` is measured with no change. The job log and step summary show the total. The floor is `COVERAGE_FLOOR` in the job's `env:`, deliberately generous (5%; `main` measured 19.61% when it was set) so it catches a collapse, not drift; tighten it later with real data. Not run by the pre-commit hook |
| Rust duplication (`PMD CPD`) | CI (`duplication` job, inside the `ci` gate) | Enforced today for every tracked `*.rs` file, including future crates. The job log reports blocks of at least 100 tokens without failing and rejects blocks of at least 300 tokens. These generous initial thresholds expose smaller clones while blocking substantial copy-paste; ordinary Rust/test boilerplate should not block work. Both thresholds live in the job's `env:`; tighten them later with real data. Lexing and input errors fail even the report pass. Not run by the pre-commit hook |
| Rust function complexity (clippy `cognitive_complexity`) | CI (`complexity` job, inside the `ci` gate) | Enforced today. Runs `cargo clippy --workspace --all-targets --locked -- -A clippy::all -D clippy::cognitive_complexity` in the registry Dockerfile's Rust `toolchain` stage, so a crate added to `members` is checked with no change. Only this lint is enabled, so the job passes or fails on complexity alone. The threshold is `cognitive-complexity-threshold` in the root `clippy.toml`, deliberately loose (30, above clippy's default of 25) so it catches badly tangled functions, not normal growth; tighten it later with real code. Not run by the pre-commit hook |

## Pre-commit hook: what passing means, and adding a language

The hook is configured in `lefthook.yml`. It is a fast local check on the files in the commit. CI is the real gate: the `ci` job is the single required check and is green only if every CI job is. CI runs every test and lint job that exists; today that is the container checks, the workflow safety lint, the full-history secret scan, the Rust workspace lint, the Rust workspace tests (registry and family app), the registry image, workspace dependency checks, workspace line coverage, Rust duplication and Rust function complexity.

**Passing** means every step that applies to the staged files exits zero. A step applies when at least one staged file matches its glob; otherwise it is skipped. Any non-zero exit blocks the commit. Placeholder steps exit zero but are not checks run: they print `PLACEHOLDER` and what they would have received, and lefthook's summary still shows them as done, so read the output, not the tick. A hook pass is not evidence that tests, duplication or complexity were checked until the real steps replace the placeholders. The dependency checks use the real cargo-deny step. The Rust lint step is real: it checks the whole workspace as it is on disk, so unstaged edits to Rust files count too.

Steps run in order and stop at the first failure. File-based steps get only the staged files matching their glob through `{staged_files}`. Rust lint and cargo-deny check the whole workspace when their glob matches. `{staged_files}` covers added, copied, modified and renamed files; deleted files are not passed.

To add a language's checks (PR 2), edit the matching step in `lefthook.yml`:

1. Set the step's `glob` to the language's files (for example `"*.rs"`), if the placeholder's glob is not right.
2. Replace `.lefthook/placeholder.sh ...` in `run` with the real command, passing `{staged_files}` when the tool accepts file arguments. Tools that must see the whole project (a test run, say) can ignore the file list but should still be scoped by the glob so they only run when that language changed.
3. Keep the step fast; the hook only helps if it is not skipped. Anything slow belongs in CI.
4. Delete the `# PLACEHOLDER` comment above the step, and update the table above and the "Running the checks" section of `AGENTS.md`.

For `cargo-deny`, the step's glob matches root and nested `Cargo.toml` and `Cargo.lock` files. It runs `cargo deny --locked check` against the workspace rather than taking individual filenames. Run that command from the repo root to check licenses, RustSec advisories (including unmaintained and yanked crates), bans and sources. `.lefthook/cargo-deny.bats` tests hook selection and commit rejection; `.lefthook/cargo-deny-policy.bats` tests the license allowlist against local fixture crates.

## Testing bash scripts

Bash scripts are tested with [bats-core](https://github.com/bats-core/bats-core), installed in the dev container. Tests live in `*.bats` files next to the script they cover and run with `bats <file>`. CI runs every tracked `*.bats` file automatically (the `image` job finds them with `git ls-files` and runs them in the built image, so the same pinned bats-core), so a new test needs no workflow change. The file must be committed (tracked) to be found, and CI fails if it finds none.

Covered today: `.devcontainer/start.sh`, `postStartCommand.sh`, `entrypoint.sh`, the token-file check, and `.lefthook/placeholder.sh`. `.lefthook/workflows.bats` and `.lefthook/rust-lint.bats` test the hook's workflow and Rust lint steps with real commits in disposable repos. Tests shim external commands (`docker`, `ssh-agent` and so on) and use temp dirs, never the real `HOME`. `.devcontainer/smoke-test.sh` is the deliberate exception: it builds and starts a real container, so it needs Docker and a network and CI runs it as its own step.

## Workflow safety

Anyone can open a PR against this public repo, so no workflow may run untrusted code with secrets or write access available. Every workflow in `.github/workflows/` follows these rules. A new or changed workflow is checked against them in review, and the `workflows` CI job checks the mechanical ones.

1. **Read-only by default.** Set `permissions: contents: read` at the top of the workflow. Anything broader is set on the one job that needs it, with a comment giving the reason.
2. **No `pull_request_target` or `workflow_run`** that checks out or runs fork code. zizmor checks dangerous trigger patterns; a reviewer still checks that no fork code runs with elevated access. Today no workflow uses either; use plain `pull_request`, which gives fork PRs a read-only token and no secrets.
3. **Fork PRs get no secrets, so workflows must not need any.** No `secrets.` reference in a workflow unless a reviewer has agreed it is safe and the reason is written next to it.
4. **No real Fly token or API key in GitHub Actions.** CI runs against the Fly mock.
5. **Untrusted input never goes straight into `run:`.** PR titles, branch names, issue text and anything else under a contributor's control (`github.head_ref`, `github.event.*`) is passed through `env:` and read as a shell variable. No `${{ }}` expression inside a `run:` script.
6. **Third-party actions are pinned by full commit SHA**, with the version in a trailing comment (`uses: owner/action@<40 hex> # v1.2.3`). Tags and branches move; a SHA does not. Downloaded binaries are pinned by version and verified against a SHA-256 taken from the project's published checksums.
7. **`persist-credentials: false` on checkout** unless the job must push.
8. **Every job has `timeout-minutes`.**

The `workflows` job runs actionlint for syntax, expressions and shellcheck checks, then `zizmor --offline --persona=pedantic .github/workflows/` for YAML-aware security analysis. The pre-commit hook uses the same tools and audit options, passing only staged workflow filenames. Both `.yml` and `.yaml` are covered. CI remains the merge gate through `ci`'s `needs:`; local hooks give earlier feedback.

zizmor checks action SHA pins (including first-party actions), checkout credential persistence, excessive permissions, dangerous triggers and template injection, including input outside shell `run:` blocks. The pedantic persona also checks missing version comments. YAML structure is parsed rather than matched as lines. Both tools are pinned by version and SHA-256 in CI and the dev container. Offline auditing needs no token or network and skips online-only checks such as whether an action is archived or a pinned commit belongs to its upstream repository. No audits are ignored today; any future ignore needs a written reason.

Reviewers still check the exact read-only permission default and reasons for broader per-job permissions, the absence of unnecessary secrets and plain API keys, all expressions in `run:` (the convention bans even trusted expressions), job timeouts, action version comments and downloaded binary checksums. zizmor does not enforce every repository-specific rule or prove that a workflow is safe.

Run `actionlint` and `zizmor --offline --persona=pedantic .github/workflows/` from the repo root. The tools are installed by a rebuilt dev container. `bats .lefthook/workflows.bats` tests lint failures and actual commit rejection in disposable repositories, including unrelated-file skips.

Repo settings for fork PRs are the owner's to apply: see [docs/github-settings.md](github-settings.md).

## Dependency updates (Dependabot)

`.github/dependabot.yml` opens a weekly PR (Mondays, UTC) for each ecosystem that exists today. Nothing is auto-merged: every update is a normal PR, reviewed, and it must pass the `ci` gate like any other.

| Ecosystem | Directory | What it updates | Open PR limit |
| --- | --- | --- | --- |
| `github-actions` | `/` | `uses:` lines in `.github/workflows/` | 5 |
| `docker` | `/.devcontainer` | The `FROM` image in `.devcontainer/Dockerfile` | 3 |
| `docker` | `/registry` | The `FROM` images in `registry/Dockerfile`, except the Rust image (see below) | 3 |
| `cargo` | `/` | Crates in the root `Cargo.toml` workspace and `Cargo.lock` | 5 |

**New ecosystem rule.** A PR that introduces a new package ecosystem (a front end's package manager, for example) adds that ecosystem's entry to `.github/dependabot.yml` in the same PR, with the same weekly schedule and an open-PR limit. Otherwise its dependencies go stale unnoticed. Reviewers check for it. The same applies to anything else the entries above do not read, none of which exists today: a Dockerfile in a directory other than `/.devcontainer` and `/registry` (each directory needs its own `docker` entry), local composite actions under `.github/actions/` (the `github-actions` entry at `/` does not cover them), a compose file with an `image:` line (the `docker` ecosystem does not read compose files; add a `docker-compose` entry), and devcontainer features or images in `devcontainer.json` (add a `devcontainers` entry). A PR adding any of these adds the matching entry in the same PR.

**How it fits this repo's conventions**

- Actions are pinned by full commit SHA with a trailing `# vX.Y.Z` comment (workflow safety rule 6). Dependabot reads that form and updates the SHA and the comment together, as long as the comment is on the same line. Keep that format or updates stop applying cleanly.
- Dependabot PRs run the `ci` workflow as fork-like runs: a read-only `GITHUB_TOKEN` and no Actions secrets (GitHub's docs: [troubleshooting Dependabot on GitHub Actions](https://docs.github.com/en/code-security/dependabot/troubleshooting-dependabot/troubleshooting-dependabot-on-github-actions), checked 2026-10-14). No workflow here uses a secret (rule 3; reviewers check references and their reasons) and the top-level permission is `contents: read`, so they run unchanged. This holds for every Dependabot-triggered `push`, `pull_request` and `schedule` run, not just PRs. Dependabot secrets are a separate store that this repo does not use, so nothing here should reach for `secrets.` in a Dependabot-triggered job.
- The `docker` entries only cover the `FROM` lines. Dependabot does not touch the version and SHA-256 pins in `ARG` and `ENV` lines, or in workflow `env:`. These stay **manually maintained**, so nobody should assume they are covered:
  - `GH_VERSION` and its checksums, `GITLEAKS_VERSION` and its checksums, `LEFTHOOK_VERSION` and its checksums, `BATS_VERSION` and `BATS_SHA256`, plus `ACTIONLINT_VERSION`, `ZIZMOR_VERSION` and `CARGO_DENY_VERSION` and their architecture-specific checksums, plus architecture-independent `PMD_VERSION` and `PMD_SHA256`, in `.devcontainer/Dockerfile`. Keep the PMD pin in sync with CI.
  - `CST_VERSION` and `CST_SHA256` in the `image` job of `.github/workflows/devcontainer-ci.yml`.
  - `ACTIONLINT_VERSION`, `ACTIONLINT_SHA256`, `ZIZMOR_VERSION` and `ZIZMOR_SHA256` in the `workflows` job of the same file.
  - `CARGO_DENY_VERSION` and `CARGO_DENY_SHA256` in the `cargo-deny` job of the same file. The 0.20.2 amd64 and arm64 pins come from the [published release checksums](https://github.com/EmbarkStudios/cargo-deny/releases/tag/0.20.2).
  - `CARGO_LLVM_COV_VERSION` and `CARGO_LLVM_COV_SHA256` in the `coverage` job of the same file. The 0.9.1 x86_64 musl pin is the asset's SHA-256 digest shown on the [release page](https://github.com/taiki-e/cargo-llvm-cov/releases/tag/v0.9.1); the project publishes no separate checksum file.
  - `PMD_VERSION` and `PMD_SHA256` in the `duplication` job of the same file. The 7.28.0 ZIP pin is the asset's SHA-256 digest on the [published release page](https://github.com/pmd/pmd/releases/tag/pmd_releases%2F7.28.0). Keep these equal to the Dockerfile's PMD pin. CI uses the Ubuntu runner's Java runtime; local `pmd` uses `default-jre-headless` from the dev container's Ubuntu archive. No package manifest is added.
  - Packages installed from the Ubuntu archive follow the base image and are not pinned.

  When bumping one by hand, take the new SHA-256 from the project's published checksums (workflow safety rule 6). There is no automation for this yet.
- The registry image stays on the dev container's Rust (1.93 from the Ubuntu archive) and the crate's `rust-version`, so the `/registry` entry ignores major and minor updates of the `rust` image. The tag `1.93-slim-trixie` floats across patch releases, which arrive on rebuild, so Dependabot proposes no Rust image bumps. The Debian runtime image still gets PRs. Bump the dev container's Rust, `rust-version` and the `rust` tag in `registry/Dockerfile` together, by hand.
- A base image bump can propose a tag the container does not suit (for example a non-LTS Ubuntu). A base-image bump is proposed by Dependabot but judged by a human: review it, and close it if it is not wanted. If the same unwanted tag would be proposed every week, add an `ignore` entry for it to the `docker` entry in `dependabot.yml`, with a comment giving the reason.

## CodeQL

CodeQL runs through GitHub's **default setup**, a repo setting, not a workflow in `.github/workflows/`. GitHub's [setup types page](https://docs.github.com/en/code-security/concepts/code-scanning/setup-types) (checked 2026-10-17) says it scans:

- on each push to the default branch or any protected branch;
- on pull requests against the default branch, except pull requests from forks;
- weekly, as long as a push or pull request has triggered a scan in the last 180 days.

It picks its own languages. Actions analysis covers workflows; Python analysis covers tracked Python code. The detected languages can change as files are added or removed. GitHub's permissions and action versions apply, not ours, so the [workflow safety](#workflow-safety) rules do not cover it.

- **Keep Actions analysis alongside zizmor.** zizmor gives fast local feedback and gates workflow changes in CI. CodeQL remains a complementary analysis with its own queries and code-scanning results; the overlap in injection checks does not require removing either tool.
- **Not a gate yet.** CodeQL is not a required check and is not in the `ci` job's `needs:`. Decide that once there is real code and a sense of its noise.
- **When to switch to a workflow (advanced setup).** Switch when we need something default setup cannot give: custom queries, a real build for a compiled language, or a CodeQL check we control and can make required. That PR adds the workflow under the workflow safety rules and switches default setup off first, because GitHub rejects advanced-setup uploads while default setup is on.

**New language rule.** GitHub says default setup adds a new CodeQL-supported language automatically once its code is in the repo (Rust is supported). A PR that introduces a language (Rust with PR 1, a front end's language later, and so on) still checks this. After it merges, `main` should show a new `Analyze (<language>)` check, and the language should be listed under **CodeQL analysis**, then **View CodeQL configuration**, in the repo settings. If it is missing, the owner adds it under **Edit**, then **Languages**. Reviewers check the PR says this was confirmed.

## What goes in the repo

The repo is public and history is permanent, so what is committed is a deliberate choice. The rule:

- **Yes:** architecture docs, engineering docs, conventions, runbooks and settings guides for the repo itself.
- **No:** pricing, positioning, go-to-market or other commercial strategy, named customers or competitors, and personal data about people. Anything commercially sensitive, including drafts of it and notes that quote it.
- **Unsure:** leave it out and ask the owner. Adding a document later is easy; removing one from history is not.

A PR that adds or changes a doc is read against this rule. Mentioning that pricing and positioning stay out (as here) is fine; the content itself is not. `docs/README.md` says what lives where.

## Working principles

These apply to every PR, not just the foundation ones.

1. **Start from a written spec.** Each PR starts from a short spec and checklist in its description. The agent restates the goal, scope and checklist and confirms it understands before touching code.
2. **Decide review depth per PR, and size the PR by it.** Each PR says whether it needs a close read or a skim (see sizing below).
3. **The agent runs the tests itself and shows them passing** before marking a PR done. The pre-commit hook backs this up on changed files; it does not replace running the tests, and CI is the real gate.
4. **A PR that adds a test or lint job adds it to `ci`'s `needs:` in the same PR.** Otherwise the job runs but does not gate merging.
5. **Keep PRs small enough for a quick loop.** Rust compile times are slow, so large PRs make the test-fix-retest loop painful for the agent.

## Sizing rule

Size a PR by how much human attention it needs, not by a fixed line count.

- Boilerplate-heavy, low-judgement work (scaffolding, generated tooling config) may be larger. It is skimmed.
- Work with real judgement calls stays small and is read line by line: API contracts, data handling, security-adjacent code.
- This rule applies to every PR. The flagged bundles (PR 2, tooling, and PR 4, the Fly mock) were broken down into factory jobs; see `first-build-pr-sequence.md`.

## Licensing

The `LICENSE` file is AGPL-3.0 and committed. `first-build-pr-sequence.md` still calls AGPL-3.0 a leaning, not final. Treat it as in place but not confirmed.

**Dependency license policy.** An agent will add whatever dependency makes the tests pass, so the check is mechanical, not remembered.

Allowed:

- Permissive licenses: MIT, Apache-2.0, BSD, ISC, and public domain (such as SQLite).

Disallowed:

- GPL-2.0-only
- Proprietary libraries
- Source-available licenses with usage restrictions: SSPL, BUSL (Business Source), Elastic, FSL, O'Saasy. `BSL-1.0` means the permissive Boost license, not Business Source; it is not currently needed or allowed by `deny.toml`

Licenses on neither list (LGPL, MPL, GPL-3.0 and so on): not yet decided. Ask the user before adding such a dependency.

Scope: dependencies linked into shipped code. Dev tooling (linters, coverage, CI) is not linked into shipped code and is out of scope.

Enforced checks:

- Root `deny.toml` explicitly allows permissive SPDX licenses, including `Unicode-3.0`, `Zlib` and `Apache-2.0 WITH LLVM-exception`. All other licenses fail. Only unpublished workspace crates are exempted for their own AGPL license; AGPL is not allowed for third-party dependencies.
- `cargo deny --locked check` covers every workspace crate in CI and in the pre-commit hook when a manifest or lockfile changes. RustSec vulnerabilities, unmaintained or unsound advisories, and yanked crates fail; any future ignored advisory needs a written reason. Duplicate transitive versions warn because their version requirements are controlled upstream; wildcard dependencies and unknown registries or git sources fail.

Open items:

- Whether to require a CLA (or similar) before accepting outside contributions. Undecided.
- The license check for the front end's dependencies. It waits on the front end's stack being chosen.

## Commits and PRs

- **One concept per PR.** If a PR needs "and" to describe it, split it.
- **Stacked branches.** Foundation PRs are stacked, one on the previous, on branches named `pr0/NN-slug`, numbered in stack order.
- **Commit messages.** A short imperative summary line (for example "Add gitleaks pre-commit hook"), then a body only if the why is not obvious. Commits made by an agent end with a `Co-Authored-By:` line naming the agent. Dependabot-authored titles are exempt: they use the prefixes set in `dependabot.yml` (`ci: Bump ...`, `devcontainer: Bump ...`, `registry: Bump ...`, `cargo: Bump ...`).
- **PR description.** Say what the PR does, state the checklist status (each item ticked, or unticked with the reason), and state the review depth that applies (close read or skim).
- **Never bypass the hook** with `--no-verify`.
- **Public text stays clean.** Commit messages, PR text and docs are public. Follow the public-repo rule in `AGENTS.md` about security concerns.
- **Pricing and positioning stay out of the repo.** See [What goes in the repo](#what-goes-in-the-repo).
