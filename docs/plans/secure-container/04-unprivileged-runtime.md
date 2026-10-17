# 04 — Run without privilege escalation

**Status:** planned

## Goal

Run ordinary development as an unprivileged user who cannot become root, change
system-installed tools, or weaken the outer isolation policy.

## Builds on

- [03 — Enforce network restrictions](03-network-restrictions.md).

## In scope

- Start the normal runtime directly as the development user and remove general
  passwordless sudo and unnecessary Docker group membership.
- Set `no-new-privileges` and drop unnecessary Linux capabilities, keeping the
  default seccomp and host security profiles unless a narrow exception is justified.
- Install system tools during image builds; keep writable caches and build output
  separate from system binaries and enforced policy.
- Preserve bind-mount UID/GID ownership, first-use volume ownership, hooks, and
  supported terminal and VS Code workflows without runtime privilege escalation.
- Assess host rootless Docker or user namespace support as additional protection;
  record compatibility and avoid silently requiring host changes.
- Add reasonable CPU, memory, and process limits where compatible with builds.

## Out of scope

- Privileged Docker execution (01), host networking enforcement (03), and full
  isolation of trusted host files (05).
- Broad seccomp/AppArmor exemptions to make inner sandboxing work; test and justify
  any minimum requirement with plan 07 instead.

## Acceptance checklist

- [ ] Effective terminal and VS Code processes run as the intended non-root user.
- [ ] Harmless negative tests show `sudo -n` cannot elevate, system tools cannot
  be overwritten, and privilege-gaining execution is blocked.
- [ ] Runtime inspection verifies `no-new-privileges`, the intended capability
  set, and active seccomp/security profiles.
- [ ] Fresh config/cache volumes are usable without restoring general sudo access.
- [ ] Local hooks and representative build/test commands work with correct file
  ownership and the chosen resource limits.
- [ ] Any rootless/user-namespace option and host prerequisites are documented;
  unsupported setups fail clearly without silently relaxing isolation.

## Review depth

**Close read.** Review effective runtime identity and privilege restrictions,
including entrypoint behavior, volume initialization, and compatibility exceptions.
