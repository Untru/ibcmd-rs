"""Execute reviewed fake children only AFTER the independent static gate clears."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

import rpc_probe as probe


def fixture():
    lease = probe.create_fixture_root()
    root = lease.root
    (root / "source").mkdir(); (root / "reference").mkdir()
    (root / "source" / "module").write_bytes(b"public synthetic module")
    (root / "reference" / "ConfigDumpInfo.xml").write_bytes(b"public synthetic reference")
    (root / "settings.toml").write_text("# public fake only; no SQL/authentication\n")
    seal = {"schema": 1, "adapter": "public-fake-stdio-v1", "candidateCommit": probe.CANDIDATE,
        "apiBase": probe.API_BASE, "protocol": probe.PROTOCOL,
        "interpreterSha256": hashlib.sha256(Path(sys.executable).resolve().read_bytes()).hexdigest(),
        "codeDigests": probe.code_digests(), "inputDigests": probe.inputs(root)}
    (root / "seal.json").write_text(json.dumps(seal))
    return lease


def retain(root, report):
    with (root / "report.json").open("x") as out:
        json.dump(report, out, indent=2)
    return report


class RpcProbe(unittest.TestCase):
    def run_case(self, case, limits=probe.Limits(wall=4, request=0.7)):
        lease = fixture(); root = lease.root
        began = time.monotonic()
        report = retain(root, probe.run_probe(lease, case, limits))
        self.assertLess(time.monotonic() - began, 8, "bounded fake run including cleanup")
        self.assertFalse(report["liveAcceptance"])
        self.assertFalse(report["productExecuted"])
        self.assertFalse(report["publishedStatus"])
        self.assertTrue(report.get("cleanupConfirmed"), report)
        self.assertNotIn(str(root), json.dumps(report))
        return root, report
    def test_seven_requests_cancel_and_retired_generation(self):
        _, report = self.run_case("ok")
        self.assertTrue(report["success"], report)
        self.assertEqual(report["requests"], 7)
        self.assertEqual(report["cancelledRequest"], 6)
        self.assertEqual(report["discardedStaleRequest"], 7)
        self.assertEqual(report["binding"]["candidateCommit"], probe.CANDIDATE)
        self.assertEqual(report["binding"]["sourceInputsSha256"].keys(), {"source", "reference", "settings"})
        self.assertEqual(report["returnCode"], 0)
    def test_stale_snapshot_is_discarded_not_published(self):
        _, report = self.run_case("stale-snapshot")
        self.assertTrue(report["success"], report)
        self.assertEqual(report["validatedNativeSnapshot"], "c" * 64)
        self.assertEqual(report["discardedStaleRequest"], 7)
    def test_hang_is_bounded_and_not_success(self):
        _, report = self.run_case("hang", probe.Limits(wall=1, request=0.25))
        self.assertFalse(report["success"])
        self.assertEqual(report["failure"], "timeout")
    def test_stdout_cap(self):
        _, report = self.run_case("stdout-cap", probe.Limits(stdout=4096))
        self.assertFalse(report["success"])
        self.assertEqual(report["failure"], "stdout-cap")
        self.assertLessEqual(report["stdoutBytes"], 4096)
    def test_stderr_cap_and_privacy(self):
        _, report = self.run_case("stderr-cap", probe.Limits(stderr=64))
        self.assertFalse(report["success"])
        self.assertEqual(report["failure"], "stderr-cap")
        self.assertNotIn("synthetic-private", json.dumps(report))
    def test_early_error_never_echoes_diagnostics(self):
        _, report = self.run_case("early-error")
        self.assertFalse(report["success"])
        for secret in ("NEVER-PUBLISH", "private-fixture-query"):
            self.assertNotIn(secret, json.dumps(report))
        self.assertEqual(report["publicError"]["errorCode"], -32014)
    def test_nonzero_exit_not_success(self):
        _, report = self.run_case("nonzero")
        self.assertFalse(report["success"])
        self.assertEqual(report["returnCode"], 37)
    def test_unknown_ids_and_frames_refused(self):
        for case in ("wrong-id", "malformed", "missing-cancel"):
            _, report = self.run_case(case)
            self.assertFalse(report["success"], report)
    def test_input_and_seal_mutation_not_success(self):
        for case in ("mutate-input", "mutate-seal"):
            _, report = self.run_case(case)
            self.assertFalse(report["success"], report)
            self.assertEqual(report["failure"], "cleanup-or-binding-unconfirmed")
    def test_descendant_cleanup_is_not_silent_success(self):
        root, report = self.run_case("descendant-hang")
        self.assertTrue((root / "descendant.pid").is_file())
        self.assertEqual((root / "descendant.ready").read_text(), "ready")
        self.assertFalse(report["success"])
        self.assertEqual(report["failure"], "process-not-clean")
    def test_trailing_frames_errors_and_flood_cannot_succeed(self):
        for case in ("trailing-frame", "trailing-error", "trailing-malformed", "trailing-flood"):
            _, report = self.run_case(case)
            self.assertFalse(report["success"], report)
    def test_trailing_valid_progress_is_exhausted(self):
        _, report = self.run_case("trailing-progress")
        self.assertTrue(report["success"], report)
        self.assertEqual(report["notifications"], 1)
    def test_missing_seal_never_spawns(self):
        lease = fixture(); root = lease.root
        (root / "seal.json").rename(root / "preserved-seal.json")
        with patch("rpc_probe.ProcessScope") as spawn:
            report = retain(root, probe.run_probe(lease))
            spawn.assert_not_called()
        self.assertFalse(report["success"])
        self.assertFalse(report["spawned"])
    def test_stale_hash_unknown_schema_commit_and_actual_adapter_never_spawn(self):
        for key, value in (("interpreterSha256", "0" * 64), ("schema", 99),
                ("candidateCommit", "a" * 40), ("adapter", "actual-ibcmd")):
            lease = fixture(); root = lease.root
            path = root / "seal.json"
            seal = json.loads(path.read_text()); seal[key] = value; path.write_text(json.dumps(seal))
            with patch("rpc_probe.ProcessScope") as spawn:
                report = retain(root, probe.run_probe(lease))
                spawn.assert_not_called()
            self.assertFalse(report["success"])
            self.assertFalse(report["spawned"])
    def test_unknown_case_and_nonfinite_limits_never_spawn(self):
        lease = fixture(); root = lease.root
        with patch("rpc_probe.ProcessScope") as spawn:
            for case, limits in (("arbitrary-command", probe.Limits()), ("ok", probe.Limits(wall=float("inf")))):
                report = probe.run_probe(lease, case, limits)
                self.assertFalse(report["success"])
            spawn.assert_not_called()




class PreparationControls(unittest.TestCase):
    def test_exited_peer_is_reaped_without_group_signalling(self):
        from process_scope import ProcessScope
        scope = ProcessScope.__new__(ProcessScope)
        scope.job = None
        scope.process = __import__("unittest.mock", fromlist=["Mock"]).Mock()
        scope.process.wait.return_value = 0
        eof = threading.Event(); eof.set()
        with patch("process_scope.os.killpg", create=True) as signal_group:
            self.assertEqual(scope.finish(eof), (0, True))
            signal_group.assert_not_called()
    def test_unowned_or_forged_fixture_never_spawns_or_writes(self):
        root = Path(tempfile.mkdtemp(prefix="unregistered-", dir=probe.FIXTURE_BASE))
        for value in (root, probe.OwnedFixture(root, object())):
            before = list(root.iterdir())
            with patch("rpc_probe.ProcessScope") as spawn:
                report = probe.run_probe(value)
                spawn.assert_not_called()
            self.assertFalse(report["success"])
            self.assertFalse(report["spawned"])
            self.assertEqual(list(root.iterdir()), before)
    def test_settings_identity_is_bound_even_for_equal_bytes(self):
        lease = fixture(); root = lease.root
        original = root / "settings.toml"
        data = original.read_bytes()
        original.rename(root / "preserved-settings.toml")
        original.write_bytes(data)
        with patch("rpc_probe.ProcessScope") as spawn:
            report = probe.run_probe(lease)
            spawn.assert_not_called()
        self.assertFalse(report["success"])
        self.assertEqual(report["failure"], "input-binding")
    def test_directory_inventory_cap_before_spawn(self):
        lease = fixture(); root = lease.root
        for i in range(70): (root / "source" / f"empty-{i}").mkdir()
        with patch("rpc_probe.ProcessScope") as spawn:
            report = probe.run_probe(lease)
            spawn.assert_not_called()
        self.assertFalse(report["success"])
        self.assertEqual(report["failure"], "inventory-cap")
    def test_preparation_deadline_before_spawn(self):
        lease = fixture()
        ticks = iter([0.0, 50.0, 50.0, 50.0])
        with patch("rpc_probe.ProcessScope") as spawn, patch("rpc_probe.time.monotonic", side_effect=lambda: next(ticks, 50.0)):
            report = probe.run_probe(lease)
            spawn.assert_not_called()
        self.assertFalse(report["success"])
        self.assertEqual(report["failure"], "preparation-timeout")
    def test_contract_is_part_of_code_seal(self):
        self.assertIn("rpc-runner-contract.md", probe.CODE)


if __name__ == "__main__": unittest.main()
