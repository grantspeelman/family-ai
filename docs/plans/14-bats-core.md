# 14 — bats-core

**Status:** in progress

## Goal

Bash scripts in this repo are tested with bats-core, and the dev container ships it. One standard runner replaces hand-rolled test scripts as more shell code lands.

## Builds on

- 01 — Dev container (the image and its structure test)
- 13 — Host-side token file (`token-file-check-test.sh`, the first shell test, which this runner replaces in plan 15)

## In scope

- `.devcontainer/Dockerfile` installs bats-core 1.14.0 from the `v1.14.0` source tarball with its own `install.sh /usr/local`. The version is an `ARG`, the SHA-256 an `ENV`, and the download is checked with `sha256sum -c` under `pipefail`, like `gh`, `gitleaks` and `lefthook`. The tarball is arch-independent, so there is one checksum. No apt packages are added; bats needs only bash.
- `container-structure-test.yaml` asserts `bats --version` prints 1.14.0.
- `docs/conventions.md` says bash scripts are tested with bats-core, in `*.bats` files; the README and the manually maintained pins list mention it.
- Helper libraries `bats-support` and `bats-assert` are not included. Nothing uses them yet; add them, pinned and checksummed the same way, in the PR that first needs them.

## Out of scope

- Converting `.devcontainer/token-file-check-test.sh` to bats. Follow-up PR.
- A CI job that runs the `.bats` files. Follow-up PR, together with the conversion.

## Acceptance checklist

- [x] The release tag and tarball URL exist: `gh api` shows `v1.14.0` as the latest release (no release assets, so GitHub's tag archive is the source).
- [x] The SHA-256 was computed from two separate downloads of that URL and matched; the archive's embedded commit id matches the tag's target commit (the tag is annotated; its own object points at eb7f42f8...).
- [x] In a temp directory, the verified tarball extracted, `install.sh <prefix>` ran, `<prefix>/bin/bats --version` printed `Bats 1.14.0`, and a trivial `.bats` file passed.
- [x] The Dockerfile step reads correctly against the existing tool installs (version `ARG`, checksum `ENV`, `sha256sum -c`, cleanup, version check).
- [ ] The image builds and `container-structure-test` passes with the new `bats` test. Not run here (no Docker daemon); confirmed by the `image` CI job.

## Review depth

**Skim.** Same pattern as the other pinned tools; check the version and checksum against the upstream tag.
