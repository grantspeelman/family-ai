# 05 — Separate source from trusted host controls

**Status:** planned

## Goal

Give agents a disposable writable working copy while keeping host launch controls
and trusted Git state outside that writable boundary. Returning to a host shell
must not silently execute agent-modified configuration.

## Builds on

- [04 — Run without privilege escalation](04-unprivileged-runtime.md).

## In scope

- Choose a disposable working-copy arrangement and a trusted host launcher/policy
  location that is not mounted writable into agent sessions.
- Keep the trusted host checkout, Git metadata, hooks, credentials, and launch
  configuration inaccessible to agent writes.
- Agents may propose changes to `.devcontainer/` in their working copy, but those
  changes must not become active host launch policy before review.
- Define transfer and review of source changes/local commits without executing
  agent-provided hooks, Git configuration, or launcher scripts on the host.
- Handle Git worktrees explicitly: a worktree whose Git directory points outside
  the mounted copy must not expose trusted host Git metadata as a workaround.
- Define session reset and cleanup while preserving changes the user wants to keep.

## Out of scope

- Making agent-written source safe to execute without review.
- Publishing credentials (02), provider credential lifecycle (06), and a general
  purpose sandbox orchestration service.

## Acceptance checklist

- [ ] Agent edits to launcher and Compose files in the disposable copy do not
  affect the trusted host launcher or effective isolation policy on restart.
- [ ] Safe fixture tests show agent writes cannot alter trusted host files or
  Git hooks/configuration, including through worktree metadata paths.
- [ ] Agents can edit, test, inspect diffs, and make local commits in their copy.
- [ ] A reviewed change can be transferred to a trusted fixture checkout without
  importing or executing agent-controlled hooks, Git configuration, or host scripts.
- [ ] Terminal and VS Code open the intended disposable copy and maintain the
  same boundary; lifecycle operations use trusted controls.
- [ ] Cleanup/reset behavior is documented and tested without deleting unreviewed
  work or unrelated checkouts.

## Review depth

**Close read.** Review mounts, Git metadata, change transfer, and every host-side
operation that consumes agent-writable files.
