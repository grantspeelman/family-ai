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

- [ ] CodeQL runs on this PR and on `main`, and passes. Not verified: needs a real CI run. Locally, `actionlint` and the rules script pass on the workflow.
- [x] Its permissions are the minimum CodeQL documents (`security-events: write` on the analysis job only). Top level is `contents: read`; the job adds `security-events: write` with a reason comment, per the CodeQL action README ("All advanced setup code scanning workflows must have the `security-events: write` permission").
- [ ] Results appear in the repo's code scanning tab. Owner action: switch off default setup if on, then check after the first run ([docs/github-settings.md](../github-settings.md#code-scanning-codeql)).
- [x] The conventions document says a new language is added to the analysis in the PR that introduces it.
- [ ] Confirmed against GitHub's current terms that code scanning is free for public repos, with the date checked. Eligibility wording ("publicly visible, or GitHub Code Security is enabled") is cited with the date 2026-10-14 in docs/github-settings.md, but no page says "free" outright and the owner's Enable flow is untested; left unticked.

## Review depth

**Skim.** Mostly GitHub's own template, with the permissions line the one thing worth checking.
