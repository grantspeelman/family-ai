# 12 — Which docs are committed

**Status:** planned

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

## Acceptance checklist

- [ ] Each existing project document has a recorded decision (committed, trimmed, or excluded) in the PR description.
- [ ] A search of the committed docs for pricing and positioning content returns nothing.
- [ ] The rule is written in the conventions document and linked from `docs/README.md`.
- [ ] `git log -p` over `docs/` shows no document that was later removed for sensitivity. If one exists, it is raised in the PR.
- [ ] The owner has read the final list of committed docs and confirmed it.

## Review depth

**Close read.** Publishing is hard to undo, so every document going into a public repo needs reading.
