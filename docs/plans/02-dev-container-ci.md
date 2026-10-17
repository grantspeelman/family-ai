# 02 — Dev container CI

**Status:** in progress

## Goal

Every push to `main` and every PR proves the dev container still builds and behaves, so the toolchain the agent depends on cannot rot unnoticed.

## Builds on

- 01 — Dev container

## In scope

- Land the existing `.github/workflows/devcontainer-ci.yml` after review against this plan.
- Jobs: compose validation, image build, structure test, full-history secret scan, and the smoke test.
- Actions pinned by commit SHA, downloaded binaries pinned by checksum.

## Out of scope

- A general CI gate for the project's own test suite (plan 07).
- Fork-PR trust rules beyond what this workflow needs (plan 08).
- Dependabot updates to pinned actions (plan 10).

## Acceptance checklist

- [ ] The workflow runs on `push` to `main` and on `pull_request`, and passes on this PR.
- [x] `permissions` is `contents: read` and nothing broader.
- [x] No secrets are referenced anywhere in the workflow.
- [x] Every `uses:` is pinned to a full commit SHA with the version in a comment.
- [x] Every downloaded binary is verified against a pinned SHA-256 before it is run.
- [x] The secret scan covers full history (`fetch-depth: 0`), not just the tip.
- [ ] A deliberately failing change (for example, a wrong version in the structure test) turns the workflow red, shown once and then reverted.

## Review depth

**Close read.** CI config on a public repo is security-adjacent: permissions, pinning and what runs on pull requests all matter.
