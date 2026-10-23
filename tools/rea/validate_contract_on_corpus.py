#!/usr/bin/env python3
"""Проверяет контракт порядка атрибутов на реальной XML-выгрузке.

Для каждого элемента формы из корпуса берётся последовательность дочерних
свойств и сверяется с порядком из `attribute-order-<версия>.json`. Скрипт
печатает число проверенных элементов, нарушений и самые частые пары
«раньше/позже».

Пример:
    python tools/rea/validate_contract_on_corpus.py \
        --corpus D:/EDT/artifacts/OneConta85-xml \
        --order docs/evidence/rea-xdto-20261022/attribute-order-8.5.1.json \
        --out docs/evidence/rea-xdto-20261022/contract-validation.md
"""

from __future__ import annotations

import argparse
import collections
import json
import os
import xml.etree.ElementTree as ET
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", required=True, type=Path)
    parser.add_argument("--order", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--max-bytes", type=int, default=6_000_000)
    args = parser.parse_args()

    order = json.loads(args.order.read_text(encoding="utf-8"))
    positions = {
        element: {attribute["name"]: index for index, attribute in enumerate(attributes)}
        for element, attributes in order.items()
    }

    checked = collections.Counter()
    violating = collections.Counter()
    pairs = collections.Counter()
    examples: dict[int, str] = {}

    for dirpath, _, filenames in os.walk(args.corpus):
        if "Form.xml" not in filenames:
            continue
        path = os.path.join(dirpath, "Form.xml")
        try:
            if os.path.getsize(path) > args.max_bytes:
                continue
            tree = ET.parse(path)
        except Exception:
            continue
        for element in tree.iter():
            tag = element.tag.split("}")[-1]
            if tag not in positions:
                continue
            names = positions[tag]
            sequence = [
                names[child.tag.split("}")[-1]]
                for child in list(element)
                if child.tag.split("}")[-1] in names
            ]
            checked[tag] += 1
            bad = [
                (sequence[i], sequence[i + 1])
                for i in range(len(sequence) - 1)
                if sequence[i] > sequence[i + 1]
            ]
            if not bad:
                continue
            violating[tag] += 1
            reversed_names = {index: name for name, index in names.items()}
            for left, right in bad:
                pairs[(tag, reversed_names[left], reversed_names[right])] += 1
            if tag not in examples:
                examples[tag] = f"{path} — " + ", ".join(
                    f"{reversed_names[a]} до {reversed_names[b]}" for a, b in bad[:3]
                )

    total = sum(checked.values())
    bad_total = sum(violating.values())
    lines = ["# Проверка контракта на XML-корпусе", ""]
    lines.append(f"Корпус: `{args.corpus}`")
    lines.append(f"Контракт: `{args.order}`")
    lines.append("")
    lines.append(f"Проверено элементов: **{total}**, с нарушениями: **{bad_total}**")
    lines.append("")
    lines.append("| Элемент | Проверено | Нарушений |")
    lines.append("|---|---|---|")
    for tag in sorted(checked):
        lines.append(f"| {tag} | {checked[tag]} | {violating.get(tag, 0)} |")
    lines.append("")
    lines.append("## Частые пары")
    lines.append("")
    for (tag, left, right), count in pairs.most_common(15):
        lines.append(f"- {tag}: `{left}` раньше `{right}` — {count}")
    lines.append("")
    lines.append("## Примеры")
    lines.append("")
    for tag, text in examples.items():
        lines.append(f"- {tag}: {text}")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"проверено {total}, нарушений {bad_total}, записано {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
