#!/usr/bin/env python3
"""Сверяет пул имён платформы с тем, что реально упоминает репозиторий.

Для каждого имени из TSV проверяется, встречается ли оно как отдельный
идентификатор в исходниках, данных схемы и документации проекта. Результат —
JSON с тремя группами: известно проекту, известно парсеру форм, неизвестно
нигде. Последняя группа — кандидаты в непокрытые свойства.

Пример:
    python tools/report-form-property-coverage.py \
        --tsv docs/rea/data/mngcore-8.5.1.1529-name-pool.tsv \
        --repo . --out docs/rea/data/form-property-coverage.json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

SKIP_DIRS = {".git", "target", "node_modules", ".venv", "__pycache__"}
TEXT_SUFFIXES = {".rs", ".json", ".toml", ".py", ".ps1", ".md", ".txt", ".yaml", ".yml"}
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def load_names(path: Path) -> list[str]:
    names: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        _, _, name = line.partition("\t")
        if name:
            names.append(name)
    return names


def collect_tokens(root: Path, skip_data: bool) -> tuple[set[str], set[str]]:
    """Возвращает (все идентификаторы репозитория, идентификаторы парсера форм)."""
    everything: set[str] = set()
    form_parser: set[str] = set()
    parser_path = root / "src" / "mssql_dump" / "form_body.rs"

    for path in root.rglob("*"):
        if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if skip_data and "docs" in path.parts and "rea" in path.parts:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        tokens = set(IDENTIFIER.findall(text))
        everything |= tokens
        if path == parser_path:
            form_parser |= tokens
    return everything, form_parser


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tsv", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    names = load_names(args.tsv)
    unique = sorted(set(names))
    everything, form_parser = collect_tokens(args.repo, skip_data=True)

    known = [n for n in unique if n in everything]
    unknown = [n for n in unique if n not in everything]
    in_parser = [n for n in unique if n in form_parser]

    report = {
        "source": str(args.tsv),
        "pool_pointers": len(names),
        "pool_unique": len(unique),
        "known_in_repository": len(known),
        "known_in_form_parser": len(in_parser),
        "unknown": len(unknown),
        "unknown_names": unknown,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print(f"имён в пуле: {len(unique)}")
    print(f"известны репозиторию: {len(known)}")
    print(f"известны парсеру form_body.rs: {len(in_parser)}")
    print(f"неизвестны нигде: {len(unknown)}")
    if unknown:
        print("примеры неизвестных: " + ", ".join(unknown[:15]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
