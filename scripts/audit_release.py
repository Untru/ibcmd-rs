#!/usr/bin/env python3
"""Fail closed when a default binary/archive contains platform-oracle payloads."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import tempfile
import zipfile


ORACLE_COMMANDS = {"infobase", "probe", "profile-run", "dump-sources"}
FORBIDDEN_BINARY_MARKERS = (
    b"ibcmd.exe",
    b"1cv8.exe",
    b"1cv8c.exe",
    b"designer.exe",
    b"jni_createjavavm",
    b"org.eclipse",
    b"org/eclipse",
    b"eclipse.osgi",
    b".jar",
)
FORBIDDEN_ARCHIVE_SUFFIXES = (".jar", ".class", ".war", ".ear", ".so", ".dylib", ".dll")


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--archive")
    parser.add_argument("--sbom")
    parser.add_argument("--checksum")
    return parser.parse_args()


def audit_binary(binary: pathlib.Path) -> None:
    data = binary.read_bytes().lower()
    for marker in FORBIDDEN_BINARY_MARKERS:
        if marker in data:
            raise SystemExit(
                f"release binary contains forbidden platform/EDT marker: {marker.decode('ascii')}"
            )

    with tempfile.TemporaryDirectory(prefix="ibcmd-rs-empty-path-") as empty_path:
        environment = os.environ.copy()
        environment["PATH"] = empty_path
        for variable in (
            "IBCMD_PATH",
            "JAVA_HOME",
            "JDK_HOME",
            "ECLIPSE_HOME",
            "ONEC_HOME",
            "1C_HOME",
        ):
            environment.pop(variable, None)
        result = subprocess.run(
            [str(binary), "--help"],
            env=environment,
            check=True,
            capture_output=True,
            text=True,
        )
    listed = set()
    in_commands = False
    for line in result.stdout.splitlines():
        if line.strip() == "Commands:":
            in_commands = True
            continue
        if in_commands and line.strip() == "Options:":
            break
        match = re.match(r"^\s{2,}([a-z0-9][a-z0-9-]*)(?:\s|$)", line)
        if in_commands and match:
            listed.add(match.group(1))
    exposed = ORACLE_COMMANDS & listed
    if exposed:
        raise SystemExit(f"default release exposes platform-oracle commands: {sorted(exposed)}")
    required = {"convert", "cf", "compatibility"}
    if not required <= listed:
        raise SystemExit(f"default release is missing standalone commands: {sorted(required - listed)}")


def audit_sbom(sbom_path: pathlib.Path) -> dict:
    bom = json.loads(sbom_path.read_text(encoding="utf-8"))
    if bom.get("bomFormat") != "CycloneDX" or bom.get("specVersion") != "1.5":
        raise SystemExit("release SBOM is not deterministic CycloneDX 1.5 JSON")
    root = bom.get("metadata", {}).get("component", {})
    if root.get("name") != "ibcmd-rs":
        raise SystemExit("release SBOM does not identify ibcmd-rs as its root component")
    names = "\n".join(
        str(component.get("name", "")) for component in bom.get("components", [])
    ).lower()
    for marker in ("eclipse", "osgi", "java", "jni", "1cv8"):
        if marker in names:
            raise SystemExit(f"release SBOM contains forbidden dependency marker: {marker}")
    return bom


def audit_archive(archive_path: pathlib.Path, binary: pathlib.Path, sbom: dict | None) -> None:
    with zipfile.ZipFile(archive_path) as archive:
        names = archive.namelist()
        if names != sorted(names) or len(names) != len(set(names)):
            raise SystemExit("release archive entries must be sorted and unique")
        roots = {name.split("/", 1)[0] for name in names}
        if len(roots) != 1:
            raise SystemExit("release archive must contain one versioned root directory")
        root = next(iter(roots))
        expected = {
            f"{root}/README.md",
            f"{root}/compatibility/matrix.json",
            f"{root}/compatibility/matrix.schema.json",
            f"{root}/ibcmd-rs",
            f"{root}/sbom.cdx.json",
        }
        if any(name.endswith("/ibcmd-rs.exe") for name in names):
            expected.remove(f"{root}/ibcmd-rs")
            expected.add(f"{root}/ibcmd-rs.exe")
        if set(names) != expected:
            unexpected = sorted(set(names) - expected)
            missing = sorted(expected - set(names))
            raise SystemExit(
                f"release archive allowlist mismatch; unexpected={unexpected}, missing={missing}"
            )
        for name in names:
            lowered = name.lower()
            if lowered.endswith(FORBIDDEN_ARCHIVE_SUFFIXES):
                raise SystemExit(f"release archive contains forbidden payload: {name}")
        binary_members = [
            name for name in names if name.endswith("/ibcmd-rs") or name.endswith("/ibcmd-rs.exe")
        ]
        if len(binary_members) != 1 or archive.read(binary_members[0]) != binary.read_bytes():
            raise SystemExit("release archive binary is missing or differs from the audited binary")
        sbom_members = [name for name in names if name.endswith("/sbom.cdx.json")]
        if len(sbom_members) != 1:
            raise SystemExit("release archive must contain exactly one CycloneDX SBOM")
        archived_sbom = json.loads(archive.read(sbom_members[0]))
        if sbom is not None and archived_sbom != sbom:
            raise SystemExit("release archive SBOM differs from the audited SBOM")


def audit_checksum(archive: pathlib.Path, checksum: pathlib.Path) -> None:
    fields = checksum.read_text(encoding="ascii").strip().split()
    expected = hashlib.sha256(archive.read_bytes()).hexdigest()
    if len(fields) != 2 or fields[0] != expected or fields[1] != archive.name:
        raise SystemExit("release SHA-256 sidecar does not match the archive")


def main() -> None:
    args = arguments()
    binary = pathlib.Path(args.binary).resolve()
    audit_binary(binary)
    sbom = audit_sbom(pathlib.Path(args.sbom).resolve()) if args.sbom else None
    if args.archive:
        archive = pathlib.Path(args.archive).resolve()
        audit_archive(archive, binary, sbom)
        if args.checksum:
            audit_checksum(archive, pathlib.Path(args.checksum).resolve())
    print(f"audited standalone release binary: {binary}")


if __name__ == "__main__":
    main()
