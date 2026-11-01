#!/usr/bin/env python3
"""Which lanes of the offline matrix a change needs.

Prints `rust=`, `crates=`, `lib=`, `release=` and `powershell=` lines
(true/false) for $GITHUB_OUTPUT. Everything runs unless the changed files
prove a lane cannot be affected:

- rust: false when every changed file is inert -- under `editors/` (its own
  workflow) or a Markdown file whose path no source, test, profile or tool
  names (tests read some documents, e.g. evidence a profile cites);
- crates: false when the workspace crates cannot see the change: every file
  is inert or root-only (`src/`, the root's own `tests/*.rs`,
  `tests/common/`, `tests/support/`, the Python test scripts); crates read
  `tests/fixtures/` and `profiles/`, so those are not root-only;
- lib: false when the root library's unit tests cannot see it: every file is
  inert, an integration suite (`tests/*.rs`, `tests/support/`) or a Python
  test script (`tests/scripts/`); the library includes `tests/common/`
  fixtures through `#[path]`, so those count.

Two lanes depend on the event rather than only on the files:

- release: the release build and the checks of that binary (GLIBC audit,
  SBOM, release-boundary audit, packaging). A pull request runs it only on
  the matrix entries that pass --pr-release (Linux x86_64), when `rust` runs;
  pushes and manual runs build it on every entry. The other entries of a
  pull request check native micro-corpus parity on the debug binary the
  integration suites built.
- powershell: the EDT corpus validator under Windows PowerShell 5.1. A pull
  request runs it when it changes the validator, its Pester test, the corpus
  it validates or the workflow that runs it; PowerShell 7 runs the same
  validator on every pull request and every target.

Usage: ci_scope.py [--base <rev>] [--all] [--pr-release]. Without a base,
or with --all (pushes to master, manual runs), every lane runs.
"""
import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# Where a document's path may be named by something a test reads.
REFERENCE_DIRS = ["src", "tests", "crates", "profiles", "compatibility", "tools", "xtask"]
# What the Windows PowerShell 5.1 run of the corpus validator depends on.
POWERSHELL_INPUTS = (
    "tools/validate-edt-corpus.ps1",
    "tests/scripts/validate-edt-corpus.Tests.ps1",
    "crates/ibcmd-schema/data/",
    ".github/workflows/offline-e2e.yml",
)


def changed_files(base):
    out = subprocess.run(
        ["git", "diff", "--name-only", f"{base}...HEAD"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return [line for line in out.splitlines() if line]


def referenced(path):
    """Whether any file under REFERENCE_DIRS names `path`."""
    dirs = [d for d in REFERENCE_DIRS if (ROOT / d).exists()]
    result = subprocess.run(
        ["git", "grep", "-q", "-F", "-e", path, "--", *dirs],
        cwd=ROOT, capture_output=True,
    )
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.returncode == 0


def inert(path):
    if path.startswith("editors/"):
        return True
    return path.endswith(".md") and not referenced(path)


def root_suite(path):
    parts = path.split("/")
    return parts[0] == "tests" and (
        (len(parts) == 2 and path.endswith(".rs")) or parts[1] in ("common", "support")
    )


def scope(files):
    if not files:
        return dict(rust=False, crates=False, lib=False)
    rust = not all(inert(f) for f in files)
    crates = rust and not all(
        inert(f) or f.startswith(("src/", "tests/scripts/")) or root_suite(f) for f in files
    )
    lib = rust and not all(
        inert(f)
        or (root_suite(f) and not f.startswith("tests/common/"))
        or f.startswith("tests/scripts/")
        for f in files
    )
    return dict(rust=rust, crates=crates, lib=lib)


def powershell(files):
    return any(f == p or (p.endswith("/") and f.startswith(p)) for f in files for p in POWERSHELL_INPUTS)


def lanes(files, pr_release):
    """The lanes of a pull request whose changed files are `files`."""
    result = scope(files)
    result["release"] = result["rust"] and pr_release
    result["powershell"] = powershell(files)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base")
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--pr-release", action="store_true",
                        help="this matrix entry builds the release binary on pull requests")
    args = parser.parse_args()
    if args.all or not args.base:
        result = dict(rust=True, crates=True, lib=True, release=True, powershell=True)
    else:
        files = changed_files(args.base)
        result = lanes(files, args.pr_release)
        print(f"{len(files)} changed files", file=sys.stderr)
    for name, value in result.items():
        print(f"{name}={'true' if value else 'false'}")


if __name__ == "__main__":
    main()
