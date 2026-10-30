"""Windows CI entry: fixed suite; no arbitrary command/test selection."""
import hashlib
import json
import os
from pathlib import Path
import sys
import unittest

from windows_gate_support import CappedLog, PublicResult, admit_artifacts, verified_sources

EXPECTED = {
    "WindowsOrderingModel.test_eof_waits_for_normal_exit_before_closing_job",
    "WindowsOrderingModel.test_eof_does_not_turn_a_stalled_exit_into_success",
    "WindowsOrderingModel.test_failed_job_assignment_reaps_peer_without_bootstrap",
    "WindowsOrderingModel.test_stale_code_seal_refuses_before_process_creation",
    "WindowsKernelControls.test_fixed_argv_and_real_job_membership_before_bootstrap",
    "WindowsKernelControls.test_descendant_handle_is_signalled_after_job_close",
    "WindowsKernelControls.test_windows_hang_flood_and_drift_never_succeed",
}


def legacy_stat_mismatch_fields(named, opened):
    # Finite field labels only: never emit raw file IDs, times, bytes or paths.
    names = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_birthtime_ns", "st_nlink")
    return [name for name in names if getattr(named, name, None) != getattr(opened, name, None)]


def leaves(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from leaves(item)
        else:
            yield item


def main():
    # This gate cannot promote a POSIX skipped run into Windows evidence.
    if os.name != "nt" or len(sys.argv) != 1:
        raise SystemExit("Windows fixed-suite gate required")
    before = verified_sources()
    manifest_path = Path(__file__).with_name("windows-source-sha256.json")
    manifest_sha256 = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    binary_path = Path(sys.executable).resolve()
    if binary_path.stat().st_size > 64 << 20:
        raise SystemExit("interpreter cap")
    binary_sha256 = hashlib.sha256(binary_path.read_bytes()).hexdigest()
    import rpc_probe as probe
    root = probe.explicit_fixture_base()
    if not root.is_dir() or any(root.iterdir()):
        raise SystemExit("new empty owned fixture base required")
    # Diagnose the old cross-family mismatch on this fixed, source-verified file.
    # These legacy values do not decide admission; consistent native witnesses do.
    with Path(__file__).open("rb") as stream:
        different = legacy_stat_mismatch_fields(Path(__file__).lstat(), os.fstat(stream.fileno()))
    print(json.dumps({"schema": 1, "profile": "public-gate-source", "legacyStatDifferentFields": different}))
    suite = unittest.defaultTestLoader.loadTestsFromName("test_windows_rpc_probe")
    tests = list(leaves(suite))
    ids = {test.id().removeprefix("test_windows_rpc_probe.") for test in tests}
    if ids != EXPECTED or len(tests) != len(EXPECTED):
        raise SystemExit("exact test inventory required")
    log = CappedLog(root / "windows-gate-tests.log")
    try:
        result = unittest.TextTestRunner(stream=log, verbosity=2, resultclass=PublicResult).run(unittest.TestSuite(tests))
    finally:
        log.close()
    try:
        unchanged = (before == verified_sources() and manifest_sha256 == hashlib.sha256(manifest_path.read_bytes()).hexdigest() and binary_sha256 == hashlib.sha256(binary_path.read_bytes()).hexdigest())
    except Exception:
        unchanged = False
    public_outcomes = {key.removeprefix("test_windows_rpc_probe."): value for key, value in result.public_outcomes.items()}
    all_named_passed = set(public_outcomes) == EXPECTED and all(value == "pass" for value in public_outcomes.values())
    passed = all_named_passed and result.wasSuccessful() and result.testsRun == len(EXPECTED) and not result.skipped and unchanged and not log.overflow
    receipt = {"schema": 1, "platform": "windows", "fixtureOnly": True,
        "productExecuted": False, "liveAcceptance": False, "publishedStatus": False,
        "success": passed, "testsRun": result.testsRun, "skipped": len(result.skipped),
        "sourcesUnchanged": unchanged, "manifestSha256": manifest_sha256, "interpreterSha256": binary_sha256, "sourceSha256": before, "logOverflow": log.overflow,
        "candidateCommit": probe.CANDIDATE, "apiBase": probe.API_BASE, "protocol": probe.PROTOCOL}
    with (root / "windows-gate-receipt.json").open("x") as out:
        json.dump(receipt, out, indent=2)
    # Fixed public test names/outcomes remain inspectable even if admission fails.
    # Never print assertion text, frames, stderr, exception messages or paths.
    outcomes = [{"test": name, "outcome": public_outcomes.get(name, "unknown")} for name in sorted(EXPECTED)]
    print(json.dumps({"schema": 1, "fixtureOnly": True, "testsRun": result.testsRun, "outcomes": outcomes}))
    # The workflow uploads only after this bounded artifact admission marker.
    admit_artifacts(root)
    print("Windows public fake gate:", "PASS" if passed else "FAIL", "tests", result.testsRun, "skips", len(result.skipped))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
