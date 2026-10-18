# GitHub settings checklist

The repo owner applies these settings in GitHub. A PR cannot apply them. This page covers the CI gate, fork PR approval, secret scanning, Dependabot and CodeQL.

## Require the CI gate before merging to `main`

The workflow `.github/workflows/devcontainer-ci.yml` ends with a job named `ci`. It succeeds only if every job in its `needs:` list succeeds. Require only `ci` in branch protection, and add each job to its `needs:` list to include it in the gate. The `ci` job uses `if: always()` so it runs even if a dependency fails. Without it, GitHub could skip `ci` and treat the skipped required check as passing.

The check name `ci` appears in GitHub's search box after the workflow first runs on a PR or on `main`. Open or merge the PR that adds it and let it run before following these steps.

1. Repository **Settings**, then **Branches** (classic branch protection) or **Rules**, then **Rulesets**. Use one of the two, not both.
2. Add a rule for `main`: **Add branch protection rule** (classic) or **New branch ruleset** targeting the default branch (ruleset).
3. Require a pull request before merging (classic: tick **Require a pull request before merging**; ruleset: enable **Require a pull request before merging**).
4. Require status checks to pass before merging (classic: tick **Require status checks to pass before merging** and **Require branches to be up to date before merging**; ruleset: enable **Require status checks to pass** and **Require branches to be up to date before merging**).
5. In the status check search, add `ci`. Choose the **GitHub Actions** source for it. Add no other check.
6. To apply the rule to admins too, tick **Do not allow bypassing the above settings** in classic branch protection or leave the bypass list empty in a ruleset. This is optional; the owner decides.
7. Save the classic rule, or set the ruleset's enforcement to **Active** and create it.

To confirm, check that a failing PR has a blocked merge button and a passing PR can merge.

When adding a workflow job, add it to `ci`'s `needs:` list. Branch protection stays the same.

## Require approval for workflows from outside contributors

GitHub requires approval for first-time contributors by default. The owner can choose a stricter setting. Approval lets a person review a fork PR before its workflows run. The workflows follow the [workflow safety rules](conventions.md#workflow-safety).

1. Repository **Settings**, then **Actions**, then **General**.
2. Under **Approval for running fork pull request workflows from contributors**, choose one of the three options:
   - **Require approval for first-time contributors who are new to GitHub** is the weakest option.
   - **Require approval for first-time contributors** is GitHub's default.
   - **Require approval for all external contributors** is the strictest option. It is recommended while the repo is new and has few contributors so the owner reviews every fork PR run first. Relax it later if approvals become a burden.

   Record which one you chose.
3. Click **Save** for that section. Each section on this page saves on its own.
4. Under **Workflow permissions**, choose **Read repository contents and packages permissions** and leave **Allow GitHub Actions to create and approve pull requests** unticked. Click **Save** for this section too.

GitHub shows the **Fork pull request workflows** section, with **Send write tokens** and **Send secrets**, only for private repositories. This public repo has no setting there. Fork PRs never receive secrets or write tokens.

Reload the page and check that both sections show your choices.

## Secret scanning and push protection

The owner enables these settings so GitHub blocks pushes containing known secret formats and alerts on any that get through. They also cover commits made outside the dev container, where the local hook does not run. [AGENTS.md](../AGENTS.md#running-the-checks) covers the local hook and CI history scan.

Availability, checked 2026-10-14 against GitHub's docs:

- [About secret scanning](https://docs.github.com/en/code-security/secret-scanning/introduction/about-secret-scanning) says that for public repositories "Secret scanning runs automatically for free."
- [Enabling secret scanning for your repository](https://docs.github.com/en/code-security/secret-scanning/enabling-secret-scanning-features/enabling-secret-scanning-for-your-repository) says secret scanning alerts "can be enabled on any free public repository that you own."
- [About GitHub Advanced Security](https://docs.github.com/en/get-started/learning-about-github/about-github-advanced-security) has a comparison table with a "Public repository without GitHub Secret Protection" column that shows both secret scanning and push protection as "Yes". This table confirms push protection is available for public repos. "Secret Protection" names the settings section here, not a paid plan.
- The push protection pages state no price for public repos, but this is weaker evidence. If any **Enable** button shows a price or a billing prompt, stop and check the docs again.

Follow these steps. Sidebar names can change; use the enabling page linked above if they differ.

1. Repository main page, then **Settings** (under the repo name; if it is hidden, open the **...** menu).
2. In the **Security and quality** section of the sidebar, click **Advanced Security**.
3. If Secret Protection is not already enabled, click **Enable** next to **Secret Protection**, then confirm with **Enable Secret Protection** if asked. This turns on secret scanning. If it already shows as enabled, go on to step 4.
4. In the **Secret Protection** section, next to **Push protection**, click **Enable**.

Secret scanning may already be on by default for public repos, so step 3 may require no action. Either way, record who confirmed or enabled it and when in the table below.

Reload **Advanced Security** and check that both settings show as enabled. With repo admin access, you can also run the read-only command `gh api repos/OWNER/REPO --jq .security_and_analysis`. It should show `secret_scanning` and `secret_scanning_push_protection` as `enabled`.

Record who applied or confirmed each setting and when:

| Setting | Turned on by | Date |
| --- | --- | --- |
| Secret scanning | Owner (confirmed enabled in repo settings) | 2026-10-17 |
| Push protection | Owner (confirmed enabled in repo settings) | 2026-10-17 |

### If a secret leaks

Follow these steps in order. Treat anything pushed to a public repo as copied immediately. Rotate first; rewriting history cannot remove those copies.

1. Rotate or revoke the credential at its provider immediately. Issue a new one, switch whatever uses it, then revoke the old one. Do this before anything else below, including deleting the commit.
2. Check the provider's access logs for use of the old credential since it was pushed.
3. Remove it from the repo in a new commit. Once the credential is revoked, you can optionally rewrite history with a tool such as `git filter-repo`, then force-push. This does not remove copies in forks, clones or GitHub's cached views. GitHub Support can clear cached views for a commit.
4. Resolve the GitHub secret scanning alert, marking it as revoked, and note what was rotated in the PR or issue without writing the secret itself.

A push protection block means nothing reached GitHub, so it is not a leak. Remove the secret from the commit and push again. For a false positive, use the bypass option and give the reason GitHub asks for.

## Dependabot alerts and security updates

The owner enables these two repo settings. Alerts report known dependency advisories. Security updates open PRs to fix them. Those PRs require review and a passing `ci` check, with no auto-merge. Version updates use `.github/dependabot.yml` and need no repo setting. See [docs/conventions.md](conventions.md#dependency-updates-dependabot).

Availability, checked 2026-10-14 against GitHub's pages:

- [GitHub's pricing page](https://github.com/pricing) lists "Dependabot security and version updates" on the Free plan, and [GitHub's plans](https://docs.github.com/en/get-started/learning-about-github/githubs-plans) gives Free "unlimited collaborators on unlimited public repositories with a full feature set".
- [About billing for GitHub Actions](https://docs.github.com/en/billing/managing-billing-for-github-actions/about-billing-for-github-actions) says standard GitHub-hosted runners are free for Dependabot and public repositories. This covers Actions billing, not Dependabot's price.
- The pages on enabling alerts and security updates state no price for public repos. The pricing page is marketing copy, not a terms document; if any **Enable** button shows a price or a billing prompt, stop and check again.

Follow these steps. If the sidebar differs, use [Configuring Dependabot alerts](https://docs.github.com/en/code-security/dependabot/dependabot-alerts/configuring-dependabot-alerts) and [Configuring Dependabot security updates](https://docs.github.com/en/code-security/dependabot/dependabot-security-updates/configuring-dependabot-security-updates).

1. Repository main page, then **Settings** (if it is hidden, open the **...** menu).
2. In the **Security and quality** section of the sidebar, click **Advanced Security**.
3. Next to **Dependabot alerts**, click **Enable**.
4. Next to **Dependabot security updates**, click **Enable**. For public repos the button can be disabled when GitHub has the feature always on; that counts as enabled.

Reload **Advanced Security** and check that both settings show as enabled. After the `dependabot.yml` PR merges to the default branch, open **Insights**, then **Dependency graph**, then **Dependabot**. Check that both ecosystems appear with no parsing error. GitHub parses the file only after it reaches the default branch.

Record who applied or confirmed each setting and when:

| Setting | Turned on by | Date |
| --- | --- | --- |
| Dependabot alerts | | |
| Dependabot security updates | | |
| `dependabot.yml` parsed with no error | | |

## Code scanning (CodeQL)

The owner enables CodeQL through GitHub's default setup. The repo setting configures it; there is no workflow file. See [docs/conventions.md](conventions.md#codeql).

Availability, checked 2026-10-14 against GitHub's docs:

- [Configuring advanced setup for code scanning](https://docs.github.com/en/code-security/how-tos/scan-code-for-vulnerabilities/configure-code-scanning/configuring-advanced-setup-for-code-scanning) lists the eligibility requirement "It is publicly visible, or GitHub Code Security is enabled", and says "You can configure code scanning for any public repository where you have write access." So public repos need no paid licence.
- [About code scanning](https://docs.github.com/en/code-security/code-scanning/introduction-to-code-scanning/about-code-scanning) says a GitHub Code Security licence is needed "on private repositories" and that runs consume GitHub Actions minutes. Standard GitHub-hosted runners are free for public repositories. See [About billing for GitHub Actions](https://docs.github.com/en/billing/managing-billing-for-github-actions/about-billing-for-github-actions).
- None of these pages explicitly says code scanning is free for public repos. The eligibility wording above is the evidence. If any button shows a price or a billing prompt, stop and check the docs again.

Follow these steps. Menu names come from [GitHub's default setup docs](https://docs.github.com/en/code-security/code-scanning/enabling-code-scanning/configuring-default-setup-for-code-scanning), checked 2026-10-17.

1. Repository main page, then **Settings**, then **Advanced Security** in the **Security and quality** section.
2. On the **CodeQL analysis** row, choose **Set up**, then **Default**, then **Enable CodeQL**. If no **Set up** button shows, default setup is already on.
3. To see what it analyses, open the row's menu and choose **View CodeQL configuration**. Languages are changed under **Edit**.

Confirm the `Analyze (...)` checks pass on `main` and appear on PRs. Leave them optional; they are not part of the `ci` gate yet.

Record who applied or confirmed each setting and when:

| Setting | Turned on by | Date |
| --- | --- | --- |
| CodeQL default setup on | | |
| `Analyze (actions)` and `Analyze (python)` passing on `main` | | |

## Record in the PR

The PR for each owner-applied setting records what was applied and when.
