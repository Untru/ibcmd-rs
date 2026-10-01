"""Negative controls for lab evidence; these do not count as installed-EDT acceptance."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
MODULE = Path(__file__).resolve().parents[2] / "scripts/edt-lab/oracle.py"
spec = importlib.util.spec_from_file_location("edt_oracle", MODULE)
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class EvidenceControls(unittest.TestCase):
    def test_nonzero_command_preserves_exact_failure_and_logs(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with self.assertRaises(oracle.OracleError):
                oracle.run_command(root, "bad", [sys.executable, "-c",
                    "import sys; print('partial output'); sys.exit(7)"], 10)
            data = json.loads((root / "bad.command.json").read_text())
            self.assertEqual(data["exit_code"], 7)
            self.assertIn(b"partial output", (root / "bad.stdout").read_bytes())

    def test_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "report.json"
            oracle.write_json(path, {"first": True})
            with self.assertRaises(FileExistsError):
                oracle.write_json(path, {"first": False})
            self.assertTrue(json.loads(path.read_text())["first"])

    def test_empty_partial_and_wrong_runtime_inputs_fail(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with self.assertRaises(oracle.OracleError):
                oracle.snapshot(root)
            (root / "Configuration.xml").write_text("<root/>")
            with self.assertRaises(oracle.OracleError):
                oracle.require_xml(root)
            for relative in (".project", "DT-INF/PROJECT.PMF", "src/Configuration/Configuration.mdo"):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("Runtime-Version: 8.3.27\n")
            with self.assertRaises(oracle.OracleError):
                oracle.require_project(root, "8.5.1")

    def test_snapshot_captures_changes_and_refuses_limits(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            path = root / "payload.bsl"
            path.write_bytes(b"first")
            before = oracle.snapshot(root)
            path.write_bytes(b"other")
            after = oracle.snapshot(root)
            self.assertNotEqual(before["tree_sha256"], after["tree_sha256"])
            with self.assertRaises(oracle.OracleError):
                oracle.snapshot(root, max_file=4)

    def test_case_collisions_refused_on_case_sensitive_system(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "A.xml").write_text("a")
            (root / "a.xml").write_text("b")
            if len(list(root.iterdir())) != 2:
                self.skipTest("filesystem is case-insensitive")
            with self.assertRaises(oracle.OracleError):
                oracle.snapshot(root)

    def test_raw_verdict_rejects_incomplete_and_does_not_hide_differences(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "three-way.json"
            report = {"rows": [{"path": "Configuration.xml", "agreement": "all_different"}],
                      "summary": {"all_equal": 0, "all_different": 1},
                      "native": {"file_count": 1}, "edt": {"file_count": 1},
                      "ours": {"file_count": 1}}
            path.write_text(json.dumps(report))
            self.assertFalse(oracle.validate_raw_report(path)["raw_all_equal"])
            report["summary"]["all_equal"] = 1
            path.write_text(json.dumps(report))
            with self.assertRaises(oracle.OracleError):
                oracle.validate_raw_report(path)
            report["summary"]["all_equal"] = 0
            report["rows"] = []
            path.write_text(json.dumps(report))
            with self.assertRaises(oracle.OracleError):
                oracle.validate_raw_report(path)

    def test_zero_exit_with_error_diagnostic_is_not_clean(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            oracle.run_command(root, "bad-diagnostic", [sys.executable, "-c",
                "print('[Validator] ERROR CompositeEValidator - Error executing EValidator')"], 10)
            report = oracle.command_diagnostics(root, ["bad-diagnostic"])
            self.assertEqual(report["bad-diagnostic"]["error_count"], 1)


if __name__ == "__main__":
    unittest.main()
