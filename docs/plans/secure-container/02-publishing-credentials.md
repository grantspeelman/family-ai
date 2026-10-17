# 02 — Remove publishing credentials by default

**Status:** in progress

## Goal

Let agents edit, test, and commit locally without giving ordinary development
sessions credentials that can publish changes or act as the user's SSH identity.

## Builds on

- [01 — Separate privileged Docker work](01-separate-privileged-docker.md).

## In scope

- Remove default GitHub token pass-through and SSH agent forwarding.
- Stop the launcher from starting an SSH agent or loading default host keys for
  a normal development session.
- Check VS Code's automatic credential and SSH forwarding separately from the
  terminal Compose configuration; enforce the same default for both.
- Establish a trusted host-side publishing step for reviewed local commits.
- If an explicit publishing mode is retained, use minimum repository permissions
  and short-lived credentials; keep it outside ordinary agent sessions.
- Update startup messages and credential setup examples to match the new default.

## Out of scope

- Agent provider authentication and config volume lifecycle (06).
- Egress restrictions (03).
- Changing repository branch protection or publishing changes during this work.

## Acceptance checklist

- [ ] With dummy host GitHub tokens set, the effective default container does
  not receive them through environment variables, mounts, or credential helpers.
- [ ] Default sessions cannot access a forwarded SSH agent through either entry
  point, including VS Code's automatic forwarding.
- [x] Launcher tests prove default startup never invokes `ssh-agent` or `ssh-add`.
- [ ] Edit, test, and local commit workflows succeed without publishing credentials.
- [x] A documented host-side publishing workflow succeeds with a disposable local
  remote fixture; no real push or real credential is needed for automated tests.
- [x] Any optional authenticated workflow states its scope, expiry, and trust
  boundary, and cannot be enabled by an ordinary agent command inside the container.

## Review depth

**Close read.** Review every credential transport and both entry points; absence
of token environment variables alone is insufficient.

## Validation notes

Launcher tests (7), isolated editor launch/preflight tests (3), the local Git
publishing fixture (1), and effective Compose checks with dummy host tokens (1)
pass. No authenticated container mode remains. Smoke checks now verify token,
SSH socket and credential helper absence, plus local build/test/commit behavior.
Docker runtime checks run in CI; the first, second and fourth acceptance items
remain open until those checks and a fresh graphical VS Code session verify the
actual runtime. IDE launch tests use a fake editor and cannot prove the behavior
of the Dev Containers extension itself.
