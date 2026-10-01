"""Negative controls for lab evidence; these do not count as installed-EDT acceptance."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
MODULE = Path(__file__).resolve().parents[2] / "scripts/edt-lab/oracle.py"
spec = importlib.util.spec_from_file_location("edt_oracle", MODULE)
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


class EvidenceControls(unittest.TestCase):
    def test_route_specific_tree_equality_keeps_serializer_differences_visible(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            native, edt = root / "native", root / "edt"
            native.mkdir(); edt.mkdir()
            (native / "Configuration.xml").write_bytes(b"<Configuration>A</Configuration>")
            (native / "ConfigDumpInfo.xml").write_bytes(b"native-generation")
            (edt / "Configuration.xml").write_bytes(b"<Configuration>\nA\n</Configuration>")
            a, b = oracle.snapshot(native), oracle.snapshot(edt)
            self.assertFalse(oracle.compare_tree_snapshots(a, b, excluded_paths=("ConfigDumpInfo.xml",))["equal"])
            self.assertTrue(oracle.compare_tree_snapshots(a, a)["equal"])
            self.assertTrue(oracle.compare_tree_snapshots(b, b)["equal"])
            # The exception is direct EDT data only; unchanged return must keep CDI.
            (edt / "Configuration.xml").write_bytes((native / "Configuration.xml").read_bytes())
            c = oracle.snapshot(edt)
            comparison = oracle.compare_tree_snapshots(a, c, excluded_paths=("ConfigDumpInfo.xml",))
            self.assertTrue(comparison["equal"])
            self.assertEqual(comparison["original_excluded_rows"][0]["left"]["sha256"], oracle.digest(native / "ConfigDumpInfo.xml"))
            self.assertFalse(oracle.compare_tree_snapshots(a, c)["equal"])

    def test_validation_no_new_compares_exact_multiset_except_timestamp_and_project(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            original, generated = root / "original.tsv", root / "generated.tsv"
            row = "2026-10-01T11:18:24+0300\tMajor\tConfiguration error\tOriginalProject\tvalidator\tmodule\tline 39\tInherited error\n"
            original.write_text(row, encoding="utf-8")
            changed_label = row.replace("11:18:24", "12:30:00").replace("OriginalProject", "GeneratedProject")
            generated.write_text(changed_label, encoding="utf-8")
            self.assertTrue(oracle.compare_validation_tsv(original, generated)["exact_multiset_equal"])
            for mutation in (changed_label * 2, changed_label.replace("line 39", "line 40"),
                             changed_label.replace("Inherited error", "Inherited  error"),
                             changed_label.replace("Major", "Critical")):
                generated.write_text(mutation, encoding="utf-8")
                self.assertFalse(oracle.compare_validation_tsv(original, generated)["no_new_diagnostics"])
            generated.write_text("", encoding="utf-8")
            self.assertTrue(oracle.compare_validation_tsv(original, generated)["no_new_diagnostics"])
            generated.write_text("truncated\trow\n", encoding="utf-8")
            with self.assertRaises(oracle.OracleError):
                oracle.compare_validation_tsv(original, generated)

    def test_ambient_control_matches_full_records_and_never_blanket_errors(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            control, original, generated = root / "control", root / "original", root / "generated"
            for directory in (control, original, generated):
                directory.mkdir()
            ambient = ("!ENTRY com._1c.g5.v8.dt.core 4 0 2026-10-01 11:18:24\n"
                "!MESSAGE Error while reading the library metainformation\n!STACK 0\n"
                "com._1c.g5.v8.dt.core.library.InvalidLibraryDescriptorException: The library compatibility mode is not specified in the library file\nExact stack\n\n")
            for directory, labels in ((control, ["edt-control-validate", "edt-control-export"]),
                                      (original, ["import"]), (generated, ["export"])):
                for label in labels:
                    (directory / f"{label}.workspace-log").write_text(ambient, encoding="utf-8")
                    (directory / f"{label}.stdout").write_bytes(b"")
                    (directory / f"{label}.stderr").write_bytes(b"")
            (generated / "export.workspace-log").write_text(ambient.replace("11:18:24", "12:00:00"), encoding="utf-8")
            stages = (control, [(original, "import")], [(generated, "export")])
            self.assertTrue(oracle.compare_ambient_diagnostics(*stages)["no_unmatched_error_diagnostics"])
            (generated / "export.workspace-log").write_text(ambient.replace("Exact stack", "Different stack"), encoding="utf-8")
            self.assertFalse(oracle.compare_ambient_diagnostics(*stages)["no_unmatched_error_diagnostics"])
            (generated / "export.workspace-log").write_text(ambient, encoding="utf-8")
            (generated / "export.stdout").write_bytes(b"ERROR source validator failed")
            self.assertFalse(oracle.compare_ambient_diagnostics(*stages)["no_unmatched_error_diagnostics"])

    def test_validation_tsv_retains_source_errors_and_unknown_categories(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "validation.tsv"
            path.write_text("2026-10-01T11:18:24+0300\tMajor\tConfiguration error\tproject\tvalidator\tmodule\tline 39\tWill not compile\n"
                            "2026-10-01T11:18:24+0300\tMinor\tWarning\tproject\tvalidator\tmodule\tline 78\tUnused method\n"
                            "2026-10-01T11:18:24+0300\tMajor\tNew category\tproject\tvalidator\tmodule\tline 90\tUnknown\n"
                            "truncated\trow\n", encoding="utf-8")
            result = oracle.summarize_validation_tsv(path)
            self.assertTrue(result["unresolved_source_diagnostics"])
            self.assertEqual(result["configuration_error_count"], 1)
            self.assertEqual(result["configuration_errors"][0]["position"], "line 39")
            self.assertEqual(len(result["malformed_rows"]), 1)
            self.assertEqual(len(result["unclassified_rows"]), 1)
            self.assertEqual(result["tsv_sha256"], oracle.digest(path))

    def test_zero_exit_validation_without_tsv_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            prepared = root / "prepared"
            project = prepared / "project"
            for relative in (".project", "DT-INF/PROJECT.PMF", "src/Configuration/Configuration.mdo"):
                path = project / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("Runtime-Version: 8.5.1\n", encoding="utf-8")
            oracle.write_json(prepared / "prepared.json", {
                "project": str(project), "source_version": "2.21", "runtime": "8.5.1",
                "edt_version": "2025.2.3.30", "edt_profile_release": "2025.2.3"})
            oracle.write_json(prepared / "authentic-project-after.json", oracle.snapshot(project))
            run = root / "run"
            run.mkdir()
            args = SimpleNamespace(prepared=prepared, source_version="2.21", runtime="8.5.1",
                edt_build="2025.2.3.30", edt_version="2025.2.3")
            with patch.object(oracle, "check_edt_version"), patch.object(oracle, "edt", return_value=b"done"):
                with self.assertRaisesRegex(oracle.OracleError, "did not produce"):
                    oracle.validate_project(args, run)
            self.assertFalse((run / "validation.json").exists())

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
            self.assertTrue(oracle.has_error_diagnostics(report.values()))

    def test_workspace_error_is_not_hidden_by_clean_stdout(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "stage.stdout").write_bytes(b"done")
            (root / "stage.stderr").write_bytes(b"")
            (root / "stage.workspace-log").write_bytes(
                b"!ENTRY org.eclipse.xtext.validation.CompositeEValidator 4 0 2026-10-01\n")
            report = oracle.command_diagnostics(root, ["stage"])
            self.assertEqual(report["stage"]["error_count"], 0)
            self.assertTrue(oracle.has_error_diagnostics(report.values()))

    def native_capture_fixture(self, root):
        baseline = {"root": str(root / "authentic-edt-xml"), "tree_sha256": "source-hash",
                    "file_count": 1, "files": [{"path": "Configuration.xml", "sha256": "source-file"}]}
        reference = root / "native-xml"
        current = {"root": str(reference), "tree_sha256": "reference-hash", "file_count": 2,
                   "files": [{"path": "Configuration.xml", "sha256": "native-file"},
                             {"path": "ConfigDumpInfo.xml", "sha256": "cdi"}]}
        native = root / "ibcmd.exe"
        native.write_bytes(b"dummy fixture executable; never launched")
        source = root / "harness-source.py"
        source.write_bytes(b"dummy fixture harness; never launched")
        database, build = "ibcmd_rs_04_edt07_test", "8.5.1.1150"
        invocation = {"database": database, "native_build": build, "input": baseline["root"],
                      "ibcmd": str(native), "harness_sha256": oracle.digest(source),
                      "restore_script": str(root / "restore-clone.ps1")}
        result = {"status": "CAPTURED", "input_unchanged": True, "database": database,
                  "native_build": build, "reference": str(reference),
                  "reference_sha256": current["tree_sha256"], "executable_sha256": oracle.digest(native)}
        for name, data in (("result.json", result), ("invocation.json", invocation),
                           ("input-before.json", baseline), ("input-after.json", baseline),
                           ("native-reference.json", current)):
            oracle.write_json(root / name, data)
        (root / "native-version.stdout").write_bytes(build.encode())
        operations = {"native-version": ["--version"], "native-create": ["infobase", "create"],
                      "native-import": ["infobase", "config", "import"],
                      "native-apply": ["infobase", "config", "apply"],
                      "native-export": ["infobase", "config", "export"]}
        for label, operation in operations.items():
            argv = [str(native), *operation]
            if label != "native-version":
                argv.extend([f"--db-name={database}", "--dbms=MSSQLServer", "--db-server=localhost",
                             f"--data={root / 'ibdata'}"])
            if label == "native-import":
                argv.append(baseline["root"])
            if label == "native-export":
                argv.append(str(reference))
            oracle.write_json(root / f"{label}.command.json", {"exit_code": 0, "argv": argv})
        oracle.write_json(root / "fresh-database.command.json", {"exit_code": 0,
            "argv": ["pwsh", "-File", invocation["restore_script"], "-Corpus", "empty",
                     "-Name", database, "-Track", "edt-native"]})
        return reference, baseline, build, current

    def test_arbitrary_reference_directory_without_native_capture_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with self.assertRaises(FileNotFoundError):
                oracle.validate_native_reference(root / "native-xml", {}, "8.5.1.1150", {})

    def test_complete_synthetic_binding_fixture_is_valid_structure_only(self):
        with tempfile.TemporaryDirectory() as folder:
            values = self.native_capture_fixture(Path(folder))
            result = oracle.validate_native_reference(*values)
            self.assertEqual(result["source_edt_xml_tree_sha256"], "source-hash")
            self.assertEqual(len(result["command_evidence_sha256"]), 5)

    def test_optional_failed_apply_probe_does_not_replace_import_export_proof(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            values = self.native_capture_fixture(root)
            invocation_path, result_path, command_path = root / "invocation.json", root / "result.json", root / "native-apply.command.json"
            invocation = json.loads(invocation_path.read_text()); invocation["probe_apply"] = True
            result = json.loads(result_path.read_text()); result["activation_probe"] = {"status": "FAIL", "exit_code": 7}
            command = json.loads(command_path.read_text()); command["exit_code"] = 7
            for path, value in ((invocation_path, invocation), (result_path, result), (command_path, command)):
                path.write_text(json.dumps(value), encoding="utf-8")
            self.assertEqual(oracle.validate_native_reference(*values)["activation_probe"]["status"], "FAIL")
            command["argv"] = [part.replace("ibcmd_rs_04_edt07_test", "some_other_db") for part in command["argv"]]
            command_path.write_text(json.dumps(command), encoding="utf-8")
            with self.assertRaises(oracle.OracleError):
                oracle.validate_native_reference(*values)

    def test_native_reference_wrong_input_failed_command_version_and_manifest_rejected(self):
        for corruption in ("input", "command", "version", "manifest", "database", "executable"):
            with self.subTest(corruption=corruption), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                values = self.native_capture_fixture(root)
                # This synthetic proof only exercises binding validation; it is
                # never passed to the actual accept entry point or native tools.
                if corruption == "input":
                    path = root / "input-before.json"
                    data = json.loads(path.read_text()); data["tree_sha256"] = "another-source"
                elif corruption == "command":
                    path = root / "native-import.command.json"
                    data = json.loads(path.read_text()); data["exit_code"] = 7
                elif corruption == "version":
                    (root / "native-version.stdout").write_bytes(b"8.5.1.11500")
                    path = None
                elif corruption == "manifest":
                    path = root / "native-reference.json"
                    data = json.loads(path.read_text()); data["files"] = []
                elif corruption == "database":
                    path = root / "native-import.command.json"
                    data = json.loads(path.read_text())
                    data["argv"] = [part.replace("ibcmd_rs_04_edt07_test", "some_user_db") for part in data["argv"]]
                else:
                    (root / "ibcmd.exe").write_bytes(b"changed")
                    path = None
                if path:
                    path.write_text(json.dumps(data))
                with self.assertRaises(oracle.OracleError):
                    oracle.validate_native_reference(*values)


if __name__ == "__main__":
    unittest.main()
