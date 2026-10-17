# Secure container plans

Seven plans for limiting the consequences of prompt injection in Claude Code and
Codex sessions. Each plan is one concept and is intended to land as a separate
PR, following the format in [the plans guide](../README.md).

Assume an agent can be persuaded to execute arbitrary commands. The goal is to
limit what those commands can read, change, publish, and reach. Agent instructions,
permission prompts, secret scanning, and code review supplement enforced boundaries.

These plans describe intended safeguards, not a guarantee that the current setup
already provides them. They apply to terminal and VS Code entry points. Record
implementation choices and validation evidence in the implementing plan, without
including credential values or additional private environment findings.

## Sequence

Work through these in order. Numbering is local to this directory and is separate
from the foundation plans and the application build sequence.

| # | Plan | Result | Review |
| --- | --- | --- | --- |
| 01 | [Separate privileged Docker work](01-separate-privileged-docker.md) | Agent sessions do not receive privileged Docker access | Close read |
| 02 | [Remove publishing credentials by default](02-publishing-credentials.md) | Development sessions have no GitHub token or forwarded SSH agent | Close read |
| 03 | [Enforce network restrictions](03-network-restrictions.md) | Egress policy is enforced outside the agent's control | Close read |
| 04 | [Run without privilege escalation](04-unprivileged-runtime.md) | The normal runtime cannot become root or change system tools | Close read |
| 05 | [Separate source from trusted host controls](05-isolated-working-copy.md) | Agent edits cannot silently alter host launch controls | Close read |
| 06 | [Reduce credential exposure and persistence](06-credentials-and-persistence.md) | Only necessary agent authentication is exposed, with limited shared state | Close read |
| 07 | [Verify agent sandbox enforcement](07-agent-sandboxing.md) | Both agents operate within tested filesystem and network boundaries | Close read |

## Validation rules

- Write focused tests before implementation changes. Test denied operations as
  well as ordinary development work, using disposable fixtures and dummy secrets.
- Never test with real credentials, actual exfiltration, destructive host access,
  or a real container escape. Use a controlled local destination for network tests.
- Exercise the effective runtime, not just configuration text. Check terminal and
  VS Code behavior; distinguish automated checks from manual verification.
- New test or lint jobs must join the existing `ci` gate. Keep CI secret-free and
  follow the workflow safety conventions.
- Keep a plan `planned` until work starts, then `in progress` until its PR is
  confirmed merged. Only then set it to `merged` and add the PR link.
- Leave checks unticked when validation has not run. Planning alone does not
  establish a security boundary.

## Sources

- [Docker runtime privileges](https://docs.docker.com/engine/containers/run/#runtime-privilege-and-linux-capabilities)
- [Docker rootless mode](https://docs.docker.com/engine/security/rootless/)
- [OpenSSH agent controls](https://man.openbsd.org/ssh-add.1)
- [Docker build context exclusions](https://docs.docker.com/build/concepts/context/#dockerignore-files)
- [Claude Code sandboxing](https://code.claude.com/docs/en/sandboxing)
- [Codex sandbox and approvals](https://learn.chatgpt.com/docs/agent-approvals-security)

Recheck current upstream documentation when implementing agent-specific settings.
