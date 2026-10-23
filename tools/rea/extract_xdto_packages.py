#!/usr/bin/env python3
"""Извлекает все XDTO-пакеты из `*_root.res` платформы 1С.

Пакеты лежат в ресурсах открытым текстом (`<package targetNamespace=...>`).
Скрипт сохраняет каждый пакет в отдельный `.xsd` и пишет манифест: какой
ресурс, какой namespace, сколько типов/свойств.

Пример:
    python tools/rea/extract_xdto_packages.py \
        --bin "C:/Program Files/1cv8/8.5.1.1529/bin" \
        --out D:/temp/xdto-all \
        --manifest docs/evidence/platform-xdto-20261022/manifest.json
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

PACKAGE = re.compile(rb"<package\b", re.S)
NAMESPACE = re.compile(rb'targetNamespace="([^"]+)"')
END = b"</package>"


def slug(namespace: str) -> str:
    return re.sub(r"[^A-Za-z0-9]+", "-", namespace).strip("-")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    entries = []
    namespaces: dict[str, list[str]] = {}

    for resource in sorted(args.bin.glob("*_root.res")):
        data = resource.read_bytes()
        start = 0
        while True:
            match = PACKAGE.search(data, start)
            if not match:
                break
            end = data.find(END, match.start())
            if end < 0:
                break
            block = data[match.start() : end + len(END)]
            namespace_match = NAMESPACE.search(block)
            if not namespace_match:
                start = end + len(END)
                continue
            namespace = namespace_match.group(1).decode("utf-8", "replace")
            types = block.count(b"<objectType ")
            properties = block.count(b"<property ")
            name = f"{resource.stem}-{slug(namespace)}.xsd"
            (args.out / name).write_bytes(block)
            entries.append(
                {
                    "resource": resource.name,
                    "namespace": namespace,
                    "file": name,
                    "bytes": len(block),
                    "object_types": types,
                    "properties": properties,
                }
            )
            namespaces.setdefault(namespace, []).append(resource.name)
            start = end + len(END)

    duplicates = {
        namespace: resources
        for namespace, resources in namespaces.items()
        if len(resources) > 1
    }
    manifest = {
        "packages": entries,
        "namespaces": len(namespaces),
        "duplicate_namespaces": duplicates,
    }
    args.manifest.parent.mkdir(parents=True, exist_ok=True)
    args.manifest.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    total = sum(entry["bytes"] for entry in entries)
    print(
        f"packages: {len(entries)}, namespaces: {len(namespaces)}, "
        f"bytes: {total}, duplicates: {len(duplicates)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
