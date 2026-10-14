# GitHub settings checklist

Settings that live in GitHub, not in the repo, so a PR cannot apply them. The repo owner applies them by hand. This page is the exact list for the CI gate; other plans add their own sections.

## Require the CI gate before merging to `main`

The workflow `.github/workflows/devcontainer-ci.yml` ends in one job named `ci`. It succeeds only if every job it depends on succeeded, so `ci` is the only status check branch protection needs to require.

The check name `ci` only appears in GitHub's search box after the workflow has run once on a PR or on `main`. Merge or open the PR that adds it first, let it run, then do the steps below.

1. Repository **Settings**, then **Branches** (or **Rules**, then **Rulesets**, if using rulesets).
2. Add a branch protection rule (or ruleset) for `main`.
3. Tick **Require a pull request before merging**.
4. Tick **Require status checks to pass before merging**, and **Require branches to be up to date before merging**.
5. In the status check search, add `ci` (the job named `ci`; pick the entry from the GitHub Actions app). Add no other check.
6. Tick **Do not allow bypassing the above settings** so it applies to admins too (optional, the owner's call).
7. Save.

Then confirm: open a PR that fails a check and see that the merge button is blocked, and that a passing PR is mergeable.

When a check is added to the workflow, add it to the `needs:` list of the `ci` job. Branch protection does not change.

## Record in the PR

The PR description for the change that adds the gate records which of these settings were applied and when.
