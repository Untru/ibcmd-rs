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


# Commands that locate or run an installed platform: never in a release.
# `infobase` is released: `config export|import` read and write SQL Server
# directly and every other native command is refused by name, never run.
ORACLE_COMMANDS = {"probe", "profile-run", "dump-sources"}
# `infobase config` research commands that run the platform's own ibcmd.
ORACLE_INFOBASE_COMMANDS = ("roundtrip", "sweep")
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
# Declarative IDs written to .project / UTF-8 prefs by the offline EDT adapter.
# These are source-format data, not runtime packages. All other Eclipse names,
# Java class paths, launchers, JNI, JARs and archive payloads remain forbidden.
EDT_SOURCE_IDS = (
    b"org.eclipse.xtext.ui.shared.xtextbuilder",
    b"org.eclipse.xtext.ui.shared.xtextnature",
    b".settings/org.eclipse.core.resources.prefs",
)


def forbidden_binary_markers(data: bytes) -> list[bytes]:
    lowered = data.lower()
    declarative = bytearray(lowered)
    for identifier in EDT_SOURCE_IDS:
        position = 0
        while (position := lowered.find(identifier, position)) != -1:
            end = position + len(identifier)
            if identifier.startswith(b".settings/") or lowered[end:end + 1] not in (b".", b"/", b"$"):
                declarative[position:end] = b"\0" * len(identifier)
            position = end
    return [marker for marker in FORBIDDEN_BINARY_MARKERS if marker in declarative]


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--archive")
    parser.add_argument("--sbom")
    parser.add_argument("--checksum")
    return parser.parse_args()


def audit_binary(binary: pathlib.Path) -> None:
    for marker in forbidden_binary_markers(binary.read_bytes()):
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
        audit_infobase_mode(binary, environment)
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
    required = {"convert", "cf", "compatibility", "infobase"}
    if not required <= listed:
        raise SystemExit(f"default release is missing standalone commands: {sorted(required - listed)}")


def audit_infobase_mode(binary: pathlib.Path, environment: dict) -> None:
    """The released `infobase` mode serves `config export|import` and refuses
    every other native command by name (exit code 1) without launching
    anything: run here with an empty PATH, where no platform could be found."""

    def run(*args: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [str(binary), "infobase", *args],
            env=environment,
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            timeout=60,
        )

    help_run = run("--help")
    if help_run.returncode != 0 or "export" not in help_run.stdout or "import" not in help_run.stdout:
        raise SystemExit("release `infobase --help` does not describe config export/import")
    for command in ORACLE_INFOBASE_COMMANDS:
        if command in help_run.stdout:
            raise SystemExit(f"release `infobase --help` names the platform-oracle command {command}")
        # an unknown command: exit 2, as the platform's ibcmd
        refused = run("config", command, "--db-name=audit")
        if refused.returncode != 2 or "Указана неполная команда" not in refused.stdout:
            raise SystemExit(f"release `infobase config {command}` is not refused as unknown")
    # What only the platform does is refused by name with exit code 1.
    # `config apply` itself is served without the platform since 0.4; its
    # dynamic update is not.
    for args in (
        ("create", "--dbms=MSSQLServer", "--db-name=audit"),
        ("config", "apply", "--dbms=MSSQLServer", "--db-name=audit", "--dynamic=force"),
    ):
        refused = run(*args)
        if refused.returncode != 1 or "не поддерживается в этой версии ibcmd-rs" not in refused.stderr:
            raise SystemExit(f"release `infobase {' '.join(args[:2])}` is not refused by name with exit code 1")


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
    formatter = [component for component in bom.get("components", [])
                 if component.get("name") == "ibcmd-number-format"]
    if len(formatter) != 1 or formatter[0].get("licenses") != [
        {"expression": "GPL-2.0-only WITH Classpath-exception-2.0"}
    ]:
        raise SystemExit("release SBOM must identify the number formatter and its actual license")
    return bom


def audit_archive(archive_path: pathlib.Path, binary: pathlib.Path, sbom: dict | None,
                  source_root: pathlib.Path | None = None) -> None:
    source_root = source_root or pathlib.Path(__file__).resolve().parent.parent
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
        formatter_sources = ("Cargo.toml", "src/lib.rs", "LICENSE", "NOTICE.md")
        expected.update(f"{root}/third-party/ibcmd-number-format/{name}"
                        for name in formatter_sources)
        expected.update(f"{root}/third-party/morph1c/{name}"
                        for name in ("LICENSE-APACHE", "NOTICE.md"))
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
        for name in formatter_sources:
            source = source_root / "crates/ibcmd-number-format" / name
            text = source.read_text(encoding="utf-8").replace("\r\n", "\n").replace("\r", "\n")
            if archive.read(f"{root}/third-party/ibcmd-number-format/{name}") != text.encode("utf-8"):
                raise SystemExit(f"release number formatter source/notice differs: {name}")
        for name in ("LICENSE-APACHE", "NOTICE.md"):
            source = source_root / "crates/ibcmd-edt/vendor/morph1c" / name
            text = source.read_text(encoding="utf-8").replace("\r\n", "\n").replace("\r", "\n")
            if archive.read(f"{root}/third-party/morph1c/{name}") != text.encode("utf-8"):
                raise SystemExit(f"release morph1c license/notice differs: {name}")
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
