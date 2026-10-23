#!/usr/bin/env python3
"""Извлекает XDTO-модель форм из `*_root.res` платформы 1С и сопоставляет её
с последовательностями ID из писателей XML.

Пакет `http://v8.1c.ru/8.3/xcf/logform` лежит в `mngbase_root.res` открытым
XML. Номер свойства (он же ID атрибута в писателе XML) — позиция свойства в
развёрнутой цепочке наследования XDTO, начиная с `PredefinedChildItems`.

Команды:
    extract  — выгрузить модель типов из .res в JSON;
    verify   — проверить, что все ID из нативных правил есть в модели;
    map      — получить карту «ID → имя/умолчание/владелец» для элемента.

Пример:
    python tools/rea/xdto_model.py extract \
        --res "C:/Program Files/1cv8/8.5.1.1529/bin/mngbase_root.res" \
        --out docs/evidence/rea-xdto-20261022/8.5.1.1529.json

    python tools/rea/xdto_model.py map \
        --xdto docs/evidence/rea-xdto-20261022/8.5.1.1529.json \
        --native docs/evidence/rea-form-writers-20261022/8.5.1.1529.json \
        --out docs/evidence/rea-xdto-20261022/8.5.1.1529-mapped.json
"""

from __future__ import annotations

import argparse
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

XDTP_NS = {"xdto": "http://v8.1c.ru/8.1/xdto"}
PACKAGE = "http://v8.1c.ru/8.3/xcf/logform"


def extract_package(res: Path) -> ET.Element:
    data = res.read_bytes()
    start = data.find(b'<model xmlns="http://v8.1c.ru/8.1/xdto"')
    if start < 0:
        raise ValueError("в файле нет XDTO-модели")
    end = data.find(b"</model>", start)
    if end < 0:
        raise ValueError("не найден конец модели")
    xml = data[start : end + len(b"</model>")].decode("utf-8-sig")
    root = ET.fromstring(xml)
    package = next(
        (p for p in root.findall("xdto:package", XDTP_NS) if p.get("targetNamespace") == PACKAGE),
        None,
    )
    if package is None:
        raise ValueError(f"пакет {PACKAGE} не найден")
    return package


def build_types(package: ET.Element) -> dict[str, ET.Element]:
    return {t.get("name"): t for t in package.findall("xdto:objectType", XDTP_NS)}


def chain(types: dict[str, ET.Element], name: str) -> list[str]:
    result: list[str] = []
    current: str | None = name
    while current:
        node = types.get(current)
        if node is None:
            raise KeyError(f"тип {current} отсутствует")
        result.append(current)
        base = node.get("base")
        current = base.split(":")[-1] if base else None
    return list(reversed(result))


def flatten(types: dict[str, ET.Element], name: str) -> list[dict]:
    rows: list[dict] = []
    for owner in chain(types, name):
        for prop in types[owner].findall("xdto:property", XDTP_NS):
            rows.append(
                {
                    "id": len(rows),
                    "hex": f"0x{len(rows):x}",
                    "name": prop.get("name"),
                    "type": prop.get("type"),
                    "default": prop.get("default"),
                    "owner": owner,
                }
            )
    return rows


def load_model(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def extract(args: argparse.Namespace) -> int:
    package = extract_package(args.res)
    types = build_types(package)
    model = {"package": PACKAGE, "source": str(args.res), "types": {}}
    for name in types:
        model["types"][name] = {
            "chain": chain(types, name),
            "properties": flatten(types, name),
        }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(model, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"типов: {len(types)}, записано {args.out}")
    return 0


def verify(args: argparse.Namespace) -> int:
    model = load_model(args.xdto)
    native = load_model(args.native)
    problems = 0
    for element in native["elements"]:
        entry = model["types"].get(element["element"])
        if entry is None:
            print(f"{element['element']}: нет типа в XDTO")
            problems += 1
            continue
        by_id = {row["id"] for row in entry["properties"]}
        missing = [a["id"] for a in element["attributes"] if int(a["id"], 16) not in by_id]
        if missing:
            print(f"{element['element']}: {len(missing)} ID вне модели: {missing[:5]}")
            problems += len(missing)
    print("проверка пройдена" if not problems else f"проблем: {problems}")
    return 1 if problems else 0


def map_elements(args: argparse.Namespace) -> int:
    model = load_model(args.xdto)
    native = load_model(args.native)
    result = {"package": model["package"], "elements": []}
    for element in native["elements"]:
        entry = model["types"].get(element["element"])
        if entry is None:
            continue
        by_id = {row["id"]: row for row in entry["properties"]}
        rows = []
        for attribute in element["attributes"]:
            identifier = int(attribute["id"], 16)
            row = by_id.get(identifier)
            if row is None:
                raise KeyError(f"{element['element']}: ID {attribute['id']} отсутствует в XDTO")
            rows.append(
                {
                    "id": attribute["id"],
                    "skip_if": attribute["skip_if"],
                    "name": row["name"],
                    "xml": row["name"][:1].lower() + row["name"][1:],
                    "default": row["default"],
                    "owner": row["owner"],
                    "type": row["type"],
                }
            )
        result["elements"].append({"element": element["element"], "properties": rows})
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"элементов: {len(result['elements'])}, записано {args.out}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    extract_parser = sub.add_parser("extract")
    extract_parser.add_argument("--res", required=True, type=Path)
    extract_parser.add_argument("--out", required=True, type=Path)

    verify_parser = sub.add_parser("verify")
    verify_parser.add_argument("--xdto", required=True, type=Path)
    verify_parser.add_argument("--native", required=True, type=Path)

    map_parser = sub.add_parser("map")
    map_parser.add_argument("--xdto", required=True, type=Path)
    map_parser.add_argument("--native", required=True, type=Path)
    map_parser.add_argument("--out", required=True, type=Path)

    args = parser.parse_args()
    if args.command == "extract":
        return extract(args)
    if args.command == "verify":
        return verify(args)
    return map_elements(args)


if __name__ == "__main__":
    sys.exit(main())
