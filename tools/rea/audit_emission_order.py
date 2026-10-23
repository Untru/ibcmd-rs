#!/usr/bin/env python3
"""Сверяет порядок XML-свойств в писателе `form_body.rs` с контрактом XDTO.

Писатель `format_form_child_item_xml` собирает свойства элемента вручную, и
порядок выведен из наблюдений за выгрузками. Контракт
`attribute-order-<версия>.json` задаёт порядок, снятый с нативного писателя.
Скрипт вытаскивает последовательность `<Свойство>` из тела функции и считает
инверсии относительно контракта по каждому элементу.

Пример:
    python tools/rea/audit_emission_order.py \
        --source src/mssql_dump/form_body.rs \
        --order docs/evidence/rea-xdto-20261022/attribute-order-8.5.1.json \
        --out docs/evidence/rea-xdto-20261022/emission-order-audit.md
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

FUNCTION = "fn format_form_child_item_xml"
EMITTED = re.compile(r"<([A-Z][A-Za-z0-9]*)>")


def function_body(source: str) -> str:
    start = source.find(FUNCTION)
    if start < 0:
        raise ValueError("функция не найдена")
    brace = source.find("{", start)
    depth = 0
    for index in range(brace, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[brace : index + 1]
    raise ValueError("не найден конец функции")


def emitted_order(body: str) -> list[str]:
    names: list[str] = []
    for match in EMITTED.finditer(body):
        name = match.group(1)
        if name not in names:
            names.append(name)
    return names


def inversions(actual: list[str], expected: list[str]) -> list[tuple[str, str]]:
    position = {name: index for index, name in enumerate(expected)}
    filtered = [name for name in actual if name in position]
    bad: list[tuple[str, str]] = []
    for i in range(len(filtered)):
        for j in range(i + 1, len(filtered)):
            if position[filtered[i]] > position[filtered[j]]:
                bad.append((filtered[i], filtered[j]))
    return bad


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--order", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    body = function_body(args.source.read_text(encoding="utf-8"))
    actual = emitted_order(body)
    order = json.loads(args.order.read_text(encoding="utf-8"))

    lines = ["# Аудит порядка XML-свойств", ""]
    lines.append(f"Источник: `{args.source}`")
    lines.append(f"Контракт: `{args.order}`")
    lines.append("")
    lines.append("| Элемент | Свойств в эмиссии | Инверсий |")
    lines.append("|---|---|---|")
    total = 0
    details: list[str] = []
    for element, attributes in order.items():
        expected = [attribute["name"] for attribute in attributes]
        names = {attribute["name"] for attribute in attributes}
        relevant = [name for name in actual if name in names]
        bad = inversions(actual, expected)
        # оставляем только инверсии, относящиеся к этому элементу
        bad = [pair for pair in bad if pair[0] in names and pair[1] in names]
        total += len(bad)
        lines.append(f"| {element} | {len(relevant)} | {len(bad)} |")
        if bad:
            details.append(f"\n## {element}\n")
            for left, right in bad[:20]:
                details.append(f"- `{left}` стоит раньше `{right}`, по контракту наоборот")

    lines.extend(details)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"инверсий всего: {total}, записано {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
