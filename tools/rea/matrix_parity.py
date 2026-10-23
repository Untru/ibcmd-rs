#!/usr/bin/env python3
"""Сравнивает две XML-выгрузки 1С побайтово (SHA-256) и пишет отчёт.

Пример:
    python tools/rea/matrix_parity.py \
        --native D:/temp/acc-native --ours tmp/acc-rs \
        --out docs/evidence/rea-matrix-20261022/accounting.md
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def collect(root: Path) -> dict[str, str]:
    return {
        path.relative_to(root).as_posix(): digest(path)
        for path in root.rglob("*")
        if path.is_file()
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", required=True, type=Path)
    parser.add_argument("--ours", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    native = collect(args.native)
    ours = collect(args.ours)
    common = native.keys() & ours.keys()
    different = sorted(path for path in common if native[path] != ours[path])
    only_native = sorted(native.keys() - ours.keys())
    only_ours = sorted(ours.keys() - native.keys())

    lines = ["# Матрица побайтового паритета", ""]
    lines.append(f"Нативная выгрузка: `{args.native}`")
    lines.append(f"Наша выгрузка: `{args.ours}`")
    lines.append("")
    lines.append("| Показатель | Значение |")
    lines.append("|---|---|")
    lines.append(f"| Файлов у нативного | {len(native)} |")
    lines.append(f"| Файлов у нас | {len(ours)} |")
    lines.append(f"| Совпали | {len(common) - len(different)} |")
    lines.append(f"| Различаются | {len(different)} |")
    lines.append(f"| Только у нативного | {len(only_native)} |")
    lines.append(f"| Только у нас | {len(only_ours)} |")
    lines.append("")
    if different:
        lines.append("## Различаются")
        lines.extend(f"- `{path}`" for path in different[:200])
        lines.append("")
    if only_native:
        lines.append("## Только у нативного (первые 200)")
        lines.extend(f"- `{path}`" for path in only_native[:200])
        lines.append("")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(
        f"native {len(native)}, ours {len(ours)}, identical "
        f"{len(common) - len(different)}, different {len(different)}, "
        f"only-native {len(only_native)}, only-ours {len(only_ours)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
