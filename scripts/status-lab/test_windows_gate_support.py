"""Pure bounded-log/artifact controls; no peer processes or Windows emulation."""
import json
import io
import os
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

import rpc_probe as probe
import windows_gate_support as support


class GateSupport(unittest.TestCase):
    def root(self):
        return probe.create_fixture_root().root
    def test_log_has_exact_byte_cap_and_overflow_marker(self):
        root = self.root()
        log = support.CappedLog(root / "capped.log", cap=5)
        try:
            log.write("abc")
            log.write("defghi")
            log.flush()
            self.assertTrue(log.overflow)
            self.assertEqual(log.size, 5)
        finally:
            log.close()
        self.assertEqual((root / "capped.log").read_bytes(), b"abcde")
    def test_artifact_inventory_counts_real_owned_bytes(self):
        root = self.root(); (root / "nested").mkdir()
        (root / "nested" / "fake.txt").write_bytes(b"abc")
        self.assertEqual(support.artifact_inventory(root), {"files": 1, "bytes": 3, "entries": 2})
    def test_single_file_cap_refuses_without_reading_contents(self):
        root = self.root()
        with (root / "oversized-public-fake").open("xb") as out:
            out.truncate((1 << 20) + 1)
        with self.assertRaisesRegex(ValueError, "artifact-boundary"):
            support.artifact_inventory(root)
    def test_entry_cap_refuses_many_empty_directories(self):
        root = self.root()
        for i in range(257): (root / str(i)).mkdir()
        with self.assertRaisesRegex(ValueError, "artifact-boundary"):
            support.artifact_inventory(root)
    def test_windows_direntry_zero_link_cache_uses_real_stat(self):
        root = self.root(); path = root / "public.txt"; path.write_bytes(b"abc")
        entry = Mock(path=str(path))
        entry.is_symlink.return_value = False
        entry.is_dir.return_value = False
        entry.is_file.return_value = True
        entry.stat.return_value = Mock(st_nlink=0, st_size=3)
        context = Mock()
        context.__enter__ = Mock(return_value=iter([entry]))
        context.__exit__ = Mock(return_value=False)
        with patch("os.scandir", return_value=context):
            self.assertEqual(support.artifact_inventory(root), {"files": 1, "bytes": 3, "entries": 1})
        entry.stat.assert_not_called()
    def test_actual_hardlinks_still_refuse(self):
        root = self.root(); path = root / "public.txt"; path.write_bytes(b"abc")
        os.link(path, root / "public-hardlink.txt")
        with self.assertRaisesRegex(ValueError, "artifact-boundary"):
            support.artifact_inventory(root)
    def assert_public_subtest_outcome(self, kind):
        class Synthetic(unittest.TestCase):
            def runTest(self):
                with self.subTest(case="public-control"):
                    if kind == "failure": self.fail("private-shaped-public-marker")
                    if kind == "error": raise ValueError("private-shaped-public-marker")
                    if kind == "skip": self.skipTest("private-shaped-public-marker")
        case = Synthetic()
        result = unittest.TextTestRunner(stream=io.StringIO(), resultclass=support.PublicResult).run(unittest.TestSuite([case]))
        self.assertEqual(result.public_outcomes, {case.id(): kind})
        self.assertNotIn("private-shaped", json.dumps(result.public_outcomes))
    def test_failed_subtest_marks_owning_named_control(self):
        self.assert_public_subtest_outcome("failure")
    def test_error_subtest_marks_owning_named_control(self):
        self.assert_public_subtest_outcome("error")
    def test_skipped_subtest_cannot_be_overwritten_as_pass(self):
        self.assert_public_subtest_outcome("skip")
    def test_committed_manifest_binds_all_executed_sources(self):
        result = support.verified_sources()
        self.assertEqual(set(result), support.SOURCE_NAMES)
    def test_failed_final_admission_never_mints_upload_marker(self):
        root = self.root()
        with patch("windows_gate_support.artifact_inventory", side_effect=[{"files": 128, "bytes": 4 << 20, "entries": 256}, ValueError("artifact-cap")]):
            with self.assertRaisesRegex(ValueError, "artifact-cap"):
                support.admit_artifacts(root)
        self.assertTrue((root / "artifact-pending.json").is_file())
        self.assertFalse((root / "artifact-safe.json").exists())
    def test_marker_is_promoted_only_after_final_admission(self):
        root = self.root()
        (root / "public.txt").write_bytes(b"abc")
        support.admit_artifacts(root)
        self.assertTrue((root / "artifact-safe.json").is_file())
        self.assertFalse((root / "artifact-pending.json").exists())
        self.assertEqual(support.artifact_inventory(root)["files"], 2)
    def test_stale_source_hash_refuses(self):
        real = support.json.loads
        def stale(data):
            manifest = real(data)
            manifest["sha256"]["windows_rpc_gate.py"] = "0" * 64
            return manifest
        with patch("windows_gate_support.json.loads", side_effect=stale):
            with self.assertRaisesRegex(ValueError, "source-binding"):
                support.verified_sources()


if __name__ == "__main__":
    unittest.main()
