#!/usr/bin/env python3
"""Pinned public Windows Plan gate: one compile, first positive, then serial suite."""
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
import time

FUNCTIONAL = "21ca0c9bc2e270e11a49d1c7cc671076205efc18"
FUNCTIONAL_TREE = "d2dc0515c95f8a84da1c4bcfdc309b583a7aff19"
HARNESS = "tests/editor_partial_import_windows.rs"
HARNESS_BLOB = "e8fe7ffed05192ccd1a661aae367473259f339de"
CONTROL = "src/mssql_source_change/files/windows_custody_tests.rs"
CONTROL_BLOB = "88990942c256850a0449214923d7ee7190332cf7"
GZIP_SHA = "090455ade0bae8bb328bce6890db1d1dd4be204f3dce1081cc4f13da65652809"
DATABASE_SHA = "02e7daca5be7329e505d1dadcf5f1ea762f9dd735e926fe4ab1f8a16daaf1646"
POSITIVE = "windows_two_stage_plan_preserves_config_complete_stage_and_backup"
PARENTS = [POSITIVE] + ["windows_plan_" + name for name in [
    "refuses_selected_source_edit", "refuses_unselected_file_add",
    "refuses_unselected_file_remove", "refuses_unselected_file_rename",
    "refuses_empty_directory_add", "refuses_empty_directory_remove",
    "refuses_empty_directory_rename", "refuses_external_configsave_change",
    "refuses_changed_current_target", "refuses_same_bytes_new_target_identity",
    "preserves_preexisting_work_artifact", "preserves_preexisting_recovery_artifact",
    "final_policy_refuses_without_publication", "late_cancellation_remains_typed",
    "source_budget_refuses_before_writes", "target_budget_refuses_before_writes",
    "committed_recovery_error_retains_original",
]]
ALLOWED = {
    b"tools/run-windows-partial-plan-ci.py", b".github/workflows/windows-partial-plan.yml",
    b"tools/run-source-custody-ci.py", b".github/workflows/source-custody.yml",
}
LOG_LIMIT = 32 * 1024 * 1024
CHILD_LOG_LIMIT = 16 * 1024 * 1024


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def require_allowed_head_changes(raw):
    if raw and not raw.endswith(b"\0"):
        raise RuntimeError("unterminated HEAD path receipt")
    for path in raw.split(b"\0")[:-1]:
        if path not in ALLOWED and not path.startswith(b"openspec/changes/verify-windows-partial-plan-integration/"):
            raise RuntimeError("HEAD source/harness differs from reviewed functional pin")


def head_diff_args(before, after):
    return ["git", "diff", "--no-renames", "--name-only", "-z", before, after, "--"]


def classify(stdout, code, name):
    # nocapture can put the first child progress line after "test NAME ... ".
    # Exact selected name plus one non-ignored passing summary bind that result.
    selected = re.findall(r"(?m)^test (\S+) \.\.\. ", stdout)
    summaries = re.findall(r"(?m)^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;", stdout)
    if (code != 0 or len(re.findall(r"(?m)^running 1 test\s*$", stdout)) != 1
            or selected != [name] or summaries != [("ok", "1", "0", "0")]):
        raise RuntimeError("missing exact non-ignored passing parent/child result")


def phases_for(name):
    if name == POSITIVE:
        return ["stage1", "inspect1", "stage2", "inspect2"]
    if name.endswith("committed_recovery_error_retains_original"):
        return ["publish-recovery", "inspect-recovery"]
    if name.endswith("final_policy_refuses_without_publication") or name.endswith("late_cancellation_remains_typed"):
        return ["final-gate"]
    if "_budget_" in name:
        return ["budget"]
    return ["refusal"]


def verify_recovery(probe, published):
    flags = ["without_handle_rename_passed", "held_rename_denied", "after_drop_rename_passed",
             "held_target_preserved", "held_source_preserved", "same_parent_attrs_match", "held_attrs_unchanged"]
    code = probe.get("denial_raw_os_error")
    if (not all(probe.get(flag) is True for flag in flags)
            or type(code) is not int or code not in (5, 32)
            or published.get("published") is not True
            or type(published.get("raw_os_error")) is not int or published["raw_os_error"] != code
            or not re.fullmatch(r"[0-9a-f]{64}", published.get("original_sha256", ""))
            or published["original_sha256"] != published.get("recovery_sha256")):
        raise RuntimeError("unproven differential denial or committed recovery image")


def verify_case(temporary, name, executable_sha):
    roots = list(temporary.glob("ibcmd-windows-plan-*"))
    if len(roots) != 1:
        raise RuntimeError("missing/ambiguous retained parent fixture")
    root = roots[0].resolve()
    root.relative_to(temporary.resolve())
    marker = json.loads((root / "owned-case.json").read_text(encoding="utf-8"))
    if (marker["test"] != name or marker["executable_sha256"] != executable_sha
            or marker["compressed_sha256"] != GZIP_SHA or marker["database_sha256"] != DATABASE_SHA
            or root.name != "ibcmd-windows-plan-" + marker["nonce"]):
        raise RuntimeError("fixture/test/executable provenance mismatch")
    phases = phases_for(name)
    if {p.name for p in root.glob("child-*")} != {"child-" + p for p in phases}:
        raise RuntimeError("missing/unexpected exact child phase")
    for phase in phases:
        logs = root / ("child-" + phase)
        process = json.loads((logs / "process.json").read_text(encoding="utf-8"))
        complete = json.loads((logs / "completed.json").read_text(encoding="utf-8"))
        if (process["test"] != name or process["phase"] != phase or process["exit"] != 0
                or process["executable_sha256"] != executable_sha
                or process["timed_out"] or process["output_cap_exceeded"]
                or process["timeout_seconds"] != 300 or process["stream_budget_bytes"] != CHILD_LOG_LIMIT
                or complete != {"nonce": marker["nonce"], "test": name, "phase": phase,
                                "executable_sha256": executable_sha}):
            raise RuntimeError("child completion/limits receipt mismatch")
        for stream in ["stdout", "stderr"]:
            path = logs / (stream + ".txt")
            if path.stat().st_size > CHILD_LOG_LIMIT or digest(path) != process[stream + "_sha256"]:
                raise RuntimeError("child raw stream mismatch or budget exceeded")
        classify((logs / "stdout.txt").read_text(encoding="utf-8"), process["exit"], name)
    if name == POSITIVE:
        for selected in ["descriptor", "module"]:
            published = json.loads((root / ("published-" + selected + ".json")).read_text(encoding="utf-8"))
            if (published["table"] != "CONFIGSAVE" or not published["pending"] or published["rows_after"] <= 0
                    or published["before_sha256"] == published["after_sha256"]
                    or published["before_sha256"] != published["backup_sha256"]):
                raise RuntimeError("positive did not publish an actual complete pending stage/backup")
    if name.endswith("committed_recovery_error_retains_original"):
        verify_recovery(json.loads((root / "recovery-denial-probe.json").read_text(encoding="utf-8")),
                        json.loads((root / "committed-recovery.json").read_text(encoding="utf-8")))
    return {"fixture": str(root), "nonce": marker["nonce"], "child_phases": phases}


def guard_self_test():
    parent = os.environ.get("RUNNER_TEMP") or os.environ.get("TMPDIR")
    if not parent:
        raise RuntimeError("self-test requires owned temporary root")
    fixture = Path(tempfile.mkdtemp(prefix="windows-partial-head-guard-", dir=parent))
    template = fixture / "empty-template"
    template.mkdir()
    env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")

    def git(*args):
        return subprocess.check_output(["git", *args], cwd=fixture, env=env)

    git("-c", "init.templateDir=" + str(template), "init", "--quiet")
    source = fixture / "src/file_import.rs"
    source.parent.mkdir()
    source.write_text("// public synthetic protected source\n", encoding="utf-8")
    git("add", "--", ".")
    before = git("write-tree").decode().strip()
    target = fixture / "openspec/changes/verify-windows-partial-plan-integration/moved.rs"
    target.parent.mkdir(parents=True)
    source.rename(target)
    git("add", "--all")
    after = git("write-tree").decode().strip()
    old = git("-c", "diff.renames=true", "diff", "--name-only", "-z", before, after, "--")
    require_allowed_head_changes(old)
    fixed = git("-c", "diff.renames=true", *head_diff_args(before, after)[1:])
    if b"src/file_import.rs" not in fixed.split(b"\0"):
        raise RuntimeError("guard fixture failed to expose source deletion")
    try:
        require_allowed_head_changes(fixed)
    except RuntimeError:
        (fixture / "receipt.json").write_text(json.dumps({
            "before": before, "after": after, "old_admitted": True,
            "protected_rename_refused": True,
            "paths": fixed.decode().split("\0")[:-1],
        }, indent=2), encoding="utf-8")
        return
    raise RuntimeError("protected source rename was admitted")


def self_test():
    name = "example::exact"
    plain = f"running 1 test\ntest {name} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;"
    progress = plain.replace("... ok", "... child progress\nok")
    classify(plain, 0, name)
    classify(progress, 0, name)
    bad = [(plain, 1), (plain.replace("running 1 test", "running 0 tests"), 0),
           (plain.replace("0 ignored", "1 ignored"), 0),
           (plain.replace(name, "other::test"), 0), (plain + "\n" + plain, 0)]
    for output, code in bad:
        try:
            classify(output, code, name)
        except RuntimeError:
            continue
        raise RuntimeError("classifier accepted an invalid result")
    for raw in [b"src/file_import.rs\0", b"tests/editor_partial_import_windows.rs\0",
                b"src/deleted.rs\0openspec/changes/verify-windows-partial-plan-integration/moved.rs\0",
                b"unterminated"]:
        try:
            require_allowed_head_changes(raw)
        except RuntimeError:
            continue
        raise RuntimeError("guard accepted an unreviewed path")
    probe = {flag: True for flag in ["without_handle_rename_passed", "held_rename_denied",
             "after_drop_rename_passed", "held_target_preserved", "held_source_preserved",
             "same_parent_attrs_match", "held_attrs_unchanged"]}
    probe["denial_raw_os_error"] = 5
    published = {"published": True, "raw_os_error": 5,
                 "original_sha256": "a" * 64, "recovery_sha256": "a" * 64}
    verify_recovery(probe, published)
    verify_recovery(dict(probe, denial_raw_os_error=32), dict(published, raw_os_error=32))
    for bad_probe, bad_published in [
        (dict(probe, without_handle_rename_passed=False), published),
        (dict(probe, after_drop_rename_passed=False), published),
        (dict(probe, denial_raw_os_error=13), dict(published, raw_os_error=13)),
        (probe, dict(published, raw_os_error=32)),
        (probe, dict(published, published=False)),
        (probe, dict(published, recovery_sha256="b" * 64)),
    ]:
        try:
            verify_recovery(bad_probe, bad_published)
        except RuntimeError:
            continue
        raise RuntimeError("recovery verifier accepted unproven publication")
    guard_self_test()
    print("classifier 2 valid/5 refusal; path guard 4 refusal; actual Git rename refusal; recovery 2 valid/6 refusal: PASS")


class Gate:
    def __init__(self):
        self.checkout = Path(__file__).resolve().parents[1]
        parent = Path(os.environ["RUNNER_TEMP"]).resolve()
        self.work = parent / "windows-partial-plan-work"
        self.evidence = parent / "windows-partial-plan-evidence"
        self.work.mkdir()
        self.evidence.mkdir()
        self.deadline = time.monotonic() + 3300
        self.receipt = {"complete": False, "os": platform.platform(), "python": sys.version,
                        "run_id": os.environ.get("GITHUB_RUN_ID"),
                        "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
                        "image_os": os.environ.get("ImageOS"),
                        "image_version": os.environ.get("ImageVersion"), "commands": []}
        self.save()

    def save(self):
        (self.evidence / "receipt.json").write_text(json.dumps(self.receipt, indent=2), encoding="utf-8")

    def run(self, args, label, cwd=None, env=None, timeout=120):
        stem = f"{len(self.receipt['commands']):02d}-{label}"
        stdout = self.evidence / (stem + ".stdout.txt")
        stderr = self.evidence / (stem + ".stderr.txt")
        limit = min(timeout, max(0, self.deadline - time.monotonic()))
        if limit <= 0:
            raise RuntimeError("overall gate deadline exceeded")
        started = time.monotonic()
        timed_out = False
        capped = False
        kill_receipt = None
        with stdout.open("xb") as out, stderr.open("xb") as err:
            process = subprocess.Popen(args, cwd=cwd or self.checkout, env=env, stdout=out, stderr=err)
            while True:
                try:
                    process.wait(timeout=0.2)
                    break
                except subprocess.TimeoutExpired:
                    timed_out = time.monotonic() - started >= limit
                    capped = stdout.stat().st_size > LOG_LIMIT or stderr.stat().st_size > LOG_LIMIT
                    if timed_out or capped:
                        # Only this runner's live owned parent tree is stopped.
                        if process.poll() is None:
                            killed = subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                                    capture_output=True, timeout=30)
                            kill_receipt = {"exit": killed.returncode,
                                            "stdout": killed.stdout.decode(errors="replace"),
                                            "stderr": killed.stderr.decode(errors="replace")}
                            if process.poll() is None:
                                process.kill()
                        process.wait(timeout=30)
                        break
        capped |= stdout.stat().st_size > LOG_LIMIT or stderr.stat().st_size > LOG_LIMIT
        command = {"args": args, "cwd": str(cwd or self.checkout), "exit": process.returncode,
                   "log_stem": stem, "timed_out": timed_out, "output_cap_exceeded": capped,
                   "timeout_seconds": limit, "stream_budget_bytes": LOG_LIMIT,
                   "stdout_sha256": digest(stdout), "stderr_sha256": digest(stderr),
                   "kill_tree": kill_receipt}
        self.receipt["commands"].append(command)
        self.save()
        if timed_out or capped or process.returncode:
            raise RuntimeError("command failed/timed out/exceeded polled log budget: " + label)
        return stdout.read_bytes()

    def checked(self, args, label, **kwargs):
        return self.run(args, label, **kwargs).decode("utf-8", errors="strict").strip()

    def execute(self):
        head = self.checked(["git", "rev-parse", "HEAD"], "head")
        self.receipt.update(head=head, functional=FUNCTIONAL, functional_tree=FUNCTIONAL_TREE)
        self.checked(["git", "merge-base", "--is-ancestor", FUNCTIONAL, head], "ancestor")
        raw = self.run(head_diff_args(FUNCTIONAL, head), "head-diff")
        require_allowed_head_changes(raw)
        if self.checked(["git", "rev-parse", FUNCTIONAL + "^{tree}"], "functional-tree") != FUNCTIONAL_TREE:
            raise RuntimeError("functional tree mismatch")
        for path, expected in [(HARNESS, HARNESS_BLOB), (CONTROL, CONTROL_BLOB)]:
            if self.checked(["git", "rev-parse", FUNCTIONAL + ":" + path], "blob-" + Path(path).stem) != expected:
                raise RuntimeError("protected source/test blob mismatch")
        self.receipt["rustc"] = self.checked(["rustc", "-Vv"], "rustc")
        if "release: 1.95.0" not in self.receipt["rustc"] or "host: x86_64-pc-windows-msvc" not in self.receipt["rustc"]:
            raise RuntimeError("unpinned compiler or non-Windows host")
        source = self.work / "source"
        self.checked(["git", "worktree", "add", "--detach", str(source), FUNCTIONAL], "checkout")
        compile_tmp = self.work / "compile-tmp"
        compile_tmp.mkdir()
        env = dict(os.environ, CARGO_BUILD_JOBS="1", CARGO_TERM_COLOR="never", IBCMD_TEST_KEEP_FIXTURES="1",
                   CARGO_TARGET_DIR=str(self.work / "target"),
                   TMP=str(compile_tmp), TEMP=str(compile_tmp), TMPDIR=str(compile_tmp))
        env["RUSTC"] = self.checked(["rustup", "which", "rustc"], "rustc-path")
        self.receipt["environment"] = {k: env[k] for k in ["RUSTC", "CARGO_BUILD_JOBS", "CARGO_TARGET_DIR", "TMP", "IBCMD_TEST_KEEP_FIXTURES"]}
        output = self.run(["cargo", "test", "--locked", "-p", "ibcmd-rs", "--test", "editor_partial_import_windows",
                           "--no-default-features", "--no-run", "--message-format=json"],
                          "compile", cwd=source, env=env, timeout=2400)
        events = [json.loads(line) for line in output.decode("utf-8").splitlines() if line.strip()]
        artifacts = [e for e in events if e.get("reason") == "compiler-artifact" and e.get("executable")
                     and e.get("profile", {}).get("test") and e.get("target", {}).get("kind") == ["test"]
                     and e["target"]["name"] == "editor_partial_import_windows"
                     and Path(e["target"]["src_path"]).resolve() == (source / HARNESS).resolve()]
        if len(artifacts) != 1 or not any(e.get("reason") == "build-finished" and e.get("success") for e in events):
            raise RuntimeError("missing/ambiguous integration compiler artifact")
        executable = Path(artifacts[0]["executable"]).resolve()
        executable.relative_to(Path(env["CARGO_TARGET_DIR"]).resolve())
        executable_sha = digest(executable)
        self.receipt.update(executable=str(executable), executable_sha256=executable_sha,
                            executable_bytes=executable.stat().st_size)
        self.save()
        listing = self.checked([str(executable), "--list"], "list", cwd=source, env=env)
        names = re.findall(r"(?m)^(\S+): test$", listing)
        if len(names) != 18 or set(names) != set(PARENTS):
            raise RuntimeError("actual Windows target does not expose all18 expected parent controls")
        fixtures = self.evidence / "fixtures"
        fixtures.mkdir()
        self.receipt["results"] = []
        # The dedicated gate cannot enter its full18 suite before this probe.
        selections = [("probe-positive", POSITIVE)] + [(f"suite-{i:02d}", name) for i, name in enumerate(PARENTS)]
        for label, name in selections:
            temporary = fixtures / label
            temporary.mkdir()
            test_env = dict(env, TMP=str(temporary), TEMP=str(temporary), TMPDIR=str(temporary))
            out = self.run([str(executable), "--exact", name, "--nocapture", "--test-threads=1", "--color", "never"],
                           label, cwd=source, env=test_env, timeout=1200 if name == POSITIVE else 600)
            classify(out.decode("utf-8"), 0, name)
            case = verify_case(temporary, name, executable_sha)
            self.receipt["results"].append({"label": label, "test": name, "outcome": "pass", **case})
            if label == "probe-positive":
                self.receipt["positive_probe_passed"] = True
            self.save()
            print(f"{label}: {name}: PASS", flush=True)
        self.receipt.update(complete=True, suite_passed=18, positive_probe_passed=True)
        self.save()


def main(mode):
    if mode == "self-test":
        self_test()
        return
    if os.name != "nt":
        raise RuntimeError("actual Plan runtime gate requires Windows")
    gate = Gate()
    try:
        gate.execute()
    except Exception as error:
        gate.receipt["error"] = str(error)
        gate.save()
        raise


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["self-test", "windows"])
    try:
        main(parser.parse_args().mode)
    except Exception as error:
        print(f"Windows partial Plan gate refused: {error}", file=sys.stderr)
        sys.exit(1)
