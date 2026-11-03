#!/usr/bin/env python3
"""Pinned source-custody controls; no platform, SQL, VM or user fixture input."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile

BASE = "6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4"
BASELINE = "751751f8f5670d84cf2d96aab86895b6c5013d83"
FIXED = "63637dbe9914709e57ff64af56b1d8a7fba27a22"
FIXED_TREE = "6a73a7710683d0b9532b981e882e8d433dc1c983"
CONTROL_FILE = "src/mssql_source_change/files/windows_custody_tests.rs"
PREFIX = "mssql_source_change::files::windows_custody_tests::"
CUSTODY = [
    PREFIX + "windows_source_directories_deny_delete_without_any_leaf_handle",
    PREFIX + "windows_preexisting_ancestor_delete_access_refuses_source_custody",
    PREFIX + "windows_zero_access_directory_probe_is_not_rename_custody",
    PREFIX + "windows_directory_anchor_preserves_idle_child_edits_without_leaf_custody",
]
RED = [
    "expected a sharing denial, not a permissive metadata handle",
    "existing ancestor DELETE access was not excluded",
]
PROFILE = ["mssql_source_change::files::windows_profile::tests::" + name for name in [
    "source_profile_refuses_remote_nonfixed_unknown_and_non_ntfs_facts",
    "source_member_volume_is_bound_to_source_root_not_a_target",
    "canonical_root_namespace_is_bounded_without_claiming_raw_operator_policy",
]]
REGRESSION = [
    "mssql_source_change::original_source_tests::windows_required_original_and_ancestors_exclude_write_delete_share",
    "mssql_source_change::original_source_tests::windows_detached_compiler_cache_retains_bytes_and_rejects_later_mutation",
] + ["mssql_source_change::traversal_budget_tests::" + name for name in [
    "consumed_cache_revalidates_file_and_ancestor_witnesses",
    "consumed_cache_rejects_ancestor_rebinding_with_unchanged_leaf",
    "empty_directory_growth_is_bounded_during_initial_and_final_capture",
    "depth_limit_refuses_instead_of_truncating_the_census",
]]
LOW_FD = "mssql_source_change::traversal_budget_tests::source_xml_custody_is_bounded_under_low_fd_limit"


def classify(output, code, name, expected_red=None):
    """A process exit alone cannot prove the intended test or counterexample."""
    if not re.search(r"(?m)^running 1 test\s*$", output):
        raise RuntimeError("missing exact single-test selection")
    if expected_red:
        valid = (code == 101 and expected_red in output
                 and f"test {name} ... FAILED" in output
                 and re.search(r"test result: FAILED\. 0 passed; 1 failed; 0 ignored;", output))
    else:
        valid = (code == 0 and f"test {name} ... ok" in output
                 and re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", output))
    if not valid:
        raise RuntimeError("unexpected test outcome; inspect retained stdout/stderr")



def head_diff_args(before, after):
    # Renames must expose the deleted source as well as the added destination.
    return ["git", "diff", "--no-renames", "--name-only", "-z", before, after, "--"]


def require_allowed_head_changes(raw):
    if raw and not raw.endswith(b"\0"):
        raise RuntimeError("unterminated HEAD path receipt")
    allowed = [b"tools/run-source-custody-ci.py", b".github/workflows/source-custody.yml"]
    for path in raw.split(b"\0")[:-1]:
        if path not in allowed and not path.startswith(b"openspec/changes/enforce-windows-source-custody/"):
            raise RuntimeError("HEAD production differs from pinned fixed candidate; update/review pins")


def head_guard_self_test():
    fixture = Path(tempfile.mkdtemp(prefix="source-custody-head-guard-",
                                   dir=os.environ.get("RUNNER_TEMP") or os.environ.get("TMPDIR")))
    # No commits, identities, hooks, remote, global config changes or teardown.
    template = fixture / "empty-template"
    template.mkdir()
    env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")

    def git(*args):
        return subprocess.check_output(["git", *args], cwd=fixture, env=env)

    git("-c", "init.templateDir=" + str(template), "init", "--quiet")
    source = fixture / "src/mssql_source_change/files/windows_profile.rs"
    source.parent.mkdir(parents=True)
    source.write_text("// public synthetic guard fixture\n", encoding="utf-8")
    git("add", "--", ".")
    before = git("write-tree").decode().strip()
    destination = fixture / "openspec/changes/enforce-windows-source-custody/moved-profile.rs"
    destination.parent.mkdir(parents=True)
    source.rename(destination)
    git("add", "--all")
    after = git("write-tree").decode().strip()
    # Prove that this fixture discriminates the old name-only guard.
    old = git("-c", "diff.renames=true", "diff", "--name-only", "-z", before, after, "--")
    require_allowed_head_changes(old)
    raw = git("-c", "diff.renames=true", *head_diff_args(before, after)[1:])
    if source.relative_to(fixture).as_posix().encode() not in raw.split(b"\0")[:-1]:
        raise RuntimeError("guard regression did not expose the protected deletion")
    try:
        require_allowed_head_changes(raw)
    except RuntimeError:
        (fixture / "guard-receipt.json").write_text(json.dumps({
            "before_tree": before, "after_tree": after, "diff.renames": True,
            "old_paths": old.decode().split("\0")[:-1],
            "guard_paths": raw.decode().split("\0")[:-1],
            "old_guard_admitted": True, "fixed_guard_refused": True,
        }, indent=2), encoding="utf-8")
        print("HEAD guard: protected-source to allowed-doc rename refused; fixture retained")
        return
    raise RuntimeError("HEAD guard accepted a protected production rename")


def self_test():
    name = "example::exact"
    passed = f"running 1 test\ntest {name} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;"
    failed = f"running 1 test\ntest {name} ... FAILED\n{RED[0]}\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;"
    classify(passed, 0, name)
    classify(failed, 101, name, RED[0])
    for text, code, assertion in [
        (passed.replace("running 1 test", "running 0 tests"), 0, None),
        (passed.replace("0 ignored", "1 ignored"), 0, None),
        (failed.replace(RED[0], "unrelated ACL error"), 101, RED[0]),
        (failed, 1, RED[0]), (passed, 0, RED[0]),
    ]:
        try:
            classify(text, code, name, assertion)
        except RuntimeError:
            continue
        raise RuntimeError("outcome verifier accepted an invalid receipt")
    print("outcome verifier: 2 valid + 5 refusal controls passed")
    head_guard_self_test()


def main(mode):
    if mode == "self-test":
        self_test()
        return
    if (mode == "paired-windows") != (os.name == "nt"):
        raise RuntimeError("execution mode does not match this host")
    checkout = Path(__file__).resolve().parents[1]
    runner_temp = Path(os.environ["RUNNER_TEMP"]).resolve()
    work = runner_temp / ("source-custody-work-" + mode)
    evidence = runner_temp / ("source-custody-evidence-" + mode)
    # Refuse reuse rather than removing a previous run's evidence or targets.
    work.mkdir()
    evidence.mkdir()
    receipt = {"mode": mode, "os": platform.platform(), "python": sys.version,
               "run_id": os.environ.get("GITHUB_RUN_ID"),
               "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
               "image_os": os.environ.get("ImageOS"),
               "image_version": os.environ.get("ImageVersion"), "commands": []}

    def save():
        (evidence / "receipt.json").write_text(json.dumps(receipt, indent=2), encoding="utf-8")

    def run(args, cwd=checkout, env=None, label="command"):
        result = subprocess.run(args, cwd=cwd, env=env, capture_output=True)
        ordinal = len(receipt["commands"])
        stem = f"{ordinal:02d}-{label}"
        for stream in ["stdout", "stderr"]:
            (evidence / (stem + "." + stream + ".txt")).write_bytes(getattr(result, stream))
        receipt["commands"].append({"args": args, "cwd": str(cwd), "exit": result.returncode,
                                    "log_stem": stem})
        save()
        return result

    def checked(args, **kwargs):
        result = run(args, **kwargs)
        if result.returncode:
            raise RuntimeError(f"command failed with exit {result.returncode}: {args[0]}")
        return result.stdout.decode("utf-8", errors="strict").strip()

    head = checked(["git", "rev-parse", "HEAD"])
    receipt["head"] = head
    checked(["git", "merge-base", "--is-ancestor", FIXED, head])
    changed = run(head_diff_args(FIXED, head), label="head-diff")
    if changed.returncode:
        raise RuntimeError("HEAD path inspection failed")
    require_allowed_head_changes(changed.stdout)
    if checked(["git", "rev-parse", FIXED + "^{tree}"]) != FIXED_TREE:
        raise RuntimeError("fixed tree mismatch")
    checked(["git", "merge-base", "--is-ancestor", BASE, BASELINE])
    blobs = [checked(["git", "rev-parse", ref + ":" + CONTROL_FILE]) for ref in [BASELINE, FIXED]]
    if blobs[0] != blobs[1]:
        raise RuntimeError("baseline/fixed isolated controls differ")
    receipt["control_blob"] = blobs[0]
    receipt["rustc"] = checked(["rustc", "-Vv"])
    expected_host = "x86_64-pc-windows-msvc" if mode == "paired-windows" else "x86_64-unknown-linux-gnu"
    if "release: 1.95.0" not in receipt["rustc"] or f"host: {expected_host}" not in receipt["rustc"]:
        raise RuntimeError("unpinned compiler or host")
    save()
    phases = [("baseline", BASELINE), ("fixed", FIXED)] if mode == "paired-windows" else [("fixed-unix", FIXED)]
    for phase, revision in phases:
        source = work / phase
        checked(["git", "worktree", "add", "--detach", str(source), revision], label=phase + "-checkout")
        phase_evidence = evidence / phase
        phase_evidence.mkdir()
        compile_temp = work / (phase + "-compile-temp")
        compile_temp.mkdir()
        fixture_temp = phase_evidence / "fixtures"
        fixture_temp.mkdir()
        env = dict(os.environ, CARGO_TARGET_DIR=str(work / (phase + "-target")),
                   CARGO_BUILD_JOBS="1", CARGO_TERM_COLOR="never", IBCMD_TEST_KEEP_FIXTURES="1",
                   TMP=str(compile_temp), TEMP=str(compile_temp), TMPDIR=str(compile_temp))
        output = checked(["cargo", "test", "--locked", "-p", "ibcmd-rs", "--lib",
                          "--no-default-features", "--no-run", "--message-format=json"],
                         cwd=source, env=env, label=phase + "-compile")
        executables = set()
        for line in output.splitlines():
            event = json.loads(line)
            if (event.get("reason") == "compiler-artifact" and event.get("executable")
                    and event.get("profile", {}).get("test")
                    and event.get("target", {}).get("kind") == ["lib"]
                    and Path(event["target"]["src_path"]).resolve() == (source / "src/lib.rs").resolve()):
                executables.add(event["executable"])
        if len(executables) != 1:
            raise RuntimeError("missing/ambiguous root lib test executable")
        executable = Path(executables.pop()).resolve()
        executable.relative_to(Path(env["CARGO_TARGET_DIR"]).resolve())
        receipt[phase] = {"commit": revision,
                          "tree": checked(["git", "rev-parse", revision + "^{tree}"]),
                          "harness_sha256": hashlib.file_digest(executable.open("rb"), "sha256").hexdigest()}
        save()
        env.update(TMP=str(fixture_temp), TEMP=str(fixture_temp), TMPDIR=str(fixture_temp))
        names = CUSTODY if phase == "baseline" else CUSTODY + PROFILE + REGRESSION
        if mode == "unix-low-fd":
            names = [LOW_FD]
        for index, name in enumerate(names):
            assertion = RED[index] if phase == "baseline" and index < 2 else None
            result = run([str(executable), name, "--exact", "--nocapture", "--test-threads=1"],
                         cwd=source, env=env, label=phase + f"-test-{index:02d}")
            combined = (result.stdout + result.stderr).decode("utf-8", errors="replace")
            classify(combined, result.returncode, name, assertion)
            receipt["commands"][-1]["verified_outcome"] = "expected_assertion_red" if assertion else "pass"
            save()
            print(f"{phase}: {name}: {'expected assertion RED' if assertion else 'PASS'}", flush=True)
    receipt["complete"] = True
    save()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["self-test", "paired-windows", "unix-low-fd"])
    try:
        main(parser.parse_args().mode)
    except Exception as error:
        print(f"source-custody gate refused: {error}", file=sys.stderr)
        sys.exit(1)
