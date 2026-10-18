# Dev container command launching

Status: in progress

## Goal

Run a chosen command and its arguments through the terminal launcher, including
Claude Code and Codex sessions, without adding a launcher flag for every option.

## Builds on

[Dev container](01-dev-container.md).

## In scope

Codex CLI support, agent launch shortcuts, command forwarding after `--`, usage
documentation, and launcher argument tests (in `.devcontainer/start.bats`).

## Out of scope

Application code and changes to VS Code entry behavior.

## Acceptance checklist

- [x] `.devcontainer/start.sh --rebuild -- claude --resume` forwards the command.
- [x] Argument boundaries, empty arguments, and literal shell text are preserved.
- [x] Claude and Codex commands retain host-side agent identification.
- [x] Existing agent shortcuts and the default Bash shell keep working.
- [x] Invalid launcher options fail before Docker is invoked.
- [x] Usage examples and the tests in `start.bats` document the behavior.

## Review depth

Close read of argument parsing and execution; skim documentation.
