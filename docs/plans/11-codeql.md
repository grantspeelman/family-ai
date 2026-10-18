# 11 — CodeQL

**Status:** in progress

## Goal

Static security analysis runs on the repo's code automatically, so common vulnerability patterns are flagged in PRs rather than found later.

## Builds on

- 07 — CI gate
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

Delivered through GitHub's default setup (the "or GitHub's default setup" option above), so there is no workflow file.

- [x] CodeQL runs on this PR and on `main`, and passes. Default setup ran `Analyze (actions)` and `Analyze (python)` on `main` at `5f44af8`, both passing. PRs against `main` get the same checks.
- [x] Its permissions are the minimum CodeQL documents (`security-events: write` on the analysis job only). Not applicable as written: default setup runs under GitHub's own configuration, not a workflow in the repo, so there is no permissions block for us to set.
- [x] Results appear in the repo's code scanning tab. The code scanning API lists `actions` and `python` analyses for `refs/heads/main` (2026-10-17), with no errors and no alerts.
- [x] The conventions document says a new language is added to the analysis in the PR that introduces it. Default setup adds it by itself; [docs/conventions.md](../conventions.md#codeql) has the PR confirm it did.
- [ ] Confirmed against GitHub's current terms that code scanning is free for public repos, with the date checked. Eligibility wording ("publicly visible, or GitHub Code Security is enabled") is cited with the date 2026-10-14 in docs/github-settings.md, but no page says "free" outright; left unticked for the owner. Default setup is already running on this public repo.

## Review depth

**Skim.** Mostly GitHub's own template, with the permissions line the one thing worth checking.
