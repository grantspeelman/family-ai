# 03 — Enforce network restrictions

**Status:** planned

## Goal

Limit outbound traffic and access to host, LAN, and metadata services using an
enforcement boundary that arbitrary agent commands cannot modify or bypass.

## Builds on

- [02 — Remove publishing credentials by default](02-publishing-credentials.md).

## In scope

- Inventory required runtime destinations for Claude Code, Codex, dependency
  downloads, and development services. Separate image-build needs from runtime needs.
- Choose and implement externally enforced egress, such as a separate proxy with
  direct outbound access blocked or a host-enforced firewall.
- Define narrow allowed destinations and the policy for DNS, IPv4, IPv6, redirects,
  direct IP access, and proxy bypass attempts.
- Deny unnecessary host, LAN, cloud metadata, and sibling-container access while
  preserving explicitly required development services.
- Keep policy and policy updates outside the agent's writable control. Startup
  must fail closed if enforcement is unavailable.
- Explain residual exfiltration risk through permitted destinations; destination
  filtering does not authorize every operation on an allowed service.

## Out of scope

- A claim that allowlisting prevents all data exfiltration.
- TLS interception or unrestricted request logging containing sensitive data.
- Provider credential storage (06) and agent-specific inner sandbox settings (07).

## Acceptance checklist

- [ ] A written destination policy states allowed services and required local
  development connections, with no broad wildcard justified only by convenience.
- [ ] Controlled tests demonstrate denied arbitrary outbound traffic, direct
  connections around the proxy, unnecessary private networks, and metadata access.
- [ ] Tests cover DNS and IPv6, or prove an unsupported protocol is disabled.
- [ ] The agent user cannot edit enforcement policy, stop the enforcer, or regain
  unrestricted access by changing environment variables or client settings.
- [ ] Enforcement failure prevents normal session startup or network access.
- [ ] Both agents can connect and a representative locked dependency fetch works;
  manual authenticated checks are identified separately from secret-free CI checks.
- [ ] Terminal and VS Code sessions enforce the same policy. Network tests use
  isolated fixtures rather than probing real host or LAN services.

## Review depth

**Close read.** Review the actual traffic path and bypass cases, including allowed
services that can accept uploads and the location of the enforcement authority.
