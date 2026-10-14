# 09 — GitHub secret scanning and push protection

**Status:** planned

## Goal

GitHub itself blocks pushes containing known secret formats and alerts on any that slip through. This is the layer that still works when a commit is made outside the container and the local hook never runs.

## Builds on

- 03 — Secret scanning pre-commit hook (the local layer; this is the remote one)

## In scope

- Turn on GitHub secret scanning and push protection for the repo.
- Record the settings in the repo as a short checklist, since they live in GitHub rather than in files.
- A note on what to do if a secret does leak: rotate first, then clean history.

## Out of scope

- Local scanning (plan 03) and the CI history scan (plan 02).
- Dependabot and CodeQL (plans 10 and 11).

## Acceptance checklist

- [ ] Secret scanning and push protection show as enabled in the repo's security settings. A screenshot or `gh api` output is in the PR description.
- [ ] A short document lists the settings, who turned them on, and the date.
- [ ] The leak-response note says to rotate the credential before anything else.
- [ ] Confirmed against GitHub's current terms that these features are free for public repos, with the date checked.

## Review depth

**Skim.** There is little code; the work is a settings change and a document.
