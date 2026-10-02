#!/usr/bin/env python3
"""Installed-EDT lab oracle. Never imported by production; inputs remain read-only."""
from __future__ import annotations

import argparse
import calendar
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import contextlib
from contextvars import ContextVar
import hashlib
import json
import math
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


_FIFO_FILES = ContextVar("edt_oracle_fifo_files", default=None)


@contextlib.contextmanager
def fifo_snapshot_scope(args, run: Path):
    """Opt-in fair tickets for each complete snapshot/copy/removal operation."""
    if not getattr(args, "fifo_snapshots", False):
        yield
        return
    if _FIFO_FILES.get() is not None:
        raise OracleError("Nested FIFO file-operation scope refused")
    token = _FIFO_FILES.set({"args": args, "run": run, "serial": 0, "held": False})
    try:
        yield
    finally:
        _FIFO_FILES.reset(token)


@contextlib.contextmanager
def fair_file_operation(kind: str, **details):
    state = _FIFO_FILES.get()
    receipt = {"operation": kind, **details}
    if state is None:
        yield receipt
        return
    if state["held"]:
        raise OracleError("Full file operation inside an acquired heavy ticket refused")
    state["serial"] += 1
    label = f"fifo-file-{state['serial']:06d}-{kind}"
    started = time.monotonic()
    receipt.update(label=label, status="FAIL", queued_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()))
    try:
        with heavy_lock(state["args"], state["run"], label):
            receipt["acquired_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            yield receipt
            receipt["status"] = "CAPTURED"
    except BaseException as error:
        receipt.update(status="FAIL", error_type=type(error).__name__)
        raise
    finally:
        receipt["wall_seconds_including_queue"] = round(time.monotonic() - started, 3)
        write_json(state["run"] / f"{label}.json", receipt)


def snapshot(root: Path, max_files=500000, max_total=32 * 1024**3,
             max_file=1024**3) -> dict:
    with fair_file_operation("snapshot", root=str(root), max_files=max_files,
                             max_total_bytes=max_total, max_file_bytes=max_file) as receipt:
        result = _snapshot_impl(root, max_files, max_total, max_file)
        receipt.update(tree_sha256=result["tree_sha256"], file_count=result["file_count"],
                       total_bytes=result["total_bytes"])
        return result


def _snapshot_impl(root: Path, max_files=500000, max_total=32 * 1024**3,
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
    state = _FIFO_FILES.get()
    if state is not None and state["held"]:
        raise OracleError("Nested heavy ticket inside FIFO file scope refused")
    argv = ["pwsh", "-NoProfile", "-File", str(args.lock_script)]
    run_command(run, f"{label}-lock-acquire", argv + ["acquire", args.lock_track,
                "-TimeoutMin", str(max(1, args.timeout // 60))], args.timeout + 60)
    if state is not None:
        state["held"] = True
    try:
        yield
    finally:
        try:
            run_command(run, f"{label}-lock-release", argv + ["release", args.lock_track], 60)
        finally:
            if state is not None:
                state["held"] = False


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


def workspace_append_delta(previous: bytes, current: bytes) -> bytes:
    """Extract a complete call boundary; never cancel or rewrite log records."""
    if not current.startswith(previous):
        raise OracleError("Workspace log was reset/truncated or its captured prefix changed")
    for value in (previous, current):
        value.decode("utf-8", errors="strict")
        if value and not value.endswith(b"\n"):
            raise OracleError("Partial workspace diagnostic record at call boundary")
    delta = current[len(previous):]
    if delta.strip() and not delta.lstrip(b"\r\n").startswith((b"!SESSION", b"!ENTRY")):
        raise OracleError("Workspace phase begins inside a diagnostic record")
    for value in (previous, delta):
        validate_warm_workspace_records(value)
    return delta


def validate_warm_workspace_records(value: bytes) -> None:
    """Malformed warm evidence is fatal even if both sides share the defect."""
    for block in re.split(r"(?m)(?=^!(?:ENTRY|SESSION)(?:\s|$))", value.decode("utf-8", errors="strict")):
        if block.startswith("!ENTRY"):
            if not re.match(r"!ENTRY (\S+) ([01248]) (\S+) [^\r\n]+\r?\n", block) \
                    or not re.search(r"(?m)^!MESSAGE [^\r\n]*\S[^\r\n]*\r?\n", block):
                raise OracleError("Malformed/partial workspace ENTRY header or MESSAGE at warm boundary")


WARM_REGISTRATION_IDENTITIES = {
    ("com._1c.g5.v8.dt.bsl.ui", "IBslDocumentProviderExtension", "com.e1c.langtool.v8.dt.internal.bsl.ui.i18n.extension.BslContentWriterExtension"),
    ("com._1c.g5.v8.dt.core", "IResourceContentExporterExtension", "com.e1c.langtool.v8.dt.internal.bp.scheme.i18n.extension.GraphicalSchemeV8ExporterExtension"),
    ("com._1c.g5.v8.dt.core", "IResourceContentExporterExtension", "com.e1c.langtool.v8.dt.internal.core.ext.BmContentExporterExtension"),
    ("com._1c.g5.v8.dt.core", "IResourceContentExporterExtension", "com.e1c.langtool.v8.dt.internal.moxel.i18n.extension.MoxelV8ExporterExtension"),
    ("com._1c.g5.v8.dt.core", "IResourceContentImporterExtension", "com.e1c.langtool.v8.dt.internal.core.ext.BmContentImporterExtension"),
    ("com._1c.g5.v8.dt.core", "IResourceContentImporterExtension", "com.e1c.langtool.v8.dt.internal.moxel.i18n.extension.MoxelV8ImporterExtension"),
    ("com._1c.g5.v8.dt.ide", "IProjectFileSystemSupportProvider", "com.e1c.langtool.v8.dt.internal.htmldocument.HtmlProjectFileSystemSupportExtension$HtmlProjectFileSystemSupport"),
    ("com._1c.g5.v8.dt.ide", "IProjectFileSystemSupportProvider", "com.e1c.langtool.v8.dt.internal.md.help.HelpPageProjectFileSystemSupportExtension$HelpPageProjectFileSystemSupport"),
    ("com._1c.g5.v8.dt.md.export.xml", "IXmlExporterExtension", "com.e1c.langtool.v8.dt.internal.bp.scheme.i18n.xml.extension.GraphicalSchemeXmlExporterExtension"),
    ("com._1c.g5.v8.dt.md.export.xml", "IXmlExporterExtension", "com.e1c.langtool.v8.dt.internal.dcs.i18n.xml.DcsXmlExporterExtension"),
    ("com._1c.g5.v8.dt.md.export.xml", "IXmlExporterExtension", "com.e1c.langtool.v8.dt.internal.moxel.i18n.xml.extension.MoxelXmlExporterExtension"),
}


def warm_workspace_multiset(log: Path, command_path: Path) -> tuple[Counter, list[dict]]:
    """ALL entries; only proven JVM identities and verified command context vary."""
    raw = log.read_bytes()
    validate_warm_workspace_records(raw)
    command = json.loads(command_path.read_text(encoding="utf-8"))
    argv = command["argv"]
    result, lexical = Counter(), []
    for block in re.split(r"(?m)(?=^!(?:ENTRY|SESSION)(?:\s|$))", raw.decode("utf-8")):
        match = re.match(r"!ENTRY (\S+) ([01248]) (\S+) [^\r\n]+\r?\n", block)
        if not match:
            continue
        body = block[match.end():].rstrip("\r\n")
        current = body
        if (match[2], match[3]) == ("2", "0"):
            for plugin, interface, implementation in WARM_REGISTRATION_IDENTITIES:
                fixed = f"!MESSAGE The external {interface} is registered: {implementation}@"
                if match[1] == plugin and re.fullmatch(re.escape(fixed) + r"[0-9a-f]{1,8}", body):
                    current = fixed + "<JVM-IDENTITY>"
                    break
        if (match[1], match[2], match[3]) == ("com.e1c.g5.v8.dt.cli.api", "1", "0") \
                and len(argv) >= 6 and argv[-6:-3] == ["-command", "validate", "--file"] \
                and argv[-2] == "--project-list":
            fixed_command = f'validate --file "{argv[-3]}" --project-list "{argv[-1]}"'
            if body in ("!MESSAGE Command to run:\n" + fixed_command, "!MESSAGE Command to run:\r\n" + fixed_command):
                current = "!MESSAGE Command to run:\n<BOUND VALIDATE PASS COMMAND>"
        result[(match[1], match[2], match[3], current)] += 1
        if body != current:
            lexical.append({"plugin": match[1], "severity": match[2], "code": match[3],
                "raw_message_and_stack": body, "comparison_message_and_stack": current,
                "kind": "verified_pass_command" if current.endswith("<BOUND VALIDATE PASS COMMAND>") else "known_registration_jvm_identity",
                "command_sha256": digest(command_path),
                "raw_body_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest()})
    return result, lexical


WARM_XTEXT_LOGGER = "org.eclipse.xtext.conversion.impl.AbstractLexerBasedConverter"
WARM_XTEXT_MESSAGE = "Only terminal rules are supported by lexer based converters but got ID which is an instance of ParserRule"


def warm_stream_multiset(run: Path, label: str) -> tuple[Counter, list[dict]]:
    """Every stream line is claimed; no arbitrary warning/error is omitted."""
    result, lexical = Counter(), []
    for suffix in ("stdout", "stderr"):
        raw = (run / f"{label}.{suffix}").read_bytes()
        text = raw.decode("utf-8", errors="strict")
        if raw and not raw.endswith(b"\n"):
            raise OracleError("Malformed/partial warm validation stream; raw output retained")
        for line in text.splitlines():
            if not line.strip():
                continue
            match = re.fullmatch(r"([0-9]+) +\[derived_data_executor_([0-9]+)\] +WARN +"
                + re.escape(WARM_XTEXT_LOGGER) + r" +- " + re.escape(WARM_XTEXT_MESSAGE), line)
            if suffix != "stdout" or not match:
                raise OracleError("Unclassified warm validation stdout/stderr; raw output retained")
            result[("WARN", WARM_XTEXT_LOGGER, WARM_XTEXT_MESSAGE)] += 1
            lexical.append({"kind": "known_xtext_converter_warn", "stream": suffix,
                "raw_line": line, "elapsed_milliseconds": match[1], "executor_number": match[2],
                "raw_line_sha256": hashlib.sha256(line.encode("utf-8")).hexdigest()})
    return result, lexical


def warm_validation_evidence(args, run: Path, project: Path, workspace: Path,
                             prefix: str, stem: str, passes: int, original: Path | None = None) -> dict:
    """Recompute every capture fact; supplied summaries never authorize a pass."""
    if passes < 2:
        raise OracleError("Invalid warm validation evidence pass count")
    before = snapshot(project)
    original_before = snapshot(original) if original else None
    if original_before and any(before[key] != original_before[key] for key in ("files", "file_count", "total_bytes", "tree_sha256")):
        raise OracleError("Warm copy differs from immutable authentic source")
    labels, records, workspace_counters, stream_counters, previous = [], [], [], [], b""
    for index in range(1, passes + 1):
        label = f"{prefix}-pass-{index:03d}"
        labels.append(label)
        tsv = run / f"{stem}-pass-{index:03d}.tsv"
        command = json.loads((run / f"{label}.command.json").read_text(encoding="utf-8"))
        expected = [str(args.edt_exe), "-data", str(workspace), "-timeout", str(args.timeout),
                    "-nl", "en_US", "-vmargs", f"-Xmx{args.heap_gib}g", "-command", "validate",
                    "--file", str(tsv), "--project-list", str(project)]
        if command.get("argv") != expected or command.get("cwd") != str(run) \
                or command.get("exit_code") != 0 or command.get("timeout") \
                or command.get("timeout_or_log_limit"):
            raise OracleError("Warm validation command/tool/workspace/project binding mismatch")
        fifo_hashes = {}
        for action in ("acquire", "release"):
            lock_path = run / f"{label}-lock-{action}.command.json"
            lock = json.loads(lock_path.read_text(encoding="utf-8"))
            expected_lock = ["pwsh", "-NoProfile", "-File", str(args.lock_script), action, args.lock_track]
            if action == "acquire":
                expected_lock.extend(["-TimeoutMin", str(max(1, args.timeout // 60))])
            if lock.get("argv") != expected_lock or lock.get("cwd") != str(run) \
                    or lock.get("exit_code") != 0 or lock.get("timeout") or lock.get("timeout_or_log_limit"):
                raise OracleError("Warm FIFO command ownership/action binding mismatch")
            fifo_hashes[action] = digest(lock_path)
        for phase in ("before", "after"):
            captured = json.loads((run / f"{label}.project-{phase}.json").read_text(encoding="utf-8"))
            if captured != before:
                raise OracleError("Warm validation modified or checked a different source project")
            if original and json.loads((run / f"{label}.immutable-{phase}.json").read_text(encoding="utf-8")) != original_before:
                raise OracleError("Immutable authentic source changed during a warm pass")
        stream_counter, stream_lexical = warm_stream_multiset(run, label)
        stream_counters.append(stream_counter)
        validation_multiset(tsv)  # malformed rows are always fatal
        raw = (run / f"{label}.workspace-log").read_bytes()
        delta = workspace_append_delta(previous, raw)
        phase_path = run / f"{label}.workspace-phase-log"
        if phase_path.read_bytes() != delta:
            raise OracleError("Warm validation phase boundary bytes changed")
        workspace_counter, lexical = warm_workspace_multiset(phase_path, run / f"{label}.command.json")
        workspace_counters.append(workspace_counter)
        record = {"label": label, "command_sha256": digest(run / f"{label}.command.json"),
                  "fifo_command_sha256": fifo_hashes,
                  "workspace_lexical_context": lexical,
                  "stream_lexical_context": stream_lexical,
                  "tsv": tsv.name, "tsv_sha256": digest(tsv), "workspace_log_sha256": digest(run / f"{label}.workspace-log"),
                  "workspace_prefix_bytes": len(previous), "workspace_phase_bytes": len(delta),
                  "workspace_phase_sha256": digest(phase_path), "project_sha256": before["tree_sha256"],
                  "before_sha256": digest(run / f"{label}.project-before.json"),
                  "after_sha256": digest(run / f"{label}.project-after.json"),
                  "stdout_sha256": digest(run / f"{label}.stdout"), "stderr_sha256": digest(run / f"{label}.stderr")}
        record["auxiliary_workspace_logs_sha256"] = {suffix: digest(run / f"{label}.{suffix}")
            for suffix in ("workspace-1cedtcli.log", "workspace-1cedtcli-shutdown-hook.log")
            if (run / f"{label}.{suffix}").exists()}
        if original:
            record.update({"immutable_before_sha256": digest(run / f"{label}.immutable-before.json"),
                           "immutable_after_sha256": digest(run / f"{label}.immutable-after.json")})
        records.append(record)
        previous = raw
    first, final = run / records[-2]["tsv"], run / records[-1]["tsv"]
    stable = validation_multiset(first) == validation_multiset(final)
    return {"version": 1, "passes": passes, "project": str(project), "workspace": str(workspace),
            "edt_executable_sha256": digest(args.edt_exe), "edt_build": args.edt_build,
            "source_version": args.source_version, "runtime": args.runtime,
            "immutable_original": str(original) if original else None,
            "final_consecutive_all_diagnostics_stable": stable,
            "final_consecutive_all_workspace_records_stable": workspace_counters[-2] == workspace_counters[-1],
            "final_consecutive_stream_records_stable": stream_counters[-2] == stream_counters[-1],
            "records": records,
            "final_label": labels[-1], "final_tsv": final.name}


def warm_validate(args, run: Path, project: Path, workspace: Path, prefix: str, stem: str,
                  original: Path | None = None) -> dict:
    passes = args.warm_validation_passes
    if passes < 2 or workspace.exists():
        raise OracleError("Warm validation requires >=2 passes and a fresh dedicated workspace")
    artifacts = [run / f"{stem}.tsv", run / f"{stem}-warm.json"]
    for index in range(1, passes + 1):
        label = f"{prefix}-pass-{index:03d}"
        artifacts.append(run / f"{stem}-pass-{index:03d}.tsv")
        artifacts.extend(run / f"{label}.{suffix}" for suffix in ("command.json", "stdout", "stderr",
            "workspace-log", "workspace-phase-log", "project-before.json", "project-after.json",
            "immutable-before.json", "immutable-after.json", "workspace-1cedtcli.log", "workspace-1cedtcli-shutdown-hook.log"))
        artifacts.extend(run / f"{label}-lock-{action}.{suffix}" for action in ("acquire", "release")
                         for suffix in ("command.json", "stdout", "stderr"))
    if any(path.exists() for path in artifacts):
        raise OracleError("Warm validation refuses existing capture artifacts; use a fresh run")
    before, previous = snapshot(project), b""
    original_before = snapshot(original) if original else None
    for index in range(1, passes + 1):
        label = f"{prefix}-pass-{index:03d}"
        tsv = run / f"{stem}-pass-{index:03d}.tsv"
        write_json(run / f"{label}.project-before.json", snapshot(project))
        if original:
            write_json(run / f"{label}.immutable-before.json", snapshot(original))
            if snapshot(original) != original_before:
                raise OracleError("Immutable source changed between warm passes")
        if snapshot(project) != before:
            raise OracleError("Warm validation source changed between passes")
        edt(args, run, label, workspace, ["validate", "--file", str(tsv), "--project-list", str(project)])
        write_json(run / f"{label}.project-after.json", snapshot(project))
        if original:
            write_json(run / f"{label}.immutable-after.json", snapshot(original))
            if snapshot(original) != original_before:
                raise OracleError("Warm validation modified immutable authentic source")
        if snapshot(project) != before:
            raise OracleError("Warm validation modified its immutable source copy")
        if not tsv.is_file():
            raise OracleError("Warm validation did not produce requested TSV")
        raw = (run / f"{label}.workspace-log").read_bytes()
        delta = workspace_append_delta(previous, raw)
        with (run / f"{label}.workspace-phase-log").open("xb") as stream:
            stream.write(delta)
        previous = raw
    evidence = warm_validation_evidence(args, run, project, workspace, prefix, stem, passes, original)
    write_json(run / f"{stem}-warm.json", evidence)
    if not evidence["final_consecutive_all_diagnostics_stable"] \
            or not evidence["final_consecutive_all_workspace_records_stable"] \
            or not evidence["final_consecutive_stream_records_stable"]:
        raise OracleError("Final consecutive ALL diagnostic Counters/workspace records are not stable; every pass retained")
    with (run / f"{stem}.tsv").open("xb") as target:
        with (run / evidence["final_tsv"]).open("rb") as source:
            shutil.copyfileobj(source, target)
    return evidence


def bind_warm_validation(args, run: Path, project: Path, workspace: Path,
                         prefix: str, stem: str, original: Path | None = None) -> dict:
    evidence = warm_validation_evidence(args, run, project, workspace, prefix, stem,
                                        args.warm_validation_passes, original)
    recorded = json.loads((run / f"{stem}-warm.json").read_text(encoding="utf-8"))
    if evidence != recorded or not evidence["final_consecutive_all_diagnostics_stable"] \
            or not evidence["final_consecutive_all_workspace_records_stable"] \
            or not evidence["final_consecutive_stream_records_stable"] \
            or digest(run / f"{stem}.tsv") != evidence["records"][-1]["tsv_sha256"]:
        raise OracleError("Warm capture summary/stability/final TSV disagrees with complete records")
    return evidence


def compare_warm_phase_errors(control: Path, baseline: Path, generated: Path,
                              before: dict, after: dict) -> dict:
    if before["passes"] != after["passes"] or len(before["records"]) != before["passes"] \
            or len(after["records"]) != after["passes"]:
        raise OracleError("Baseline/candidate warm pass strategy differs")
    known = set()
    for label in ("edt-control-validate", "edt-control-export"):
        known.update(record for record in workspace_error_multiset(control / f"{label}.workspace-log")
                     if is_ambient_record(record))
    phases = []
    def rows(counter):
        return [{"plugin": key[0], "severity": key[1], "code": key[2], "message_and_stack": key[3], "count": count}
                for key, count in sorted(counter.items())]
    for original, candidate in zip(before["records"], after["records"]):
        source_phase = baseline / f"{original['label']}.workspace-phase-log"
        target_phase = generated / f"{candidate['label']}.workspace-phase-log"
        validate_warm_workspace_records(source_phase.read_bytes())
        validate_warm_workspace_records(target_phase.read_bytes())
        a, source_lexical = warm_workspace_multiset(source_phase, baseline / f"{original['label']}.command.json")
        b, target_lexical = warm_workspace_multiset(target_phase, generated / f"{candidate['label']}.command.json")
        a = Counter({key: count for key, count in a.items() if key not in known})
        b = Counter({key: count for key, count in b.items() if key not in known})
        structured = compare_validation_tsv(baseline / original["tsv"], generated / candidate["tsv"])
        source_streams, source_stream_lexical = warm_stream_multiset(baseline, original["label"])
        target_streams, target_stream_lexical = warm_stream_multiset(generated, candidate["label"])
        phases.append({"baseline_label": original["label"], "candidate_label": candidate["label"],
                       "all_tsv_diagnostics": structured,
                       "baseline_lexical_context": source_lexical, "candidate_lexical_context": target_lexical,
                       "baseline_stream_lexical_context": source_stream_lexical, "candidate_stream_lexical_context": target_stream_lexical,
                       "baseline_stream_records": [{"severity": key[0], "logger": key[1], "message": key[2], "count": count} for key, count in sorted(source_streams.items())],
                       "candidate_stream_records": [{"severity": key[0], "logger": key[1], "message": key[2], "count": count} for key, count in sorted(target_streams.items())],
                       "new_stream_record_count": sum((target_streams - source_streams).values()),
                       "baseline_tsv_unresolved": summarize_validation_tsv(baseline / original["tsv"])["unresolved_source_diagnostics"],
                       "candidate_tsv_unresolved": summarize_validation_tsv(generated / candidate["tsv"])["unresolved_source_diagnostics"],
                       "baseline_nonambient": rows(a), "candidate_nonambient": rows(b),
                       "new_nonambient": rows(b - a), "removed_nonambient": rows(a - b)})
    return {"no_new_nonambient_errors": not any(phase["new_nonambient"] for phase in phases),
            "no_new_all_tsv_diagnostics": all(phase["all_tsv_diagnostics"]["no_new_diagnostics"] for phase in phases),
            "no_new_stream_records": not any(phase["new_stream_record_count"] for phase in phases),
            "clean_source": not any(any(row["severity"] in ("4", "8") for row in phase["baseline_nonambient"]) or phase["baseline_tsv_unresolved"] for phase in phases),
            "clean_generated_validation": not any(any(row["severity"] in ("4", "8") for row in phase["candidate_nonambient"]) or phase["candidate_tsv_unresolved"] for phase in phases),
            "phases": phases, "workspace_record_scope": "ALL ENTRY severities 0/1/2/4/8; closed registration JVM identity and bound validation command context only",
            "note": "Each phase compares complete records/counts separately; no cross-phase cancellation or error waiver"}


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
    warm_passes = 0 if control else invocation.get("warm_validation_passes", 0)
    if not control and warm_passes != getattr(args, "warm_validation_passes", 0):
        raise OracleError("Baseline/candidate warm validation pass options disagree")
    warm = None
    if warm_passes:
        if invocation["heap_gib"] != args.heap_gib or invocation["timeout"] != args.timeout:
            raise OracleError("Baseline/candidate warm validation command resource options disagree")
        capture_args = argparse.Namespace(**{**vars(args), "timeout": invocation["timeout"],
                                             "heap_gib": invocation["heap_gib"],
                                             "lock_script": Path(invocation["lock_script"]),
                                             "lock_track": invocation["lock_track"]})
        warm = bind_warm_validation(capture_args, capture, capture / "project-copy",
                    capture / "validation-workspace", "edt-validate", "validation", Path(project_snapshot["root"]))
    label = "edt-control-validate" if control else "edt-validate"
    tsv = capture / ("control-validation.tsv" if control else "validation.tsv")
    result = json.loads((capture / ("control.json" if control else "validation.json")).read_text(encoding="utf-8"))
    if result["status"] != "CAPTURED" or result.get("template_project_unchanged" if control else "prepared_project_unchanged") is not True:
        raise OracleError("Diagnostic capture did not complete with unchanged original")
    labels = ["edt-version", label, "edt-control-export"] if control else \
        ["edt-version", *[record["label"] for record in warm["records"]]] if warm else ["edt-version", label]
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
        stage_tsv = capture / next(record["tsv"] for record in warm["records"] if record["label"] == stage) \
            if warm and operation == "validate" else tsv
        if operation == "validate" and ("--file" not in argv or argv[argv.index("--file") + 1] != str(stage_tsv)):
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
        if warm and copied != snapshot(capture / "project-copy"):
            raise OracleError("Warm validation source copy changed since the captured passes")
        if warm:
            for record in warm["records"]:
                for name in (record["tsv"], f"{record['label']}.workspace-phase-log",
                             f"{record['label']}.project-before.json", f"{record['label']}.project-after.json",
                             f"{record['label']}.immutable-before.json", f"{record['label']}.immutable-after.json"):
                    hashes[name] = digest(capture / name)
            hashes["validation-warm.json"] = digest(capture / "validation-warm.json")
    return {"capture": str(capture), "tsv_sha256": digest(tsv), "harness_sha256": invocation["harness_sha256"],
            "capture_runtime": invocation["runtime"], "capture_source_version": invocation["source_version"],
            "command_hashes": hashes, "raw_diagnostic_hashes": raw_hashes,
            "result_sha256": digest(capture / ("control.json" if control else "validation.json")),
            **({"warm_validation": warm} if warm else {})}


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
    with fair_file_operation("copy-project", source=str(project), target=str(disposable)):
        shutil.copytree(project, disposable, symlinks=False)
    output = run / "validation.tsv"
    warm = None
    if getattr(args, "warm_validation_passes", 0):
        warm = warm_validate(args, run, disposable, run / "validation-workspace", "edt-validate", "validation", project)
    else:
        edt(args, run, "edt-validate", run / "validation-workspace", ["validate", "--file",
            str(output), "--project-list", str(disposable)])
    if not output.is_file():
        raise OracleError("Installed EDT validate did not produce the requested TSV")
    no_links(output)
    diagnostics = command_diagnostics(run, [record["label"] for record in warm["records"]] if warm else ["edt-validate"])
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
        **({"warm_validation_passes": warm["passes"],
            "warm_final_label": warm["final_label"], "warm_all_diagnostics_stable": True} if warm else {}),
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


AFFINITY1_CAPTURE_PINS = {
    "harness-source.py": ("harness_sha256", "4f25467815b42d4e2621cdf209e6c4a05325a59b4096cccddf4c87d44a3ff869"),
    "oracle.py": ("oracle_sha256", "e74311f5c501b907db7fa7386896b3efc376ba586db1226267cc381c32d97985"),
    "restore-clone-source.ps1": ("restore_sha256", "dd04bcc443e67134263d16a8c005f90c7188f205fe1388c0af5047873049a478"),
    "heavy-lock-source.ps1": ("lock_sha256", "a4e35ef94eaed6b5636e1f0f441d15f1b143efab3720fcb0dac8651dfeb8a7a3"),
}


def validate_affinity1_native_reference(reference: Path, baseline: dict,
                                        native_build: str, current_reference: dict) -> dict:
    """Bind the reviewed lab experiment; never change its NOT_ACCEPTED outcome.

    This mode proves a completed fresh native load/export and observed process
    affinity. Sampling cannot prove internal scheduling or the cause of earlier
    failures. It does not alter the converter or the standard native capture.
    """
    capture = reference.parent
    no_links(capture)
    def evidence(name):
        path = capture / name
        no_links(path)
        value = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(value, dict):
            raise OracleError(f"Affinity native evidence must be an object: {name}")
        return value
    result, invocation = evidence("result.json"), evidence("invocation.json")
    database = "ibcmd_rs_04_edt07_uha85_affinity_r1"
    track = "edt-uha85-affinity"
    flags = ("all_native_commands_rc0_and_observed_affinity1", "input_unchanged",
             "native_unchanged", "helpers_unchanged", "frozen_helpers_unchanged")
    if result.get("status") != "CAPTURED_NOT_ACCEPTED" or any(result.get(k) is not True for k in flags) \
            or result.get("database") != database or invocation.get("database") != database \
            or invocation.get("scope") != "EXPERIMENT_ONLY_NOT_ACCEPTANCE" \
            or invocation.get("track") != track or result.get("initial_affinity_hex") != "1" \
            or invocation.get("initial_affinity_hex") != "1" or invocation.get("timeout") != 21600 \
            or invocation.get("start_documentation") != "https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/start":
        raise OracleError("Affinity native experiment did not complete with its reviewed scope")
    if native_build != "8.5.1.1150" or invocation.get("native_build") != native_build:
        raise OracleError("Affinity native exact build mismatch")
    if reference != capture / "native-xml" or evidence("native-reference.json") != current_reference \
            or Path(current_reference["root"]) != reference \
            or evidence("input-before.json") != baseline or evidence("input-after.json") != baseline \
            or Path(invocation["input"]) != Path(baseline["root"]):
        raise OracleError("Affinity native source/output full inventories disagree")
    pins = {}
    for name, (key, expected) in AFFINITY1_CAPTURE_PINS.items():
        no_links(capture / name)
        if invocation.get(key) != expected or digest(capture / name) != expected:
            raise OracleError(f"Unreviewed/changed affinity capture helper: {name}")
        pins[name] = expected
    native, restore, lock = (Path(invocation[k]) for k in ("native", "restore_script", "lock_script"))
    for path, key in ((native, "native_sha256"), (restore, "restore_sha256"), (lock, "lock_sha256")):
        no_links(path)
        if not path.is_absolute() or ".." in path.parts or digest(path) != invocation[key]:
            raise OracleError("Affinity native tool/helper identity changed")
    if native.name.casefold() != "ibcmd.exe" \
            or (capture / "native-version.stdout").read_bytes().decode("utf-8").strip() != native_build:
        raise OracleError("Affinity native executable/version mismatch")
    prior_path = Path(invocation["baseline_invocation"])
    no_links(prior_path)
    if digest(prior_path) != invocation["baseline_invocation_sha256"]:
        raise OracleError("Independent affinity input binding changed")
    prior = json.loads(prior_path.read_text(encoding="utf-8"))
    if Path(prior["input"]) != Path(baseline["root"]) or Path(prior["ibcmd"]) != native \
            or prior["native_build"] != native_build \
            or json.loads((prior_path.parent / "input-before.json").read_text(encoding="utf-8")) != baseline:
        raise OracleError("Affinity capture differs from the independent genuine input binding")
    common = ["--dbms=MSSQLServer", "--db-server=localhost", f"--db-name={database}",
              f"--data={capture / 'ibdata'}"]
    argv_by_label = {
        "native-version": [str(native), "--version"],
        "fresh-database": ["pwsh", "-NoProfile", "-File", str(capture / "restore-clone-source.ps1"),
            "-Corpus", "empty", "-Name", database, "-Track", track, "-Purpose",
            "UH85 initial CPU affinity1 research, fresh DB only, not acceptance"],
        "native-create": [str(native), "infobase", "create", *common, "--locale=ru_RU"],
        "native-import": [str(native), "infobase", "config", "import", *common, baseline["root"]],
        "native-export": [str(native), "infobase", "config", "export", *common, "--threads=4", str(reference)],
    }
    commands, intervals = {}, {}
    def command(label, expected, affinity=False):
        record = evidence(f"{label}.command.json")
        if record.get("argv") != expected or record.get("cwd") != str(capture) \
                or type(record.get("exit_code")) is not int or record["exit_code"] != 0 \
                or type(record.get("launcher_exit_code")) is not int or record["launcher_exit_code"] != 0 \
                or record.get("timeout") or record.get("timeout_or_log_limit") \
                or record.get("timeout_seconds") != 21600 \
                or type(record.get("launcher_pid")) is not int or record["launcher_pid"] <= 0 \
                or record.get("creationflags") != "CREATE_NO_WINDOW" or record.get("stdin") != "DEVNULL" \
                or record.get("initial_affinity_hex") != ("1" if affinity else None):
            raise OracleError(f"Affinity native required command scope/outcome mismatch: {label}")
        for stream in ("stdout", "stderr"):
            path = capture / f"{label}.{stream}"
            no_links(path)
            if digest(path) != record.get(f"{stream}_sha256"):
                raise OracleError(f"Affinity native raw stream changed: {label}.{stream}")
        duration = record.get("duration_seconds")
        if type(duration) not in (float, int) or not math.isfinite(duration) or not 0 < duration <= 21630:
            raise OracleError("Affinity native command duration invalid")
        start = calendar.timegm(time.strptime(record["started_utc"], "%Y-%m-%dT%H:%M:%SZ"))
        if affinity:
            # The launcher can return zero when ibcmd fails. Its held child
            # handle exit and mask observations are both mandatory evidence.
            observation = record.get("affinity_observation", {})
            if not isinstance(observation, dict):
                raise OracleError("Malformed affinity child observation")
            children = observation.get("observed_children", [])
            if not isinstance(children, list) or any(not isinstance(row, dict) or not isinstance(row.get("image"), str) for row in children):
                raise OracleError("Malformed affinity observed children")
            matches = [row for row in children if Path(row["image"]) == native]
            if observation.get("one_expected_native_child") is not True \
                    or observation.get("mask_one_observed") is not True \
                    or observation.get("expected_image") != str(native.resolve()).casefold() \
                    or type(observation.get("observed_native_exit_code")) is not int \
                    or observation["observed_native_exit_code"] != 0 or len(matches) != 1:
                raise OracleError("Affinity native authoritative child proof missing")
            child = matches[0]
            if type(child.get("pid")) is not int or child["pid"] <= 0 \
                    or type(record.get("launcher_pid")) is not int or record["launcher_pid"] <= 0 \
                    or type(child.get("parent_pid")) is not int \
                    or child.get("parent_pid") != record["launcher_pid"] or child["pid"] == record["launcher_pid"] \
                    or type(child.get("observed_exit_code")) is not int or child["observed_exit_code"] != 0 \
                    or child.get("observed_process_masks") != [1] \
                    or type(child["observed_process_masks"][0]) is not int \
                    or type(child.get("samples")) is not int or child["samples"] < 1 \
                    or not child.get("observed_system_masks") \
                    or any(type(mask) is not int or mask < 1 or not mask & 1 for mask in child["observed_system_masks"]):
                raise OracleError("Affinity native child identity/exit/mask mismatch")
            first, last = (child.get(k) for k in ("first_sample_unix", "last_sample_unix"))
            if any(type(value) not in (float, int) or not math.isfinite(value) for value in (first, last)) \
                    or not start - 2 <= first <= last <= start + duration + 2:
                raise OracleError("Affinity native sampling interval is outside the command")
            launch = record.get("launch")
            suffix = ' /d /v:off /s /c "start "" /b /wait /affinity 1 ' + ' '.join('"' + arg + '"' for arg in expected) + '"'
            if any(re.search(r'["%!^&|<>\r\n\x00]', arg) for arg in expected) \
                    or not isinstance(launch, str) or not launch.endswith(suffix) \
                    or not re.fullmatch(r'"[A-Za-z]:\\(?:[^"\\]+\\)*System32\\cmd\.exe"',
                                        launch[:-len(suffix)], re.IGNORECASE):
                raise OracleError("Affinity native documented hidden start launcher mismatch")
        elif record.get("launch") != expected or "affinity_observation" in record:
            raise OracleError("Unexpected affinity launcher for direct command")
        commands[label] = digest(capture / f"{label}.command.json")
        intervals[label] = (start, start + duration)
        return record
    for label, expected in argv_by_label.items():
        command(label, expected, label in ("native-create", "native-import", "native-export"))
    # Snapshots and each operation acquire/release separately; no combined lock
    # or command to an existing/unrelated database is accepted by this mode.
    labels = ["input-before", *argv_by_label, "native-reference", "input-after"]
    for label in labels:
        names = ["heavy", "native"] if label in argv_by_label else ["heavy"]
        for name in names:
            prefix = ["pwsh", "-NoProfile", "-File", str(lock)]
            command(f"{label}-{name}-acquire", [*prefix, "acquire", track, "-Name", name, "-TimeoutMin", "360"])
            command(f"{label}-{name}-release", [*prefix, "release", track, "-Name", name])
        if label not in argv_by_label:
            record = evidence(f"{label}.command.json")
            argv = record["argv"]
            tree = reference if label == "native-reference" else Path(baseline["root"])
            if not argv or Path(argv[0]).name.casefold() != "python.exe":
                raise OracleError("Affinity snapshot interpreter mismatch")
            command(label, [argv[0], str(capture / "harness-source.py"), "--snapshot", str(tree),
                            "--snapshot-output", str(capture / f"{label}.json")])
        operation_start, operation_end = intervals[label]
        for name in names:
            acquired = intervals[f"{label}-{name}-acquire"]
            released = intervals[f"{label}-{name}-release"]
            # Recorded starts have one-second precision, durations and samples
            # retain fractions. Permit only that timestamp quantization.
            if acquired[1] > operation_start + 2 or operation_end > released[0] + 2:
                raise OracleError("Affinity native operation is outside its FIFO ownership interval")
        if len(names) == 2 and (intervals[f"{label}-heavy-acquire"][1] > intervals[f"{label}-native-acquire"][0] + 2 \
                or intervals[f"{label}-native-release"][1] > intervals[f"{label}-heavy-release"][0] + 2):
            raise OracleError("Affinity native lock nesting differs from reviewed helper")
    if any(intervals[f"{left}-heavy-release"][1] > intervals[f"{right}-heavy-acquire"][0] + 2
           for left, right in zip(labels, labels[1:])):
        raise OracleError("Affinity native operations are not separately ordered")
    return {"mode": "affinity1", "capture": str(capture), "database": database, "native_build": native_build,
        "capture_status": result["status"], "result_sha256": digest(capture / "result.json"),
        "reviewed_helpers_sha256": pins, "command_evidence_sha256": commands,
        "source_edt_xml_tree_sha256": baseline["tree_sha256"],
        "native_reference_tree_sha256": current_reference["tree_sha256"],
        "activation_probe": {"status": "NOT_REQUESTED"},
        "scope": "Fresh genuine-input native import/export, documented initial affinity1 and sampled child masks; no causality or unsampled scheduling proof; converter acceptance remains separate"}


def validate_native_reference(reference: Path, baseline: dict, native_build: str,
                              current_reference: dict, mode: str = "standard") -> dict:
    """Bind the native branch to the completed fresh-DB capture, not a copied tree."""
    if mode == "affinity1":
        return validate_affinity1_native_reference(reference, baseline, native_build, current_reference)
    if mode != "standard":
        raise OracleError("Unknown native reference mode")
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


def capture_raw_comparison(args, run: Path, label: str, candidate: Path,
                           reference: Path, baseline: Path, ours_version: str,
                           edt_version: str) -> dict:
    report_path = run / f"{label}.three-way.json"
    run_conversion(args, run, f"{label}-oracle", [str(args.ours_exe), "source-three-way-oracle",
        "--native", str(reference), "--edt", str(baseline), "--ours", str(candidate),
        "--source-version", args.source_version, "--native-tool-version", args.native_tool_version,
        "--edt-tool-version", edt_version, "--ours-tool-version", ours_version,
        "--max-files", "500000", "--max-total-bytes", str(32 * 1024**3),
        "--max-file-bytes", str(1024**3), "--output", str(report_path),
        "--markdown", str(run / f"{label}.three-way.md")])
    return validate_raw_report(report_path)


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
                                                 args.native_tool_version, reference_before,
                                                 getattr(args, "native_reference_mode", "standard"))
    write_json(run / "native-reference-binding.json", reference_binding)
    write_json(run / "native-post-edt-reference.json", reference_before)
    write_json(run / "native-before.json", native_before)
    write_json(run / "authentic-project-before.json", project_before)
    ours_hash = digest(args.ours_exe)
    ours_version = run_command(run, "ours-version", [str(args.ours_exe), "--version"], 60).decode("utf-8").strip()
    write_json(run / "candidate.json", {"executable": str(args.ours_exe),
        "executable_sha256": ours_hash, "version": ours_version})
    actual_edt_version = check_edt_version(args, run)
    profile_xml = f"xml-{args.source_version}"
    profile_edt = f"edt-{args.edt_version}-xml-{args.source_version}"
    converted_xml = run / "ours-authentic-edt-xml"
    generated = run / "ours-generated-edt"
    verdicts = {}
    for label, source, target, source_format, target_format, source_profile, target_profile in (
        ("convert-authentic-edt", project, converted_xml, "edt", "xml", profile_edt, profile_xml),
        ("convert-native-xml", args.native, generated, "xml", "edt", profile_xml, profile_edt),
    ):
        run_conversion(args, run, label, [str(args.ours_exe), "convert", str(source), str(target),
            "--source-format", source_format, "--target-format", target_format,
            "--source-profile", source_profile, "--target-profile", target_profile,
            "--report", str(run / f"{label}.report.json")])
        if label == "convert-authentic-edt":
            # Preserve this route's evidence before starting the independent
            # reverse route, which may reject an unsupported source artifact.
            require_xml(converted_xml, require_dump_info=False)
            converted_before = snapshot(converted_xml)
            write_json(run / "ours-authentic-edt-xml.json", converted_before)
            direct_comparison = compare_tree_snapshots(reference_before, converted_before,
                                                       excluded_paths=("ConfigDumpInfo.xml",))
            write_json(run / "direct-edt-native-sdk-comparison.json", direct_comparison)
            verdicts["authentic-edt-to-xml"] = capture_raw_comparison(args, run,
                "authentic-edt-to-xml", converted_xml, reference, baseline_xml,
                ours_version, actual_edt_version)
    # Authentic EDT carries no native storage-generation dump manifest. Direct
    # conversion is complete configuration data without inventing such a file.
    require_xml(converted_xml, require_dump_info=False)
    require_project(generated, args.runtime)
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
    with fair_file_operation("copy-project", source=str(generated), target=str(stripped)):
        shutil.copytree(generated, stripped, symlinks=False)
    provenance = stripped / ".ibcmd-provenance"
    if not provenance.is_dir():
        raise OracleError("Generated EDT provenance missing: the stripping control is unproven")
    no_links(provenance)
    if provenance.resolve().parent != stripped.resolve():
        raise OracleError("Unsafe provenance removal target")
    with fair_file_operation("remove-owned-provenance", target=str(provenance)):
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
    warm_generated = None
    if getattr(args, "warm_validation_passes", 0):
        warm_generated = warm_validate(args, run, stripped, run / "generated-validation-workspace",
                                       "edt-validate-generated", "generated-validation")
    else:
        edt(args, run, "edt-validate-generated", run / "generated-workspace", ["validate", "--file", str(generated_tsv),
            "--project-list", str(stripped)])
    if not generated_tsv.is_file():
        raise OracleError("Generated project validate did not produce TSV")
    validation_comparison = compare_validation_tsv(validation_capture / "validation.tsv", generated_tsv)
    write_json(run / "generated-validation-summary.json", summarize_validation_tsv(generated_tsv))
    write_json(run / "validation-differential.json", validation_comparison)
    write_json(run / "stripped-project-after.json", snapshot(stripped))
    verdicts["generated-edt-installed-export"] = capture_raw_comparison(args, run,
        "generated-edt-installed-export", generated_xml, reference, baseline_xml,
        ours_version, actual_edt_version)
    if snapshot(args.native) != native_before or snapshot(project) != project_before \
            or snapshot(baseline_xml) != baseline_before or snapshot(reference) != reference_before:
        raise OracleError("An immutable input changed during acceptance")
    if digest(args.ours_exe) != ours_hash:
        raise OracleError("Candidate executable changed during acceptance")
    if getattr(args, "native_reference_mode", "standard") == "affinity1" \
            and validate_native_reference(reference, baseline_before, args.native_tool_version,
                                          reference_before, mode="affinity1") != reference_binding:
        raise OracleError("Experimental native reference evidence changed during acceptance")
    if bind_diagnostic_capture(validation_capture, args, project_snapshot=project_before) != validation_binding \
            or bind_diagnostic_capture(ambient_control, args, control=True) != control_binding:
        raise OracleError("Diagnostic capture evidence changed during acceptance")
    # Older preparation captures predate workspace severity collection. Read
    # their preserved raw stage logs rather than trusting an obsolete summary.
    original_diagnostics = command_diagnostics(prepared_root,
                                               ["edt-import-native", "edt-export-native"])
    write_json(run / "prepared-diagnostics-recomputed.json", original_diagnostics)
    generated_diagnostics = command_diagnostics(run, ["edt-export-generated",
        *([record["label"] for record in warm_generated["records"]] if warm_generated else ["edt-validate-generated"])])
    write_json(run / "edt-diagnostics.json", generated_diagnostics)
    if warm_generated:
        if bind_warm_validation(args, run, stripped, run / "generated-validation-workspace",
                                "edt-validate-generated", "generated-validation") != warm_generated:
            raise OracleError("Generated warm validation evidence changed during acceptance")
        # Import/export remains a separate strict gate. Warm repeats cannot
        # hide any earlier candidate load failure or cancel errors across passes.
        ambient_comparison = compare_ambient_diagnostics(ambient_control,
            [(prepared_root, "edt-import-native"), (prepared_root, "edt-export-native")],
            [(run, "edt-export-generated")])
        warm_errors = compare_warm_phase_errors(ambient_control, validation_capture, run,
                                               validation_binding["warm_validation"], warm_generated)
        write_json(run / "warm-phase-error-differential.json", warm_errors)
    else:
        warm_errors = None
        ambient_comparison = compare_ambient_diagnostics(ambient_control,
            [(prepared_root, "edt-import-native"), (prepared_root, "edt-export-native"), (validation_capture, "edt-validate")],
            [(run, "edt-export-generated"), (run, "edt-validate-generated")])
    write_json(run / "ambient-diagnostic-differential.json", ambient_comparison)
    generated_comparison = compare_tree_snapshots(baseline_before, generated_before)
    write_json(run / "generated-edt-same-serializer-comparison.json", generated_comparison)
    diagnostic_failure = not ambient_comparison["no_unmatched_error_diagnostics"] or not validation_comparison["no_new_diagnostics"] \
        or bool(warm_errors and (not warm_errors["no_new_nonambient_errors"]
                                 or not warm_errors["no_new_all_tsv_diagnostics"] or not warm_errors["no_new_stream_records"]))
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
        **({"warm_validation_passes": warm_generated["passes"],
            "warm_phase_no_new_nonambient_errors": warm_errors["no_new_nonambient_errors"],
            "warm_phase_all_tsv_no_new_diagnostics": warm_errors["no_new_all_tsv_diagnostics"],
            "warm_phase_stream_no_new_diagnostics": warm_errors["no_new_stream_records"],
            "clean_source": warm_errors["clean_source"] and not summarize_validation_tsv(validation_capture / "validation.tsv")["unresolved_source_diagnostics"],
            "clean_import": warm_errors["clean_generated_validation"] and ambient_comparison["no_unmatched_error_diagnostics"],
            "error_free_generated_configuration": not summarize_validation_tsv(generated_tsv)["unresolved_source_diagnostics"]
                    and warm_errors["clean_generated_validation"] and ambient_comparison["no_unmatched_error_diagnostics"]} if warm_generated else {}),
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
    result.add_argument("--native-reference-mode", choices=("standard", "affinity1"), default="standard",
                        help="Explicit reviewed lab capture mode; affinity1 binds experimental provenance and sampled affinity, not causality")
    result.add_argument("--fifo-snapshots", action="store_true",
                        help="Opt-in separate fair heavy tickets for every full snapshot, project copy and owned provenance removal")
    result.add_argument("--validation-capture", type=Path, help="Completed structured validation of this authentic prepared EDT project")
    result.add_argument("--ambient-control", type=Path, help="Completed zero-source-error empty installed-EDT diagnostic control")
    result.add_argument("--heap-gib", type=int, default=8)
    result.add_argument("--warm-validation-passes", type=int, default=0,
                        help="Opt-in matched fresh-workspace validation repeats (>=2); ALL diagnostic gate remains exact")
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
        if args.warm_validation_passes < 0 or args.warm_validation_passes == 1 \
                or args.warm_validation_passes and args.mode not in ("validate", "accept"):
            raise OracleError("Warm passes require validate/accept and either 0 or >=2")
        if args.native_reference_mode != "standard" and args.mode != "accept":
            raise OracleError("Experimental native-reference mode requires accept")
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
        with fifo_snapshot_scope(args, args.run):
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
