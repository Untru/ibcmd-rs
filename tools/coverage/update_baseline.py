#!/usr/bin/env python3
"""Adds the variants the coverage test reports as newly equal to their sets'
`baseline.json`.

    cargo test --locked -p ibcmd-rs --test coverage_corpus --no-default-features \
        -- --nocapture | python3 tools/coverage/update_baseline.py

Only additions: a variant leaves a baseline by hand, with the reason in the PR.
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "coverage"


def set_dirs():
    out = {}
    for manifest in ROOT.rglob("manifest.json"):
        out[json.loads(manifest.read_text(encoding="utf-8"))["set"]] = manifest.parent
    return out


def main():
    dirs = set_dirs()
    current = None
    added = {}
    for line in sys.stdin:
        if m := re.match(r"coverage (\S+): ", line):
            current = m.group(1)
        elif (m := re.match(r"\s+newly equal \(add to baseline\.json\): (\S+)", line)) and current:
            added.setdefault(current, []).append(m.group(1))
    for name, variants in sorted(added.items()):
        path = dirs[name] / "baseline.json"
        equal = set(json.loads(path.read_text(encoding="utf-8"))["equal"]) if path.exists() else set()
        equal.update(variants)
        path.write_text(json.dumps({"equal": sorted(equal)}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"{name}: +{len(variants)}", file=sys.stderr)


if __name__ == "__main__":
    main()
