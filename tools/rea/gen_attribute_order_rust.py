#!/usr/bin/env python3
"""Генерирует Rust-модуль порядка атрибутов формы по контракту писателя.

Вход — JSON, собранный из нативных последовательностей ID и XDTO-имён
(`attribute-order-<версия>.json`). Выход — таблица для писателя XML:
для каждого элемента порядок `{id, name, default}`.

Пример:
    python tools/rea/gen_attribute_order_rust.py \
        --order docs/evidence/rea-xdto-20261022/attribute-order-8.5.1.json \
        --platform 8.5.1.1529 \
        --out crates/ibcmd-schema/src/form_attribute_order_v851.rs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def rust_str(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--order", required=True, type=Path)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    order = json.loads(args.order.read_text(encoding="utf-8"))
    lines: list[str] = [
        "//! Порядок атрибутов формы в писателе XML.",
        "//!",
        f"//! Платформа {args.platform}. Сгенерировано `tools/rea/gen_attribute_order_rust.py`",
        "//! из нативных последовательностей ID и XDTO-имён.",
        "",
        "#![allow(dead_code)]",
        "",
        "/// Атрибут в порядке записи.",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct AttributeDef {",
        "    pub id: u16,",
        "    pub name: &'static str,",
        "    pub default: Option<&'static str>,",
        "}",
        "",
        f'pub const PLATFORM: &str = "{args.platform}";',
        "",
    ]

    names: list[tuple[str, str]] = []
    for element, attributes in order.items():
        const = "ORDER_" + "".join(ch if ch.isalnum() else "_" for ch in element).upper()
        names.append((element, const))
        lines.append(f"pub static {const}: &[AttributeDef] = &[")
        for attribute in attributes:
            default = (
                "None"
                if attribute["default"] is None
                else f"Some({rust_str(attribute['default'])})"
            )
            lines.append(
                "    AttributeDef { id: %d, name: %s, default: %s },"
                % (
                    int(attribute["id"], 16),
                    rust_str(attribute["name"]),
                    default,
                )
            )
        lines.append("];")
        lines.append("")

    lines.append("/// Порядок атрибутов элемента по его XML-тегу.")
    lines.append("pub fn for_element(element: &str) -> Option<&'static [AttributeDef]> {")
    lines.append("    match element {")
    for element, const in names:
        lines.append(f"        {rust_str(element)} => Some({const}),")
    lines.append("        _ => None,")
    lines.append("    }")
    lines.append("}")
    lines.append("")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(lines), encoding="utf-8")
    total = sum(len(attributes) for attributes in order.values())
    print(f"элементов: {len(order)}, атрибутов: {total}, записано {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
