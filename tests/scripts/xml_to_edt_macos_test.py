"""Synthetic negative controls only: no installed EDT acceptance is claimed.

Each fixture is removed after its test, also when it fails, unless
IBCMD_TEST_KEEP_FIXTURES=1 keeps it for debugging; the lab runner sets TMPDIR
to the external disk.
"""
import importlib.util
import json
import os
from pathlib import Path, PureWindowsPath
import shutil
import sys
import tempfile
import subprocess
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.dont_write_bytecode = True
source = Path(__file__).resolve().parents[2] / "scripts/edt-lab/xml_to_edt_macos.py"
spec = importlib.util.spec_from_file_location("forward_edt", source)
lab = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lab)


def write(path, content=b"fixture"):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)


def project(root, runtime="8.5.1"):
    for name, content in ((".project", b"synthetic"), ("DT-INF/PROJECT.PMF", f"Runtime-Version: {runtime}\n".encode()),
                          ("src/Configuration/Configuration.mdo", b"synthetic")):
        write(root / name, content)


class Controls(unittest.TestCase):
    def symlink_directory_or_skip(self, link, target):
        try:
            link.symlink_to(target, target_is_directory=True)
        except OSError as error:
            if os.name == "nt" and getattr(error, "winerror", None) == 1314:
                self.skipTest("Windows symlink privilege is unavailable (WinError 1314)")
            raise

    def fixture(self):
        root = Path(tempfile.mkdtemp(prefix="issue357-control-"))
        if os.environ.get("IBCMD_TEST_KEEP_FIXTURES") != "1":
            self.addCleanup(shutil.rmtree, root, ignore_errors=True)
        native, run = root / "native", root / "run"
        write(native / "Configuration.xml", b"original XML bytes")
        write(native / "ConfigDumpInfo.xml", b"original dump bytes")
        run.mkdir()
        edt = root / "EDT.app/Contents/Eclipse/1cedtcli"
        java = root / "jdk/bin/java"
        runtime = [edt, java, edt.parent / "1cedt.ini", edt.parent / "configuration/config.ini",
                   edt.parent / "1cedtcli-startup.txt", edt.parent.parent / "Info.plist",
                   edt.parent / "configuration/org.eclipse.equinox.simpleconfigurator/bundles.info",
                   edt.parent.parent / "MacOS/1cedt",
                   java.parent.parent / "release"]
        for path in [root / "ours", *runtime]:
            write(path)
        write(edt.parent / "1cedt.ini", b"-startup\n../Eclipse/plugins/synthetic.jar\n-vmargs\n-Dosgi.debug=synthetic.options\n-Xmx16g\n")
        plugins = edt.parent / "plugins"
        write(plugins / "model.jar", b"synthetic; never loaded")
        manifest = {"schema_version": 1, "status": "INVENTORIED_NOT_RUNTIME_VERIFIED",
                    "edt_cli": str(edt), "edt_cli_sha256": lab.oracle.digest(edt),
                    "plugins": lab.oracle.snapshot(plugins),
                    "runtime_files": [{"path": str(path), "bytes": path.stat().st_size,
                                       "sha256": lab.oracle.digest(path)} for path in runtime]}
        lab.oracle.write_json(root / "installation.json", manifest)
        return SimpleNamespace(lab_root=root, edt_installation_manifest=root / "installation.json", run=run,
            native=native, ours_exe=root / "ours", edt_exe=edt, edt_vm=java,
            edt_vm_library=None, source_version="2.21", edt_build="2025.2.3.30", timeout=1, heap_gib=4)

    def capture(self, args, *, version=None, diagnostics=b"", native_diagnostics=b"", mutation=None, missing_tsv=False):
        class SyntheticCapture:
            def __init__(self, _):
                self.labels = []
                self.ini = args.run / "1cedt.ini"
                write(self.ini, b"synthetic INI; never executed")
                self.ini_sha256 = lab.oracle.digest(self.ini)
            def command(self, label, argv, workspace=None):
                self.labels.append(label)
                for suffix in ("stdout", "stderr"):
                    write(args.run / f"{label}.{suffix}", b'{"loss_policy": "error"}\n' if suffix == "stdout" and label in ("xml-to-edt", "edt-to-xml") else b"")
                return b"synthetic; never executed"
            def edt(self, label, operation, workspace_name=None):
                self.command(label, operation)
                if mutation == "ambient-errors" and label in ("edt-export-native", "edt-export-generated", "edt-validate-native", "edt-validate-generated"):
                    write(args.run / f"{label}.workspace-log", b"!ENTRY sdk.bundle 4 0 timestamp\n!MESSAGE exact ambient\n")
                if label == "edt-version":
                    return (version or args.edt_build).encode()
                if label == "edt-import-native":
                    project(args.run / "native-workspace/NativeControl", {"2.20": "8.3.27", "2.21": "8.5.1"}[args.source_version])
                elif label in ("edt-export-native", "edt-export-generated"):
                    target = Path(operation[operation.index("--configuration-files") + 1])
                    write(target / "Configuration.xml", b"different" if mutation == "serializer" and label == "edt-export-generated" else b"same installed serializer")
                elif label in ("edt-validate-native", "edt-validate-generated"):
                    if label == "edt-validate-native":
                        write(args.run / "native-validation.tsv", native_diagnostics)
                        return b""
                    if not missing_tsv:
                        write(args.run / "generated-validation.tsv", diagnostics)
                    if mutation == "workspace-error":
                        write(args.run / f"{label}.workspace-log", b"!ENTRY org.example 4 0 2026-10-23\n!MESSAGE fail\n")
                    if mutation in ("cli-error", "shutdown-error"):
                        suffix = "1cedtcli.log" if mutation == "cli-error" else "1cedtcli-shutdown-hook.log"
                        write(args.run / f"{label}.workspace-{suffix}", b"ERROR failed\n")
                    if mutation == "mixed-fatal":
                        write(args.run / f"{label}.stdout", b"[Fatal Error] :1:1: Premature end of file.\n")
                    if mutation == "stream-error":
                        write(args.run / f"{label}.stderr", b"ERROR failed\n")
                return b""
            def convert(self, label, source, target, reverse=False):
                self.command(label, [])
                if reverse:
                    for name in ("Configuration.xml", "ConfigDumpInfo.xml"):
                        write(target / name, (args.native / name).read_bytes())
                    if mutation == "changed":
                        write(target / "Configuration.xml", b"different")
                    if mutation == "added":
                        write(target / "extra.xml")
                    if mutation == "missing-dump":
                        # Retain the file outside the returned tree; never delete.
                        (target / "ConfigDumpInfo.xml").rename(args.run / "retained-dump")
                    if mutation == "stripped":
                        write(args.run / "generated-without-provenance/src/Configuration/Configuration.mdo", b"mutated")
                    if mutation == "native":
                        write(args.native / "Configuration.xml", b"mutated")
                    if mutation == "generated":
                        write(source / "src/Configuration/Configuration.mdo", b"mutated")
                    if mutation == "tool":
                        write(args.ours_exe, b"changed executable")
                else:
                    project(target, {"2.20": "8.3.27", "2.21": "8.5.1"}[args.source_version])
                    write(target / ".ibcmd-provenance/manifest.json")
        with patch.object(lab, "Capture", SyntheticCapture):
            return lab.execute(args)

    def test_clean_synthetic_flow_and_manifest_receipts(self):
        args = self.fixture()
        result = self.capture(args)
        self.assertEqual(result["status"], "PASS")
        self.assertTrue(all(result["gates"].values()))
        self.assertTrue((args.run / "generated-project/.ibcmd-provenance/manifest.json").is_file())
        self.assertFalse((args.run / "generated-without-provenance/.ibcmd-provenance").exists())
        self.assertEqual(json.loads((args.run / "exact-return-comparison.json").read_text())["excluded_paths"], [])

    def test_actual_wrong_version_stops_before_project_import(self):
        args = self.fixture()
        with self.assertRaisesRegex(lab.Error, "exact version mismatch"):
            self.capture(args, version="2026.2.1.3")
        self.assertFalse((args.run / "native-workspace").exists())

    def test_source_validation_errors_unknown_and_malformed_rows_fail(self):
        for category in ("Configuration error", "Unfamiliar", "malformed"):
            with self.subTest(category=category):
                args = self.fixture()
                row = b"malformed" if category == "malformed" else (
                    f"2026-10-23T10:00:00Z\tError\t{category}\tp\tv\to\t1\tbad\n").encode()
                if category == "malformed":
                    with self.assertRaisesRegex(lab.Error, "Malformed validation"):
                        self.capture(args, diagnostics=row)
                    continue
                result = self.capture(args, diagnostics=row)
                self.assertEqual(result["status"], "FAIL")
                self.assertFalse(result["gates"]["no_source_errors_or_unknown_validation_rows"])

    def test_new_warning_and_code_style_rows_fail_against_authentic_control(self):
        for category in ("Warning", "Code style"):
            row = f"2026-10-23T10:00:00Z\tMinor\t{category}\tGenerated\tvalidator\tobject\t1\tnew diagnostic\n".encode()
            args = self.fixture()
            result = self.capture(args, diagnostics=row)
            self.assertTrue(result["gates"]["no_source_errors_or_unknown_validation_rows"])
            self.assertFalse(result["gates"]["no_new_structured_validation_diagnostics"])
            self.assertEqual(result["status"], "FAIL")
            differential = json.loads((args.run / "validation-differential.json").read_text())
            self.assertEqual(differential["original_row_count"], 0)
            self.assertEqual(differential["generated_row_count"], 1)
            self.assertEqual(differential["added_diagnostics"][0]["count"], 1)

    def test_structured_validation_multisets_preserve_changes_removals_and_counts(self):
        native = b"2026-10-23T10:00:00Z\tMinor\tCode style\tNativeControl\tvalidator\tobject\t1\tobserved\n"
        same = native.replace(b"10:00:00Z", b"11:00:00Z").replace(b"NativeControl", b"Generated")
        for label, candidate, expected_pass, added, removed in (
            ("same", same, True, 0, 0),
            ("removed", b"", True, 0, 1),
            ("changed", same.replace(b"observed", b"changed"), False, 1, 1),
            ("duplicated", same * 2, False, 1, 0),
        ):
            args = self.fixture()
            result = self.capture(args, native_diagnostics=native, diagnostics=candidate)
            with self.subTest(label=label):
                self.assertEqual(result["status"], "PASS" if expected_pass else "FAIL")
                differential = json.loads((args.run / "validation-differential.json").read_text())
                self.assertEqual(sum(row["count"] for row in differential["added_diagnostics"]), added)
                self.assertEqual(sum(row["count"] for row in differential["removed_diagnostics"]), removed)
                self.assertEqual(differential["exact_multiset_equal"], label == "same")

    def test_authentic_control_configuration_error_cannot_be_hidden_by_clean_candidate(self):
        native = b"2026-10-23T10:00:00Z\tError\tConfiguration error\tNativeControl\tv\to\t1\tfailure\n"
        result = self.capture(self.fixture(), native_diagnostics=native)
        self.assertTrue(result["gates"]["no_new_structured_validation_diagnostics"])
        self.assertFalse(result["gates"]["authentic_native_no_source_errors_or_unknown_rows"])
        self.assertEqual(result["status"], "FAIL")

    def test_all_exact_tree_and_immutable_gates_fail_closed(self):
        gates = {"serializer": "same_installed_serializer_exact_tree",
                 "stripped": "stripped_project_unchanged", "changed": "exact_original_xml_tree_including_dump_info",
                 "added": "exact_original_xml_tree_including_dump_info",
                 "native": "native_unchanged", "generated": "generated_project_unchanged",
                 "tool": "tools_unchanged", "workspace-error": "no_command_or_workspace_errors",
                 "stream-error": "no_command_or_workspace_errors",
                 "mixed-fatal": "no_command_or_workspace_errors",
                 "cli-error": "no_command_or_workspace_errors",
                 "shutdown-error": "no_command_or_workspace_errors"}
        for mutation, gate in gates.items():
            with self.subTest(mutation=mutation):
                result = self.capture(self.fixture(), mutation=mutation)
                self.assertEqual(result["status"], "FAIL")
                self.assertFalse(result["gates"][gate])

    def test_required_artifacts_cannot_be_absent(self):
        for options in ({"missing_tsv": True}, {"mutation": "missing-dump"}):
            with self.subTest(options=options), self.assertRaises(lab.Error):
                self.capture(self.fixture(), **options)

    def test_failed_route_still_audits_sources_installation_and_created_projects(self):
        args = self.fixture()
        with self.assertRaisesRegex(lab.Error, "validation did not produce"):
            self.capture(args, missing_tsv=True)
        audit = json.loads((args.run / "terminal-audit.json").read_text())
        self.assertEqual(audit["route_status"], "FAILED_BEFORE_VERDICT")
        self.assertTrue(audit["trees"]["native"]["unchanged"])
        self.assertTrue(audit["trees"]["authentic-project"]["unchanged"])
        self.assertTrue(audit["trees"]["generated-project"]["unchanged"])
        self.assertTrue(audit["installation_unchanged"])
        self.assertTrue(audit["tools_unchanged"])
        self.assertTrue(audit["immutable_audits_passed"])
        self.assertFalse((args.run / "acceptance.json").exists())

    def test_terminal_publication_failure_preserves_primary_route_error(self):
        for primary_failure in (True, False):
            args = self.fixture()
            original_write = lab.oracle.write_json
            def failing_write(path, value):
                if path.name == "terminal-audit.json":
                    raise OSError("synthetic full disk")
                return original_write(path, value)
            with self.subTest(primary_failure=primary_failure), patch.object(lab.oracle, "write_json", side_effect=failing_write):
                expected = "validation did not produce" if primary_failure else "Terminal audit publication failed"
                with self.assertRaisesRegex(lab.Error, expected) as raised:
                    self.capture(args, missing_tsv=primary_failure)
                if primary_failure:
                    self.assertIn("Terminal audit publication failed", raised.exception.__notes__[0])
            self.assertFalse((args.run / "acceptance.json").exists())

    def test_project_links_refused_before_copy(self):
        args = self.fixture()
        original = args.run / "project"
        project(original)
        write(original / ".ibcmd-provenance/manifest.json")
        self.symlink_directory_or_skip(original / "linked", args.native)
        with self.assertRaisesRegex(lab.Error, "Symlink"):
            lab.copy_without_provenance(original, args.run / "copy")
        self.assertFalse((args.run / "copy").exists())

    def test_command_launch_failure_retains_exclusive_receipt(self):
        args = self.fixture()
        capture = lab.Capture(args)
        with self.assertRaisesRegex(lab.Error, "could not launch"):
            capture.command("missing", [args.run / "missing-executable"])
        record = json.loads((args.run / "missing.command.json").read_text())
        self.assertIn("launch_error", record)
        self.assertEqual(Path(record["isolated_home"]).parent, args.run)
        with self.assertRaises(FileExistsError):
            lab.Capture(args)

    @unittest.skipUnless(os.name == "posix", "POSIX process-group runtime")
    def test_timeout_and_cancellation_kill_only_owned_process_group(self):
        for error in (subprocess.TimeoutExpired("synthetic", 1), KeyboardInterrupt()):
            args = self.fixture()
            cap = lab.Capture(args)
            process = Mock(pid=1234567, returncode=-9)
            process.wait.side_effect = [error, -9]
            expected = lab.Error if isinstance(error, subprocess.TimeoutExpired) else KeyboardInterrupt
            with patch.object(lab.subprocess, "Popen", return_value=process), \
                 patch.object(lab.os, "killpg", create=True) as kill, \
                 self.assertRaises(expected):
                cap.command("cancelled", ["synthetic-never-executed"])
            kill.assert_called_once_with(process.pid, lab.signal.SIGKILL)
            record = json.loads((args.run / "cancelled.command.json").read_text())
            self.assertEqual(record["exit_code"], -9)
            self.assertTrue(record["timeout" if isinstance(error, subprocess.TimeoutExpired) else "cancelled"])

    def test_scope_guard_refuses_existing_outside_and_linked_paths(self):
        args = self.fixture()
        with self.assertRaises(lab.Error):
            lab.contained(args.native, args.run)
        with self.assertRaises(lab.Error):
            lab.contained(args.run, args.run)
        with self.subTest("symlink boundary"):
            linked = args.run / "link"
            self.symlink_directory_or_skip(linked, args.native)
            with self.assertRaises(lab.Error):
                lab.contained(linked, args.run)

    def test_declared_wrong_build_is_refused(self):
        for build in ("2025.2.3", "2026.2.1+3", "not-a-build", "2025.2.3.30 extra"):
            with self.subTest(build=build), self.assertRaises(lab.Error):
                lab.require_exact_build(build)
        lab.require_exact_build("2025.2.3.30")
        lab.require_exact_build("2026.2.1.3")

    def test_profiles_preserve_legacy_model_and_name_current_tool_separately(self):
        for profile in ("2.20", "2.21"):
            args = self.fixture()
            args.source_version, args.edt_build = profile, "2026.2.1.3"
            result = self.capture(args)
            self.assertEqual(result["status"], "PASS")
            self.assertEqual(result["installed_edt_build"], "2026.2.1.3")
            self.assertEqual(result["writer_model_profile"], f"edt-2025.2.3-xml-{profile}")

    def test_preflight_valid_run_and_existing_run_and_wrong_build(self):
        args = self.fixture()
        args.lab_root = args.run.parent
        args.run = args.run.parent / "new-run"
        for tool in (args.ours_exe, args.edt_exe, args.edt_vm):
            tool.chmod(0o700)
        with patch.object(lab.sys, "platform", "darwin"), patch.object(lab, "is_external_lab_root", return_value=True):
            lab.preflight(args)
            args.edt_build = "2026.2.1+3"
            with self.assertRaisesRegex(lab.Error, "four-component"):
                lab.preflight(args)
            args.edt_build = "2026.2.1.3"
            args.run.mkdir()
            with self.assertRaisesRegex(lab.Error, "Run must be new"):
                lab.preflight(args)

    def test_preflight_refuses_run_anywhere_inside_whole_app_bundle(self):
        for relative in ("capture", "Contents/capture", "Contents/Resources/capture",
                         "Contents/MacOS/capture", "Contents/Eclipse/configuration/capture"):
            args = self.fixture()
            bundle = args.edt_exe.parents[2]
            args.run = bundle / relative
            args.run.parent.mkdir(parents=True, exist_ok=True)
            for tool in (args.ours_exe, args.edt_exe, args.edt_vm):
                tool.chmod(0o700)
            with self.subTest(relative=relative), \
                 patch.object(lab.sys, "platform", "darwin"), \
                 patch.object(lab, "is_external_lab_root", return_value=True), \
                 patch.object(lab, "bind_installation") as bind, \
                 self.assertRaisesRegex(lab.Error, "app bundle"):
                lab.preflight(args)
            bind.assert_not_called()
            self.assertFalse(args.run.exists())

    def test_installation_guard_refuses_alias_ancestors_and_requires_app_layout(self):
        args = self.fixture()
        bundle = args.edt_exe.parents[2]
        lab.require_disjoint_installation(args.run, args.edt_exe)
        for run in (bundle, bundle.parent):
            with self.subTest(run=run), self.assertRaisesRegex(lab.Error, "app bundle"):
                lab.require_disjoint_installation(run, args.edt_exe)
        alias = args.lab_root / "bundle-alias"
        alias.mkdir()
        samefile = Path.samefile
        def aliased_samefile(path, other):
            return (path == alias and other == bundle) or samefile(path, other)
        with patch.object(Path, "samefile", aliased_samefile), \
             self.assertRaisesRegex(lab.Error, "app bundle"):
            lab.require_disjoint_installation(alias / "capture", args.edt_exe)
        with self.assertRaisesRegex(lab.Error, "physical .app"):
            lab.require_disjoint_installation(args.run, args.lab_root / "Contents/Eclipse/1cedtcli")

    def test_environment_cannot_inject_jvm_options_or_internal_temp(self):
        args = self.fixture()
        with patch.dict(lab.os.environ, {"JAVA_TOOL_OPTIONS": "-Duser.home=/internal", "JDK_JAVA_OPTIONS": "injected", "_JAVA_OPTIONS": "injected"}):
            cap = lab.Capture(args)
        self.assertTrue(all(key not in cap.env for key in ("JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS")))
        self.assertEqual(cap.env["TMPDIR"], str(args.run / "tmp"))
        self.assertEqual(cap.env["TEMP"], cap.env["TMPDIR"])
        self.assertEqual(cap.env["TMP"], cap.env["TMPDIR"])
        self.assertEqual(cap.env["JAVA_HOME"], str(args.edt_vm.parent.parent))

    def test_jvm_home_tmp_and_vm_are_explicit_external_arguments(self):
        args = self.fixture()
        cap = lab.Capture(args)
        with patch.object(cap, "command", return_value=b"") as command:
            cap.edt("version", ["version"])
        argv = command.call_args.args[1]
        self.assertEqual(argv[argv.index("-ini-file") + 1], cap.ini)
        self.assertNotIn("-vm", argv)
        self.assertNotIn("-Dosgi.debug=synthetic.options", cap.ini.read_text())
        ini = cap.ini.read_text().splitlines()
        self.assertEqual(Path(ini[ini.index("-startup") + 1]), args.edt_exe.parent / "plugins/synthetic.jar")
        self.assertEqual(ini[ini.index("-vm") + 1], str(args.edt_vm))
        self.assertEqual(ini[ini.index("-user") + 1], str(args.run / "user"))
        self.assertIn(f"-Duser.home={args.run / 'home'}", argv)
        self.assertIn(f"-Djava.io.tmpdir={args.run / 'tmp'}", argv)

    def test_windows_startup_path_spelling_keeps_exact_external_target(self):
        # Launcher INI replacement preserves the vendor's '/' suffix. Windows
        # accepts both separators; target identity, rather than string spelling,
        # is the contract exercised by the portable synthetic control.
        external = PureWindowsPath("D:/lab/tools/Contents/Eclipse")
        mixed = str(external) + "/plugins/synthetic.jar"
        self.assertEqual(PureWindowsPath(mixed), external / "plugins/synthetic.jar")
        self.assertTrue(PureWindowsPath(mixed).is_absolute())
        self.assertNotEqual(PureWindowsPath(mixed), PureWindowsPath("C:/internal/plugins/synthetic.jar"))
        self.assertNotEqual(PureWindowsPath(mixed), external / "plugins/other.jar")

    def test_installation_binding_refuses_changed_plugins_and_runtime(self):
        for mutation in ("plugin", "java", "manifest-cli", "wrong-plugins-root", "omit-config"):
            args = self.fixture()
            lab.bind_installation(args)
            if mutation == "plugin":
                write(args.edt_exe.parent / "plugins/model.jar", b"modified plugin")
            elif mutation == "java":
                write(args.edt_vm, b"modified java")
            else:
                manifest = json.loads(args.edt_installation_manifest.read_text())
                if mutation == "manifest-cli":
                    manifest["edt_cli"] = str(args.edt_vm)
                elif mutation == "wrong-plugins-root":
                    write(args.lab_root / "unrelated-plugins/model.jar")
                    manifest["plugins"] = lab.oracle.snapshot(args.lab_root / "unrelated-plugins")
                else:
                    manifest["runtime_files"] = [row for row in manifest["runtime_files"] if not row["path"].endswith("config.ini")]
                args.edt_installation_manifest.write_text(json.dumps(manifest))
            with self.subTest(mutation=mutation), self.assertRaises(lab.Error):
                lab.bind_installation(args)

    def test_launcher_ini_mutation_and_missing_vm_section_fail(self):
        args = self.fixture()
        cap = lab.Capture(args)
        cap.ini.write_text("tampered")
        with self.assertRaisesRegex(lab.Error, "INI changed"):
            cap.edt("version", ["version"])
        other = self.fixture()
        write(other.edt_exe.parent / "1cedt.ini", b"no JVM section")
        with self.assertRaisesRegex(lab.Error, "JVM section"):
            lab.Capture(other)

    def test_matched_phase_rule_keeps_exact_text_counts_streams_and_phase(self):
        args = self.fixture()
        for phase in ("export", "validate"):
            for who in ("native", "generated"):
                label = f"edt-{phase}-{who}"
                write(args.run / f"{label}.stdout", b"[Fatal Error] :1:1: Premature end of file.\n")
                write(args.run / f"{label}.stderr", b"")
                stamp = "1" if who == "native" else "2"
                write(args.run / f"{label}.workspace-log", (f"!ENTRY sdk.bundle 4 0 timestamp-{stamp}\n!MESSAGE exact ambient\n").encode())
        self.assertTrue(lab.compare_matched_phase_errors(args.run)["no_new_error_records"])
        label = "edt-validate-generated"
        write(args.run / f"{label}.stderr", b"ERROR new converter failure\n")
        result = lab.compare_matched_phase_errors(args.run)
        self.assertFalse(result["no_new_error_records"])
        self.assertTrue(result["phases"]["export"]["exact_error_multiset_equal"])
        write(args.run / f"{label}.stderr", b"")
        write(args.run / f"{label}.stdout", b"[Fatal Error] :1:1: Premature end of file.\n" * 2)
        self.assertFalse(lab.compare_matched_phase_errors(args.run)["no_new_error_records"])
        # A removed export error cannot cancel an added validation error.
        write(args.run / "edt-export-native.stdout", b"[Fatal Error] :1:1: Premature end of file.\n" * 2)
        self.assertFalse(lab.compare_matched_phase_errors(args.run)["no_new_error_records"])
        write(args.run / f"{label}.stdout", b"[Fatal Error] :1:1: Premature end of file.\n")
        write(args.run / f"{label}.workspace-log", b"!ENTRY sdk.bundle 4 0 timestamp-2\n!MESSAGE changed candidate error\n")
        self.assertFalse(lab.compare_matched_phase_errors(args.run)["no_new_error_records"])

    def test_matching_ambient_errors_never_make_strict_acceptance_pass(self):
        result = self.capture(self.fixture(), mutation="ambient-errors")
        self.assertTrue(result["gates"]["no_new_matched_phase_errors"])
        self.assertFalse(result["gates"]["no_command_or_workspace_errors"])
        self.assertEqual(result["status"], "FAIL")

    def test_explicit_server_library_is_used_and_must_be_bound(self):
        args = self.fixture()
        args.edt_vm_library = args.edt_vm.parent.parent / "lib/server/libjvm.dylib"
        write(args.edt_vm_library, b"synthetic Server VM; never executed")
        with self.assertRaisesRegex(lab.Error, "omits required"):
            lab.bind_installation(args)
        manifest = json.loads(args.edt_installation_manifest.read_text())
        manifest["runtime_files"].append({"path": str(args.edt_vm_library), "bytes": args.edt_vm_library.stat().st_size,
                                          "sha256": lab.oracle.digest(args.edt_vm_library)})
        args.edt_installation_manifest.write_text(json.dumps(manifest))
        lab.bind_installation(args)
        cap = lab.Capture(args)
        ini = cap.ini.read_text().splitlines()
        self.assertEqual(ini[ini.index("-vm") + 1], str(args.edt_vm_library))

    @unittest.skipUnless(os.name == "posix", "POSIX lock runtime")
    def test_native_slot_refuses_concurrent_capture(self):
        args = self.fixture()
        (args.lab_root / "tmp").mkdir()
        with lab.exclusive_native_slot(args):
            with self.assertRaisesRegex(lab.Error, "shared slot"):
                with lab.exclusive_native_slot(args):
                    self.fail("Concurrent native capture must not run")
        with lab.exclusive_native_slot(args):
            pass

    def test_parent_traversal_cannot_escape_external_root(self):
        root = Path("/Volumes/lab")
        for path in (root / "../outside/run", root / "nested/../../outside/run"):
            with self.subTest(path=path), self.assertRaisesRegex(lab.Error, "traversal"):
                lab.contained(path, root)


if __name__ == "__main__":
    unittest.main()
