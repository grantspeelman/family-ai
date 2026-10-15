# Family AI — First Build: PR Sequence

*Last updated 6 October 2026.*

Breaks the account-to-VM skeleton spec into an ordered sequence of PRs for agentic development, with tooling and testing built in from the start rather than bolted on.

## PR 0 — Foundation

Single dev container (not separate per-service containers) holding the full toolchain: Rust plus whatever the registry app's stack needs, all able to run and talk to each other over localhost. No attempt to mimic production's separate Firecracker isolation here — this is purely for a fast agentic dev loop.

Also in PR 0:

- Claude agent harness basics (AGENTS.md or equivalent, repo conventions).
- Written conventions and baseline principles for Claude to follow through the rest of the build.
- A pre-commit hook that **blocks the commit outright on failure**, scoped to **changed files only** (lint, tests, duplication/complexity checks on what changed). CI runs the full suite separately as the real gate before merge — the hook is a fast local check, not a substitute for CI.

**Working principles captured here** (apply throughout the build, not just PR 0):

- Every PR starts from a short written spec or checklist Claude confirms it understands before touching code.
- Decide per PR whether it needs a close human read or a skim, and size the PR accordingly (see working notes below).
- Claude runs tests itself and shows they pass before marking a PR done — the pre-commit hook backs this up automatically rather than relying on memory.
- Keep PRs small enough that Rust's slow compile times don't turn the test-fix-retest loop painful for the agent.

**Public repo hygiene** (the repo is public from the first commit, so history is public too):

- Make the AGPL-3.0 `LICENSE` file part of the first commit.
- Secret scanning in the pre-commit hook (gitleaks or similar), plus GitHub secret scanning with push protection switched on.
- No real Fly token or API key in GitHub Actions. CI runs against the Fly mock, so it never needs one.
- Fork PRs: CI workflows must not run untrusted code with secrets available.
- Dependabot and CodeQL configured in the repo (plans 10 and 11). Switching them on in GitHub settings is the owner's step, with the free-for-public-repos evidence recorded in `docs/github-settings.md`.
- Decide which project docs get committed. Architecture docs are fine if you are happy sharing them; keep pricing and positioning out of the repo (the rule is in [docs/conventions.md](docs/conventions.md#what-goes-in-the-repo)).

## PR sequence

| # | PR | Notes |
| --- | --- | --- |
| 1 | Scaffold the registry app | Empty GraphQL schema, deployable. No logic yet. |
| 2 | Registry app tooling | Linting, duplication detection, complexity detection, coverage reporting, a dependency license check against the AGPL-compatible allowlist, wired into CI. Likely splits into separate PRs per tool — size each by how much judgement it needs from Gman. |
| 3 | Account creation mutation | Minimal — email or family name, stored in the registry's own metadata store. No Fly integration yet. |
| 4 | Fly mock service | Imitates the create-app, create-volume, create-Machine endpoints. Actually boots the real family Rust app as its "Machine" and hands back its address. No Firecracker — API shape only. Likely splits into the mock API shell first, then wiring it to actually spawn the family app. |
| 5 | Fly integration on the registry | Account creation calls the mock's API to create the family's app, volume and Machine. Swapping the mock for real Fly later shouldn't change this code, only the base URL. |
| 6 | Scaffold the Rust family app | Boots, connects to SQLite on its volume, runs migrations, creates the table. |
| 7 | Family app tooling | Rust-flavoured equivalent of PR 2: clippy, coverage, duplication, complexity, cargo-deny (license allowlist, plus advisories and bans), in CI before real endpoints land. Likely splits the same way as PR 2. |
| 8 | Round-trip endpoint | Write a row, read it back — real proof of the database round trip, not a hello-world reply. |
| 9 | End-to-end test suite | Registry + mock + family app together, asserting the full chain end to end. |
| 10 | Front end: form and waiting state | Account creation form, waiting state while the Machine boots. |
| 11 | Front end: direct handoff | Once ready, talks directly to the family app's address (not proxied through the registry) and shows the real response. |
| 12 | Persistence re-check | After an idle period (an hour or more), re-run the registry flow and confirm the earlier row is still on the volume. |

**PR 12 addition — boot timing.** Record cold boot time end to end (app create → volume attach → Machine start → app ready) and Rust family app startup time as logged metrics during the persistence check, rather than a tight blocking gate. The architecture notes (not in this repo) flag this as genuinely unmeasured — Firecracker itself boots in roughly 125 milliseconds, but the rest of the path (disk attach, mount, app start, migrations) has no real numbers yet. Assert only a generous sanity ceiling in CI for now (e.g. under 5 seconds, not 500 milliseconds) so a real regression still fails the build without chasing CI noise. Once real numbers exist, tighten the threshold with data instead of a guess.

Real Fly credentials swap in after this list, as a later step — not part of the twelve.

## Working notes: sizing and splitting PRs

Before starting any PR flagged above as a likely bundle, pause and break it down live rather than pre-deciding the exact cut up front.

Size each resulting PR by how much of Gman's attention it actually needs, not by a fixed line count:

- Boilerplate-heavy, low-judgement work (scaffolding, generated tooling config) can run bigger — it's skimmed, not read closely.
- Anything with real judgement calls — API contracts, data handling, security-adjacent code — stays small enough to read closely, line by line.

This rule applies as a standing step throughout the build, not just at the two flagged bundles (PR 2 and PR 7).

## Licensing

**Leaning towards AGPL-3.0, not yet final.** It is genuinely open source, and its network clause means anyone modifying it and running it as a service must offer their users the source. It does not stop a competitor running an unmodified public clone. O'Saasy and FSL were considered and set aside for now: both are source-available rather than open source, and both are less established.

**Dependency license gate.** An agent will add whatever dependency makes the tests pass without considering its license, so the check is mechanical rather than remembered:

- Permissive licenses (MIT, Apache-2.0, BSD, ISC, public domain such as SQLite) are fine.
- Disallow GPL-2.0-only, proprietary libraries, and source-available licenses with usage restrictions (SSPL, BSL, Elastic, FSL, O'Saasy).
- Rust side: `cargo-deny` with a license allowlist, run in CI (PR 7) and in the pre-commit hook when dependencies change.
- Registry app side: the equivalent license-check tool for whichever stack it uses, added in PR 2.
- Dev tooling (linters, coverage, CI) is not linked into shipped code, so it is out of scope for the gate.

**Also decided to track:**

- A `LICENSE` file goes into the repo in PR 0 once the license is confirmed.

**Open questions:**

- Whether to require a CLA (or similar) before accepting outside contributions. Without one, the copyright holder cannot easily relicense or dual-license later, and App Store distribution of the native clients gets harder.
- Third-party license check for the front end's dependencies, once its stack is chosen.
