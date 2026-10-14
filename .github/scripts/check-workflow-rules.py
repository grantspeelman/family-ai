#!/usr/bin/env python3
"""Check .github/workflows/*.yml against the workflow safety rules in
docs/conventions.md. Line-based on purpose (standard library only, no YAML
parser), so it assumes the plain block style the workflows already use.
actionlint covers syntax and expressions; this covers the rules it does not.
Exit 1 and print one line per violation if any rule is broken."""
import re
import sys
from pathlib import Path

PIN = re.compile(r"^[^@\s]+@[0-9a-f]{40}\s+#\s*\S")
errors = []


def err(path, n, msg):
    errors.append(f"{path}:{n}: {msg}")


def indent(line):
    return len(line) - len(line.lstrip(" "))


files = sorted(Path(".github/workflows").glob("*.y*ml"))
if not files:
    sys.exit("no workflows found; run from the repo root")

for f in files:
    lines = f.read_text().splitlines()
    if not any(re.match(r"^permissions:", l) for l in lines):
        err(f, 1, "no top-level `permissions:` (default must be set, read-only)")
    for l in lines:
        m = re.match(r"^permissions:\s*(\S.*)?$", l)
        if m and (m.group(1) or "").strip() in ("write-all", "read-all-write"):
            err(f, 1, "top-level permissions must be read-only")
    in_top_perms = False
    run_indent = None  # indentation of the `run:` key while inside a run block
    for n, l in enumerate(lines, 1):
        s = l.strip()
        if s.startswith("#"):
            continue
        # top-level permissions values
        if re.match(r"^permissions:\s*$", l):
            in_top_perms = True
            continue
        if in_top_perms:
            if l and not l.startswith(" "):
                in_top_perms = False
            elif ":" in s and s.split(":", 1)[1].strip() == "write":
                err(f, n, "top-level permissions must be read-only; put `write` on one job with a reason")
        # run blocks
        if run_indent is not None:
            if s and indent(l) <= run_indent:
                run_indent = None
            elif "${{" in l:
                err(f, n, "`${{ }}` expression inside `run:`; pass it through `env:` instead")
        if run_indent is None:
            m = re.match(r"^(\s*)(?:- )?run:\s*(.*)$", l)
            if m:
                base = len(m.group(1)) + (2 if s.startswith("- ") else 0)
                if m.group(2) in ("|", ">", "|-", ">-", "|+", ">+"):
                    run_indent = base
                elif "${{" in m.group(2):
                    err(f, n, "`${{ }}` expression inside `run:`; pass it through `env:` instead")
        # checkout persist-credentials
        m = re.search(r"uses:\s*(\S+)", l)
        if m and not s.startswith("#"):
            ref = s.split("uses:", 1)[1].strip()
            if ref.startswith("./"):
                pass  # local action
            elif ref.startswith("docker://"):
                if "@sha256:" not in ref:
                    err(f, n, "docker:// action must be pinned by @sha256 digest")
            elif not PIN.match(ref):
                err(f, n, f"action not pinned by full commit SHA with a version comment: {ref}")
            if ref.startswith("actions/checkout@"):
                # look ahead in this step for persist-credentials: false
                step_indent = indent(l)
                ok = False
                for k in range(n, len(lines)):
                    nxt = lines[k]
                    if nxt.strip() and indent(nxt) < step_indent and not nxt.lstrip().startswith("#"):
                        break
                    if re.match(r"^\s*- ", nxt) and indent(nxt) <= step_indent and k >= n:
                        break
                    if re.match(r"^\s*persist-credentials:\s*false\b", nxt):
                        ok = True
                        break
                if not ok:
                    err(f, n, "checkout without `persist-credentials: false`")
        if re.search(r"\bsecrets\.", l) and not s.startswith("#"):
            err(f, n, "`secrets.` reference; workflows must not need secrets")
        if re.search(r"\bpull_request_target\b|\bworkflow_run\b", l) and not s.startswith("#"):
            err(f, n, "pull_request_target / workflow_run are not allowed")

for e in errors:
    print(e)
if errors:
    sys.exit(1)
print(f"workflow rules: {len(files)} workflow(s) pass")
