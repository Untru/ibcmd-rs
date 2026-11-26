"""Synthetic collector tests; all fixtures remain under the supplied external root."""
import importlib.util
import json
import io
from contextlib import contextmanager, redirect_stderr
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock
import uuid

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/compare-sql-ut11.py"
spec = importlib.util.spec_from_file_location("collector", SCRIPT)
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


class CollectorTests(unittest.TestCase):
    def setUp(self):
        self.work = Path(os.environ["IBCMD_SQL_UT11_TEST_ROOT"]) / uuid.uuid4().hex
        self.work.mkdir(parents=True)
        self.trees = [self.work / name for name in ("native", "ours", "roundtrip")]
        for tree in self.trees:
            tree.mkdir()
            (tree / "Configuration.xml").write_bytes(b"configuration")
            (tree / "ConfigDumpInfo.xml").write_bytes(b"versions")

    def test_equal_is_not_sql_acceptance(self):
        report = collector.collect(self.trees)
        self.assertTrue(report["all_bytes_equal"])
        self.assertEqual(report["evidence_kind"], "offline_tree_comparison_only")
        self.assertNotEqual(report["sql_acceptance"], "passed")
        self.assertEqual(len(report["trees"]["native_source"]), 2)

    def test_full_diff_includes_dumpinfo_missing_and_extra(self):
        (self.trees[1] / "ConfigDumpInfo.xml").write_bytes(b"changed")
        (self.trees[0] / "missing.xml").write_bytes(b"missing")
        (self.trees[1] / "extra.bin").write_bytes(b"extra")
        report = collector.collect(self.trees)
        self.assertFalse(report["all_bytes_equal"])
        self.assertEqual(report["comparisons"]["export"]["summary"],
                         dict(unchanged=1, different=1, left_only=1, right_only=1))
        self.assertEqual(len(report["comparisons"]["roundtrip"]["differences"]), 1)

    def test_independent_roundtrip_failure(self):
        (self.trees[2] / "Configuration.xml").write_bytes(b"changed")
        report = collector.collect(self.trees)
        self.assertEqual(report["comparisons"]["export"]["differences"], [])
        self.assertFalse(report["all_bytes_equal"])

    def test_budget_refusals(self):
        for kwargs in ({"max_files": 5}, {"max_bytes": 2}):
            with self.assertRaises(ValueError):
                collector.collect(self.trees, **kwargs)

    def test_alias_and_nested_refusals(self):
        with self.assertRaises(ValueError):
            collector.collect([self.trees[0], self.trees[0], self.trees[2]])
        nested = self.trees[0] / "nested"
        nested.mkdir()
        with self.assertRaises(ValueError):
            collector.collect([self.trees[0], nested, self.trees[2]])

    def test_physical_root_alias_refusal(self):
        original = collector.directory_identity
        first = original(self.trees[0])
        def identity(path):
            return first if Path(path) == self.trees[1] else original(path)
        with mock.patch.object(collector, "directory_identity", identity):
            with self.assertRaises(ValueError):
                collector.collect(self.trees)

    def test_physical_ancestor_alias_refusal(self):
        # Models a differently named mount exposing an ancestor of root 0.
        original = collector.directory_identity
        ancestor = original(self.work)
        def identity(path):
            return ancestor if Path(path) == self.trees[1] else original(path)
        with mock.patch.object(collector, "directory_identity", identity):
            with self.assertRaises(ValueError):
                collector.collect(self.trees)

    def test_shared_mounted_subtree_refusal(self):
        children = [tree / "child" for tree in self.trees[:2]]
        for child in children:
            child.mkdir()
        original = collector.directory_identity
        first = original(children[0])
        def identity(path):
            return first if Path(path) == children[1] else original(path)
        with mock.patch.object(collector, "directory_identity", identity):
            with self.assertRaises(ValueError):
                collector.collect(self.trees)

    def test_repeated_mounted_directory_refusal(self):
        child = self.trees[0] / "child"
        child.mkdir()
        original = collector.directory_identity
        first = original(self.trees[0])
        def identity(path):
            return first if Path(path) == child else original(path)
        with mock.patch.object(collector, "directory_identity", identity):
            with self.assertRaises(ValueError):
                collector.collect(self.trees)

    def test_physical_output_alias_refusal(self):
        # A mount of an input child outside the input's lexical parent chain.
        child = self.trees[0] / "child"
        child.mkdir()
        alias = self.work / "output-alias"
        alias.mkdir()
        original = collector.directory_identity
        first = original(child)
        def identity(path):
            return first if Path(path) == alias else original(path)
        with mock.patch.object(collector, "directory_identity", identity):
            with self.assertRaises(ValueError):
                collector.collect(self.trees, output=alias / "report.json")
        self.assertFalse((alias / "report.json").exists())

    def test_real_case_alias_and_nested_refusals(self):
        alias = self.work / "NATIVE"
        if not alias.exists() or not alias.samefile(self.trees[0]):
            self.skipTest("requires case-insensitive volume; physical aliases also tested portably")
        with self.assertRaises(ValueError):
            collector.collect([self.trees[0], alias, self.trees[2]])
        child = self.trees[0] / "child"
        child.mkdir()
        with self.assertRaises(ValueError):
            collector.collect([self.trees[0], alias / "child", self.trees[2]])
        output = alias / "report.json"
        result = subprocess.run([sys.executable, str(SCRIPT), *map(str, self.trees),
                                 "--output", str(output)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertFalse(output.exists())

    def test_real_symlink_loop_cli_redaction(self):
        loop = self.work / "private-loop"
        loop.symlink_to("private-loop")
        for inputs, output in (([loop, *self.trees[1:]], self.work / "loop.json"),
                               (self.trees, loop / "report.json")):
            result = subprocess.run([sys.executable, str(SCRIPT), *map(str, inputs),
                                     "--output", str(output)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertNotIn("Traceback", result.stderr)
            self.assertNotIn(str(self.work), result.stderr)
            self.assertEqual(result.stdout, "")
        self.assertFalse((self.work / "loop.json").exists())

    def test_older_python_runtimeerror_resolution_is_redacted(self):
        argv = [str(SCRIPT), *map(str, self.trees), "--output", str(self.work / "error.json")]
        error = RuntimeError("Symlink loop from " + str(self.work))
        captured = io.StringIO()
        with mock.patch.object(sys, "argv", argv), \
                mock.patch.object(collector.Path, "resolve", side_effect=error), \
                redirect_stderr(captured):
            self.assertEqual(collector.main(), 2)
        self.assertNotIn(str(self.work), captured.getvalue())
        self.assertNotIn("Traceback", captured.getvalue())
        self.assertFalse((self.work / "error.json").exists())

    def test_symlink_refusal(self):
        (self.trees[1] / "alias").symlink_to(self.trees[0], target_is_directory=True)
        with self.assertRaises(ValueError):
            collector.collect(self.trees)

    def test_failed_subtree_scan_is_not_silent_equality(self):
        def broken_walk(root, *, followlinks, onerror):
            onerror(PermissionError("synthetic failed subtree scan"))
            yield  # Preserve os.walk's generator interface.
        with mock.patch.object(collector.os, "walk", broken_walk):
            with self.assertRaises(PermissionError):
                collector.collect(self.trees)

    def test_replaced_live_path_during_hashing_is_refused(self):
        original_fdopen = collector.os.fdopen
        replaced = []
        target = self.trees[0] / "Configuration.xml"
        @contextmanager
        def replace_after_read(fd, *args, **kwargs):
            selected = not replaced and os.fstat(fd).st_size == len(b"configuration")
            with original_fdopen(fd, *args, **kwargs) as stream:
                yield stream
            if selected:
                target.rename(self.work / "retained-original.xml")
                target.write_bytes(b"replacement configuration")
                replaced.append(True)
        with mock.patch.object(collector.os, "fdopen", replace_after_read):
            with self.assertRaisesRegex(ValueError, "changed while hashing"):
                collector.collect(self.trees)
        self.assertEqual(replaced, [True])
        self.assertEqual((self.work / "retained-original.xml").read_bytes(), b"configuration")

    def test_partial_tree_refusal(self):
        partial = self.work / "partial"
        partial.mkdir()
        with self.assertRaises(ValueError):
            collector.collect([self.trees[0], partial, self.trees[2]])

    def test_cli_exits_and_no_overwrite(self):
        output = self.work / "result.json"
        cmd = [sys.executable, str(SCRIPT), *map(str, self.trees), "--output", str(output)]
        self.assertEqual(subprocess.run(cmd, capture_output=True).returncode, 0)
        original = output.read_bytes()
        self.assertTrue(json.loads(original)["all_bytes_equal"])
        self.assertEqual(subprocess.run(cmd, capture_output=True).returncode, 2)
        self.assertEqual(output.read_bytes(), original)
        output2 = self.work / "different.json"
        (self.trees[2] / "ConfigDumpInfo.xml").write_bytes(b"different")
        cmd[-1] = str(output2)
        self.assertEqual(subprocess.run(cmd, capture_output=True).returncode, 1)

    def test_cli_rejects_output_inside_tree(self):
        output = self.trees[0] / "report.json"
        cmd = [sys.executable, str(SCRIPT), *map(str, self.trees), "--output", str(output)]
        self.assertEqual(subprocess.run(cmd, capture_output=True).returncode, 2)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
