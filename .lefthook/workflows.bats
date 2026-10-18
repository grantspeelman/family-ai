#!/usr/bin/env bats
# Integration checks using real linters and real commits in disposable repos.
bats_require_minimum_version 1.5.0

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
    cd "$BATS_TEST_TMPDIR"
    git init -q
    git config user.name 'Workflow test'
    git config user.email 'workflow-test@example.invalid'
    git config commit.gpgsign false
    git config core.hooksPath .git/hooks
    cp "$REPO_ROOT/lefthook.yml" .
    mkdir -p .github/workflows
    cat > .github/workflows/check.yml <<'YAML'
name: Check
on: pull_request
permissions:
  contents: read
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - run: echo OK
YAML
    lefthook install --reset-hooks-path
}

commit_workflow() {
    git add .github/workflows
    run git commit -m 'Test workflow'
}

assert_rejected() {
    local finding="$1"
    local hook_finding="${2:-$1}"
    # Same audit options as CI and the hook; a single filename also works in CI.
    run zizmor --offline --persona=pedantic .github/workflows/
    [ "$status" -ne 0 ]
    [[ "$output" == *"$finding"* ]]
    commit_workflow
    [ "$status" -ne 0 ]
    [[ "$output" == *"$hook_finding"* ]]
    run git rev-parse --verify HEAD
    [ "$status" -ne 0 ]
}

@test "valid staged workflow passes both linters and commits" {
    commit_workflow
    [ "$status" -eq 0 ]
    [[ "$output" == *actionlint* ]]
    [[ "$output" == *zizmor* ]]
}

@test "tag-pinned action fails CI lint and blocks a commit" {
    sed -i 's/@3d3c42e5aac5ba805825da76410c181273ba90b1/@v7.0.1/' .github/workflows/check.yml
    assert_rejected unpinned-uses
}

@test "flow-style YAML cannot hide an unpinned action" {
    sed -i '/      - uses:/,+2c\      - {uses: actions/checkout@v7.0.1, with: {persist-credentials: false}} # v7.0.1' .github/workflows/check.yml
    assert_rejected unpinned-uses
}

@test "missing persist-credentials fails CI lint and blocks a commit" {
    sed -i '/        with:/,+1d' .github/workflows/check.yml
    assert_rejected artipacked
}

@test "branch name injection fails CI lint and blocks a commit" {
    sed -i 's/echo OK/echo "${{ github.head_ref }}"/' .github/workflows/check.yml
    # actionlint catches this first; piped hooks stop before zizmor runs.
    assert_rejected template-injection '"github.head_ref" is potentially untrusted'
}

@test "invalid workflow expression is blocked by actionlint" {
    sed -i 's/echo OK/echo "${{ nonexistent.value }}"/' .github/workflows/check.yml
    commit_workflow
    [ "$status" -ne 0 ]
    [[ "$output" == *actionlint* ]]
    [[ "$output" == *nonexistent* ]]
}

@test "yaml extension and spaces in a workflow filename are checked" {
    mv .github/workflows/check.yml '.github/workflows/other check.yaml'
    sed -i 's/@3d3c42e5aac5ba805825da76410c181273ba90b1/@v7.0.1/' '.github/workflows/other check.yaml'
    assert_rejected unpinned-uses
}

@test "valid yaml workflow with spaces in its filename commits" {
    mv .github/workflows/check.yml '.github/workflows/other check.yaml'
    commit_workflow
    [ "$status" -eq 0 ]
    [[ "$output" == *actionlint* ]]
    [[ "$output" == *zizmor* ]]
}

@test "unrelated staged file skips both workflow linters" {
    printf 'An unrelated change.\n' > note.md
    git add note.md
    run git commit -m 'Test unrelated file'
    [ "$status" -eq 0 ]
    [[ "$output" == *"actionlint (skip)"* ]]
    [[ "$output" == *"zizmor (skip)"* ]]
}
