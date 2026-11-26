"""Pure in-memory controls: no fixture extraction, teardown, or installed EDT."""
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import MagicMock, Mock, patch
from types import SimpleNamespace
import zipfile

spec = importlib.util.spec_from_file_location("model_inventory", Path(__file__).with_name("model_inventory.py"))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class BundleInventory(unittest.TestCase):
    def expected(self):
        return {"symbolicName": "example.model", "version": "1.0",
                "modelTypes": ["example.Present", "example.Missing"],
                "importers": [], "exporters": ["example.Present"]}

    def jar(self, manifest=None, entries=None):
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as archive:
            archive.writestr("META-INF/MANIFEST.MF", manifest or
                b"Manifest-Version: 1.0\r\nBundle-SymbolicName: example.model;\r\n singleton:=true\r\nBundle-Version: 2.0\r\n\r\n")
            for name, raw in (entries or {"example/Present.class": b"bytecode", "model/model.ecore": b"model"}).items():
                archive.writestr(name, raw)
        data.seek(0)
        return data

    def test_all_categories_and_resource_bytes_accounted(self):
        row = m.inspect_bundle(self.jar(), self.expected())
        self.assertEqual(row["installed_version"], "2.0")
        self.assertEqual(row["baseline_class_delta"]["modelTypes"],
                         {"baseline_count": 2, "present_count": 1, "missing": ["example.Missing"]})
        self.assertEqual(row["baseline_class_delta"]["exporters"]["present_count"], 1)
        self.assertEqual(row["model_resources"],
                         [{"path": "model/model.ecore", "bytes": 5, "sha256": m.digest(b"model")}])

    def test_changed_resource_bytes_change_digest(self):
        one = m.inspect_bundle(self.jar(), self.expected())
        two = m.inspect_bundle(self.jar(entries={"example/Present.class": b"bytecode", "model/model.ecore": b"new"}), self.expected())
        self.assertNotEqual(one["model_resources"], two["model_resources"])

    def test_identity_mismatch_rejected(self):
        with self.assertRaisesRegex(ValueError, "identity mismatch"):
            m.inspect_bundle(self.jar(manifest=b"Bundle-SymbolicName: other\nBundle-Version: 2\n"), self.expected())

    def test_manifest_duplicate_and_orphan_rejected(self):
        for raw in [b"Bundle-Version: 1\nBundle-Version: 2\n", b" continuation\n"]:
            with self.assertRaises(ValueError):
                m.manifest_fields(raw)

    def test_named_manifest_sections_do_not_replace_main_identity(self):
        fields = m.manifest_fields(b"Bundle-Version: 1\n\nName: example\nBundle-Version: 2\n")
        self.assertEqual(fields, {"Bundle-Version": "1"})

    def test_duplicate_baseline_class_rejected(self):
        expected = self.expected()
        expected["modelTypes"].append("example.Present")
        with self.assertRaisesRegex(ValueError, "duplicate baseline"):
            m.inspect_bundle(self.jar(), expected)

    def test_missing_and_ambiguous_bundles_are_explicit(self):
        plugins = Mock()
        plugins.glob.return_value = []
        baseline = {"source": {"release": "legacy"}, "bundles": [self.expected()]}
        missing = m.inventory(plugins, baseline, "current")
        self.assertEqual(missing["summary"]["inspected_bundles"], 0)
        self.assertEqual(missing["bundles"][0]["candidate_count"], 0)
        self.assertEqual(missing["semantic_compatibility"], "NOT_ESTABLISHED")
        plugins.glob.return_value = [Path("one.jar"), Path("two.jar")]
        ambiguous = m.inventory(plugins, baseline, "current")
        self.assertEqual(ambiguous["bundles"][0]["candidate_count"], 2)
        self.assertEqual(ambiguous["summary"]["baseline_bundles"], 1)

    def test_duplicate_baseline_bundle_rejected(self):
        plugins = Mock()
        plugins.glob.return_value = []
        with self.assertRaisesRegex(ValueError, "duplicate baseline bundle"):
            m.inventory(plugins, {"source": {}, "bundles": [self.expected(), self.expected()]}, "current")

    def test_internal_relative_and_traversing_output_rejected(self):
        for path, root in [("/tmp/result.json", "/tmp/lab"),
                           ("result.json", "/Volumes/Work/lab"),
                           ("/Volumes/Work/lab/../result.json", "/Volumes/Work/lab"),
                           ("/Volumes/Other/result.json", "/Volumes/Work/lab")]:
            with self.assertRaises(ValueError):
                m.external_output(Path(path), Path(root))

    def test_resource_streaming_limit_rejected(self):
        with self.assertRaisesRegex(ValueError, "byte limit"):
            m.stream_digest(io.BytesIO(b"12345"), 4)
        self.assertEqual(m.stream_digest(io.BytesIO(b"12345"), 5), (5, m.digest(b"12345")))

    def test_oversized_manifest_rejected(self):
        with self.assertRaisesRegex(ValueError, "manifest.*byte limit"):
            m.inspect_bundle(self.jar(manifest=b"x" * (m.MANIFEST_LIMIT + 1)), self.expected())

    def test_large_signed_manifest_tail_does_not_change_main_identity(self):
        manifest = b"Bundle-SymbolicName: example.model\nBundle-Version: 2\n\nName: file\nDigest: " + b"x" * (m.MANIFEST_LIMIT * 2)
        row = m.inspect_bundle(self.jar(manifest=manifest), self.expected())
        self.assertEqual(row["installed_version"], "2")

    def test_candidate_open_cannot_block_on_fifo(self):
        plugins = Mock()
        plugins.glob.return_value = [Path("example.model_1.jar")]
        with patch.object(m.os, "open", side_effect=OSError("stop before descriptor ownership")) as opened:
            with self.assertRaises(OSError):
                m.inventory(plugins, {"source": {}, "bundles": [self.expected()]}, "current")
        flags = opened.call_args.args[1]
        self.assertTrue(flags & m.os.O_NONBLOCK)
        self.assertTrue(flags & m.os.O_NOFOLLOW)

    def test_all_legal_manifest_terminators_ignore_large_signed_tails(self):
        for eol in [b"\r", b"\n", b"\r\n"]:
            manifest = eol.join([b"Bundle-SymbolicName: example.model;", b" singleton:=true",
                                 b"Bundle-Version: 2", b"", b"Name: file", b"Digest: " + b"x" * 70000])
            row = m.inspect_bundle(self.jar(manifest=manifest), self.expected())
            self.assertEqual(row["installed_version"], "2")

    def test_one_budget_covers_multiple_resources_and_bundles(self):
        entries = {"a.ecore": b"123", "b.ecore": b"456"}
        first = m.DecompressionBudget()
        m.inspect_bundle(self.jar(entries=entries), self.expected(), first)
        shared = m.DecompressionBudget(first.used * 2 - 1)
        m.inspect_bundle(self.jar(entries=entries), self.expected(), shared)
        with self.assertRaises(m.BudgetExceeded) as failure:
            m.inspect_bundle(self.jar(entries=entries), self.expected(), shared)
        self.assertEqual(failure.exception.evidence["scope"], "run")
        self.assertEqual(failure.exception.evidence["context"], "example.model:b.ecore")
        self.assertGreater(failure.exception.evidence["attempted_total"], shared.limit)

    def test_many_small_resources_exhaust_shared_budget(self):
        budget = m.DecompressionBudget(5)
        self.assertEqual(m.stream_digest(io.BytesIO(b"123"), 4, budget)[0], 3)
        with self.assertRaises(m.BudgetExceeded) as failure:
            m.stream_digest(io.BytesIO(b"456"), 4, budget, "second.ecore")
        self.assertEqual(failure.exception.evidence,
                         {"code": "decompression-budget-exceeded", "scope": "run", "limit": 5,
                          "consumed": 3, "attempted_total": 6, "context": "second.ecore"})

    def storage_info(self):
        return {"MountPoint": "/Volumes/External", "Internal": False, "OSInternalMedia": False,
                "SystemImage": False, "BusProtocol": "USB", "DeviceIdentifier": "disk7s1",
                "FilesystemType": "apfs", "APFSPhysicalStores": [{"APFSPhysicalStore": "disk6s2"}]}

    def stat_device(self, path):
        return SimpleNamespace(st_dev=1 if path == Path("/") else 2)

    def test_backing_mount_and_physical_store_are_verified(self):
        backing = dict(self.storage_info(), DeviceIdentifier="disk6s2")
        with patch.object(Path, "is_mount", return_value=True), \
             patch.object(Path, "stat", autospec=True, side_effect=self.stat_device), \
             patch.object(m, "disk_info", side_effect=[self.storage_info(), backing]) as reader:
            proof = m.external_storage(Path("/Volumes/External/lab"), Path("/Volumes/External/lab/logs"))
        self.assertEqual(proof["physical_stores"], ["disk6s2"])
        self.assertEqual(reader.call_args_list[1].args, ("disk6s2",))

    def test_stale_mount_internal_device_and_unverifiable_store_fail_closed(self):
        with patch.object(Path, "is_mount", return_value=False):
            with self.assertRaisesRegex(ValueError, "mount is absent"):
                m.external_storage(Path("/Volumes/External/lab"), Path("/Volumes/External/lab/logs"))
        for info in [dict(self.storage_info(), Internal=True),
                     dict(self.storage_info(), BusProtocol="DiskImage"),
                     dict(self.storage_info(), APFSPhysicalStores=[]),
                     dict(self.storage_info(), OSInternalMedia=None)]:
            with patch.object(Path, "is_mount", return_value=True), \
                 patch.object(Path, "stat", autospec=True, side_effect=self.stat_device), \
                 patch.object(m, "disk_info", return_value=info):
                with self.assertRaises(ValueError):
                    m.external_storage(Path("/Volumes/External/lab"), Path("/Volumes/External/lab/logs"))
        with patch.object(m.subprocess, "run", side_effect=OSError("unavailable")):
            with self.assertRaisesRegex(ValueError, "unverifiable"):
                m.disk_info("/Volumes/External")

    def test_internal_physical_store_and_nested_mount_fail_closed(self):
        with patch.object(Path, "is_mount", return_value=True), \
             patch.object(Path, "stat", autospec=True, side_effect=self.stat_device), \
             patch.object(m, "disk_info", side_effect=[self.storage_info(),
                               dict(self.storage_info(), DeviceIdentifier="disk6s2", Internal=True)]):
            with self.assertRaisesRegex(ValueError, "physical store"):
                m.external_storage(Path("/Volumes/External/lab"), Path("/Volumes/External/lab/logs"))
        def devices(path):
            return SimpleNamespace(st_dev=3 if str(path).endswith("logs") else (1 if path == Path("/") else 2))
        with patch.object(Path, "is_mount", return_value=True), \
             patch.object(Path, "stat", autospec=True, side_effect=devices), \
             patch.object(m, "disk_info", side_effect=[self.storage_info(),
                               dict(self.storage_info(), DeviceIdentifier="disk6s2")]):
            with self.assertRaisesRegex(ValueError, "different backing mount"):
                m.external_storage(Path("/Volumes/External/lab"), Path("/Volumes/External/lab/logs"))

    def test_budget_failure_publishes_explicit_failure_report_and_nonzero_exit(self):
        output = MagicMock()
        baseline = Mock()
        baseline.read_bytes.return_value = b"{}"
        args = SimpleNamespace(output=output, lab_root=Path("/Volumes/External/lab"),
                               baseline=baseline, plugins=Mock(), installed_release="current")
        failure = m.BudgetExceeded("run", 5, 3, 6, "second.ecore")
        with patch.object(m.argparse.ArgumentParser, "parse_args", return_value=args), \
             patch.object(m, "external_output", return_value=output), \
             patch.object(m, "write_report") as published, \
             patch.object(m, "inventory", side_effect=failure):
            with self.assertRaises(SystemExit) as exited:
                m.main()
        self.assertEqual(exited.exception.code, 1)
        report = published.call_args.args[2]
        self.assertEqual(report["status"], "LIMIT_EXCEEDED")
        self.assertEqual(report["semantic_compatibility"], "NOT_ESTABLISHED")
        self.assertEqual(report["error"], failure.evidence)
        self.assertEqual(published.call_args.args[:2], (output, args.lab_root))

    def test_report_creation_rejects_changed_directory_device_before_writing(self):
        with patch.object(m, "external_output"), \
             patch.object(m, "external_storage", return_value={"device_number": 2}), \
             patch.object(m.os, "open", return_value=42) as opened, \
             patch.object(m.os, "fstat", return_value=SimpleNamespace(st_dev=3)), \
             patch.object(m.os, "close") as closed:
            with self.assertRaisesRegex(ValueError, "backing device changed"):
                m.write_report(Path("/Volumes/External/lab/result.json"), Path("/Volumes/External/lab"), {})
        self.assertEqual(opened.call_count, 1)  # directory only; no output opened
        closed.assert_called_once_with(42)

    def test_report_creation_is_exclusive_and_relative_to_verified_directory(self):
        stream = MagicMock()
        with patch.object(m, "external_output"), \
             patch.object(m, "external_storage", return_value={"device_number": 2}), \
             patch.object(m.os, "open", side_effect=[42, 43]) as opened, \
             patch.object(m.os, "fstat", return_value=SimpleNamespace(st_dev=2)), \
             patch.object(m.os, "close") as closed, \
             patch.object(m.os, "fdopen", return_value=stream):
            m.write_report(Path("/Volumes/External/lab/result.json"), Path("/Volumes/External/lab"), {})
        child = opened.call_args_list[1]
        self.assertEqual(child.args[0], "result.json")
        self.assertTrue(child.args[1] & m.os.O_EXCL)
        self.assertTrue(child.args[1] & m.os.O_NOFOLLOW)
        self.assertEqual(child.kwargs["dir_fd"], 42)
        closed.assert_called_once_with(42)

    def test_empty_version_rejected(self):
        with self.assertRaisesRegex(ValueError, "empty bundle version"):
            m.inspect_bundle(self.jar(manifest=b"Bundle-SymbolicName: example.model\nBundle-Version: \n"), self.expected())


if __name__ == "__main__":
    unittest.main()
