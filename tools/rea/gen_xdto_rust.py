#!/usr/bin/env python3
"""Генерирует Rust-модуль со свойствами XDTO-пакета logform.

Источник — JSON, полученный `tools/rea/xdto_model.py extract` из
`mngbase_root.res` платформы. Модуль содержит по типам упорядоченный список
свойств: ID, имя, умолчание и владелец (тип, где свойство объявлено).

Пример:
    python tools/rea/gen_xdto_rust.py \
        --model docs/evidence/rea-xdto-20261022/8.5.1.1529.json \
        --platform 8.5.1.1529 \
        --out crates/ibcmd-schema/src/xdto_logform_v851.rs
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def rust_str(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    model = json.loads(args.model.read_text(encoding="utf-8"))
    lines: list[str] = [
        "//! Свойства пакета `http://v8.1c.ru/8.3/xcf/logform`.",
        "//!",
        f"//! Платформа {args.platform}. Сгенерировано `tools/rea/gen_xdto_rust.py`",
        "//! из `mngbase_root.res`; ID — позиция свойства в цепочке наследования.",
        "",
        "#![allow(dead_code)]",
        "",
        "/// Одно свойство XDTO-типа.",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct PropertyDef {",
        "    /// Номер свойства: позиция в развёрнутой цепочке наследования.",
        "    pub id: u16,",
        "    /// Имя свойства в XDTO.",
        "    pub name: &'static str,",
        "    /// Умолчание из модели, если задано.",
        "    pub default: Option<&'static str>,",
        "    /// Тип, в котором свойство объявлено.",
        "    pub owner: &'static str,",
        "}",
        "",
        f'pub const PLATFORM: &str = "{args.platform}";',
        "",
    ]

    names: list[str] = []
    for type_name, entry in model["types"].items():
        const = "PROPERTIES_" + "".join(
            ch if ch.isalnum() else "_" for ch in type_name
        ).upper()
        names.append((type_name, const))
        lines.append(f"pub static {const}: &[PropertyDef] = &[")
        for prop in entry["properties"]:
            default = "None" if prop["default"] is None else f"Some({rust_str(prop['default'])})"
            lines.append(
                "    PropertyDef { id: %d, name: %s, default: %s, owner: %s },"
                % (prop["id"], rust_str(prop["name"]), default, rust_str(prop["owner"]))
            )
        lines.append("];")
        lines.append("")

    lines.append("/// Свойства типа по его имени в XDTO.")
    lines.append("pub fn properties(type_name: &str) -> Option<&'static [PropertyDef]> {")
    lines.append("    match type_name {")
    for type_name, const in names:
        lines.append(f"        {rust_str(type_name)} => Some({const}),")
    lines.append("        _ => None,")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append("/// Свойство по типу и ID.")
    lines.append("pub fn property(type_name: &str, id: u16) -> Option<&'static PropertyDef> {")
    lines.append("    properties(type_name)?.iter().find(|prop| prop.id == id)")
    lines.append("}")
    lines.append("")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(lines), encoding="utf-8")
    total = sum(len(entry["properties"]) for entry in model["types"].values())
    print(f"типов: {len(model['types'])}, свойств: {total}, записано {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
