# 07 — Verify agent sandbox enforcement

**Status:** planned

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
- [ ] Relevant native file tools and enabled external tools are evaluated, with
  any enforcement differences explicitly documented and scoped.
- [ ] Sandbox initialization failure does not execute the requested command in
  an unrestricted fallback; a negative test demonstrates the behavior.
- [ ] Agent-writable configuration changes cannot remove mandatory outer controls.
- [ ] Representative edit/build/test/commit work succeeds in terminal and supported
  IDE sessions, with manual authenticated checks distinguished from CI tests.
- [ ] Relevant tests replace assertions that merely require permission bypass;
  required CI jobs include the new focused checks.

## Review depth

**Close read.** Review actual enforcement across shell and non-shell tools,
configuration precedence, sandbox failures, and any compatibility exceptions.
