# 01 — Separate privileged Docker work

**Status:** planned

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
- [ ] Launcher tests cover agent shortcuts, arbitrary commands, and rejected
  privileged-mode options without contacting a real Docker daemon.
- [ ] The replacement route for Docker-dependent checks is documented and a
  representative check succeeds through it.
- [ ] Container smoke tests no longer require privileged agent runtime access;
  any privileged CI-only test has a separately justified boundary.
- [ ] Existing image and relevant launcher/runtime checks pass, with any CI
  changes included in the required `ci` gate.

## Review depth

**Close read.** Review effective privileges, mounts, alternate entry points, and
the separation between the agent and Docker execution authority.
