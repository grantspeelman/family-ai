# 01 — Separate privileged Docker work

**Status:** in progress

## Goal

Keep agent development sessions outside privileged Docker environments. Docker
work must have an explicit, separate execution boundary appropriate for running
untrusted commands.

## Builds on

- The existing dev container and its launcher, structure tests, and smoke tests.

## In scope

- Remove privileged Docker access from agent launch paths, including arbitrary
  command launches and the VS Code entry point.
- Choose where Docker-dependent checks run: existing CI is the initial option;
  a separate disposable VM may be used if local checks are needed.
- Define a supported way to request and inspect those check results without
  exposing a host Docker socket or unrestricted Docker API to the agent.
- Update launcher behavior, tests, and usage documentation consistently. An
  obsolete privileged-mode flag must fail with actionable guidance.
- Distinguish native Linux host isolation from Docker Desktop's VM boundary.

## Out of scope

- Replacing Docker throughout the project.
- Building a local rootless builder unless CI cannot meet this plan's needs;
  that would need a separate scoped plan.
- Network policy (03) and removing general sudo access (04).

## Acceptance checklist

- [ ] Normal terminal and VS Code agent sessions are not privileged and expose
  no host Docker socket, host device mounts, or unrestricted Docker API.
- [x] Launcher tests cover agent shortcuts, arbitrary commands, and rejected
  privileged-mode options without contacting a real Docker daemon.
- [ ] The replacement route for Docker-dependent checks is documented and a
  representative check succeeds through it.
- [x] Container smoke tests no longer require privileged agent runtime access;
  any privileged CI-only test has a separately justified boundary.
- [ ] Existing image and relevant launcher/runtime checks pass, with any CI
  changes included in the required `ci` gate.

## Review depth

**Close read.** Review effective privileges, mounts, alternate entry points, and
the separation between the agent and Docker execution authority.

## Validation notes

- Six launcher tests pass, including rejected `--docker` options and preserved
  arbitrary command arguments.
- Effective Compose checks pass for terminal, VS Code, and smoke entry points
  without a Docker daemon.
- Shell syntax, workflow safety rules, pinned actionlint 1.7.12, and whitespace
  checks pass.
- Docker check requests and result inspection are documented in the dev
  container README; the existing required CI jobs perform image and runtime
  validation. Image builds, structure tests, and runtime smoke checks await CI;
  no Docker daemon was available in the implementation environment.
