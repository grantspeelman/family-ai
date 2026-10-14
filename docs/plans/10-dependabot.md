# 10 — Dependabot

**Status:** planned

## Goal

Dependency and action updates and security advisories arrive as PRs without anyone remembering to look, so pinned versions do not quietly go stale.

## Builds on

- 07 — CI full-suite gate (Dependabot PRs must pass the same gate)
- 08 — Workflow safety (Dependabot PRs run with restricted permissions and no secrets)

## In scope

- `.github/dependabot.yml` for the ecosystems that exist today: GitHub Actions, and the dev container's Docker base image.
- A sensible schedule and a limit on open PRs, so updates do not flood the review queue.
- A documented rule for adding an ecosystem when a stack is introduced (Cargo, and the registry app's package manager).
- Dependabot security alerts and security updates turned on in settings.

## Out of scope

- The Cargo and registry-app entries, which arrive with PRs 1 and 6 when those manifests exist.
- Auto-merge. Every update is reviewed.
- The dependency license check (PRs 2 and 7).

## Acceptance checklist

- [ ] `dependabot.yml` is valid; GitHub's Dependabot page shows no parsing error.
- [ ] It covers `github-actions` and `docker` for `.devcontainer/`.
- [ ] Open-PR limits are set.
- [ ] Dependabot alerts and security updates show as enabled; evidence is in the PR description.
- [ ] The conventions document says new ecosystems get a `dependabot.yml` entry in the PR that adds them.
- [ ] Confirmed against GitHub's current terms that Dependabot is free for public repos, with the date checked.

## Review depth

**Skim.** Declarative config with a small surface.
