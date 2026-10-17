# 07 — Verify agent sandbox enforcement

**Status:** in progress

## Goal

Run Claude Code and Codex inside tested filesystem and network restrictions, with
routine development allowed within those boundaries and no silent unrestricted
fallback. Keep the outer container controls authoritative.

## Builds on

- [06 — Reduce credential exposure and persistence](06-credentials-and-persistence.md).

## In scope

- Replace blanket permission-bypass defaults with an explicit supported policy
  for each agent. Distinguish tool permissions from OS-enforced sandboxing.
- Verify current upstream configuration semantics and effective settings for the
  installed CLI versions; include the IDE entry points where supported.
- Define allowed source/cache writes and prohibited sensitive reads/writes, and
  align inner network restrictions with the externally enforced policy from 03.
- Prevent an unrestricted fallback when the inner sandbox cannot start. Any
  exception needs a documented reason and equivalent outer enforcement.
- Keep mandatory policy outside agent-writable settings where enforcement requires
  it. Evaluate native read/edit tools and plugins/MCP tools separately from shell
  subprocess restrictions; do not imply a Bash sandbox covers every tool.
- Preserve ordinary development without adding approval prompts for every action.

## Out of scope

- Treating model obedience, approval prompts, or secret scanning as the sole
  enforcement boundary.
- Claiming complete prompt-injection prevention or enabling new external tools.
- Weakening the outer capability/network controls solely to avoid setup errors.

## Acceptance checklist

- [ ] Each agent has a documented permission and sandbox policy, with effective
  settings checked against its installed version and upstream documentation.
- [ ] Harmless fixture commands demonstrate denied sensitive reads, writes outside
  allowed paths, and unexpected network destinations for both agents.
- [x] Relevant native file tools and enabled external tools are evaluated, with
  any enforcement differences explicitly documented and scoped.
- [x] Sandbox initialization failure does not execute the requested command in
  an unrestricted fallback; a negative test demonstrates the behavior.
- [x] Agent-writable configuration changes cannot remove mandatory outer controls.
- [ ] Representative edit/build/test/commit work succeeds in terminal and supported
  IDE sessions, with manual authenticated checks distinguished from CI tests.
- [x] Relevant tests replace assertions that merely require permission bypass;
  required CI jobs include the new focused checks.

## Review depth

**Close read.** Review actual enforcement across shell and non-shell tools,
configuration precedence, sandbox failures, and any compatibility exceptions.

## Implementation notes

This plan uses the explicit equivalent-outer-enforcement exception: both CLIs
run inside a read-only, non-root container with disposable writable development
paths and externally enforced networking. Nested user namespaces are not
required and runtime capabilities are not broadened. The image entrypoint
refuses to execute its command when mandatory OS boundaries are absent.
System agent policies replace Claude's forced bypass mode and disable newly
provisioned external integrations. Native file tools and local tool subprocesses
share the outer boundary; remote tool effects and the selected agent's own login
are not claimed to be isolated from that agent.

Focused policy and effective Compose tests pass locally. Actual container probes
are included in the existing required smoke job and await Docker-capable CI.
The CLI semantics were checked with empty provider homes against Codex 0.163.0
and Claude Code 2.1.300 and current official documentation. Authenticated terminal
and IDE edit/build/test/commit acceptance remains unrun; the related checklist
items stay open. See the dev container README for commands and exact scope.
