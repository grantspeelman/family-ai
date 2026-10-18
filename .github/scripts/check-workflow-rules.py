#!/usr/bin/env python3
"""Check .github/workflows/*.yml against the workflow safety rules in
docs/conventions.md. Line-based on purpose (standard library only, no YAML
parser), so it assumes the plain block style the workflows already use.
actionlint covers syntax and expressions; this covers the rules it does not.
Exit 1 and print one line per violation if any rule is broken.

Checks: top-level `permissions:` present and never `write` (or any inline map
other than `{}`); no `pull_request_target` / `workflow_run` at all (stricter
than the rule text, which only forbids running fork code); no `secrets`
reference; no `${{ }}` in a `run:` value (the whole indented block counts,
whatever form the first line takes); every `uses:` pinned by full SHA with a
version comment; every `actions/checkout` step has `persist-credentials:
false`; every job has `timeout-minutes` (jobs that call a reusable workflow
are exempt).

Known limits (a pass here is not proof): flow-style YAML (`{...}` or `[...]`)
other than the `permissions: {}` case is not parsed; YAML anchors and aliases
are not followed; text that merely looks like a key inside a multi-line
scalar other than `run:` is read as a key; per-job `permissions:` are not
reviewed (the reason comment is for the reviewer); expressions built from untrusted input in places other than `run:`
(for example `actions/github-script` `script:`) are not checked; `uses:` text
inside a `run:` block is skipped, so it is not checked either; binary
checksums are not verified."""
import re
import sys
from pathlib import Path

PIN = re.compile(r"^[^@\s]+@[0-9a-f]{40}\s+#\s*\S")
SECRETS = re.compile(r"\bsecrets\.|\bsecrets\b\s*(\[|:\s*inherit)|toJSON\(\s*secrets")
EXPR_MSG = "`${{ }}` expression inside `run:`; pass it through `env:` instead"
errors = []


def err(path, n, msg):
    errors.append(f"{path}:{n}: {msg}")


def indent(line):
    return len(line) - len(line.lstrip(" "))


def value_of(text):
    """Value after `key:` with a trailing comment and quotes removed."""
    v = re.sub(r"\s+#.*$", "", text.strip())
    return v.strip().strip("'\"").strip()


def check_perm_value(f, n, v):
    if v == "write" or v == "write-all":
        err(f, n, "top-level permissions must be read-only; put `write` on one job with a reason")


files = sorted(Path(".github/workflows").glob("*.y*ml"))
if not files:
    sys.exit("no workflows found; run from the repo root")

for f in files:
    lines = f.read_text().splitlines()
    if not any(re.match(r"^permissions:", l) for l in lines):
        err(f, 1, "no top-level `permissions:` (default must be set, read-only)")
    in_top_perms = False
    run_base = None  # column of the `run:` key while inside its value
    in_jobs = False
    job_name = None  # (name, line number, has_timeout, is_reusable)
    jobs = []

    for n, l in enumerate(lines, 1):
        s = l.strip()
        in_run = False
        # Inside a run value: any blank or more-indented line belongs to it.
        if run_base is not None:
            if s and indent(l) <= run_base:
                run_base = None
            else:
                in_run = True
                if "${{" in l:
                    err(f, n, EXPR_MSG)
        if not in_run:
            if s.startswith("#"):
                continue
            m = re.match(r"^(\s*)(- )?run:\s*(.*)$", l)
            if m:
                run_base = len(m.group(1)) + (2 if m.group(2) else 0)
                if "${{" in m.group(3):
                    err(f, n, EXPR_MSG)

            # top-level permissions
            m = re.match(r"^permissions:\s*(.*)$", l)
            if m:
                v = value_of(m.group(1))
                in_top_perms = v == ""
                if v and v not in ("read-all", "{}"):
                    if v.startswith("{"):
                        err(f, n, "top-level permissions as an inline map; use a block, or `{}`")
                    check_perm_value(f, n, v)
                continue
            if in_top_perms:
                if l and not l.startswith(" "):
                    in_top_perms = False
                elif ":" in s:
                    check_perm_value(f, n, value_of(s.split(":", 1)[1]))

            # jobs and their timeouts
            if re.match(r"^jobs:\s*$", l):
                in_jobs = True
            elif l and not l.startswith(" ") and not s.startswith("#"):
                in_jobs = False
            if in_jobs:
                m = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", l)
                if m:
                    jobs.append({"name": m.group(1), "n": n, "timeout": False, "reusable": False})
                elif jobs:
                    if re.match(r"^    timeout-minutes:", l):
                        jobs[-1]["timeout"] = True
                    if re.match(r"^    uses:", l):
                        jobs[-1]["reusable"] = True

            # uses
            m = re.match(r"^(\s*)(- )?uses:\s*(\S.*)$", l)
            if m:
                ref = m.group(3).strip()
                if ref.startswith("./"):
                    pass  # local action
                elif ref.startswith("docker://"):
                    if "@sha256:" not in ref:
                        err(f, n, "docker:// action must be pinned by @sha256 digest")
                elif not PIN.match(ref):
                    err(f, n, f"action not pinned by full commit SHA with a version comment: {ref}")
                if ref.startswith("actions/checkout@"):
                    # Find this step's range (its `- ` line, then until the next step
                    # or a dedent) and look for persist-credentials: false anywhere in it.
                    start = n - 1
                    if not m.group(2):
                        while start > 0 and not (re.match(r"^\s*- ", lines[start]) and indent(lines[start]) < indent(l)):
                            start -= 1
                    step_indent = indent(lines[start])
                    end = start + 1
                    while end < len(lines):
                        nl = lines[end]
                        if nl.strip() and (indent(nl) < step_indent or (indent(nl) == step_indent and nl.lstrip().startswith("- "))):
                            break
                        end += 1
                    if not any(re.match(r"^\s*persist-credentials:\s*['\"]?false['\"]?\s*(#.*)?$", x) for x in lines[start:end]):
                        err(f, n, "checkout without `persist-credentials: false`")

        if SECRETS.search(l) and not s.startswith("#"):
            err(f, n, "`secrets` reference; workflows must not need secrets")
        if not s.startswith("#") and re.search(r"\bpull_request_target\b|\bworkflow_run\b", l):
            err(f, n, "pull_request_target / workflow_run are not allowed")

    for j in jobs:
        if not j["timeout"] and not j["reusable"]:
            err(f, j["n"], f"job `{j['name']}` has no `timeout-minutes`")

for e in errors:
    print(e)
if errors:
    sys.exit(1)
print(f"workflow rules: {len(files)} workflow(s) pass")
