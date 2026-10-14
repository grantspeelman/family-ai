# 05 — Conventions and working principles

**Status:** planned

## Goal

The baseline principles for the whole build are written down once, so every PR is held to the same standard and the standard does not live only in someone's head.

## Builds on

- 04 — Agent harness (links to this document)

## In scope

A conventions document (for example `docs/conventions.md`) capturing:

- **Working principles:** each PR starts from a short written spec the agent restates first; review depth is decided per PR and PRs are sized by it; the agent runs tests itself and shows them passing before marking a PR done; PRs stay small enough that Rust compile times keep the fix-and-retest loop quick.
- **Sizing rule:** boilerplate-heavy, low-judgement work may be larger and is skimmed; API contracts, data handling and security-adjacent code stay small and are read line by line. Bundled PRs are broken down live before starting.
- **Licensing:** AGPL-3.0, and the dependency license policy (permissive allowed; GPL-2.0-only, proprietary and source-available such as SSPL, BSL, Elastic, FSL and O'Saasy disallowed; dev tooling out of scope). The mechanical check lands in PRs 2 and 7.
- **Commit and PR conventions:** message style, PR description contents, one concept per PR.

## Out of scope

- Enforcing any of this with tooling (plans 06 and 07, and PRs 2 and 7).
- The CLA question, which stays open in `first-build-pr-sequence.md`.
- Pricing and positioning, which stay out of the repo (plan 12).

## Acceptance checklist

- [ ] The document exists and `AGENTS.md` links to it.
- [ ] Each principle in the PR 0 section of `first-build-pr-sequence.md` appears, or the PR description says why one was left out.
- [ ] The dependency license policy lists allowed and disallowed licenses explicitly.
- [ ] Nothing in it states a decision the build has not made; open items are marked as open.
- [ ] No pricing or positioning content.

## Review depth

**Close read.** These are standing rules, and the license policy is a legal-adjacent decision.
