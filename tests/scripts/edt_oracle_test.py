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
    def warm_fixture(self, root, *, rows=None, logs=None, streams=None, mutate=None):
        """Synthetic captures exercise bindings only; no EDT executable is run."""
        run, original = root / "run", root / "original"
        run.mkdir(parents=True)
        original.mkdir()
        (original / "source.mdo").write_bytes(b"immutable source")
        project = run / "project-copy"
        project.mkdir()
        (project / "source.mdo").write_bytes(b"immutable source")
        exe = root / "edt.exe"
        exe.write_bytes(b"synthetic tool; never executed")
        args = SimpleNamespace(edt_exe=exe, edt_build="2025.2.3.30", timeout=120,
            heap_gib=4, warm_validation_passes=3, source_version="2.20", runtime="8.3.27",
            lock_script=root / "synthetic-lock.ps1", lock_track="edt-synthetic-warm")
        rows = [b"", b"", b""] if rows is None else rows
        logs = [b"", b"", b""] if logs is None else logs
        streams = [(b"", b"")] * 3 if streams is None else streams
        calls, history = [], b""
        def capture(_, __, label, workspace, operation):
            nonlocal history
            index = len(calls)
            calls.append(label)
            workspace.mkdir(exist_ok=True)
            tsv = Path(operation[operation.index("--file") + 1])
            tsv.write_bytes(rows[index])
            argv = [str(exe), "-data", str(workspace), "-timeout", str(args.timeout),
                "-nl", "en_US", "-vmargs", f"-Xmx{args.heap_gib}g", "-command", *operation]
            oracle.write_json(run / f"{label}.command.json", {"argv": argv, "cwd": str(run), "exit_code": 0})
            for action in ("acquire", "release"):
                lock_argv = ["pwsh", "-NoProfile", "-File", str(args.lock_script), action, args.lock_track]
                if action == "acquire":
                    lock_argv.extend(["-TimeoutMin", str(max(1, args.timeout // 60))])
                oracle.write_json(run / f"{label}-lock-{action}.command.json",
                    {"argv": lock_argv, "cwd": str(run), "exit_code": 0})
            for suffix, content in zip(("stdout", "stderr"), streams[index]):
                (run / f"{label}.{suffix}").write_bytes(content)
            history += f"!SESSION synthetic-pass-{index + 1}\n".encode() + logs[index]
            (run / f"{label}.workspace-log").write_bytes(history)
            if mutate:
                mutate(project, original, index)
        with patch.object(oracle, "edt", side_effect=capture):
            evidence = oracle.warm_validate(args, run, project, run / "validation-workspace",
                                            "edt-validate", "validation", original)
        return run, args, project, original, evidence, calls

    def warm_binding_fixture(self, root):
        run, args, project, original, evidence, calls = self.warm_fixture(root)
        source = run / "harness-source.py"
        source.write_bytes(b"synthetic harness; never executed")
        oracle.write_json(run / "invocation.json", {**vars(args), "edt_exe": str(args.edt_exe), "lock_script": str(args.lock_script),
            "mode": "validate", "harness_sha256": oracle.digest(source)})
        oracle.write_json(run / "edt-version.json", {"actual": args.edt_build,
            "executable_sha256": oracle.digest(args.edt_exe)})
        oracle.write_json(run / "validation.json", {"status": "CAPTURED", "prepared_project_unchanged": True,
            "tsv_sha256": oracle.digest(run / "validation.tsv")})
        for name in ("authentic-project-before.json", "authentic-project-after.json"):
            oracle.write_json(run / name, oracle.snapshot(original))
        oracle.write_json(run / "validated-project-copy.json", oracle.snapshot(project))
        oracle.write_json(run / "edt-version.command.json", {"argv": [str(args.edt_exe), "-data",
            str(run / "version-workspace"), "-command", "version"], "cwd": str(run), "exit_code": 0})
        for suffix in ("stdout", "stderr", "workspace-log"):
            (run / f"edt-version.{suffix}").write_bytes(args.edt_build.encode() if suffix == "stdout" else b"")
        return run, args, project, original, evidence, calls

    def test_warm_capture_three_passes_retains_exact_logs_and_rebinds_all_records(self):
        row = b"2026-10-01T11:18:24+0300\tMinor\tCodeStyle\tproject\tvalidator\tmodule\tline 1\tStyle\n"
        with tempfile.TemporaryDirectory() as folder:
            run, args, project, original, evidence, calls = self.warm_fixture(Path(folder), rows=[b"", row, row])
            self.assertEqual(calls, [f"edt-validate-pass-{n:03d}" for n in range(1, 4)])
            self.assertTrue(evidence["final_consecutive_all_diagnostics_stable"])
            self.assertEqual((run / "validation.tsv").read_bytes(), row)
            self.assertEqual(evidence, oracle.bind_warm_validation(args, run, project,
                run / "validation-workspace", "edt-validate", "validation", original))
            for record in evidence["records"]:
                self.assertEqual(record["workspace_phase_bytes"], len((run / f"{record['label']}.workspace-phase-log").read_bytes()))
            # Replay refuses an existing workspace and never overwrites history.
            with self.assertRaisesRegex(oracle.OracleError, "fresh dedicated"):
                oracle.warm_validate(args, run, project, run / "validation-workspace", "edt-validate", "validation")

    def test_warm_capture_binding_rejects_command_tool_source_workspace_and_summary_tampering(self):
        for corruption in (None, "tool", "project", "original", "workspace", "command", "tsv",
                           "phase", "source_before", "summary", "passes", "heap", "timeout", "profile", "fifo"):
            with self.subTest(corruption=corruption), tempfile.TemporaryDirectory() as folder:
                run, args, project, original, evidence, _ = self.warm_binding_fixture(Path(folder))
                if corruption in ("tool", "project", "original"):
                    {"tool": args.edt_exe, "project": project / "source.mdo", "original": original / "source.mdo"}[corruption].write_bytes(b"changed")
                elif corruption in ("workspace", "command"):
                    path = run / "edt-validate-pass-001.command.json"
                    data = json.loads(path.read_text(encoding="utf-8"))
                    data["argv"][data["argv"].index("-data" if corruption == "workspace" else "--project-list") + 1] = "wrong-scope"
                    path.write_text(json.dumps(data), encoding="utf-8")
                elif corruption == "tsv":
                    (run / "validation-pass-001.tsv").write_bytes(b"truncated\trow\n")
                elif corruption == "phase":
                    (run / "edt-validate-pass-001.workspace-phase-log").write_bytes(b"!SESSION invented\n")
                elif corruption == "source_before":
                    path = run / "edt-validate-pass-001.project-before.json"
                    data = json.loads(path.read_text(encoding="utf-8")); data["tree_sha256"] = "forged"
                    path.write_text(json.dumps(data), encoding="utf-8")
                elif corruption == "summary":
                    path = run / "validation-warm.json"
                    data = json.loads(path.read_text(encoding="utf-8")); data["records"] = []
                    path.write_text(json.dumps(data), encoding="utf-8")
                elif corruption == "passes":
                    args.warm_validation_passes = 0
                elif corruption in ("heap", "timeout"):
                    setattr(args, "heap_gib" if corruption == "heap" else "timeout", 99)
                elif corruption == "profile":
                    args.source_version = "2.21"
                elif corruption == "fifo":
                    path = run / "edt-validate-pass-002-lock-acquire.command.json"
                    data = json.loads(path.read_text(encoding="utf-8")); data["argv"][5] = "other-owner"
                    path.write_text(json.dumps(data), encoding="utf-8")
                if corruption:
                    with self.assertRaises(oracle.OracleError):
                        oracle.bind_diagnostic_capture(run, args, project_snapshot=oracle.snapshot(original))
                else:
                    bound = oracle.bind_diagnostic_capture(run, args, project_snapshot=oracle.snapshot(original))
                    self.assertEqual(bound["warm_validation"], evidence)
                    self.assertIn("validation-pass-001.tsv", bound["command_hashes"])

    def test_warm_capture_rejects_mutation_during_any_pass_and_retains_failed_after_snapshot(self):
        for target in ("copy", "original"):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as folder:
                def mutate(project, original, index):
                    if index == 1:
                        ((project if target == "copy" else original) / "source.mdo").write_bytes(b"mutated during validation")
                with self.assertRaisesRegex(oracle.OracleError, "modified"):
                    self.warm_fixture(Path(folder), mutate=mutate)
                run = Path(folder) / "run"
                self.assertTrue((run / "edt-validate-pass-002.project-after.json").is_file())
                self.assertFalse((run / "validation.tsv").exists())

    def test_warm_unstable_final_all_counters_fail_with_every_attempt_retained(self):
        row = b"2026-10-01T11:18:24+0300\tMinor\tWarning\tproject\tvalidator\tmodule\tline 1\tWarning\n"
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(oracle.OracleError, "ALL diagnostic Counters"):
                self.warm_fixture(Path(folder), rows=[b"", row, b""])
            run = Path(folder) / "run"
            self.assertFalse((run / "validation.tsv").exists())
            self.assertFalse(json.loads((run / "validation-warm.json").read_text(encoding="utf-8"))["final_consecutive_all_diagnostics_stable"])
            self.assertEqual(len(list(run.glob("validation-pass-*.tsv"))), 3)
            self.assertEqual(len(list(run.glob("*.workspace-log"))), 3)

    def test_warm_stable_tsv_cannot_hide_unstable_final_workspace_error_counters(self):
        error = b"!ENTRY plugin 4 0 time\n!MESSAGE Error\n"
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(oracle.OracleError, "workspace records are not stable"):
                self.warm_fixture(Path(folder), logs=[b"", error, b""])
            run = Path(folder) / "run"
            evidence = json.loads((run / "validation-warm.json").read_text(encoding="utf-8"))
            self.assertTrue(evidence["final_consecutive_all_diagnostics_stable"])
            self.assertFalse(evidence["final_consecutive_all_workspace_records_stable"])
            self.assertFalse((run / "validation.tsv").exists())

    def test_warm_boundaries_refuse_reset_truncation_partial_or_invalid_utf8_records(self):
        full = b"!SESSION first\n!ENTRY plugin 4 0 time\n!MESSAGE Error\n"
        self.assertEqual(oracle.workspace_append_delta(full, full + b"!SESSION next\n"), b"!SESSION next\n")
        for previous, current in ((full, b"!SESSION reset\n"), (full, full[:-1]),
            (b"!SESSION first\n!ENTRY plugin 4 0 time\n", full), (full, full + b"continuing stack\n"),
            (b"", b"!SESSION first\n!ENTRY plugin 4 0 time\n"), (b"", b"!SESSION bad\xff\n")):
            with self.subTest(previous=previous, current=current), self.assertRaises((oracle.OracleError, UnicodeError)):
                oracle.workspace_append_delta(previous, current)

    def test_warm_unclassified_stdout_and_missing_final_copy_cannot_be_laundered(self):
        for corruption in ("stdout", "stderr", "final", "raw"):
            with self.subTest(corruption=corruption), tempfile.TemporaryDirectory() as folder:
                run, args, project, original, _, _ = self.warm_fixture(Path(folder))
                path = run / ("validation.tsv" if corruption == "final" else
                    f"edt-validate-pass-001.{corruption if corruption != 'raw' else 'workspace-log'}")
                path.write_bytes(b"UNKNOWN diagnostic\n")
                with self.assertRaises(oracle.OracleError):
                    oracle.bind_warm_validation(args, run, project, run / "validation-workspace",
                                                "edt-validate", "validation", original)

    def test_warm_existing_artifacts_are_never_overwritten_even_with_fresh_workspace(self):
        with tempfile.TemporaryDirectory() as folder:
            run, args, project, _, _, _ = self.warm_fixture(Path(folder))
            existing = run / "validation-pass-001.tsv"
            existing.write_bytes(b"prior failed attempt must survive")
            with patch.object(oracle, "edt") as execute, self.assertRaisesRegex(oracle.OracleError, "existing capture"):
                oracle.warm_validate(args, run, project, run / "unused-new-workspace", "edt-validate", "validation")
            execute.assert_not_called()
            self.assertEqual(existing.read_bytes(), b"prior failed attempt must survive")

    def test_warm_matched_early_tsv_error_remains_inherited_and_not_error_free(self):
        row = b"2026-10-01T11:18:24+0300\tMajor\tConfiguration error\tproject\tvalidator\tmodule\tline 1\tInherited\n"
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            baseline = self.warm_fixture(root / "baseline", rows=[row, b"", b""])
            candidate = self.warm_fixture(root / "candidate", rows=[row, b"", b""])
            control = root / "control"; control.mkdir()
            for label in ("edt-control-validate", "edt-control-export"):
                (control / f"{label}.workspace-log").write_bytes(b"")
            result = oracle.compare_warm_phase_errors(control, baseline[0], candidate[0], baseline[4], candidate[4])
            self.assertTrue(result["no_new_all_tsv_diagnostics"])
            self.assertTrue(result["no_new_nonambient_errors"])
            self.assertFalse(result["clean_source"])
            self.assertFalse(result["clean_generated_validation"])

    def test_warm_same_phase_keeps_inherited_errors_and_rejects_early_new_errors_or_tsv(self):
        error = b"!ENTRY plugin 4 0 time\n!MESSAGE inherited\n!STACK 0\nExact stack\n"
        row = b"2026-10-01T11:18:24+0300\tMajor\tConfiguration error\tproject\tvalidator\tmodule\tline 1\tError\n"
        for mutation in (None, "new", "count", "severity", "plugin", "body", "stack", "tsv"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                baseline = self.warm_fixture(root / "baseline", logs=[error, b"", b""])
                changed = error
                if mutation == "new": changed += error.replace(b"inherited", b"new")
                if mutation == "count": changed *= 2
                if mutation == "severity": changed = error.replace(b" 4 0 ", b" 8 0 ")
                if mutation == "plugin": changed = error.replace(b"plugin", b"other.plugin")
                if mutation == "body": changed = error.replace(b"inherited", b"edited")
                if mutation == "stack": changed = error.replace(b"Exact stack", b"Different stack")
                candidate = self.warm_fixture(root / "candidate", logs=[changed, b"", b""],
                    rows=[row, b"", b""] if mutation == "tsv" else None)
                control = root / "control"; control.mkdir()
                for label in ("edt-control-validate", "edt-control-export"):
                    (control / f"{label}.workspace-log").write_bytes(b"")
                result = oracle.compare_warm_phase_errors(control, baseline[0], candidate[0], baseline[4], candidate[4])
                self.assertFalse(result["clean_source"])
                self.assertFalse(result["clean_generated_validation"])
                self.assertTrue(candidate[4]["final_consecutive_all_diagnostics_stable"])
                self.assertEqual(result["no_new_nonambient_errors"], mutation in (None, "tsv"))
                self.assertEqual(result["no_new_all_tsv_diagnostics"], mutation != "tsv")
                self.assertEqual(result["phases"][-1]["new_nonambient"], [])

    def test_warm_exact_control_record_is_separate_from_similar_new_stack(self):
        ambient = (b"!ENTRY com._1c.g5.v8.dt.core 4 0 time\n"
            b"!MESSAGE Error while reading the library metainformation\n!STACK 0\n"
            b"com._1c.g5.v8.dt.core.library.InvalidLibraryDescriptorException: The library compatibility mode is not specified in the library file\nExact stack\n")
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            baseline = self.warm_fixture(root / "baseline")
            candidate = self.warm_fixture(root / "candidate", logs=[ambient, ambient, ambient])
            control = root / "control"; control.mkdir()
            for label in ("edt-control-validate", "edt-control-export"):
                (control / f"{label}.workspace-log").write_bytes(ambient)
            result = oracle.compare_warm_phase_errors(control, baseline[0], candidate[0], baseline[4], candidate[4])
            self.assertTrue(result["no_new_nonambient_errors"])
            self.assertTrue(result["clean_generated_validation"])
            path = candidate[0] / "edt-validate-pass-001.workspace-phase-log"
            path.write_bytes(path.read_bytes().replace(b"Exact stack", b"Different stack"))
            self.assertFalse(oracle.compare_warm_phase_errors(control, baseline[0], candidate[0], baseline[4], candidate[4])["no_new_nonambient_errors"])

    def test_warm_identical_malformed_entries_never_cancel_between_baseline_and_candidate(self):
        defects = (b"!ENTRY broken\n!MESSAGE error retained\n",
            b"!ENTRY plugin unknown 0 time\n!MESSAGE error retained\n",
            b"!ENTRY plugin 12 0 time\n!MESSAGE error retained\n",
            b"!ENTRY plugin 4 0 time\n", b"!ENTRY plugin 4 0 time\n!MESSAGE\n",
            b"!ENTRY plugin 4 0 time\n!MESSAGE   \n")
        for defect in defects:
            with self.subTest(defect=defect), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                baseline = self.warm_fixture(root / "baseline")
                candidate = self.warm_fixture(root / "candidate")
                for capture in (baseline, candidate):
                    (capture[0] / "edt-validate-pass-001.workspace-phase-log").write_bytes(defect)
                control = root / "control"; control.mkdir()
                for label in ("edt-control-validate", "edt-control-export"):
                    (control / f"{label}.workspace-log").write_bytes(b"")
                with self.assertRaisesRegex(oracle.OracleError, "Malformed/partial"):
                    oracle.compare_warm_phase_errors(control, baseline[0], candidate[0], baseline[4], candidate[4])
                with self.assertRaisesRegex(oracle.OracleError, "Malformed/partial"):
                    oracle.workspace_append_delta(b"", defect)

    def test_warm_all_entries_normalize_only_known_registration_identity_and_verified_command(self):
        plugin, interface, implementation = sorted(oracle.WARM_REGISTRATION_IDENTITIES)[0]
        registration = f"!MESSAGE The external {interface} is registered: {implementation}@"
        for mutation in (None, "plugin", "class", "interface", "body", "stack", "neighbor_hex", "long_hex", "code", "severity", "command_body", "command_path"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                baseline = self.warm_fixture(root / "baseline")
                candidate = self.warm_fixture(root / "candidate")
                counters = []
                for capture, identity in ((baseline, "1a"), (candidate, "2b")):
                    run = capture[0]
                    body, owner, severity, code = registration + identity, plugin, "2", "0"
                    command = json.loads((run / "edt-validate-pass-001.command.json").read_text(encoding="utf-8"))
                    argv = command["argv"]
                    cli = f'!MESSAGE Command to run:\nvalidate --file "{argv[-3]}" --project-list "{argv[-1]}"'
                    if capture is candidate:
                        if mutation == "plugin": owner = "other.plugin"
                        if mutation == "class": body = body.replace(implementation, "unknown.Exporter")
                        if mutation == "interface": body = body.replace(interface, "IUnknownExtension")
                        if mutation == "body": body = body.replace("registered", "rejected")
                        if mutation == "stack": body += "\n!STACK 0\nNew failure"
                        if mutation == "neighbor_hex": body += " neighboring@ff"
                        if mutation == "long_hex": body = registration + "123456789"
                        if mutation == "code": code = "1"
                        if mutation == "severity": severity = "4"
                        if mutation == "command_body": cli += " EXTRA"
                        if mutation == "command_path": cli = cli.replace(argv[-3], "another-output.tsv")
                    path = run / "edt-validate-pass-001.workspace-phase-log"
                    path.write_text(f"!ENTRY {owner} {severity} {code} time\n{body}\n"
                        f"!ENTRY com.e1c.g5.v8.dt.cli.api 1 0 time\n{cli}\n", encoding="utf-8")
                    counter, ledger = oracle.warm_workspace_multiset(path, run / "edt-validate-pass-001.command.json")
                    counters.append(counter)
                    self.assertEqual(sum(counter.values()), 2)
                    if mutation is None:
                        self.assertEqual({row["kind"] for row in ledger}, {"known_registration_jvm_identity", "verified_pass_command"})
                        self.assertTrue(all(row["raw_body_sha256"] and row["command_sha256"] for row in ledger))
                self.assertEqual(counters[0] == counters[1], mutation is None)

    def test_warm_workspace_only_warning_is_not_ignored_when_final_passes_are_clean(self):
        warning = b"!ENTRY unrecognized.plugin 2 0 time\n!MESSAGE New semantic warning\n"
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            baseline = self.warm_fixture(root / "baseline")
            candidate = self.warm_fixture(root / "candidate", logs=[warning, b"", b""])
            control = root / "control"; control.mkdir()
            for label in ("edt-control-validate", "edt-control-export"):
                (control / f"{label}.workspace-log").write_bytes(b"")
            result = oracle.compare_warm_phase_errors(control, baseline[0], candidate[0], baseline[4], candidate[4])
            self.assertFalse(result["no_new_nonambient_errors"])
            self.assertEqual(result["phases"][0]["new_nonambient"][0]["severity"], "2")

    def test_warm_exact_xtext_warning_claims_context_but_never_ignores_changed_or_unknown_lines(self):
        line = f"0    [derived_data_executor_9] WARN  {oracle.WARM_XTEXT_LOGGER}  - {oracle.WARM_XTEXT_MESSAGE}\n"
        for mutation in (None, "context", "logger", "body", "severity", "thread", "stderr", "partial", "extra", "count"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                value = line
                if mutation == "context": value = line.replace("0    [", "124    [").replace("executor_9", "executor_31")
                if mutation == "logger": value = line.replace(oracle.WARM_XTEXT_LOGGER, "unknown.Logger")
                if mutation == "body": value = line.replace("ParserRule", "TerminalRule")
                if mutation == "severity": value = line.replace("WARN", "ERROR")
                if mutation == "thread": value = line.replace("derived_data_executor_9", "Exporter_9")
                if mutation == "partial": value = line.rstrip("\n")
                if mutation == "extra": value = line + "Unknown warning\n"
                if mutation == "count": value = line * 2
                (root / "pass.stdout").write_text("" if mutation == "stderr" else value, encoding="utf-8")
                (root / "pass.stderr").write_text(value if mutation == "stderr" else "", encoding="utf-8")
                if mutation not in (None, "context", "count"):
                    with self.assertRaises(oracle.OracleError): oracle.warm_stream_multiset(root, "pass")
                else:
                    counter, lexical = oracle.warm_stream_multiset(root, "pass")
                    self.assertEqual(sum(counter.values()), 2 if mutation == "count" else 1)
                    self.assertEqual(next(iter(counter)), ("WARN", oracle.WARM_XTEXT_LOGGER, oracle.WARM_XTEXT_MESSAGE))
                    self.assertTrue(all(row["raw_line_sha256"] for row in lexical))

    def test_warm_xtext_warning_count_and_phase_are_compared_before_clean_final_passes(self):
        line = f"0    [derived_data_executor_9] WARN  {oracle.WARM_XTEXT_LOGGER}  - {oracle.WARM_XTEXT_MESSAGE}\n".encode()
        for mutation in (None, "count", "new_phase"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                baseline = self.warm_fixture(root / "baseline", streams=[(line, b""), (b"", b""), (b"", b"")])
                first = line * 2 if mutation == "count" else line.replace(b"executor_9", b"executor_3")
                later = line if mutation == "new_phase" else b""
                candidate = self.warm_fixture(root / "candidate", streams=[(first, b""), (later, b""), (later, b"")])
                control = root / "control"; control.mkdir()
                for label in ("edt-control-validate", "edt-control-export"):
                    (control / f"{label}.workspace-log").write_bytes(b"")
                result = oracle.compare_warm_phase_errors(control, baseline[0], candidate[0], baseline[4], candidate[4])
                self.assertEqual(result["no_new_stream_records"], mutation is None)
                self.assertTrue(candidate[4]["final_consecutive_stream_records_stable"])

    def test_warm_final_stream_counter_must_stabilize_as_well_as_tsv_and_workspace(self):
        line = f"0    [derived_data_executor_9] WARN  {oracle.WARM_XTEXT_LOGGER}  - {oracle.WARM_XTEXT_MESSAGE}\n".encode()
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(oracle.OracleError, "not stable"):
                self.warm_fixture(Path(folder), streams=[(b"", b""), (line, b""), (b"", b"")])
            run = Path(folder) / "run"
            evidence = json.loads((run / "validation-warm.json").read_text(encoding="utf-8"))
            self.assertTrue(evidence["final_consecutive_all_diagnostics_stable"])
            self.assertTrue(evidence["final_consecutive_all_workspace_records_stable"])
            self.assertFalse(evidence["final_consecutive_stream_records_stable"])
            self.assertFalse((run / "validation.tsv").exists())

    def test_partial_route_keeps_raw_divergences_and_uses_heavy_fifo(self):
        with tempfile.TemporaryDirectory() as folder:
            run = Path(folder)
            args = SimpleNamespace(lock_script=Path("fixture-lock.ps1"), lock_track="edt-oracle",
                timeout=120, ours_exe=Path("fixture.exe"), source_version="2.20",
                native_tool_version="8.3.27.2214")
            report = {"rows": [{"path": "Configuration.xml", "agreement": "edt_ours_not_native"}],
                "summary": {"all_equal": 0, "edt_ours_not_native": 1},
                "native": {"file_count": 1}, "edt": {"file_count": 1}, "ours": {"file_count": 1}}
            calls = []
            def command(root, label, argv, timeout):
                calls.append((label, argv))
                if "source-three-way-oracle" in argv:
                    oracle.write_json(Path(argv[argv.index("--output") + 1]), report)
                return b"captured"
            with patch.object(oracle, "run_command", side_effect=command):
                result = oracle.capture_raw_comparison(args, run, "direct", Path("candidate"),
                    Path("native-reference"), Path("installed-baseline"), "0.4.0", "2025.2.3.30")
            self.assertEqual([row[0] for row in calls],
                ["direct-oracle-lock-acquire", "direct-oracle", "direct-oracle-lock-release"])
            self.assertFalse(result["raw_all_equal"])
            self.assertFalse(result["configuration_data_all_equal"])
            self.assertEqual(result["derived_comparison"]["different_rows"], 1)
            self.assertEqual(json.loads((run / "direct.three-way.json").read_text()), report)
            self.assertFalse((run / "acceptance.json").exists())

    def test_conversion_fifo_is_released_on_success_and_failure(self):
        args = SimpleNamespace(lock_script=Path("fixture-lock.ps1"), lock_track="edt-oracle", timeout=120)
        run = Path("fixture-run")
        for fails in (False, True):
            with self.subTest(fails=fails):
                calls = []
                def command(root, label, argv, timeout):
                    calls.append((label, argv))
                    if fails and label == "conversion":
                        raise oracle.OracleError("fixture conversion failure")
                    return b"captured"
                with patch.object(oracle, "run_command", side_effect=command):
                    if fails:
                        with self.assertRaises(oracle.OracleError):
                            oracle.run_conversion(args, run, "conversion", ["fixture.exe", "convert"])
                    else:
                        self.assertEqual(oracle.run_conversion(args, run, "conversion", ["fixture.exe", "convert"]), b"captured")
                self.assertEqual([row[0] for row in calls], ["conversion-lock-acquire", "conversion", "conversion-lock-release"])
                self.assertEqual(calls[0][1][-4:-2], ["acquire", "edt-oracle"])
                self.assertEqual(calls[-1][1][-2:], ["release", "edt-oracle"])

    def test_malformed_workspace_errors_are_preserved_and_never_ambient(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "workspace.log"
            for content in ("!ENTRY plugin 4 0\n!MESSAGE real error\n",
                            "!ENTRY plugin 8 0", "!ENTRY plugin unknown 0\n!MESSAGE error\n",
                            "!ENTRY plugin 12 0\n!MESSAGE unknown severity\n", "!ENTRY plugin\n",
                            "!ENTRY\tplugin\t4\t0\n!MESSAGE tab-delimited error\n", "!ENTRY\n"):
                with self.subTest(content=content):
                    path.write_bytes(content.encode("utf-8"))
                    records = oracle.workspace_error_multiset(path)
                    self.assertEqual(sum(records.values()), 1)
                    self.assertFalse(any(oracle.is_ambient_record(row) for row in records))
                    self.assertEqual(next(iter(records))[3], content)

    def diagnostic_control_fixture(self, root):
        capture, prepared = root / "control", root / "prepared"
        project, template = capture / "EmptyEdtDiagnosticControl", prepared / "project"
        for directory in (project, template):
            for relative in (".project", "DT-INF/PROJECT.PMF", "src/Configuration/Configuration.mdo",
                             ".settings/org.eclipse.core.resources.prefs"):
                path = directory / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"fixture source; never launched")
        source = capture / "harness-source.py"
        source.write_bytes(b"fixture harness; never launched")
        exe = root / "edt.exe"
        exe.write_bytes(b"fixture executable; never launched")
        args = SimpleNamespace(edt_build="2025.2.3.30", edt_exe=exe)
        oracle.write_json(prepared / "prepared.json", {"project": str(template)})
        oracle.write_json(prepared / "authentic-project-after.json", oracle.snapshot(template))
        oracle.write_json(capture / "invocation.json", {"mode": "control", "harness_sha256": oracle.digest(source),
            "prepared": str(prepared), "runtime": "8.3.27", "source_version": "2.20"})
        oracle.write_json(capture / "edt-version.json", {"actual": args.edt_build, "executable_sha256": oracle.digest(exe)})
        oracle.write_json(capture / "control.json", {"status": "CAPTURED", "template_project_unchanged": True,
            "unresolved_source_diagnostics": False})
        tsv = capture / "control-validation.tsv"
        tsv.write_bytes(b"")
        oracle.write_json(capture / "control-validation-summary.json", oracle.summarize_validation_tsv(tsv))
        for name in ("template-project-before.json", "template-project-after.json"):
            oracle.write_json(capture / name, oracle.snapshot(template))
        oracle.write_json(capture / "control-project-before.json", oracle.snapshot(project))
        oracle.write_json(capture / "control-project-after.json", oracle.snapshot(project))
        exported = capture / "control-installed-export"
        exported.mkdir()
        (exported / "Configuration.xml").write_bytes(b"<Configuration/>")
        oracle.write_json(capture / "control-installed-export.json", oracle.snapshot(exported))
        for label, operation in (("edt-version", "version"), ("edt-control-validate", "validate"), ("edt-control-export", "export")):
            argv = [str(exe), "-data", str(capture / ("version-workspace" if operation == "version" else "control-workspace")), "-command", operation]
            if operation == "validate":
                argv.extend(["--file", str(tsv), "--project-list", str(project)])
            elif operation == "export":
                argv.extend(["--project-name", project.name, "--configuration-files", str(exported)])
            oracle.write_json(capture / f"{label}.command.json", {"argv": argv, "exit_code": 0, "cwd": str(capture)})
            for suffix in ("stdout", "stderr", "workspace-log"):
                (capture / f"{label}.{suffix}").write_bytes(args.edt_build.encode() if label == "edt-version" and suffix == "stdout" else b"")
        return capture, args

    def test_control_capture_binds_original_tsv_manifests_and_command_scope(self):
        corruptions = (None, "truncated_tsv", "template", "project", "export", "workspace", "export_project", "export_target")
        for corruption in corruptions:
            with self.subTest(corruption=corruption), tempfile.TemporaryDirectory() as folder:
                capture, args = self.diagnostic_control_fixture(Path(folder))
                if corruption == "truncated_tsv":
                    # A previously nonempty captured TSV cannot become a clean
                    # empty control just by truncating the current file.
                    path = capture / "control-validation-summary.json"
                    data = json.loads(path.read_text()); data["tsv_sha256"] = "previous-nonempty-hash"
                    path.write_text(json.dumps(data), encoding="utf-8")
                elif corruption in ("template", "project", "export"):
                    path = {"template": capture.parent / "prepared/project/.project",
                            "project": capture / "EmptyEdtDiagnosticControl/.project",
                            "export": capture / "control-installed-export/Configuration.xml"}[corruption]
                    path.write_bytes(b"modified after capture")
                elif corruption:
                    path = capture / "edt-control-export.command.json"
                    data = json.loads(path.read_text())
                    flag = {"workspace": "-data", "export_project": "--project-name", "export_target": "--configuration-files"}[corruption]
                    data["argv"][data["argv"].index(flag) + 1] = "another-scope"
                    path.write_text(json.dumps(data), encoding="utf-8")
                if corruption:
                    with self.assertRaises(oracle.OracleError):
                        oracle.bind_diagnostic_capture(capture, args, control=True)
                else:
                    bound = oracle.bind_diagnostic_capture(capture, args, control=True)
                    self.assertIn("control-validation-summary.json", bound["command_hashes"])

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
