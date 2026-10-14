# 11 — CodeQL

**Status:** planned

## Goal

Static security analysis runs on the repo's code automatically, so common vulnerability patterns are flagged in PRs rather than found later.

## Builds on

- 07 — CI full-suite gate
- 08 — Workflow safety (CodeQL runs under the same permission rules)

## In scope

- A CodeQL workflow (or GitHub's default setup) on PRs, on pushes to `main`, and on a weekly schedule.
- Analysis of what exists today: the GitHub Actions workflows.
- A documented rule for adding a language to the analysis when a stack is introduced (Rust, and whatever the registry app uses).

## Out of scope

- Rust and registry-app analysis, which arrive with PRs 1 and 6 when there is code to analyse.
- Making CodeQL a required status check. Decide that once there is real code and a sense of its noise.
- Fixing findings in code that does not exist yet.

## Acceptance checklist

- [ ] CodeQL runs on this PR and on `main`, and passes.
- [ ] Its permissions are the minimum CodeQL documents (`security-events: write` on the analysis job only).
- [ ] Results appear in the repo's code scanning tab.
- [ ] The conventions document says a new language is added to the analysis in the PR that introduces it.
- [ ] Confirmed against GitHub's current terms that code scanning is free for public repos, with the date checked.

## Review depth

**Skim.** Mostly GitHub's own template, with the permissions line the one thing worth checking.
