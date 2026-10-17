# 02 — Remove publishing credentials by default

**Status:** planned

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
- [ ] Launcher tests prove default startup never invokes `ssh-agent` or `ssh-add`.
- [ ] Edit, test, and local commit workflows succeed without publishing credentials.
- [ ] A documented host-side publishing workflow succeeds with a disposable local
  remote fixture; no real push or real credential is needed for automated tests.
- [ ] Any optional authenticated workflow states its scope, expiry, and trust
  boundary, and cannot be enabled by an ordinary agent command inside the container.

## Review depth

**Close read.** Review every credential transport and both entry points; absence
of token environment variables alone is insufficient.
