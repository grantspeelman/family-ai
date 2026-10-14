# 06 — Changed-files pre-commit checks

**Status:** planned

## Goal

A commit that fails lint, tests or duplication and complexity checks on the files it changes is blocked outright, quickly enough that the agent never skips the hook. CI stays the real gate; this is a fast local check, not a substitute.

## Builds on

- 03 — Secret scanning pre-commit hook (extends the same lefthook setup)
- 05 — Conventions (defines what "passing" means)

## In scope

- Extend `lefthook.yml` so checks run on **changed files only** and a failure **blocks the commit**.
- The mechanism and its wiring for lint, tests, and duplication and complexity checks. Where no tool exists yet, the step is a documented placeholder that does nothing and says so.
- A documented way to add a language's checks later, so PRs 2 and 7 plug in without reworking the hook.
- A `cargo-deny` step that runs when dependencies change, as a placeholder until the tool is installed in PR 7.

## Out of scope

- Choosing or installing the real linters, coverage, duplication and complexity tools (PRs 2 and 7).
- The full-suite run in CI (plan 07).
- Bypass conventions such as `--no-verify`. The README just says CI will still catch it.

## Acceptance checklist

- [ ] A commit touching only docs runs only the checks that apply to docs, and passes.
- [ ] A check that exits non-zero blocks the commit. Shown once with a throwaway failing check, then reverted.
- [ ] Checks receive only the changed files, not the whole tree.
- [ ] Placeholder steps print that they are placeholders; none silently pass as if they had run something.
- [ ] The README explains how to add a language's checks.
- [ ] The hook finishes in a few seconds on a small change.

## Review depth

**Close read.** A hook that silently passes would defeat the point, so how failures and placeholders behave needs reading line by line.
