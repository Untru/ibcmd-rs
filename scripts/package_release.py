#!/usr/bin/env python3
"""Create a deterministic standalone release ZIP and SHA-256 sidecar."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import io
import os
import pathlib
import re
import subprocess
import tomllib
import zipfile


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--sbom", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--repository-root", default=".")
    return parser.parse_args()


def source_date_epoch(root: pathlib.Path) -> int:
    configured = os.environ.get("SOURCE_DATE_EPOCH")
    if configured is not None:
        return int(configured)
    return int(
        subprocess.check_output(
            ["git", "log", "-1", "--format=%ct"], cwd=root, text=True
        ).strip()
    )


def normalized_text(path: pathlib.Path) -> bytes:
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n").replace("\r", "\n")
    return text.encode("utf-8")


def build_zip(files: list[tuple[str, bytes, int]], timestamp: tuple[int, ...]) -> bytes:
    target = io.BytesIO()
    with zipfile.ZipFile(target, "w", compression=zipfile.ZIP_STORED) as archive:
        for name, data, mode in sorted(files):
            info = zipfile.ZipInfo(name, timestamp)
            info.create_system = 3
            info.external_attr = mode << 16
            info.compress_type = zipfile.ZIP_STORED
            archive.writestr(info, data)
    return target.getvalue()


def main() -> None:
    args = arguments()
    requested_version = args.version.removeprefix("v")
    if not re.fullmatch(r"[0-9A-Za-z][0-9A-Za-z._-]*", requested_version):
        raise SystemExit(f"invalid release version: {args.version}")
    if not re.fullmatch(r"[0-9A-Za-z][0-9A-Za-z._-]*", args.target):
        raise SystemExit(f"invalid release target: {args.target}")

    root = pathlib.Path(args.repository_root).resolve()
    with (root / "Cargo.toml").open("rb") as stream:
        manifest_version = tomllib.load(stream)["package"]["version"]
    if requested_version != manifest_version:
        raise SystemExit(
            f"release version {requested_version} does not match Cargo.toml {manifest_version}"
        )
    binary = pathlib.Path(args.binary).resolve()
    sbom = pathlib.Path(args.sbom).resolve()
    output = pathlib.Path(args.output).resolve()
    package_root = f"ibcmd-rs-{requested_version}-{args.target}"
    binary_name = "ibcmd-rs.exe" if binary.suffix.lower() == ".exe" else "ibcmd-rs"
    files = [
        (f"{package_root}/{binary_name}", binary.read_bytes(), 0o100755),
        (f"{package_root}/README.md", normalized_text(root / "README.md"), 0o100644),
        (
            f"{package_root}/compatibility/matrix.json",
            normalized_text(root / "compatibility/matrix.json"),
            0o100644,
        ),
        (
            f"{package_root}/compatibility/matrix.schema.json",
            normalized_text(root / "compatibility/matrix.schema.json"),
            0o100644,
        ),
        (f"{package_root}/sbom.cdx.json", normalized_text(sbom), 0o100644),
    ]

    epoch = max(source_date_epoch(root), 315532800)
    stamp = datetime.datetime.fromtimestamp(epoch, datetime.UTC)
    timestamp = (stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second)
    first = build_zip(files, timestamp)
    second = build_zip(files, timestamp)
    if first != second:
        raise SystemExit("deterministic package self-check failed")

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(first)
    digest = hashlib.sha256(first).hexdigest()
    checksum = output.with_name(output.name + ".sha256")
    checksum.write_text(f"{digest}  {output.name}\n", encoding="ascii", newline="\n")
    print(f"{output.name} {digest}")


if __name__ == "__main__":
    main()
