# 12 — Which docs are committed

**Status:** in progress

## Goal

What the public repo says about the project is a deliberate choice. Architecture is shared on purpose; pricing and positioning stay out. Once the repo is public, anything committed stays in history.

## Builds on

- 05 — Conventions (the rule is recorded there)

## In scope

- A decision for each existing project document: commit it, commit a trimmed version, or keep it out of the repo. That covers `first-build-pr-sequence.md` and the architecture doc it refers to.
- A short rule, recorded in the conventions document, for what is allowed in the repo: architecture and engineering docs yes; pricing, positioning and anything commercially sensitive no.
- A `docs/README.md` that says what lives where.
- Removal of anything that fails the rule from the working tree before it is committed.

## Out of scope

- Rewriting history. If something sensitive is already in history, that is a separate, explicit decision.
- Marketing or user-facing documentation.

## Decisions

| Document | Decision | Reason |
| --- | --- | --- |
| `first-build-pr-sequence.md` | Committed (already in history since its first commit), one line updated | Build sequence, working principles and licensing notes only; no pricing, positioning, customer or competitor names, or personal data. The line on Dependabot and CodeQL was stale after plans 10 and 11 and now points to `docs/github-settings.md`. |
| Architecture doc it refers to | Not in the repo or working tree, so no decision to make | Searched the repo, all branches and the filesystem by name; nothing found. The sequence doc still mentions it; it is not committed. If it is added later it is read against the rule first. |
| `docs/` (conventions, github-settings, plans) | Committed | Engineering docs; read against the rule. |
| `AGENTS.md`, `CLAUDE.md` | Committed | Engineering and agent instructions. |

## Acceptance checklist

- [x] Each existing project document has a recorded decision (committed, trimmed, or excluded) in the PR description. (Table under Decisions below, to copy into the PR description.)
- [x] A search of the committed docs for pricing and positioning content returns nothing. (Every line of `first-build-pr-sequence.md` read; a keyword search finds only the rule itself, and GitHub's own pricing pages cited in `docs/github-settings.md`. No pricing, positioning, customer or competitor names, or personal data of ours.)
- [x] The rule is written in the conventions document and linked from `docs/README.md`.
- [x] `git log -p` over `docs/` shows no document that was later removed for sensitivity. If one exists, it is raised in the PR. (Checked all branches: no deletions under `docs/` or of `first-build-pr-sequence.md`.)
- [ ] The owner has read the final list of committed docs and confirmed it. Owner action; not done.

## Review depth

**Close read.** Publishing is hard to undo, so every document going into a public repo needs reading.
