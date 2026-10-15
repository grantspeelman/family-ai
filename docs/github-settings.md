# GitHub settings checklist

Settings that live in GitHub, not in the repo, so a PR cannot apply them. The repo owner applies them by hand. This page is the exact list for the CI gate, fork PR approval, secret scanning, Dependabot and CodeQL; other plans add their own sections.

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

## Secret scanning and push protection

Owner action. The local hook and the CI history scan are covered in [AGENTS.md](../AGENTS.md#running-the-checks). These two settings are the remote layer: GitHub blocks a push that contains a known secret format, and alerts on any that get through. They still work for commits made outside the dev container, where the local hook never runs.

Availability, checked 2026-10-14 against GitHub's docs:

- [About secret scanning](https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning) says that for public repositories "Secret scanning runs automatically for free."
- [Enabling secret scanning for your repository](https://docs.github.com/en/code-security/secret-scanning/enabling-secret-scanning-features/enabling-secret-scanning-for-your-repository) says secret scanning alerts "can be enabled on any free public repository that you own."
- [About GitHub Advanced Security](https://docs.github.com/en/get-started/learning-about-github/about-github-advanced-security) has a comparison table with a "Public repository without GitHub Secret Protection" column that shows both secret scanning and push protection as "Yes". That table is the authority for push protection on public repos. "Secret Protection" is just the name of the settings section there, not a paid plan.
- Weaker evidence: the push protection pages themselves state no price for public repos. If any **Enable** button shows a price or a billing prompt, stop and check the docs again.

Steps (GitHub's current wording; the sidebar names change now and then, so go by the page above if they differ):

1. Repository main page, then **Settings** (under the repo name; if it is hidden, open the **...** menu).
2. In the **Security and quality** section of the sidebar, click **Advanced Security**.
3. If Secret Protection is not already enabled, click **Enable** next to **Secret Protection**, then confirm with **Enable Secret Protection** if asked. This turns on secret scanning. If it already shows as enabled, go on to step 4.
4. In the **Secret Protection** section, next to **Push protection**, click **Enable**.

Secret scanning may already be on by default for public repos, so step 3 can be a no-op. The record table below still needs filling in either way (who confirmed or enabled it, and when).

Then confirm: reload **Advanced Security** and check that both show as enabled. A read-only check with `gh api repos/OWNER/REPO --jq .security_and_analysis` also shows `secret_scanning` and `secret_scanning_push_protection` as `enabled` (it needs repo admin access).

Record it:

| Setting | Turned on by | Date |
| --- | --- | --- |
| Secret scanning | | |
| Push protection | | |

### If a secret leaks

Do these in order. Rotation comes first because anything pushed to a public repo should be treated as copied the moment it lands, and rewriting history does not take it back.

1. **Rotate or revoke the credential at its provider, straight away.** Issue a new one, switch whatever uses it, then revoke the old one. Do this before anything else below, including deleting the commit.
2. Check the provider's access logs for use of the old credential since it was pushed.
3. Only then remove it from the repo. Delete it in a new commit first. Rewriting history (for example with `git filter-repo`, then a force-push) is optional once the credential is dead, and does not remove copies in forks, clones or GitHub's cached views; GitHub Support can clear cached views for a commit.
4. Resolve the GitHub secret scanning alert, marking it as revoked, and note what was rotated in the PR or issue without writing the secret itself.

Push protection blocks are not leaks, since nothing reached GitHub. Remove the secret from the commit and push again. If a block is a false positive, use the bypass option with the reason GitHub asks for, rather than leaving it unexplained.

## Dependabot alerts and security updates

Owner action. `.github/dependabot.yml` (version updates, covered in [docs/conventions.md](conventions.md#dependency-updates-dependabot)) is in the repo and needs no setting. These two are separate, repo-level settings: alerts tell you when a dependency has a known advisory, and security updates open a PR to fix it. The PRs follow the same rules as any other: reviewed, no auto-merge, `ci` must pass.

Availability, checked 2026-10-14 against GitHub's pages:

- [GitHub's pricing page](https://github.com/pricing) lists "Dependabot security and version updates" on the Free plan, and [GitHub's plans](https://docs.github.com/en/get-started/learning-about-github/githubs-plans) gives Free "unlimited collaborators on unlimited public repositories with a full feature set".
- Side point only, about Actions billing and not about whether Dependabot is free: [About billing for GitHub Actions](https://docs.github.com/en/billing/managing-billing-for-github-actions/about-billing-for-github-actions) says standard GitHub-hosted runners are free for Dependabot and for public repositories.
- The pages on enabling alerts and security updates state no price for public repos. The pricing page is marketing copy, not a terms document; if any **Enable** button shows a price or a billing prompt, stop and check again.

Steps (GitHub's current wording; go by the pages [Configuring Dependabot alerts](https://docs.github.com/en/code-security/dependabot/dependabot-alerts/configuring-dependabot-alerts) and [Configuring Dependabot security updates](https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/configuring-dependabot-security-updates) if the sidebar differs):

1. Repository main page, then **Settings** (if it is hidden, open the **...** menu).
2. In the **Security and quality** section of the sidebar, click **Advanced Security**.
3. Next to **Dependabot alerts**, click **Enable**.
4. Next to **Dependabot security updates**, click **Enable**. For public repos the button can be disabled when GitHub has the feature always on; that counts as enabled.

Then confirm: reload **Advanced Security** and check that both show as enabled. After the `dependabot.yml` PR is on the default branch, open **Insights**, then **Dependency graph**, then **Dependabot** and check that both ecosystems are listed with no parsing error (GitHub parses the file only once it is on the default branch).

Record it:

| Setting | Turned on by | Date |
| --- | --- | --- |
| Dependabot alerts | | |
| Dependabot security updates | | |
| `dependabot.yml` parsed with no error | | |

## Code scanning (CodeQL)

Owner action. `.github/workflows/codeql.yml` (covered in [docs/conventions.md](conventions.md#codeql)) is an advanced-setup workflow. It runs without any setting, but its results only appear in the **Security** tab once it has run on the default branch, and GitHub refuses its uploads while **default setup** is on.

Availability, checked 2026-10-14 against GitHub's docs:

- [Configuring advanced setup for code scanning](https://docs.github.com/en/code-security/how-tos/scan-code-for-vulnerabilities/configure-code-scanning/configuring-advanced-setup-for-code-scanning) lists the eligibility requirement "It is publicly visible, or GitHub Code Security is enabled", and says "You can configure code scanning for any public repository where you have write access." So public repos need no paid licence.
- [About code scanning](https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning) says a GitHub Code Security licence is needed "on private repositories" and that runs consume GitHub Actions minutes; standard GitHub-hosted runners are free for public repositories ([About billing for GitHub Actions](https://docs.github.com/en/billing/managing-billing-for-github-actions/about-billing-for-github-actions)).
- None of these pages says "free" in so many words for public repos; the eligibility wording above is the evidence. If any button shows a price or a billing prompt, stop and check the docs again.

Steps. The menu names below are taken from GitHub's docs (checked 2026-10-14), not from the live UI; if the UI differs, go by GitHub's page.

1. Repository main page, then **Settings**, then **Advanced Security** in the **Security and quality** section.
2. Look at the **CodeQL analysis** row. If it says default setup is on, open its menu and choose **Switch to advanced**, then **Disable CodeQL** in the pop-up. Default setup and an advanced workflow conflict: with default setup on, GitHub rejects the workflow's uploads ("Upload with CodeQL results rejected due to default setup", see [Troubleshooting SARIF uploads](https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/troubleshoot-sarif-uploads/default-setup-enabled)). If the row shows nothing is set up, there is nothing to switch off. Do not choose **Set up** then **Default**.

   API fallback, if the menu is not where described (needs repo admin access): `gh api -X PATCH repos/OWNER/REPO/code-scanning/default-setup -f state=not-configured`. The endpoint `PATCH /repos/{owner}/{repo}/code-scanning/default-setup` and its `state` values `configured` and `not-configured` are from [GitHub's REST docs for code scanning](https://docs.github.com/en/rest/code-scanning/code-scanning) (checked 2026-10-15); the `gh` command itself has not been run.
3. Once the `codeql` workflow has run on `main`, open **Security**, then **Code scanning**, and check an `Analyze (actions)` analysis is listed with no configuration error.

Then confirm: the check `Analyze (actions)` passes on a PR, and the code scanning tab lists the analysis for `main`. Do not add it as a required check (it is not part of the `ci` gate yet).

Record it:

| Setting | Turned on by | Date |
| --- | --- | --- |
| Default setup off (if it was on) | | |
| `codeql` results visible in the code scanning tab | | |

## Record in the PR

The PR for each owner-applied setting records what was applied and when.
