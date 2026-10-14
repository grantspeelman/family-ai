# 03 — Secret scanning pre-commit hook

**Status:** planned

## Goal

A commit containing a secret is blocked on the developer's machine before it exists in history. The repo is public from the first commit, so history is public too.

## Builds on

- 01 — Dev container (provides `gitleaks` and `lefthook`, and installs the hook on start)

## In scope

- Land the existing `lefthook.yml` with its `gitleaks` pre-commit command on staged changes.
- A gitleaks config only if the defaults need an allowlist for known-safe values (for example, the pinned GitHub SSH host keys).

## Out of scope

- Lint, test, duplication and complexity checks (plan 06).
- GitHub-side secret scanning and push protection (plan 09).
- The CI full-history scan, which already lives in plan 02.

## Acceptance checklist

- [ ] After container start, `.git/hooks/pre-commit` exists and calls lefthook.
- [ ] Staging a file with a fake token and running `git commit` fails, with the finding redacted in the output. Shown once, then reverted.
- [ ] Staging a clean change commits normally.
- [ ] Any allowlist entry has a comment saying why it is safe.
- [ ] The README says what happens for commits made outside the container (the hook is skipped; CI is the backstop).

## Review depth

**Skim.** Small configuration, with one command and no logic.
