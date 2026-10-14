# GitHub settings checklist

Settings that live in GitHub, not in the repo, so a PR cannot apply them. The repo owner applies them by hand. This page is the exact list for the CI gate; other plans add their own sections.

## Require the CI gate before merging to `main`

The workflow `.github/workflows/devcontainer-ci.yml` ends in one job named `ci`. It succeeds only if every job listed in its `needs:` succeeded, so `ci` is the only status check branch protection needs to require. A job added to that `needs:` list is gated automatically.

The check name `ci` only appears in GitHub's search box after the workflow has run once on a PR or on `main`. Merge or open the PR that adds it first, let it run, then do the steps below.

1. Repository **Settings**, then **Branches** (classic branch protection) or **Rules**, then **Rulesets**. Use one of the two, not both.
2. Add a rule for `main`: **Add branch protection rule** (classic) or **New branch ruleset** targeting the default branch (ruleset).
3. Require a pull request before merging (classic: tick **Require a pull request before merging**; ruleset: enable **Require a pull request before merging**).
4. Require status checks to pass before merging (classic: tick **Require status checks to pass before merging** and **Require branches to be up to date before merging**; ruleset: enable **Require status checks to pass** and **Require branches to be up to date before merging**).
5. In the status check search, add `ci`. Choose the **GitHub Actions** source for it. Add no other check.
6. Applying to admins too (optional, the owner's call). Classic: tick **Do not allow bypassing the above settings**. Ruleset: leave the bypass list empty.
7. Save (classic) or set enforcement to **Active** and create (ruleset).

Then confirm: open a PR that fails a check and see that the merge button is blocked, and that a passing PR is mergeable.

When a job is added to the workflow, add it to the `needs:` list of the `ci` job. Branch protection does not change.

## Require approval for workflows from outside contributors

Owner action. Without this, a first-time contributor's PR runs workflows as soon as it is opened. The workflows follow the safety rules in [docs/conventions.md](conventions.md#workflow-safety), but approval means a person looks at the PR before anything runs.

1. Repository **Settings**, then **Actions**, then **General**.
2. Under **Fork pull request workflows from outside collaborators**, choose **Require approval for all outside collaborators** (stricter) or at least **Require approval for first-time contributors**. The owner's call; record which.
3. Under **Workflow permissions**, choose **Read repository contents and packages permissions**, and leave **Allow GitHub Actions to create and approve pull requests** unticked.
4. Under **Fork pull request workflows**, leave **Send write tokens to workflows from pull requests** and **Send secrets and variables to workflows from pull requests** off. These are only offered for private repos; on a public repo, fork PRs never get them.
5. Save.

Then confirm: the settings page shows the choice you made.

## Record in the PR

The PR description for the change that adds the gate records which of these settings were applied and when.
