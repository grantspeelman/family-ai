# 17 — Workflow linters in CI and pre-commit

**Status:** in progress

## Goal

Replace the bespoke workflow checker with zizmor and catch workflow mistakes
before committing as well as in CI. Implements [issue #22](https://github.com/grantspeelman/family-ai/issues/22).

## Builds on

- 08 — Workflow safety
- 14 — bats-core
- 15 — bats in CI

## In scope

- Keep actionlint and add a pinned, SHA-256-verified zizmor to the workflows job.
- Install both tools for amd64 and arm64 in the dev container and assert versions.
- Run both tools on staged workflow files through lefthook.
- Remove the bespoke checker and document automated coverage and reviewer checks.
- Retain CodeQL default setup alongside zizmor.

## Out of scope

- Changes to GitHub settings or CodeQL setup.
- Application tooling and placeholder Rust checks.

## Acceptance checklist

- [x] Existing workflows pass actionlint and zizmor without unexplained ignores.
- [x] An unpinned action, injected branch name and missing checkout credential
      setting each fail the CI lint command and block a real commit in a temporary
      repository; the deliberate violations are discarded afterwards.
- [x] Valid workflows pass the hook; unrelated files skip both workflow steps.
- [x] Both workflow extensions and filenames containing spaces work.
- [x] Both tools have version and checksum pins and image version tests.
- [x] The workflows job remains required by the ci job.
- [x] Documentation describes coverage, reviewer checks and the CodeQL decision.
- [ ] Built-image structure checks and live GitHub CI pass; demonstrate a deliberate
      lint violation in GitHub CI once, then revert it.

Local validation: the workflow passes actionlint (including shellcheck) and
offline pedantic zizmor without findings. Bats integration tests exercise real
commits, including a flow-style YAML violation. Both architectures' release
archives match their published SHA-256 digests. The exact Dockerfile installation
commands run successfully on amd64 and report the pinned versions. Image tests
and a live GitHub CI run remain pending.

## Review depth

**Close read.** Workflow policy and commit checks control which changes pass CI.
