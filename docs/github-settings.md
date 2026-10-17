# GitHub settings checklist

Settings that live in GitHub, not in the repo, so a PR cannot apply them. The repo owner applies them by hand. This page is the exact list for the CI gate; other plans add their own sections.

## Require the CI gate before merging to `main`

The workflow `.github/workflows/devcontainer-ci.yml` ends in one job named `ci`. It succeeds only if every job listed in its `needs:` succeeded, so `ci` is the only status check branch protection needs to require. A job is gated only once it is in that `needs:` list. The `ci` job uses `if: always()`: without it a failed dependency would make `ci` skip, and a skipped required check can be treated as passing.

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

Owner action. By default GitHub already requires approval for first-time contributors, but a stricter choice is available. The workflows follow the safety rules in [docs/conventions.md](conventions.md#workflow-safety); approval means a person looks at a fork PR before anything runs.

1. Repository **Settings**, then **Actions**, then **General**.
2. Under **Approval for running fork pull request workflows from contributors**, choose one of the three options:
   - **Require approval for first-time contributors who are new to GitHub**: the weakest.
   - **Require approval for first-time contributors**: GitHub's default.
   - **Require approval for all external contributors**: the strictest. Recommended while the repo is new and has few contributors, because every fork PR run is then seen by the owner first. Relax it later if the approvals become a burden.

   Record which one you chose.
3. Click **Save** for that section. Each section on this page saves on its own.
4. Under **Workflow permissions**, choose **Read repository contents and packages permissions** and leave **Allow GitHub Actions to create and approve pull requests** unticked. Click **Save** for this section too.

The **Fork pull request workflows** section (with **Send write tokens** and **Send secrets**) is shown for private repositories only. This repo is public, where fork PRs never receive secrets or write tokens, so there is nothing to set there.

Then confirm: reload the page and check that both sections show the choices you made.

## Record in the PR

The PR description for the change that adds the gate records which of these settings were applied and when.
