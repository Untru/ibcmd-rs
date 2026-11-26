#!/usr/bin/env python3
"""Read-only EDT model evidence; structural comparison is not codec acceptance."""
import argparse
import hashlib
import json
import os
import plistlib
import re
import stat
import subprocess
from pathlib import Path
import zipfile


CATEGORIES = ("modelTypes", "importers", "exporters")
MODEL_SUFFIXES = (".ecore", ".xcore", ".genmodel")
MANIFEST_LIMIT = 65536
RESOURCE_LIMIT = 256 * 1024 * 1024
RUN_DECOMPRESSION_LIMIT = 256 * 1024 * 1024


class BudgetExceeded(ValueError):
    def __init__(self, scope, limit, consumed, attempted, context):
        super().__init__(f"diagnostic {scope} byte limit exceeded")
        self.evidence = {"code": "decompression-budget-exceeded", "scope": scope,
                         "limit": limit, "consumed": consumed,
                         "attempted_total": attempted, "context": context}


class DecompressionBudget:
    def __init__(self, limit=RUN_DECOMPRESSION_LIMIT):
        self.limit = limit
        self.used = 0

    def consume(self, count, context):
        if self.used + count > self.limit:
            raise BudgetExceeded("run", self.limit, self.used, self.used + count, context)
        self.used += count


def disk_info(target):
    try:
        result = subprocess.run(["/usr/sbin/diskutil", "info", "-plist", str(target)],
                                check=True, capture_output=True, timeout=10)
        info = plistlib.loads(result.stdout)
        if not isinstance(info, dict):
            raise ValueError("diskutil returned no device dictionary")
        return info
    except (OSError, subprocess.SubprocessError, plistlib.InvalidFileException) as error:
        raise ValueError("external backing device is unverifiable") from error


def external_storage(lab, parent):
    mount = Path(*lab.parts[:3])
    if not mount.is_mount() or mount.stat().st_dev == Path("/").stat().st_dev:
        raise ValueError("external lab mount is absent or backed by the root filesystem")
    info = disk_info(mount)
    if (info.get("MountPoint") != str(mount)
            or info.get("Internal") is not False
            or info.get("OSInternalMedia") is not False
            or info.get("SystemImage") is not False
            or info.get("BusProtocol") not in {"USB", "Thunderbolt", "FireWire"}):
        raise ValueError("external backing device is unverifiable or internal")
    device = info.get("DeviceIdentifier", "")
    if not re.fullmatch(r"disk[0-9]+(?:s[0-9]+)*", device):
        raise ValueError("unverifiable mounted device identity")
    stores = info.get("APFSPhysicalStores", [])
    if info.get("FilesystemType") == "apfs" and not stores:
        raise ValueError("APFS backing stores are unverifiable")
    physical = []
    for store in stores:
        identifier = store.get("APFSPhysicalStore", "") if isinstance(store, dict) else ""
        if not re.fullmatch(r"disk[0-9]+(?:s[0-9]+)*", identifier):
            raise ValueError("unverifiable physical store identity")
        backing = disk_info(identifier)
        if (backing.get("DeviceIdentifier") != identifier
                or backing.get("Internal") is not False
                or backing.get("OSInternalMedia") is not False
                or backing.get("SystemImage") is not False
                or backing.get("BusProtocol") not in {"USB", "Thunderbolt", "FireWire"}):
            raise ValueError("external physical store is unverifiable or internal")
        physical.append(identifier)
    if not (lab.stat().st_dev == parent.stat().st_dev == mount.stat().st_dev):
        raise ValueError("output is on a different backing mount")
    return {"mount_point": str(mount), "device": device, "physical_stores": physical,
            "internal": False, "bus_protocol": info["BusProtocol"],
            "device_number": mount.stat().st_dev}



def external_output(path, lab):
    # This Mac laboratory must never create evidence or caches on its internal disk.
    if not lab.is_absolute() or lab.parts[:2] != ("/", "Volumes") or len(lab.parts) < 4:
        raise ValueError("lab root must name an external /Volumes laboratory")
    if not path.is_absolute() or ".." in path.parts or ".." in lab.parts:
        raise ValueError("output and lab root must be absolute without parent traversal")
    path.relative_to(lab)  # lexical containment before inspecting ancestors
    for ancestor in [*reversed(path.parent.parents), path.parent]:
        mode = ancestor.lstat().st_mode
        if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
            raise ValueError("output has a linked or non-directory ancestor")
    if not lab.is_dir() or path == lab:
        raise ValueError("output must be a file below the existing lab")
    external_storage(lab, path.parent)
    return path


def write_report(output, lab, report):
    external_output(output, lab)
    storage = external_storage(lab, output.parent)
    # Hold the verified directory through exclusive output creation. If a
    # pathname is replaced or unmounted afterwards, it cannot redirect this FD.
    parent_fd = os.open(output.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        if os.fstat(parent_fd).st_dev != storage["device_number"]:
            raise ValueError("output directory backing device changed before creation")
        report["output_storage"] = storage
        fd = os.open(output.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                     0o600, dir_fd=parent_fd)
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            json.dump(report, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
    finally:
        os.close(parent_fd)


def stream_digest(stream, limit, budget=None, context="resource"):
    total = 0
    sha = hashlib.sha256()
    while True:
        allowance = min(65536, limit - total + 1)
        if budget is not None:
            allowance = min(allowance, budget.limit - budget.used + 1)
        chunk = stream.read(allowance)
        if not chunk:
            break
        if total + len(chunk) > limit:
            raise BudgetExceeded("entry", limit, total, total + len(chunk), context)
        if budget is not None:
            budget.consume(len(chunk), context)
        total += len(chunk)
        sha.update(chunk)
    return total, sha.hexdigest()


def manifest_main(stream, budget):
    """Read only the bounded main section; CR, LF and CRLF are legal terminators."""
    raw = bytearray()
    at_start = True
    previous_cr = False
    while byte := stream.read(1):
        budget.consume(1, "manifest main section")
        if byte == b"\n" and previous_cr:
            raw.extend(byte)
            previous_cr = False
        elif byte in (b"\r", b"\n"):
            if at_start:
                break
            raw.extend(byte)
            at_start = True
            previous_cr = byte == b"\r"
        else:
            raw.extend(byte)
            at_start = False
            previous_cr = False
        if len(raw) > MANIFEST_LIMIT:
            raise BudgetExceeded("manifest", MANIFEST_LIMIT, len(raw) - 1,
                                 len(raw), "manifest main section")
    return bytes(raw)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def manifest_fields(raw):
    """Unfold JAR manifest continuation lines before extracting bundle identity."""
    lines = []
    for line in raw.decode("utf-8").splitlines():
        if not line:
            break  # bundle identity belongs to the main section only
        if line.startswith(" "):
            if not lines:
                raise ValueError("orphan manifest continuation")
            lines[-1] += line[1:]
        else:
            lines.append(line)
    fields = {}
    for line in lines:
        key, separator, value = line.partition(": ")
        if not separator or key in fields:
            raise ValueError("malformed or duplicate manifest field")
        fields[key] = value
    return fields


def inspect_bundle(source, expected, budget=None):
    """Inspect a path or in-memory ZIP without extracting or executing code."""
    if budget is None:
        budget = DecompressionBudget()
    with zipfile.ZipFile(source) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise ValueError("duplicate ZIP entry")
        # Signed JAR manifests contain large per-entry digest sections. Only
        # the bounded main section contains bundle identity; do not buffer tails.
        with archive.open("META-INF/MANIFEST.MF") as stream:
            raw = manifest_main(stream, budget)
        fields = manifest_fields(raw)
        identity = fields["Bundle-SymbolicName"].split(";", 1)[0]
        if identity != expected["symbolicName"]:
            raise ValueError("bundle filename/manifest identity mismatch")
        version = fields["Bundle-Version"]
        if not version:
            raise ValueError("empty bundle version")
        classes = {name[:-6].replace("/", ".") for name in names
                   if name.endswith(".class") and not name.startswith("META-INF/")}
        delta = {}
        for category in CATEGORIES:
            previous = expected[category]
            if len(previous) != len(set(previous)):
                raise ValueError("duplicate baseline class identity")
            delta[category] = {
                "baseline_count": len(previous),
                "present_count": len(set(previous) & classes),
                "missing": sorted(set(previous) - classes),
            }
        resources = []
        for name in sorted(names):
            if name.endswith(MODEL_SUFFIXES):
                with archive.open(name) as stream:
                    size, sha = stream_digest(stream, RESOURCE_LIMIT, budget, identity + ":" + name)
                resources.append({"path": name, "bytes": size, "sha256": sha})
        return {"symbolicName": identity, "baseline_version": expected["version"],
                "installed_version": version, "class_count": len(classes),
                "baseline_class_delta": delta, "model_resources": resources}


def inventory(plugins, baseline, release):
    """Account for every baseline bundle; missing/ambiguous jars stay explicit."""
    rows = []
    identities = set()
    budget = DecompressionBudget()
    for expected in baseline["bundles"]:
        identity = expected["symbolicName"]
        if identity in identities:
            raise ValueError("duplicate baseline bundle identity")
        identities.add(identity)
        candidates = sorted(plugins.glob(identity + "_*.jar"))
        if len(candidates) != 1:
            rows.append({"symbolicName": identity, "status": "unresolved",
                         "candidate_count": len(candidates)})
            continue
        path = candidates[0]
        # One descriptor binds ZIP inspection and both hashes. Path replacement
        # or in-place mutation invalidates the capture rather than mixing evidence.
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, "rb") as stream:
            initial = os.fstat(stream.fileno())
            if not stat.S_ISREG(initial.st_mode):
                raise ValueError("bundle must be a regular unlinked JAR")
            before = hashlib.file_digest(stream, "sha256").hexdigest()
            stream.seek(0)
            row = inspect_bundle(stream, expected, budget)
            stream.seek(0)
            after = hashlib.file_digest(stream, "sha256").hexdigest()
            final = os.fstat(stream.fileno())
            current = path.lstat()
            identity = lambda st: (st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns, st.st_ctime_ns)
            if before != after or identity(initial) != identity(final) or identity(final) != identity(current):
                raise ValueError("bundle changed during inspection")
        row["jar_sha256"] = before
        row["jar_bytes"] = initial.st_size
        row["status"] = "inspected"
        rows.append(row)
    return {"schema_version": 1, "declared_installed_release": release,
            "baseline_source": baseline["source"],
            "decompression": {"run_limit": budget.limit, "consumed_bytes": budget.used,
                              "resource_entry_limit": RESOURCE_LIMIT,
                              "manifest_main_limit": MANIFEST_LIMIT},
            "scope": "Every legacy inventory bundle; unrelated/new installed bundles are outside this comparison.",
            "semantic_compatibility": "NOT_ESTABLISHED",
            "summary": {"baseline_bundles": len(rows),
                        "inspected_bundles": sum(r["status"] == "inspected" for r in rows),
                        "missing_baseline_class_names": sum(
                            len(d["missing"]) for r in rows
                            for d in r.get("baseline_class_delta", {}).values())},
            "bundles": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plugins", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--installed-release", required=True,
                        help="Exact separately verified build label; never inferred as latest")
    parser.add_argument("--lab-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = external_output(args.output, args.lab_root)
    raw = args.baseline.read_bytes()
    failed = False
    try:
        report = inventory(args.plugins, json.loads(raw), args.installed_release)
    except BudgetExceeded as error:
        failed = True
        report = {"schema_version": 1, "status": "LIMIT_EXCEEDED",
                  "declared_installed_release": args.installed_release,
                  "semantic_compatibility": "NOT_ESTABLISHED", "error": error.evidence}
    report["baseline_sha256"] = digest(raw)
    write_report(output, args.lab_root, report)
    if failed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
