#!/usr/bin/env python3
"""Installed-EDT lab oracle. Never imported by production; inputs remain read-only."""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import xml.etree.ElementTree as ET


class OracleError(RuntimeError):
    pass


def write_json(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write("\n")


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def no_links(path: Path) -> None:
    # Windows junctions are reparse points too; resolve() alone would hide them.
    for part in [path, *path.parents]:
        if part.exists():
            st = part.lstat()
            if part.is_symlink() or getattr(st, "st_file_attributes", 0) & 0x400:
                raise OracleError(f"Symlink/reparse point refused: {part}")


def no_link_entry(path: Path) -> None:
    st = path.lstat()
    if path.is_symlink() or getattr(st, "st_file_attributes", 0) & 0x400:
        raise OracleError(f"Symlink/reparse point refused: {path}")


def snapshot(root: Path, max_files=500000, max_total=32 * 1024**3,
             max_file=1024**3) -> dict:
    no_links(root)
    if not root.is_dir():
        raise OracleError(f"Missing input tree: {root}")
    entries, keys, total = [], set(), 0
    directories = [root]
    while directories:
        directory = directories.pop()
        # DirEntry caches native enumeration metadata on Windows. Repeated
        # Path.stat/is_file/is_symlink calls cost minutes on 140,000-file corpora.
        with os.scandir(directory) as iterator:
            for entry in iterator:
                st = entry.stat(follow_symlinks=False)
                path = Path(entry.path)
                if entry.is_symlink() or getattr(st, "st_file_attributes", 0) & 0x400:
                    raise OracleError(f"Symlink/reparse point refused: {path}")
                if entry.is_dir(follow_symlinks=False):
                    directories.append(path)
                    continue
                if not entry.is_file(follow_symlinks=False):
                    raise OracleError(f"Non-file input: {path}")
                relative = path.relative_to(root).as_posix()
                key = relative.casefold()
                if key in keys:
                    raise OracleError(f"Case-colliding input: {relative}")
                keys.add(key)
                size = st.st_size
                total += size
                if size > max_file or total > max_total or len(entries) >= max_files:
                    raise OracleError(f"Corpus resource limit exceeded: {path}")
                entries.append((path, relative, size))

    def hash_entry(entry):
        path, relative, size = entry
        no_link_entry(path)
        result, read_size = hashlib.sha256(), 0
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                result.update(chunk)
                read_size += len(chunk)
                if read_size > max_file:
                    raise OracleError(f"Corpus resource limit exceeded while reading: {path}")
        if read_size != size:
            raise OracleError(f"Input size changed while hashing: {path}")
        return {"path": relative, "bytes": size, "sha256": result.hexdigest()}

    # Four independent reads hide Windows per-file latency without oversubscribing
    # CPU or launching extra EDT JVMs. Equality is still the complete SHA inventory.
    with ThreadPoolExecutor(max_workers=4) as pool:
        files = list(pool.map(hash_entry, entries))
    if not files:
        raise OracleError(f"Empty tree refused: {root}")
    files.sort(key=lambda row: row["path"])
    tree_hash = hashlib.sha256(json.dumps(files, ensure_ascii=False,
                                        separators=(",", ":")).encode()).hexdigest()
    return {"root": str(root), "file_count": len(files), "total_bytes": total,
            "tree_sha256": tree_hash, "files": files}


def run_command(run: Path, label: str, argv: list[str], timeout: int,
                *, edt=False) -> bytes:
    record = {"argv": argv, "cwd": str(run), "started_utc": time.strftime(
        "%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "timeout_seconds": timeout}
    started = time.monotonic()
    # Pipe bytes, not text: launcher's -help is UTF-16LE; EDT commands emit UTF-8.
    # Files are exclusive so a resumed run cannot silently overwrite evidence.
    with (run / f"{label}.stdout").open("xb") as stdout, \
            (run / f"{label}.stderr").open("xb") as stderr:
        proc = subprocess.Popen(argv, cwd=run, stdout=stdout, stderr=stderr,
                                stdin=subprocess.DEVNULL)
        try:
            record["exit_code"] = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            record["timeout"] = True
            # Kill only the process tree started here. The launcher starts 1cedtc;
            # killing only the launcher would leave a JVM holding the workspace.
            if os.name == "nt":
                subprocess.run(["taskkill", "/PID", str(proc.pid), "/T", "/F"],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               check=False)
            else:
                proc.kill()
            proc.wait(timeout=30)
            record["exit_code"] = proc.returncode
        finally:
            record["duration_seconds"] = round(time.monotonic() - started, 3)
            write_json(run / f"{label}.command.json", record)
    if record.get("timeout") or record["exit_code"] != 0:
        raise OracleError(f"{label} failed: {record}; see preserved logs")
    return (run / f"{label}.stdout").read_bytes()


@contextlib.contextmanager
def heavy_lock(args, run: Path, label: str):
    if not args.lock_script:
        raise OracleError("Heavy lab commands require the shared FIFO lock script")
    argv = ["pwsh", "-NoProfile", "-File", str(args.lock_script)]
    run_command(run, f"{label}-lock-acquire", argv + ["acquire", args.lock_track,
                "-TimeoutMin", str(max(1, args.timeout // 60))], args.timeout + 60)
    try:
        yield
    finally:
        run_command(run, f"{label}-lock-release", argv + ["release", args.lock_track], 60)


def run_conversion(args, run: Path, label: str, argv: list[str]) -> bytes:
    # Full configuration models can consume as much memory as the EDT JVM.
    # Share the same FIFO with each EDT/native command, including a return
    # conversion that may read provenance and rebuild the complete model.
    with heavy_lock(args, run, label):
        return run_command(run, label, argv, args.timeout)


def edt(args, run: Path, label: str, workspace: Path, command: list[str]) -> bytes:
    no_links(workspace)
    with heavy_lock(args, run, label):
        try:
            return run_command(run, label, [str(args.edt_exe), "-data", str(workspace),
                "-timeout", str(args.timeout), "-nl", "en_US", "-vmargs", f"-Xmx{args.heap_gib}g",
                "-command", *command], args.timeout + 120, edt=True)
        finally:
            for name in (".log", "1cedtcli.log", "1cedtcli-shutdown-hook.log"):
                source = workspace / ".metadata" / name
                if source.is_file():
                    with (run / f"{label}.workspace-{name.lstrip('.')}").open("xb") as target:
                        with source.open("rb") as stream:
                            shutil.copyfileobj(stream, target)


def command_diagnostics(run: Path, labels: list[str]) -> dict:
    result = {}
    for label in labels:
        failures = []
        for suffix in ("stdout", "stderr"):
            path = run / f"{label}.{suffix}"
            for line_number, line in enumerate(path.read_bytes().decode("utf-8", errors="replace").splitlines(), 1):
                if re.search(r"\b(?:ERROR|FATAL)\b|^Exception in thread ", line):
                    failures.append({"file": path.name, "line": line_number, "text": line})
        workspace_entries = []
        log = run / f"{label}.workspace-log"
        if log.is_file():
            for line_number, line in enumerate(log.read_bytes().decode("utf-8", errors="replace").splitlines(), 1):
                if re.match(r"!ENTRY \S+ [48] ", line):
                    workspace_entries.append({"file": log.name, "line": line_number, "entry": line})
        result[label] = {"error_lines": failures, "error_count": len(failures),
                         "workspace_error_entries": workspace_entries,
                         "workspace_error_count": len(workspace_entries)}
    return result


def has_error_diagnostics(values) -> bool:
    return any(item["error_count"] or item.get("workspace_error_count", 0) for item in values)


def summarize_validation_tsv(path: Path) -> dict:
    # Installed 2025.2.3.30 emits headerless TSV. Preserve every original byte;
    # classify only observed categories and mark unfamiliar/malformed rows.
    severities, categories, errors, malformed, unknown = {}, {}, [], [], []
    lines = path.read_bytes().decode("utf-8-sig", errors="strict").splitlines()
    for number, line in enumerate(lines, 1):
        fields = line.split("\t")
        if len(fields) < 8 or not re.match(r"^\d{4}-\d{2}-\d{2}T", fields[0]):
            malformed.append({"line": number, "columns": len(fields), "text": line})
            continue
        severity, category = fields[1:3]
        severities[severity] = severities.get(severity, 0) + 1
        categories[category] = categories.get(category, 0) + 1
        row = {"line": number, "severity": severity, "category": category,
               "project": fields[3], "validator": fields[4], "object": fields[5],
               "position": fields[6], "message": "\t".join(fields[7:])}
        if category == "Configuration error":
            errors.append(row)
        elif category not in ("Code style", "Warning"):
            unknown.append(row)
    return {"tsv_sha256": digest(path), "raw_line_count": len(lines),
            "severity_counts": severities, "category_counts": categories,
            "configuration_error_count": len(errors), "configuration_errors": errors,
            "malformed_rows": malformed, "unclassified_rows": unknown,
            "unresolved_source_diagnostics": bool(errors or malformed or unknown)}


def validation_multiset(path: Path) -> Counter:
    result = Counter()
    for number, line in enumerate(path.read_bytes().decode("utf-8-sig").splitlines(), 1):
        fields = line.split("\t")
        if len(fields) < 8 or not re.match(r"^\d{4}-\d{2}-\d{2}T", fields[0]):
            raise OracleError(f"Malformed validation TSV row {number}")
        # Ignore ONLY the execution timestamp and imported project label.
        result[tuple(fields[1:3] + fields[4:])] += 1
    return result


def compare_validation_tsv(original: Path, generated: Path) -> dict:
    before, after = validation_multiset(original), validation_multiset(generated)
    def rows(values):
        return [{"diagnostic": list(row), "count": count} for row, count in sorted(values.items())]
    added, removed = after - before, before - after
    return {"no_new_diagnostics": not added, "exact_multiset_equal": before == after,
            "ignored_columns": ["timestamp", "project_label"],
            "original_sha256": digest(original), "generated_sha256": digest(generated),
            "original_row_count": sum(before.values()), "generated_row_count": sum(after.values()),
            "added_diagnostics": rows(added), "removed_diagnostics": rows(removed)}


def compare_tree_snapshots(left: dict, right: dict, *, excluded_paths=()) -> dict:
    a = {row["path"]: row for row in left["files"]}
    b = {row["path"]: row for row in right["files"]}
    differing, excluded = [], []
    for path in sorted(a.keys() | b.keys()):
        row = {"path": path, "left": a.get(path), "right": b.get(path)}
        if path in excluded_paths:
            excluded.append(row)
        elif a.get(path) != b.get(path):
            differing.append(row)
    return {"equal": not differing, "left_tree_sha256": left["tree_sha256"],
            "right_tree_sha256": right["tree_sha256"], "differing_rows": differing,
            "excluded_paths": list(excluded_paths), "original_excluded_rows": excluded}


def workspace_error_multiset(log: Path) -> Counter:
    if not log.is_file():
        raise OracleError(f"Missing captured workspace diagnostics: {log}")
    result = Counter()
    for block in re.split(r"(?m)(?=^!(?:ENTRY|SESSION)(?:\s|$))", log.read_bytes().decode("utf-8")):
        match = re.match(r"!ENTRY (\S+) ([48]) (\S+) [^\r\n]+\r?\n", block)
        if match:
            # Full plugin/severity/code/message/stack; exclude only header time
            # and separator blank lines. Never collapse message/stack whitespace.
            key = (match[1], match[2], match[3], block[match.end():].rstrip("\r\n"))
            result[key] += 1
        elif block.startswith("!ENTRY"):
            header = block.splitlines()[0].split()
            # A missing timestamp/newline cannot make an error disappear. Even
            # an unreadable severity is a malformed diagnostic, not evidence
            # that the entry was informational. Preserve its complete bytes.
            if len(header) < 3 or header[2] not in ("0", "1", "2"):
                result[("MALFORMED_ENTRY", "4", "unparsed", block)] += 1
    return result


def is_ambient_record(record: tuple) -> bool:
    plugin, severity, code, body = record
    if code != "0":
        return False
    if plugin == "com._1c.g5.v8.dt.core" and severity == "4":
        return body.splitlines()[:3] == ["!MESSAGE Error while reading the library metainformation", "!STACK 0",
            "com._1c.g5.v8.dt.core.library.InvalidLibraryDescriptorException: The library compatibility mode is not specified in the library file"]
    if plugin == "com.e1c.g5.dt.applications.infobases.ui" and severity == "4":
        return body == "!MESSAGE Skipping the lifecycle event because workbench is not running"
    return plugin == "com._1c.g5.v8.dt.common" and severity == "8" and body in (
        "!MESSAGE Updating workspace state", "!MESSAGE Updating infobase states")


def bind_diagnostic_capture(capture: Path, args, *, control=False, project_snapshot=None) -> dict:
    no_links(capture)
    invocation = json.loads((capture / "invocation.json").read_text(encoding="utf-8"))
    if digest(capture / "harness-source.py") != invocation["harness_sha256"]:
        raise OracleError("Diagnostic capture harness identity changed")
    version = json.loads((capture / "edt-version.json").read_text(encoding="utf-8"))
    if version["actual"] != args.edt_build or version["executable_sha256"] != digest(args.edt_exe):
        raise OracleError("Diagnostic capture installed EDT identity mismatch")
    if not control and (invocation["source_version"] != args.source_version or invocation["runtime"] != args.runtime):
        raise OracleError("Diagnostic capture profile mismatch")
    if (capture / "edt-version.stdout").read_bytes().decode("utf-8").strip() != args.edt_build:
        raise OracleError("Diagnostic capture actual version output mismatch")
    label = "edt-control-validate" if control else "edt-validate"
    tsv = capture / ("control-validation.tsv" if control else "validation.tsv")
    result = json.loads((capture / ("control.json" if control else "validation.json")).read_text(encoding="utf-8"))
    if result["status"] != "CAPTURED" or result.get("template_project_unchanged" if control else "prepared_project_unchanged") is not True:
        raise OracleError("Diagnostic capture did not complete with unchanged original")
    labels = ["edt-version", label, "edt-control-export"] if control else ["edt-version", label]
    hashes, raw_hashes = {}, {}
    for stage in labels:
        command_path = capture / f"{stage}.command.json"
        command = json.loads(command_path.read_text(encoding="utf-8"))
        argv = command["argv"]
        operation = "export" if stage.endswith("export") else "version" if stage == "edt-version" else "validate"
        if command["exit_code"] != 0 or command.get("timeout") or Path(argv[0]) != args.edt_exe \
                or "-command" not in argv or argv[argv.index("-command") + 1] != operation:
            raise OracleError("Diagnostic capture command failed or uses another tool")
        workspace = capture / ("version-workspace" if operation == "version" else
                               "control-workspace" if control else "validation-workspace")
        if command.get("cwd") != str(capture) or "-data" not in argv or \
                argv[argv.index("-data") + 1] != str(workspace):
            raise OracleError("Diagnostic command uses another capture/workspace")
        if operation == "validate" and ("--file" not in argv or argv[argv.index("--file") + 1] != str(tsv)):
            raise OracleError("Diagnostic command did not write this exact TSV")
        if operation == "validate":
            source_project = capture / ("EmptyEdtDiagnosticControl" if control else "project-copy")
            if "--project-list" not in argv or argv[argv.index("--project-list") + 1:] != [str(source_project)]:
                raise OracleError("Diagnostic command checked another project")
        if operation == "export" and ("--project-name" not in argv or
                argv[argv.index("--project-name") + 1] != "EmptyEdtDiagnosticControl" or
                "--configuration-files" not in argv or
                argv[argv.index("--configuration-files") + 1] != str(capture / "control-installed-export")):
            raise OracleError("Diagnostic control export belongs to another project/output")
        hashes[stage] = digest(command_path)
        for suffix in ("stdout", "stderr", "workspace-log"):
            raw = capture / f"{stage}.{suffix}"
            raw_hashes[raw.name] = digest(raw)
    if control:
        summary_path = capture / "control-validation-summary.json"
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        if invocation["mode"] != "control" or validation_multiset(tsv) or \
                summary != summarize_validation_tsv(tsv) or summary["unresolved_source_diagnostics"] or \
                result.get("unresolved_source_diagnostics") is not False:
            raise OracleError("Ambient control is not a genuinely validated empty diagnostic project")
        for stage in ("edt-control-validate", "edt-control-export"):
            if command_diagnostics(capture, [stage])[stage]["error_count"] or any(
                    not is_ambient_record(record) for record in workspace_error_multiset(capture / f"{stage}.workspace-log")):
                raise OracleError("Empty diagnostic control contains an unapproved runtime error")
        before = json.loads((capture / "control-project-before.json").read_text(encoding="utf-8"))
        if {row["path"] for row in before["files"]} != {".project", "DT-INF/PROJECT.PMF", "src/Configuration/Configuration.mdo",
                                                       ".settings/org.eclipse.core.resources.prefs"}:
            raise OracleError("Ambient control contains more than an empty configuration scaffold")
        if before != snapshot(capture / "EmptyEdtDiagnosticControl"):
            raise OracleError("Ambient control project changed after its captured validation")
        after_path = capture / "control-project-after.json"
        if after_path.exists() and json.loads(after_path.read_text(encoding="utf-8")) != before:
            raise OracleError("Ambient control project changed during validation/export")
        template_before = json.loads((capture / "template-project-before.json").read_text(encoding="utf-8"))
        template_after = json.loads((capture / "template-project-after.json").read_text(encoding="utf-8"))
        prepared_root = Path(invocation["prepared"])
        prepared = json.loads((prepared_root / "prepared.json").read_text(encoding="utf-8"))
        if template_before != template_after or template_before != snapshot(Path(prepared["project"])) or \
                template_before != json.loads((prepared_root / "authentic-project-after.json").read_text(encoding="utf-8")):
            raise OracleError("Ambient control template identity or immutability mismatch")
        require_xml(capture / "control-installed-export", require_dump_info=False)
        export_snapshot = json.loads((capture / "control-installed-export.json").read_text(encoding="utf-8"))
        if export_snapshot != snapshot(capture / "control-installed-export"):
            raise OracleError("Ambient control installed export changed after capture")
        for name in ("control-validation-summary.json", "control-project-before.json", "template-project-before.json",
                     "template-project-after.json", "control-installed-export.json"):
            hashes[name] = digest(capture / name)
        if after_path.exists():
            hashes[after_path.name] = digest(after_path)
    else:
        if invocation["mode"] != "validate" or digest(tsv) != result["tsv_sha256"]:
            raise OracleError("Validation TSV identity mismatch")
        for name in ("authentic-project-before.json", "authentic-project-after.json"):
            if json.loads((capture / name).read_text(encoding="utf-8")) != project_snapshot:
                raise OracleError("Validation capture belongs to another authentic EDT project")
        copied = json.loads((capture / "validated-project-copy.json").read_text(encoding="utf-8"))
        if Path(copied["root"]) != capture / "project-copy" or any(
                copied[key] != project_snapshot[key] for key in ("files", "file_count", "total_bytes", "tree_sha256")):
            raise OracleError("Validated disposable copy was not byte equal to the authentic project")
    return {"capture": str(capture), "tsv_sha256": digest(tsv), "harness_sha256": invocation["harness_sha256"],
            "capture_runtime": invocation["runtime"], "capture_source_version": invocation["source_version"],
            "command_hashes": hashes, "raw_diagnostic_hashes": raw_hashes,
            "result_sha256": digest(capture / ("control.json" if control else "validation.json"))}


def compare_ambient_diagnostics(control: Path, original: list[tuple[Path, str]],
                                generated: list[tuple[Path, str]]) -> dict:
    known = Counter()
    for label in ("edt-control-validate", "edt-control-export"):
        known.update({key: count for key, count in workspace_error_multiset(control / f"{label}.workspace-log").items()
                      if is_ambient_record(key)})
    def inspect(stages):
        values, stdout_errors = Counter(), []
        for root, label in stages:
            values.update(workspace_error_multiset(root / f"{label}.workspace-log"))
            stdout_errors.extend(command_diagnostics(root, [label])[label]["error_lines"])
        unmatched = {key: count for key, count in values.items() if key not in known}
        return values, unmatched, stdout_errors
    before, before_unknown, before_stdout = inspect(original)
    after, after_unknown, after_stdout = inspect(generated)
    def rows(values):
        return [{"plugin": key[0], "severity": key[1], "code": key[2],
                 "message_and_stack": key[3], "count": count} for key, count in sorted(values.items())]
    return {"no_unmatched_error_diagnostics": not (before_unknown or after_unknown or before_stdout or after_stdout),
            "exact_control_classes": rows(known), "original_workspace_entries": rows(before),
            "generated_workspace_entries": rows(after), "added_workspace_entries": rows(after - before),
            "removed_workspace_entries": rows(before - after),
            "original_unmatched_entries": rows(before_unknown), "generated_unmatched_entries": rows(after_unknown),
            "original_stdout_errors": before_stdout, "generated_stdout_errors": after_stdout,
            "note": "Only exact full records observed in the zero-source-error empty installed-EDT control are classified as ambient; all raw errors remain preserved"}


def require_xml(root: Path, *, require_dump_info=True) -> None:
    names = ("Configuration.xml", "ConfigDumpInfo.xml") if require_dump_info else ("Configuration.xml",)
    for name in names:
        path = root / name
        if not path.is_file() or not path.stat().st_size:
            raise OracleError(f"Incomplete configuration XML: missing/empty {path}")


def require_project(root: Path, runtime: str) -> None:
    for relative in (".project", "DT-INF/PROJECT.PMF", "src/Configuration/Configuration.mdo"):
        path = root / relative
        if not path.is_file() or not path.stat().st_size:
            raise OracleError(f"Incomplete EDT project: {path}")
    manifest = (root / "DT-INF/PROJECT.PMF").read_text(encoding="utf-8-sig")
    if f"Runtime-Version: {runtime}" not in manifest.splitlines():
        raise OracleError(f"Unexpected EDT Runtime-Version: {manifest!r}")


def check_edt_version(args, run: Path) -> str:
    actual = edt(args, run, "edt-version", run / "version-workspace", ["version"])
    text = actual.decode("utf-8", errors="strict").strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+\.\d+", text) or text != args.edt_build \
            or ".".join(text.split(".")[:3]) != args.edt_version:
        raise OracleError(f"Installed EDT version was not positively verified: {text!r}")
    write_json(run / "edt-version.json", {"expected": args.edt_build, "profile_release": args.edt_version, "actual": text,
                                         "executable_sha256": digest(args.edt_exe)})
    return text


def prepare(args, run: Path) -> None:
    require_xml(args.native)
    before = snapshot(args.native)
    write_json(run / "native-before.json", before)
    version = check_edt_version(args, run)
    workspace = run / "authentic-workspace"
    project = workspace / "OracleConfiguration"
    edt(args, run, "edt-import-native", workspace, ["import", "--version", args.runtime,
        "--configuration-files", str(args.native), "--project-name", project.name,
        "--build", "true"])
    require_project(project, args.runtime)
    write_json(run / "authentic-project.json", snapshot(project))
    exported = run / "edt-native-xml"
    edt(args, run, "edt-export-native", workspace, ["export", "--project-name",
        project.name, "--configuration-files", str(exported)])
    # Genuine installed EDT export does not emit ConfigDumpInfo.xml. Account for
    # its absence in raw oracle rows; never synthesize it from native provenance.
    require_xml(exported, require_dump_info=False)
    write_json(run / "edt-native-xml.json", snapshot(exported))
    write_json(run / "authentic-project-after.json", snapshot(project))
    after = snapshot(args.native)
    write_json(run / "native-after.json", after)
    if before != after:
        raise OracleError("EDT modified the original native corpus")
    diagnostics = command_diagnostics(run, ["edt-import-native", "edt-export-native"])
    write_json(run / "edt-diagnostics.json", diagnostics)
    write_json(run / "prepared.json", {"status": "PREPARED_WITH_DIAGNOSTICS" if
        has_error_diagnostics(diagnostics.values()) else "PREPARED", "native": str(args.native),
        "source_version": args.source_version, "native_tool_version": args.native_tool_version,
        "edt_version": version, "edt_profile_release": args.edt_version,
        "runtime": args.runtime, "project": str(project),
        "edt_xml": str(exported), "note": "Preparation is not conversion acceptance"})


def validate_project(args, run: Path) -> None:
    """Capture actual EDT validation independently of conversion acceptance."""
    if not args.prepared:
        raise OracleError("validate requires --prepared")
    prepared_root = args.prepared.absolute()
    no_links(prepared_root)
    prepared = json.loads((prepared_root / "prepared.json").read_text(encoding="utf-8"))
    if prepared["source_version"] != args.source_version or prepared["runtime"] != args.runtime \
            or prepared["edt_version"] != args.edt_build \
            or prepared["edt_profile_release"] != args.edt_version:
        raise OracleError("Validation profile disagrees with prepared project")
    project = Path(prepared["project"])
    if not project.is_relative_to(prepared_root):
        raise OracleError("Prepared project escapes its evidence directory")
    require_project(project, args.runtime)
    before = snapshot(project)
    if before != json.loads((prepared_root / "authentic-project-after.json").read_text(encoding="utf-8")):
        raise OracleError("Prepared project changed before validation")
    write_json(run / "authentic-project-before.json", before)
    check_edt_version(args, run)
    disposable = run / "project-copy"
    shutil.copytree(project, disposable, symlinks=False)
    output = run / "validation.tsv"
    edt(args, run, "edt-validate", run / "validation-workspace", ["validate", "--file",
        str(output), "--project-list", str(disposable)])
    if not output.is_file():
        raise OracleError("Installed EDT validate did not produce the requested TSV")
    no_links(output)
    diagnostics = command_diagnostics(run, ["edt-validate"])
    write_json(run / "edt-diagnostics.json", diagnostics)
    source_diagnostics = summarize_validation_tsv(output)
    write_json(run / "validation-tsv-summary.json", source_diagnostics)
    after = snapshot(project)
    write_json(run / "authentic-project-after.json", after)
    if before != after:
        raise OracleError("Validation modified the immutable prepared project")
    write_json(run / "validated-project-copy.json", snapshot(disposable))
    write_json(run / "validation.json", {"status": "CAPTURED",
        "tsv": str(output), "tsv_sha256": digest(output), "tsv_bytes": output.stat().st_size,
        "prepared_project_unchanged": True,
        "unresolved_source_diagnostics": source_diagnostics["unresolved_source_diagnostics"],
        "unresolved_edt_error_diagnostics": has_error_diagnostics(diagnostics.values()),
        "note": "Raw installed-EDT TSV and workspace diagnostics require inspection; this capture is not acceptance PASS"})


def empty_project_control(args, run: Path) -> None:
    if not args.prepared:
        raise OracleError("control requires an authentic --prepared template")
    prepared_root = args.prepared.absolute()
    no_links(prepared_root)
    prepared = json.loads((prepared_root / "prepared.json").read_text(encoding="utf-8"))
    project = Path(prepared["project"])
    if not project.is_relative_to(prepared_root) or prepared["runtime"] != args.runtime:
        raise OracleError("Control template profile/path mismatch")
    before = snapshot(project)
    require_project(project, args.runtime)
    if before != json.loads((prepared_root / "authentic-project-after.json").read_text(encoding="utf-8")):
        raise OracleError("Authentic template changed before control")
    write_json(run / "template-project-before.json", before)
    version = check_edt_version(args, run)
    control = run / "EmptyEdtDiagnosticControl"
    (control / "DT-INF").mkdir(parents=True)
    (control / "src/Configuration").mkdir(parents=True)
    shutil.copyfile(project / "DT-INF/PROJECT.PMF", control / "DT-INF/PROJECT.PMF")
    # Installed EDT otherwise adds this standard encoding setting during
    # import. Copy the genuine template setting before taking the source hash.
    (control / ".settings").mkdir()
    shutil.copyfile(project / ".settings/org.eclipse.core.resources.prefs",
                    control / ".settings/org.eclipse.core.resources.prefs")
    descriptor = ET.parse(project / ".project")
    project_name = descriptor.getroot().find("name")
    if project_name is None:
        raise OracleError("Authentic project descriptor has no name")
    project_name.text = control.name
    descriptor.write(control / ".project", encoding="utf-8", xml_declaration=True)
    metadata = ET.parse(project / "src/Configuration/Configuration.mdo")
    kept = {"name", "containedObjects", "configurationExtensionCompatibilityMode", "defaultRunMode",
            "usePurposes", "scriptVariant", "defaultLanguage", "dataLockControlMode",
            "objectAutonumerationMode", "modalityUseMode", "interfaceCompatibilityMode",
            "compatibilityMode", "languages"}
    for child in list(metadata.getroot()):
        if child.tag not in kept:
            metadata.getroot().remove(child)
    configuration_name = metadata.getroot().find("name")
    if configuration_name is None:
        raise OracleError("Authentic configuration has no name")
    configuration_name.text = control.name
    metadata.write(control / "src/Configuration/Configuration.mdo", encoding="utf-8", xml_declaration=True)
    write_json(run / "control-project-before.json", snapshot(control))
    workspace = run / "control-workspace"
    tsv = run / "control-validation.tsv"
    edt(args, run, "edt-control-validate", workspace, ["validate", "--file", str(tsv),
        "--project-list", str(control)])
    if not tsv.is_file():
        raise OracleError("Control validation produced no TSV")
    summary = summarize_validation_tsv(tsv)
    write_json(run / "control-validation-summary.json", summary)
    exported = run / "control-installed-export"
    edt(args, run, "edt-control-export", workspace, ["export", "--project-name", control.name,
        "--configuration-files", str(exported)])
    require_xml(exported, require_dump_info=False)
    write_json(run / "control-installed-export.json", snapshot(exported))
    control_after = snapshot(control)
    if control_after != json.loads((run / "control-project-before.json").read_text(encoding="utf-8")):
        raise OracleError("EDT modified the empty diagnostic control source project")
    write_json(run / "control-project-after.json", control_after)
    diagnostics = command_diagnostics(run, ["edt-control-validate", "edt-control-export"])
    write_json(run / "edt-diagnostics.json", diagnostics)
    after = snapshot(project)
    if before != after:
        raise OracleError("Control modified the original authentic template")
    write_json(run / "template-project-after.json", after)
    write_json(run / "control.json", {"status": "CAPTURED", "edt_version": version,
        "unresolved_source_diagnostics": summary["unresolved_source_diagnostics"],
        "unresolved_edt_error_diagnostics": has_error_diagnostics(diagnostics.values()),
        "template_project_unchanged": True,
        "note": "Synthetic empty-project diagnostic control only. It neither represents BSP/UH acceptance nor automatically waives any log error."})


def validate_raw_report(path: Path) -> dict:
    report = json.loads(path.read_text(encoding="utf-8"))
    rows = report["rows"]
    counts = report["summary"]
    if not rows or len({row["path"] for row in rows}) != len(rows):
        raise OracleError("Empty/duplicate oracle rows")
    if sum(counts.values()) != len(rows):
        raise OracleError("Incomplete oracle summary")
    actual = {key: sum(row["agreement"] == key for row in rows) for key in counts}
    if actual != counts:
        raise OracleError("Oracle row and summary verdicts disagree")
    if any(report[key]["file_count"] <= 0 for key in ("native", "edt", "ours")):
        raise OracleError("Oracle accepted an empty input tree")
    excluded = [row for row in rows if row["path"] == "ConfigDumpInfo.xml"]
    configuration = [row for row in rows if row["path"] != "ConfigDumpInfo.xml"]
    return {"summary": counts, "row_count": len(rows),
            "raw_all_equal": counts["all_equal"] == len(rows),
            "configuration_data_all_equal": bool(configuration) and all(
                row["agreement"] == "all_equal" for row in configuration),
            "derived_comparison": {"excluded_paths": ["ConfigDumpInfo.xml"],
                "reason": "Installed EDT does not emit native storage-generation dump information",
                "original_excluded_rows": excluded, "raw_report_sha256": digest(path),
                "configuration_rows": len(configuration),
                "different_rows": sum(row["agreement"] != "all_equal" for row in configuration)}}


def validate_native_reference(reference: Path, baseline: dict, native_build: str,
                              current_reference: dict) -> dict:
    """Bind the native branch to the completed fresh-DB capture, not a copied tree."""
    capture = reference.parent
    no_links(capture)
    def evidence(name):
        path = capture / name
        no_links(path)
        data = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            raise OracleError(f"Native reference evidence must be an object: {name}")
        return data
    result = evidence("result.json")
    invocation = evidence("invocation.json")
    if result.get("status") != "CAPTURED" or result.get("input_unchanged") is not True:
        raise OracleError("Native reference capture did not complete")
    database = result.get("database", "")
    if not re.fullmatch(r"ibcmd_rs_04_edt07_[a-z0-9_]+", database) \
            or invocation.get("database") != database:
        raise OracleError("Native reference is not a fresh database owned by the EDT lab")
    if result.get("native_build") != native_build or invocation.get("native_build") != native_build:
        raise OracleError("Native reference exact build mismatch")
    if Path(result["reference"]).absolute() != reference or result["reference_sha256"] != current_reference["tree_sha256"]:
        raise OracleError("Native reference result belongs to another output")
    if evidence("input-before.json") != baseline or evidence("input-after.json") != baseline \
            or Path(invocation["input"]).absolute() != Path(baseline["root"]):
        raise OracleError("Native reference was not loaded from this exact installed-EDT XML export")
    if evidence("native-reference.json") != current_reference:
        raise OracleError("Native export manifest does not match the current reference")
    source = capture / "harness-source.py"
    if invocation["harness_sha256"] != digest(source):
        raise OracleError("Native reference harness evidence changed")
    native = Path(invocation["ibcmd"])
    no_links(native)
    if native.name.casefold() != "ibcmd.exe" or digest(native) != result["executable_sha256"]:
        raise OracleError("Native reference executable identity changed")
    if (capture / "native-version.stdout").read_bytes().decode("utf-8").strip() != native_build:
        raise OracleError("Native reference actual version output mismatches its declared build")
    operations = {
        "native-version": ["--version"],
        "native-create": ["infobase", "create"],
        "native-import": ["infobase", "config", "import"],
        "native-export": ["infobase", "config", "export"],
    }
    commands = {}
    for label in ["fresh-database", *operations]:
        record = evidence(f"{label}.command.json")
        if record.get("exit_code") != 0 or record.get("timeout") or record.get("timeout_or_log_limit"):
            raise OracleError(f"Native reference required command failed: {label}")
        argv = record["argv"]
        if label == "fresh-database":
            helper = Path(invocation["restore_script"])
            if "-File" not in argv or argv.index("-File") + 1 >= len(argv) \
                    or Path(argv[argv.index("-File") + 1]) != helper:
                raise OracleError("Native reference used a different fresh-database helper")
            for flag, expected in (("-Corpus", "empty"), ("-Name", database), ("-Track", "edt-native")):
                if flag not in argv or argv.index(flag) + 1 >= len(argv) or argv[argv.index(flag) + 1] != expected:
                    raise OracleError("Native reference fresh-database ownership command mismatch")
        else:
            if Path(argv[0]) != native or argv[1:1 + len(operations[label])] != operations[label]:
                raise OracleError(f"Native reference command identity mismatch: {label}")
            if label != "native-version" and f"--db-name={database}" not in argv:
                raise OracleError(f"Native reference command targets another database: {label}")
            if label != "native-version" and any(flag not in argv for flag in (
                    "--dbms=MSSQLServer", "--db-server=localhost", f"--data={capture / 'ibdata'}")):
                raise OracleError(f"Native reference command targets another server/cache: {label}")
            if label == "native-import" and str(Path(baseline["root"])) not in argv:
                raise OracleError("Native import command did not use this exact EDT export")
            if label == "native-export" and str(reference) not in argv:
                raise OracleError("Native export command did not create this reference")
        commands[label] = digest(capture / f"{label}.command.json")
    activation = result.get("activation_probe", {"status": "NOT_REQUESTED"})
    if invocation.get("probe_apply", False):
        probe = evidence("native-apply.command.json")
        argv = probe["argv"]
        if probe.get("timeout") or probe.get("timeout_or_log_limit") or Path(argv[0]) != native \
                or argv[1:4] != ["infobase", "config", "apply"] \
                or f"--db-name={database}" not in argv \
                or any(flag not in argv for flag in ("--dbms=MSSQLServer", "--db-server=localhost", f"--data={capture / 'ibdata'}")) \
                or activation.get("exit_code") != probe["exit_code"] \
                or activation.get("status") != ("PASS" if probe["exit_code"] == 0 else "FAIL"):
            raise OracleError("Optional activation probe evidence does not match its recorded outcome")
        commands["optional-native-apply"] = digest(capture / "native-apply.command.json")
    elif activation.get("status") != "NOT_REQUESTED":
        raise OracleError("Unrequested activation outcome was supplied")
    return {"capture": str(capture), "database": database, "native_build": native_build,
            "result_sha256": digest(capture / "result.json"), "command_evidence_sha256": commands,
            "activation_probe": activation,
            "source_edt_xml_tree_sha256": baseline["tree_sha256"],
            "native_reference_tree_sha256": current_reference["tree_sha256"]}


def accept(args, run: Path) -> None:
    if not args.prepared or not args.ours_exe or not args.reference or not args.validation_capture or not args.ambient_control:
        raise OracleError("accept requires --prepared, --ours-exe, --reference, --validation-capture and --ambient-control")
    prepared_root = args.prepared.absolute()
    no_links(prepared_root)
    prepared = json.loads((prepared_root / "prepared.json").read_text(encoding="utf-8"))
    if prepared["status"] not in ("PREPARED", "PREPARED_WITH_DIAGNOSTICS") or Path(prepared["native"]) != args.native:
        raise OracleError("Prepared run is incomplete or belongs to another native corpus")
    if prepared["source_version"] != args.source_version or prepared["runtime"] != args.runtime:
        raise OracleError("Prepared run profile mismatch")
    if prepared["native_tool_version"] != args.native_tool_version \
            or prepared["edt_version"] != args.edt_build \
            or prepared["edt_profile_release"] != args.edt_version:
        raise OracleError("Prepared run exact tool provenance mismatch")
    native_before = snapshot(args.native)
    original = json.loads((prepared_root / "native-before.json").read_text(encoding="utf-8"))
    if native_before != original:
        raise OracleError("Native input changed since EDT preparation")
    project = Path(prepared["project"])
    baseline_xml = Path(prepared["edt_xml"])
    if not project.is_relative_to(prepared_root) or not baseline_xml.is_relative_to(prepared_root):
        raise OracleError("Prepared project/output escaped its disposable run")
    require_project(project, args.runtime)
    require_xml(baseline_xml, require_dump_info=False)
    project_before = snapshot(project)
    validation_capture = args.validation_capture.absolute()
    ambient_control = args.ambient_control.absolute()
    validation_binding = bind_diagnostic_capture(validation_capture, args, project_snapshot=project_before)
    control_binding = bind_diagnostic_capture(ambient_control, args, control=True)
    write_json(run / "baseline-validation-binding.json", validation_binding)
    write_json(run / "ambient-control-binding.json", control_binding)
    baseline_before = snapshot(baseline_xml)
    if baseline_before != json.loads((prepared_root / "edt-native-xml.json").read_text(encoding="utf-8")):
        raise OracleError("Installed EDT baseline export changed")
    reference = args.reference.absolute()
    if reference == args.native or reference.is_relative_to(prepared_root):
        raise OracleError("Native reference must be an independent post-EDT native load/export capture")
    require_xml(reference)
    reference_before = snapshot(reference)
    reference_binding = validate_native_reference(reference, baseline_before,
                                                 args.native_tool_version, reference_before)
    write_json(run / "native-reference-binding.json", reference_binding)
    write_json(run / "native-post-edt-reference.json", reference_before)
    write_json(run / "native-before.json", native_before)
    write_json(run / "authentic-project-before.json", project_before)
    ours_hash = digest(args.ours_exe)
    ours_version = run_command(run, "ours-version", [str(args.ours_exe), "--version"], 60).decode("utf-8").strip()
    actual_edt_version = check_edt_version(args, run)
    profile_xml = f"xml-{args.source_version}"
    profile_edt = f"edt-{args.edt_version}-xml-{args.source_version}"
    converted_xml = run / "ours-authentic-edt-xml"
    generated = run / "ours-generated-edt"
    for label, source, target, source_format, target_format, source_profile, target_profile in (
        ("convert-authentic-edt", project, converted_xml, "edt", "xml", profile_edt, profile_xml),
        ("convert-native-xml", args.native, generated, "xml", "edt", profile_xml, profile_edt),
    ):
        run_conversion(args, run, label, [str(args.ours_exe), "convert", str(source), str(target),
            "--source-format", source_format, "--target-format", target_format,
            "--source-profile", source_profile, "--target-profile", target_profile,
            "--report", str(run / f"{label}.report.json")])
    # Authentic EDT carries no native storage-generation dump manifest. Direct
    # conversion is complete configuration data without inventing such a file.
    require_xml(converted_xml, require_dump_info=False)
    require_project(generated, args.runtime)
    converted_before = snapshot(converted_xml)
    write_json(run / "ours-authentic-edt-xml.json", converted_before)
    write_json(run / "ours-generated-edt.json", snapshot(generated))
    returned_xml = run / "ours-unchanged-return-xml"
    run_conversion(args, run, "convert-generated-edt-unchanged", [str(args.ours_exe), "convert", str(generated), str(returned_xml),
        "--source-format", "edt", "--target-format", "xml", "--source-profile", profile_edt,
        "--target-profile", profile_xml, "--report", str(run / "convert-generated-edt-unchanged.report.json")])
    require_xml(returned_xml)
    returned_before = snapshot(returned_xml)
    write_json(run / "ours-unchanged-return-xml.json", returned_before)
    unchanged_return = compare_tree_snapshots(native_before, returned_before)
    write_json(run / "unchanged-return-exact-comparison.json", unchanged_return)
    stripped = run / "generated-edt-without-provenance"
    # Validate and copy the complete project. Only the disposable copy is edited.
    shutil.copytree(generated, stripped, symlinks=False)
    provenance = stripped / ".ibcmd-provenance"
    if not provenance.is_dir():
        raise OracleError("Generated EDT provenance missing: the stripping control is unproven")
    no_links(provenance)
    if provenance.resolve().parent != stripped.resolve():
        raise OracleError("Unsafe provenance removal target")
    shutil.rmtree(provenance)
    stripped_before = snapshot(stripped)
    write_json(run / "stripped-project-before.json", stripped_before)
    if any(row["path"].split("/")[0] == ".ibcmd-provenance" for row in stripped_before["files"]):
        raise OracleError("Provenance survived stripping")
    generated_xml = run / "edt-generated-xml"
    edt(args, run, "edt-export-generated", run / "generated-workspace", ["export", "--project",
        str(stripped), "--configuration-files", str(generated_xml)])
    require_xml(generated_xml, require_dump_info=False)
    generated_before = snapshot(generated_xml)
    write_json(run / "edt-generated-xml.json", generated_before)
    generated_tsv = run / "generated-validation.tsv"
    edt(args, run, "edt-validate-generated", run / "generated-workspace", ["validate", "--file", str(generated_tsv),
        "--project-list", str(stripped)])
    if not generated_tsv.is_file():
        raise OracleError("Generated project validate did not produce TSV")
    validation_comparison = compare_validation_tsv(validation_capture / "validation.tsv", generated_tsv)
    write_json(run / "generated-validation-summary.json", summarize_validation_tsv(generated_tsv))
    write_json(run / "validation-differential.json", validation_comparison)
    write_json(run / "stripped-project-after.json", snapshot(stripped))
    verdicts = {}
    for label, candidate in (("authentic-edt-to-xml", converted_xml),
                             ("generated-edt-installed-export", generated_xml)):
        report_path = run / f"{label}.three-way.json"
        run_command(run, f"{label}-oracle", [str(args.ours_exe), "source-three-way-oracle",
            "--native", str(reference), "--edt", str(baseline_xml), "--ours", str(candidate),
            "--source-version", args.source_version, "--native-tool-version", args.native_tool_version,
            "--edt-tool-version", actual_edt_version, "--ours-tool-version", ours_version,
            "--max-files", "500000", "--max-total-bytes", str(32 * 1024**3),
            "--max-file-bytes", str(1024**3), "--output", str(report_path),
            "--markdown", str(run / f"{label}.three-way.md")], args.timeout)
        verdicts[label] = validate_raw_report(report_path)
    if snapshot(args.native) != native_before or snapshot(project) != project_before \
            or snapshot(baseline_xml) != baseline_before or snapshot(reference) != reference_before:
        raise OracleError("An immutable input changed during acceptance")
    if digest(args.ours_exe) != ours_hash:
        raise OracleError("Candidate executable changed during acceptance")
    if bind_diagnostic_capture(validation_capture, args, project_snapshot=project_before) != validation_binding \
            or bind_diagnostic_capture(ambient_control, args, control=True) != control_binding:
        raise OracleError("Diagnostic capture evidence changed during acceptance")
    # Older preparation captures predate workspace severity collection. Read
    # their preserved raw stage logs rather than trusting an obsolete summary.
    original_diagnostics = command_diagnostics(prepared_root,
                                               ["edt-import-native", "edt-export-native"])
    write_json(run / "prepared-diagnostics-recomputed.json", original_diagnostics)
    generated_diagnostics = command_diagnostics(run, ["edt-export-generated", "edt-validate-generated"])
    write_json(run / "edt-diagnostics.json", generated_diagnostics)
    ambient_comparison = compare_ambient_diagnostics(ambient_control,
        [(prepared_root, "edt-import-native"), (prepared_root, "edt-export-native"), (validation_capture, "edt-validate")],
        [(run, "edt-export-generated"), (run, "edt-validate-generated")])
    write_json(run / "ambient-diagnostic-differential.json", ambient_comparison)
    direct_comparison = compare_tree_snapshots(reference_before, converted_before, excluded_paths=("ConfigDumpInfo.xml",))
    generated_comparison = compare_tree_snapshots(baseline_before, generated_before)
    write_json(run / "direct-edt-native-sdk-comparison.json", direct_comparison)
    write_json(run / "generated-edt-same-serializer-comparison.json", generated_comparison)
    diagnostic_failure = not ambient_comparison["no_unmatched_error_diagnostics"] or not validation_comparison["no_new_diagnostics"]
    passed = not diagnostic_failure and direct_comparison["equal"] and generated_comparison["equal"] and unchanged_return["equal"]
    result = {"status": "PASS" if passed else "FAIL",
        "verdicts": verdicts, "ours_version": ours_version, "ours_executable_sha256": ours_hash,
        "route_criteria": {"direct_edt_matches_post_edt_native_sdk": direct_comparison["equal"],
            "generated_edt_matches_authentic_installed_edt_serializer": generated_comparison["equal"],
            "unchanged_return_exact_including_dump_info": unchanged_return["equal"]},
        "structured_validation_no_new_diagnostics": validation_comparison["no_new_diagnostics"],
        "edt_version": actual_edt_version, "native_unchanged": True,
        "native_reference": str(reference),
        "native_reference_scope": "Native export after loading the authentic installed-EDT XML export; native capture runs outside this offline comparator",
        "unresolved_edt_error_diagnostics": diagnostic_failure,
        "provenance_removed_for_installed_export": True,
        "note": "Raw divergences require investigation; EDT rc=0 is not equality acceptance"}
    write_json(run / "acceptance.json", result)
    if result["status"] != "PASS":
        raise OracleError("Route equality or diagnostic regression gate failed; see acceptance.json")


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("mode", choices=("prepare", "validate", "control", "accept"))
    result.add_argument("--native", type=Path, required=True)
    result.add_argument("--source-version", choices=("2.20", "2.21"), required=True)
    result.add_argument("--runtime", choices=("8.3.27", "8.5.1"), required=True)
    result.add_argument("--native-tool-version", required=True)
    result.add_argument("--edt-exe", type=Path, required=True)
    result.add_argument("--edt-version", required=True)
    result.add_argument("--edt-build", required=True, help="Exact verified installed build, e.g. 2025.2.3.30")
    result.add_argument("--run", type=Path, required=True)
    result.add_argument("--timeout", type=int, default=7200)
    result.add_argument("--lock-script", type=Path, required=True)
    result.add_argument("--lock-track", default="edt-oracle")
    result.add_argument("--prepared", type=Path)
    result.add_argument("--ours-exe", type=Path)
    result.add_argument("--reference", type=Path, help="Independent native export after loading authentic EDT project/export")
    result.add_argument("--validation-capture", type=Path, help="Completed structured validation of this authentic prepared EDT project")
    result.add_argument("--ambient-control", type=Path, help="Completed zero-source-error empty installed-EDT diagnostic control")
    result.add_argument("--heap-gib", type=int, default=8)
    return result


def main() -> int:
    args = parser().parse_args()
    args.native = args.native.absolute()
    args.run = args.run.absolute()
    created = False
    try:
        if os.name != "nt" or args.run.drive.upper() != "F:":
            raise OracleError("This installed-EDT lab harness requires a disposable F: run")
        no_links(args.run)
        if args.run == args.native or args.run.is_relative_to(args.native) or args.native.is_relative_to(args.run):
            raise OracleError("Run and immutable native input must not overlap")
        if args.timeout < 1 or not 1 <= args.heap_gib <= 64 or "_" in args.lock_track or not args.lock_track:
            raise OracleError("Invalid timeout/lock track")
        expected = {"2.20": "8.3.27", "2.21": "8.5.1"}[args.source_version]
        if expected != args.runtime:
            raise OracleError("Explicit XML profile and runtime disagree")
        args.run.mkdir(parents=True, exist_ok=False)
        created = True
        harness_source = Path(__file__).read_bytes()
        (args.run / "harness-source.py").write_bytes(harness_source)
        write_json(args.run / "invocation.json", {
            **{key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()},
            "harness_sha256": hashlib.sha256(harness_source).hexdigest()})
        if args.mode == "prepare":
            prepare(args, args.run)
        elif args.mode == "validate":
            validate_project(args, args.run)
        elif args.mode == "control":
            empty_project_control(args, args.run)
        else:
            accept(args, args.run)
        status = json.loads((args.run / "prepared.json").read_text(encoding="utf-8"))["status"] \
            if args.mode == "prepare" else "CAPTURED" if args.mode in ("validate", "control") else "PASS"
        print(f"{status}: {args.run}", flush=True)
        return 0
    except (OracleError, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        if created and args.run.is_dir() and not (args.run / "failure.json").exists():
            write_json(args.run / "failure.json", {"status": "FAIL", "error": str(error)})
        print(f"FAIL: {error}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())
