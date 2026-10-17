# 06 — Reduce credential exposure and persistence

**Status:** in progress

## Goal

Expose only authentication needed by the selected agent and prevent unnecessary
configuration sharing across agents, checkouts, and trust levels. Keep unrelated
secrets out of runtime mounts and image build contexts.

## Builds on

- [05 — Separate source from trusted host controls](05-isolated-working-copy.md).

## In scope

- Mount only the selected agent's necessary state; define explicit behavior for
  a plain shell session and switching between Claude Code and Codex.
- Isolate persistent state by checkout or trust level instead of globally sharing
  all agent configuration. Inventory settings, plugins, hooks, and other executable
  state that can persist alongside logins.
- Separate disposable settings/caches from authentication where supported. State
  honestly when the agent process must retain access to its own provider credentials;
  filesystem permissions for the same user do not hide them from arbitrary commands.
- Define reset, logout, migration, and credential rotation/revocation procedures
  without assuming an image rebuild cleans named volumes.
- Keep real environment files and unrelated secrets outside the agent workspace.
- Add `.dockerignore` exclusions for secrets, Git metadata, and unnecessary build
  inputs while preserving files needed by image builds and checks.

## Out of scope

- GitHub/SSH publishing authority (02) and provider authentication proxy design
  unless supported tooling makes a narrowly scoped integration practical.
- Automatically migrating, deleting, or revoking a user's existing credentials.

## Acceptance checklist

- [x] Dummy-marker tests show Claude sessions cannot read Codex state and vice
  versa; plain shells expose only explicitly selected state.
- [x] Separate checkouts/trust levels do not share writable configuration unless
  deliberately configured and documented.
- [x] A reset removes disposable agent-controlled configuration, with the
  treatment of authentication stated and verified using fixtures.
- [x] Existing login storage has a documented migration path that preserves it
  until the user deliberately chooses migration or deletion.
- [ ] Dummy `.env`/key files outside the working copy are unavailable at runtime,
  and a build-context fixture proves excluded secrets cannot be copied by a build.
- [ ] Image builds still succeed and each agent's first login, restart, rebuild,
  and logout behavior is verified, using manual checks where authentication is needed.

## Validation evidence

Fixture-only state tests cover selected mounts, provider markers, no-follow reset,
retained dummy authentication and stop-before-reset across terminal/IDE projects.
Launcher and effective Compose tests cover terminal/editor selection. CI's
build-context fixture uses a scratch `COPY` build against ordinary and installed
control layouts, asserting only image inputs are copied; smoke recreates selected
Claude, Codex and shell profiles with dummy state. These Docker-host checks are
wired into CI and were not run in the editing environment. Real first-login,
restart/rebuild/logout and graphical editor acceptance remain manual; the final
two checklist items stay open until that evidence is available.

## Review depth

**Close read.** Review effective mounts and state lifecycle, especially persistent
executable configuration and the limits of protecting an agent's own credentials.
