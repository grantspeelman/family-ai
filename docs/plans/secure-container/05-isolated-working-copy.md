# 05 — Separate source from trusted host controls

**Status:** in progress

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

- [x] Agent edits to launcher and Compose files in the disposable copy do not
  affect the trusted host launcher or effective isolation policy on restart.
- [x] Safe fixture tests show agent writes cannot alter trusted host files or
  Git hooks/configuration, including through worktree metadata paths.
- [x] Agents can edit, test, inspect diffs, and make local commits in their copy.
- [x] A reviewed change can be transferred to a trusted fixture checkout without
  importing or executing agent-controlled hooks, Git configuration, or host scripts.
- [ ] Terminal and VS Code open the intended disposable copy and maintain the
  same boundary; lifecycle operations use trusted controls.
- [x] Cleanup/reset behavior is documented and tested without deleting unreviewed
  work or unrelated checkouts.

## Review depth

**Close read.** Review mounts, Git metadata, change transfer, and every host-side
operation that consumes agent-writable files.

## Validation evidence

- `python3 -I .devcontainer/test-isolation.py` exercises independent Git state,
  local commits, reviewed data-only transfer, hostile hooks/configuration,
  worktree pointers, symlink rejection, directory replacement and existing-session
  preservation using disposable fixtures.
- `test-start.py` and `test-vscode.py` exercise installed entry points with stubbed
  Docker/editor commands. `test-runtime-config.py` checks effective Compose mounts
  and policy for terminal, editor and smoke-test configurations.
- Terminal and editor configuration open the same disposable source boundary,
  with build/lifecycle controls read from the separately installed snapshot.
  A real Docker launch and graphical editor acceptance run remain required;
  fixture tests do not establish host filesystem access controls or editor runtime behavior.
