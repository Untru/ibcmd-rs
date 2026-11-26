#!/usr/bin/env python3
"""Strict, retained macOS XML→EDT lab capture; never production code.

A PASS describes this corpus/build only, not unrestricted EDT compatibility.
All installed-EDT workspaces, tool homes, logs and intermediate trees are new.
No cleanup runs: move an entire completed run to Trash when no longer needed.
"""
from __future__ import annotations

import argparse
import contextlib
from collections import Counter
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

sys.dont_write_bytecode = True
_spec = importlib.util.spec_from_file_location("xml_edt_oracle", Path(__file__).with_name("oracle.py"))
oracle = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(oracle)
Error = oracle.OracleError


def contained(path: Path, root: Path) -> None:
    if ".." in path.parts or ".." in root.parts:
        raise Error("Parent traversal is refused")
    oracle.no_links(path)
    if path == root or not path.is_relative_to(root):
        raise Error(f"Path must be below the external lab root: {path}")


def require_exact_build(build: str) -> None:
    if not re.fullmatch(r"20\d{2}\.\d+\.\d+\.\d+", build):
        raise Error("An exact four-component installed EDT build is required")


def is_external_lab_root(path: Path) -> bool:
    return path.is_dir() and str(path).startswith("/Volumes/")


def require_disjoint_installation(run: Path, edt: Path) -> None:
    eclipse, contents, bundle = edt.parent, edt.parent.parent, edt.parents[2]
    if edt.name != "1cedtcli" or eclipse.name != "Eclipse" or contents.name != "Contents" \
            or bundle.suffix != ".app":
        raise Error("EDT CLI must have the physical .app/Contents/Eclipse/1cedtcli layout")
    # The immutable installation extends beyond Contents and Eclipse. Existing
    # ancestors can also alias it through case-insensitive names or mount paths.
    def contains(outer: Path, inner: Path) -> bool:
        if inner.is_relative_to(outer):
            return True
        return outer.exists() and any(ancestor.exists() and ancestor.samefile(outer)
            for ancestor in (inner, *inner.parents))
    if contains(bundle, run) or contains(run, bundle):
        raise Error("Whole EDT app bundle and run must not overlap")


def preflight(args) -> None:
    if sys.platform != "darwin":
        raise Error("This capture requires macOS; control tests are not acceptance")
    for field in ("lab_root", "native", "run", "ours_exe", "edt_exe", "edt_vm", "edt_installation_manifest"):
        path = getattr(args, field).absolute()
        if ".." in path.parts:
            raise Error("Parent traversal is refused")
        oracle.no_links(path)
        setattr(args, field, path)
    if args.edt_vm_library is not None:
        args.edt_vm_library = args.edt_vm_library.absolute()
        oracle.no_links(args.edt_vm_library)
        if ".." in args.edt_vm_library.parts or not args.edt_vm_library.is_file():
            raise Error("Explicit VM library must be a physical file")
    if not is_external_lab_root(args.lab_root):
        raise Error("An existing external /Volumes/ lab root is required")
    contained(args.run, args.lab_root)
    contained(args.native, args.lab_root)
    contained(args.edt_installation_manifest, args.lab_root)
    contained(args.edt_exe, args.lab_root)  # Use #358’s complete external installation, including p2/configuration.
    if args.run.exists() or not args.run.parent.is_dir():
        raise Error("Run must be new with an existing parent; resuming evidence is refused")
    require_disjoint_installation(args.run, args.edt_exe)
    if args.run.is_relative_to(args.native) or args.native.is_relative_to(args.run):
        raise Error("Run and immutable native input must not overlap")
    require_exact_build(args.edt_build)
    if not 1 <= args.heap_gib <= 64 or args.timeout < 1:
        raise Error("Invalid heap or timeout")
    for tool in (args.ours_exe, args.edt_exe, args.edt_vm):
        if not tool.is_file() or not os.access(tool, os.X_OK):
            raise Error(f"Missing executable: {tool}")
    bind_installation(args)
    oracle.require_xml(args.native)
    oracle.snapshot(args.native)  # Reject links, special files and collisions before any launch.


def write_launcher_ini(args) -> Path:
    original = (args.edt_exe.parent / "1cedt.ini").read_text(encoding="utf-8")
    lines = original.splitlines()
    if lines.count("-vmargs") != 1:
        raise Error("Installed launcher INI must have exactly one JVM section")
    split = lines.index("-vmargs")
    launcher, vmargs = lines[:split], lines[split + 1:]
    launcher = [line.replace("../Eclipse/", str(args.edt_exe.parent) + "/") for line in launcher]
    # Installed debug tracing prefixes the version command with framework dumps.
    # Disable only that opt-in trace in this retained copy; errors/logs stay gated.
    vmargs = [line for line in vmargs if line != "-Dosgi.debug" and not line.startswith("-Dosgi.debug=")]
    # The custom CLI accepts -ini-file, not generic Eclipse launcher flags.
    for option in ("-vm", "-configuration", "-user", "-nl"):
        while option in launcher:
            index = launcher.index(option)
            if index + 1 >= len(launcher):
                raise Error("Malformed installed launcher INI")
            del launcher[index:index + 2]
    launcher += ["-vm", str(args.edt_vm_library or args.edt_vm), "-configuration", str(args.edt_exe.parent / "configuration"),
                 "-user", str(args.run / "user"), "-nl", "en_US"]
    path = args.run / "1cedt.ini"
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        stream.write("\n".join([*launcher, "-vmargs", *vmargs]) + "\n")
    return path


class Capture:
    def __init__(self, args):
        self.args, self.run = args, args.run
        self.env = dict(os.environ)
        for key in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS", "JDK_JAVA_OPTIONS"):
            self.env.pop(key, None)
        for name, directory in (("HOME", "home"), ("TMPDIR", "tmp"),
                                ("XDG_CACHE_HOME", "cache"), ("XDG_CONFIG_HOME", "config"),
                                ("XDG_DATA_HOME", "data"), ("XDG_STATE_HOME", "state")):
            path = self.run / directory
            path.mkdir()
            self.env[name] = str(path)
        self.env["TEMP"] = self.env["TMP"] = self.env["TMPDIR"]
        self.env["JAVA_HOME"] = str(args.edt_vm.parent.parent)
        self.env["IBCMD_TEST_KEEP_FIXTURES"] = "1"
        self.labels = []
        self.ini = write_launcher_ini(args)
        self.ini_sha256 = oracle.digest(self.ini)

    def command(self, label, argv, workspace=None):
        record = {"argv": [str(x) for x in argv], "cwd": str(self.run),
                  "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                  "timeout_seconds": self.args.timeout, "isolated_home": self.env["HOME"],
                  "launcher_ini": str(self.ini), "launcher_ini_sha256": self.ini_sha256}
        started = time.monotonic()
        try:
            with (self.run / f"{label}.stdout").open("xb") as out, \
                 (self.run / f"{label}.stderr").open("xb") as err:
                proc = subprocess.Popen(record["argv"], cwd=self.run, env=self.env,
                    stdin=subprocess.DEVNULL, stdout=out, stderr=err, start_new_session=True)
                try:
                    record["exit_code"] = proc.wait(timeout=self.args.timeout)
                except BaseException as error:
                    record["timeout" if isinstance(error, subprocess.TimeoutExpired) else "cancelled"] = True
                    try:
                        os.killpg(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    proc.wait()
                    record["exit_code"] = proc.returncode
                    if not isinstance(error, subprocess.TimeoutExpired):
                        raise
        except OSError as error:
            record["launch_error"] = str(error)
            raise Error(f"{label} could not launch; see preserved receipt") from error
        finally:
            record["duration_seconds"] = round(time.monotonic() - started, 3)
            oracle.write_json(self.run / f"{label}.command.json", record)
            if workspace:
                for name in (".log", "1cedtcli.log", "1cedtcli-shutdown-hook.log"):
                    source = workspace / ".metadata" / name
                    if source.exists():
                        oracle.no_links(source)
                        shutil.copyfile(source, self.run / f"{label}.workspace-{name.lstrip('.')}")
        self.labels.append(label)
        if record.get("timeout") or record.get("exit_code") != 0:
            raise Error(f"{label} failed; see preserved receipt and streams")
        return (self.run / f"{label}.stdout").read_bytes()

    def edt(self, label, operation, workspace_name=None):
        workspace = self.run / (workspace_name or f"{label}-workspace")
        if workspace_name is None and workspace.exists():
            raise Error("EDT workspace must be fresh")
        if oracle.digest(self.ini) != self.ini_sha256:
            raise Error("Captured launcher INI changed")
        return self.command(label, [self.args.edt_exe, "-ini-file", self.ini, "-data", workspace,
            "-timeout", str(self.args.timeout), "-vmargs",
            f"-Xmx{self.args.heap_gib}g", f"-Duser.home={self.run / 'home'}",
            f"-Djava.io.tmpdir={self.run / 'tmp'}", "-command", *operation], workspace)

    def convert(self, label, source, target, reverse=False):
        xml, edt = f"xml-{self.args.source_version}", f"edt-2025.2.3-xml-{self.args.source_version}"
        self.command(label, [self.args.ours_exe, "convert", source, target,
            "--source-format", "edt" if reverse else "xml",
            "--target-format", "xml" if reverse else "edt",
            "--source-profile", edt if reverse else xml,
            "--target-profile", xml if reverse else edt,
            "--report", self.run / f"{label}.report.json"])


def copy_without_provenance(project: Path, target: Path) -> dict:
    before = oracle.snapshot(project)
    if not any(row["path"].startswith(".ibcmd-provenance/") for row in before["files"]):
        raise Error("Generated provenance is missing; stripping control is unproven")
    target.mkdir()
    for row in before["files"]:
        if row["path"].split("/")[0] == ".ibcmd-provenance":
            continue
        output = target / row["path"]
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(project / row["path"], output)
    after = oracle.snapshot(target)
    expected = [row for row in before["files"] if row["path"].split("/")[0] != ".ibcmd-provenance"]
    if after["files"] != expected or oracle.snapshot(project) != before:
        raise Error("Project copy or immutable generated project changed")
    return after


def diagnostics_for_all_logs(run: Path, labels: list[str]) -> dict:
    result = oracle.command_diagnostics(run, labels)
    for label in labels:
        seen = {(row["file"], row["line"]) for row in result[label]["error_lines"]}
        for suffix in ("stdout", "stderr", "workspace-1cedtcli.log", "workspace-1cedtcli-shutdown-hook.log"):
            path = run / f"{label}.{suffix}"
            if not path.is_file():
                continue
            for number, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
                if re.search(r"\b(?:ERROR|FATAL)\b|^\s*(?i:\[(?:Fatal\s+Error|Error)\]|(?:Fatal\s+Error|Error)\s*:)|^Exception in thread |^!ENTRY \S+ [48] ", line):
                    if (path.name, number) not in seen:
                        result[label]["error_lines"].append({"file": path.name, "line": number, "text": line})
                        result[label]["error_count"] += 1
                        seen.add((path.name, number))
    return result


def bind_installation(args) -> dict:
    path = args.edt_installation_manifest
    oracle.no_links(path)
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if manifest.get("schema_version") != 1 or Path(manifest.get("edt_cli", "")) != args.edt_exe:
        raise Error("Installation manifest schema or CLI binding mismatch")
    if manifest.get("edt_cli_sha256") != oracle.digest(args.edt_exe):
        raise Error("Installation manifest CLI hash mismatch")
    plugins = manifest["plugins"]
    plugin_root = Path(plugins["root"])
    contained(plugin_root, args.lab_root)
    if plugin_root != args.edt_exe.parent / "plugins":
        raise Error("Plugin snapshot must belong to the CLI installation")
    if oracle.snapshot(plugin_root) != plugins:
        raise Error("Installed plugin tree differs from installation manifest")
    runtime_files = manifest["runtime_files"]
    eclipse = args.edt_exe.parent
    required = {str(path) for path in (args.edt_exe, args.edt_vm,
        eclipse / "1cedt.ini", eclipse / "configuration/config.ini", eclipse / "1cedtcli-startup.txt",
        eclipse / "configuration/org.eclipse.equinox.simpleconfigurator/bundles.info",
        eclipse.parent / "MacOS/1cedt",
        eclipse.parent / "Info.plist", args.edt_vm.parent.parent / "release")}
    if args.edt_vm_library is not None:
        required.add(str(args.edt_vm_library))
    listed = [row["path"] for row in runtime_files]
    if len(listed) != len(set(listed)) or not required.issubset(listed):
        raise Error("Installation runtime manifest omits required launcher/configuration/Java payloads")
    for row in runtime_files:
        runtime = Path(row["path"])
        oracle.no_links(runtime)
        if not runtime.is_absolute() or ".." in runtime.parts or runtime.stat().st_size != row["bytes"] or oracle.digest(runtime) != row["sha256"]:
            raise Error("Installation runtime file changed")
    return {"manifest_path": str(path), "manifest_sha256": oracle.digest(path),
            "plugins_tree_sha256": plugins["tree_sha256"], "runtime_files": runtime_files,
            "status": manifest.get("status")}


def phase_error_records(run: Path, label: str) -> Counter:
    """Exact error records; only Eclipse header timestamps are omitted."""
    diagnostics = diagnostics_for_all_logs(run, [label])[label]
    records = Counter()
    for row in diagnostics["error_lines"]:
        records[(row["file"].removeprefix(label + "."), row["text"])] += 1
    path = run / f"{label}.workspace-log"
    if path.is_file():
        current, body = None, []
        def save():
            if current is not None:
                records[("workspace-log", current, "\n".join(body))] += 1
        for line in path.read_text(encoding="utf-8", errors="strict").splitlines():
            if line.startswith("!ENTRY "):
                save()
                match = re.fullmatch(r"!ENTRY (\S+) ([0-9]+) ([0-9]+) (.+)", line)
                if match is None:
                    raise Error("Malformed workspace diagnostic entry")
                bundle, severity, code, _timestamp = match.groups()
                current = (bundle, severity, code) if severity in ("4", "8") else None
                body = []
            elif line.startswith("!SESSION "):
                save()
                current, body = None, []
            elif current is not None:
                body.append(line)
        save()
    return records


def compare_matched_phase_errors(run: Path) -> dict:
    phases = {}
    for phase in ("export", "validate"):
        baseline = phase_error_records(run, f"edt-{phase}-native")
        candidate = phase_error_records(run, f"edt-{phase}-generated")
        def rows(counter):
            return [{"record": record, "count": count} for record, count in sorted(counter.items(), key=lambda item: repr(item[0]))]
        phases[phase] = {"no_new_error_records": not (candidate - baseline),
                         "exact_error_multiset_equal": baseline == candidate,
                         "added": rows(candidate - baseline), "removed": rows(baseline - candidate)}
    return {"no_new_error_records": all(row["no_new_error_records"] for row in phases.values()),
            "phases": phases, "rule": "Match same-build, same-INI, same-phase fresh-workspace error multisets by stream kind and exact record text/count. Only Eclipse !ENTRY timestamp is omitted. No message, stack, severity, bundle, project path or stream normalization; no cancellation between phases. Raw error-free acceptance remains separate."}


def execute_routes(args) -> dict:
    cap = Capture(args)
    installation_before = bind_installation(args)
    oracle.write_json(args.run / "installation-binding-before.json", installation_before)
    legacy_schema = Path(__file__).resolve().parents[2] / "crates/ibcmd-schema/data"
    schema_files = ("model-inventory", "package-features", "writer-rules", "metadata-order")
    oracle.write_json(args.run / "writer-schema-binding.json", {
        "model_release": "2025.2.3", "current_schema_equivalence_claimed": False,
        "resources": [{"path": f"crates/ibcmd-schema/data/edt-2025.2.3-{name}.json",
                       "sha256": oracle.digest(legacy_schema / f"edt-2025.2.3-{name}.json")} for name in schema_files]})
    native_before = oracle.snapshot(args.native)
    oracle.write_json(args.run / "native-before.json", native_before)
    tools = {name: {"path": str(path), "sha256": oracle.digest(path)}
             for name, path in (("candidate", args.ours_exe), ("edt", args.edt_exe), ("java", args.edt_vm))}
    oracle.write_json(args.run / "tools-before.json", tools)
    cap.command("ours-version", [args.ours_exe, "--version"])
    actual = cap.edt("edt-version", ["version"]).decode("utf-8", errors="strict").strip()
    oracle.write_json(args.run / "edt-version.json", {"actual": actual, "expected": args.edt_build})
    if actual != args.edt_build:
        raise Error(f"Installed EDT exact version mismatch: {actual!r}")
    runtime = {"2.20": "8.3.27", "2.21": "8.5.1"}[args.source_version]
    # A genuine import/export from the same XML supplies the same-serializer control.
    cap.edt("edt-import-native", ["import", "--version", runtime,
        "--configuration-files", str(args.native), "--project-name", "NativeControl", "--build", "true"],
        "native-workspace")
    baseline = args.run / "native-workspace/NativeControl"
    oracle.require_project(baseline, runtime)
    baseline_project_before = oracle.snapshot(baseline)
    oracle.write_json(args.run / "authentic-native-control-project-before.json", baseline_project_before)
    authentic = args.run / "authentic-xml"
    cap.edt("edt-export-native", ["export", "--project", str(baseline),
        "--configuration-files", str(authentic)])
    oracle.require_xml(authentic, require_dump_info=False)
    authentic_before = oracle.snapshot(authentic)
    oracle.write_json(args.run / "authentic-xml.json", authentic_before)
    native_tsv = args.run / "native-validation.tsv"
    cap.edt("edt-validate-native", ["validate", "--file", str(native_tsv), "--project-list", str(baseline)])
    if not native_tsv.is_file():
        raise Error("Authentic native-control validation did not produce TSV")
    native_validation = oracle.summarize_validation_tsv(native_tsv)
    oracle.write_json(args.run / "native-validation.json", native_validation)
    oracle.write_json(args.run / "authentic-native-control-project.json", oracle.snapshot(baseline))
    project = args.run / "generated-project"
    cap.convert("xml-to-edt", args.native, project)
    oracle.require_project(project, runtime)
    project_before = oracle.snapshot(project)
    oracle.write_json(args.run / "generated-project-before.json", project_before)
    stripped = args.run / "generated-without-provenance"
    stripped_before = copy_without_provenance(project, stripped)
    oracle.write_json(args.run / "stripped-project-before.json", stripped_before)
    installed = args.run / "installed-generated-xml"
    cap.edt("edt-export-generated", ["export", "--project", str(stripped),
        "--configuration-files", str(installed)])
    oracle.require_xml(installed, require_dump_info=False)
    installed_before = oracle.snapshot(installed)
    oracle.write_json(args.run / "installed-generated-xml.json", installed_before)
    tsv = args.run / "generated-validation.tsv"
    cap.edt("edt-validate-generated", ["validate", "--file", str(tsv), "--project-list", str(stripped)])
    if not tsv.is_file():
        raise Error("Installed EDT validation did not produce its diagnostic TSV")
    validation = oracle.summarize_validation_tsv(tsv)
    oracle.write_json(args.run / "validation.json", validation)
    validation_comparison = oracle.compare_validation_tsv(native_tsv, tsv)
    oracle.write_json(args.run / "validation-differential.json", validation_comparison)
    ambient = compare_matched_phase_errors(args.run)
    oracle.write_json(args.run / "matched-phase-diagnostics.json", ambient)
    returned = args.run / "returned-xml"
    cap.convert("edt-to-xml", project, returned, reverse=True)
    oracle.require_xml(returned)
    returned_before = oracle.snapshot(returned)
    oracle.write_json(args.run / "returned-xml.json", returned_before)
    exact = oracle.compare_tree_snapshots(native_before, returned_before)
    serialized = oracle.compare_tree_snapshots(authentic_before, installed_before)
    oracle.write_json(args.run / "exact-return-comparison.json", exact)
    oracle.write_json(args.run / "installed-serializer-comparison.json", serialized)
    diagnostics = diagnostics_for_all_logs(args.run, cap.labels)
    oracle.write_json(args.run / "diagnostics.json", diagnostics)
    native_after, project_after, stripped_after = (oracle.snapshot(path) for path in (args.native, project, stripped))
    for label, value in (("native", native_after), ("generated-project", project_after), ("stripped-project", stripped_after)):
        oracle.write_json(args.run / f"{label}-after.json", value)
    tools_unchanged = all(oracle.digest(Path(row["path"])) == row["sha256"] for row in tools.values())
    installation_after = bind_installation(args)
    oracle.write_json(args.run / "installation-binding-after.json", installation_after)
    gates = {"installation_unchanged_and_bound": installation_before == installation_after,
             "launcher_ini_unchanged": oracle.digest(cap.ini) == cap.ini_sha256,
             "exact_original_xml_tree_including_dump_info": exact["equal"],
             "same_installed_serializer_exact_tree": serialized["equal"],
             "authentic_native_no_source_errors_or_unknown_rows": not native_validation["unresolved_source_diagnostics"],
             "authentic_native_project_unchanged": oracle.snapshot(baseline) == baseline_project_before,
             "no_source_errors_or_unknown_validation_rows": not validation["unresolved_source_diagnostics"],
             "no_new_structured_validation_diagnostics": validation_comparison["no_new_diagnostics"],
             "no_new_matched_phase_errors": ambient["no_new_error_records"],
             "no_command_or_workspace_errors": not oracle.has_error_diagnostics(diagnostics.values()),
             "native_unchanged": native_before == native_after,
             "generated_project_unchanged": project_before == project_after,
             "stripped_project_unchanged": stripped_before == stripped_after,
             "tools_unchanged": tools_unchanged,
             "authentic_export_unchanged": oracle.snapshot(authentic) == authentic_before,
             "installed_export_unchanged": oracle.snapshot(installed) == installed_before}
    return {"status": "PASS" if all(gates.values()) else "FAIL", "gates": gates,
        "installed_edt_build": actual, "writer_model_profile": f"edt-2025.2.3-xml-{args.source_version}",
        "model_schema_evidence": "Legacy model profile exercised against the named installed build; no new model/schema profile is asserted",
        "scope": "This supplied corpus and exact tool build only. Exact XML return uses the retained generated project with provenance; installed EDT loads the separate provenance-free copy. This does not claim exact native XML from EDT's own serializer or unrestricted configuration support."}


def execute(args) -> dict:
    """Audit immutable inputs and every published stage even when a route raises."""
    native_before = oracle.snapshot(args.native)
    installation_before = bind_installation(args)
    tool_paths = [args.ours_exe, args.edt_exe, args.edt_vm]
    if args.edt_vm_library is not None:
        tool_paths.append(args.edt_vm_library)
    tools_before = {str(path): oracle.digest(path) for path in tool_paths}
    oracle.write_json(args.run / "terminal-audit-before.json", {
        "native": native_before, "installation": installation_before, "tools": tools_before})
    result, route_error, route_exception = None, None, None
    try:
        result = execute_routes(args)
        return result
    except BaseException as error:
        route_exception = error
        route_error = {"type": type(error).__name__, "message": str(error)}
        raise
    finally:
        audit = {"route_status": result["status"] if result is not None else "FAILED_BEFORE_VERDICT",
                 "route_error": route_error, "trees": {}, "errors": []}
        watched = [
            ("native", args.native, native_before),
            ("authentic-project", args.run / "native-workspace/NativeControl", "authentic-native-control-project-before.json"),
            ("authentic-export", args.run / "authentic-xml", "authentic-xml.json"),
            ("generated-project", args.run / "generated-project", "generated-project-before.json"),
            ("stripped-project", args.run / "generated-without-provenance", "stripped-project-before.json"),
            ("installed-export", args.run / "installed-generated-xml", "installed-generated-xml.json"),
            ("returned-xml", args.run / "returned-xml", "returned-xml.json"),
        ]
        for name, path, baseline in watched:
            try:
                if isinstance(baseline, str):
                    manifest = args.run / baseline
                    baseline = json.loads(manifest.read_text()) if manifest.is_file() else None
                if not path.exists() and baseline is None:
                    audit["trees"][name] = {"exists": False, "compared": False}
                    continue
                current = oracle.snapshot(path)
                oracle.write_json(args.run / f"terminal-{name}.json", current)
                row = {"exists": True, "compared": baseline is not None,
                       "unchanged": current == baseline if baseline is not None else None}
                audit["trees"][name] = row
                if row["unchanged"] is False:
                    audit["errors"].append(f"Immutable {name} changed")
            except Exception as error:
                audit["errors"].append(f"{name}: {type(error).__name__}: {error}")
        try:
            installation_after = bind_installation(args)
            oracle.write_json(args.run / "terminal-installation-binding.json", installation_after)
            audit["installation_unchanged"] = installation_after == installation_before
            if not audit["installation_unchanged"]:
                audit["errors"].append("Installation binding changed")
        except Exception as error:
            audit["installation_unchanged"] = False
            audit["errors"].append(f"installation: {type(error).__name__}: {error}")
        try:
            audit["tools_unchanged"] = all(oracle.digest(Path(path)) == value for path, value in tools_before.items())
            if not audit["tools_unchanged"]:
                audit["errors"].append("Tool payload changed")
        except Exception as error:
            audit["tools_unchanged"] = False
            audit["errors"].append(f"tools: {type(error).__name__}: {error}")
        audit["immutable_audits_passed"] = not audit["errors"]
        audit["scope"] = "Immutability compares available saved baselines; partial stages without baselines are captured only."
        try:
            oracle.write_json(args.run / "terminal-audit.json", audit)
        except Exception as error:
            if route_exception is None:
                raise Error(f"Terminal audit publication failed: {error}") from error
            route_exception.add_note(f"Terminal audit publication failed: {type(error).__name__}: {error}")
        if result is not None:
            result["gates"]["terminal_immutable_audits_passed"] = audit["immutable_audits_passed"]
            if not audit["immutable_audits_passed"]:
                result["status"] = "FAIL"


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    for name in ("lab-root", "native", "run", "ours-exe", "edt-exe", "edt-vm", "edt-installation-manifest"):
        result.add_argument(f"--{name}", type=Path, required=True)
    result.add_argument("--edt-vm-library", type=Path, help="Explicit verified Server VM library; must be included in the installation manifest")
    result.add_argument("--source-version", choices=("2.20", "2.21"), required=True)
    result.add_argument("--edt-build", required=True)
    result.add_argument("--timeout", type=int, default=7200)
    result.add_argument("--heap-gib", type=int, default=4)
    return result


@contextlib.contextmanager
def exclusive_native_slot(args):
    import fcntl  # macOS runtime only; synthetic controls remain host portable.
    lock = args.lab_root / "tmp/edt-native-macos.lock"
    contained(lock, args.lab_root)
    with lock.open("a+b") as slot:
        try:
            fcntl.flock(slot, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise Error("Another EDT/native capture owns the shared slot") from error
        yield


def main():
    args = parser().parse_args()
    def cancelled(signum, frame):
        raise KeyboardInterrupt(f"Capture cancelled by signal {signum}")
    signal.signal(signal.SIGTERM, cancelled)
    created = False
    try:
        preflight(args)
        args.run.mkdir()
        created = True
        for path in (Path(__file__), Path(oracle.__file__)):
            shutil.copyfile(path, args.run / path.name)
        oracle.write_json(args.run / "invocation.json", {
            **{key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()},
            "harness_sha256": oracle.digest(Path(__file__)), "oracle_sha256": oracle.digest(Path(oracle.__file__))})
        with exclusive_native_slot(args):
            result = execute(args)
        oracle.write_json(args.run / "acceptance.json", result)
        print(json.dumps(result, ensure_ascii=False))
        return 0 if result["status"] == "PASS" else 1
    except (Error, OSError, ValueError, KeyError, TypeError, KeyboardInterrupt) as error:
        result = {"status": "FAIL", "error": str(error), "installed_acceptance": False}
        if getattr(error, "__notes__", None):
            result["notes"] = error.__notes__
        if created:
            try:
                oracle.write_json(args.run / "failure.json", result)
            except Exception as receipt_error:
                result["failure_receipt_error"] = f"{type(receipt_error).__name__}: {receipt_error}"
        print(json.dumps(result, ensure_ascii=False), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
