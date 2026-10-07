#!/usr/bin/env python3
"""Verify the release ELF architecture and Ubuntu 22.04 GLIBC baseline."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess


TARGET_MACHINES = {"x86_64-unknown-linux-gnu": 62, "aarch64-unknown-linux-gnu": 183}
MAX_GLIBC = (2, 35)


def elf_machine(header: bytes) -> int:
    if len(header) < 64 or header[:4] != b"\x7fELF" or header[4:7] != b"\x02\x01\x01":
        raise ValueError("expected a complete ELF64 little-endian header")
    return struct.unpack_from("<H", header, 18)[0]


def glibc_requirements(output: str) -> list[tuple[int, ...]]:
    # Read requirement records, not symbol names or this executable's definitions.
    in_needs = False
    versions = set()
    for line in output.splitlines():
        if line.startswith("Version "):
            in_needs = line.startswith("Version needs section ")
        if not in_needs:
            continue
        for name in re.findall(r"\bName: (GLIBC_\S+)", line):
            match = re.fullmatch(r"GLIBC_(\d+(?:\.\d+)+)", name)
            if match is None:
                raise ValueError(f"unsupported GLIBC requirement: {name}")
            versions.add(tuple(map(int, match[1].split("."))))
    if not versions:
        raise ValueError("readelf reported no GLIBC version requirements")
    if max(versions) > MAX_GLIBC:
        required = ".".join(map(str, max(versions)))
        raise ValueError(f"GLIBC {required} exceeds Ubuntu 22.04 baseline 2.35")
    return sorted(versions)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=TARGET_MACHINES, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    with args.binary.open("rb") as stream:
        machine = elf_machine(stream.read(64))
        stream.seek(0)
        sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
    if machine != TARGET_MACHINES[args.target]:
        raise SystemExit(f"ELF machine {machine} does not match {args.target}")
    environment = os.environ.copy()
    environment["LC_ALL"] = "C"
    result = subprocess.run(
        ["readelf", "--version-info", "--wide", str(args.binary)],
        env=environment, check=True, capture_output=True, text=True,
    )
    try:
        versions = glibc_requirements(result.stdout)
    except ValueError as error:
        raise SystemExit(str(error)) from error
    receipt = {"target": args.target, "binary_sha256": sha256, "elf_machine": machine,
               "glibc_requirements": [".".join(map(str, v)) for v in versions],
               "max_glibc": "2.35", "pass": True}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt))


if __name__ == "__main__":
    main()
