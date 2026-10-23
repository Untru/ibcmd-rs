#!/usr/bin/env python3
"""Извлекает правила записи XML-атрибутов форм из mngbase.dll платформы 1С.

Для каждого элемента формы скрипт находит строку-имя элемента, её ссылки в
коде, определяет функции-писатели и разбирает последовательность вызовов
`write_attribute(writer, 0xID, value)` вместе с условиями пропуска значений
по умолчанию.

Результат — JSON с правилами и Markdown-сводка. Сравнение двух версий
платформы показывает, что изменилось между релизами.

Требуется REA (rea-agents) с настроенной Ghidra и переменная
GHIDRA_INSTALL_DIR.

Пример:
    python tools/rea/form_writer_rules.py extract \
        --dll "C:/Program Files/1cv8/8.5.1.1529/bin/mngbase.dll" \
        --elements UsualGroup Table Button Field \
        --out docs/evidence/rea-form-writers-20261022/8.5.1.1529.json

    python tools/rea/form_writer_rules.py compare \
        --left  .../8.3.27.2214.json --right .../8.5.1.1529.json \
        --out   docs/evidence/rea-form-writers-20261022/diff.md
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
from pathlib import Path

REA_ENTRY = os.environ.get(
    "REA_ENTRY",
    "C:/Users/Pavel/AppData/Roaming/npm/node_modules/rea-agents/scripts/rea.mjs",
)
ATTRIBUTE_LINE = re.compile(r"\(\*(\w+) \+ 0x28\)\)\(\1,0x([0-9a-f]+)\)")
ELEMENT_CREATE = re.compile(r"\(\*(\w+) \+ 0x20\)\)\(\1,L\"([^\"]+)\"\)")
DEFAULT_CHECK = re.compile(r"!= (0x[0-9a-f]+|\d+)")


class Mcp:
    """Минимальный MCP-клиент к `rea mcp` (stdio, JSON-RPC)."""

    def __init__(self) -> None:
        env = dict(os.environ)
        env.setdefault("GHIDRA_INSTALL_DIR", r"C:\Tools\ghidra_12.1.4_PUBLIC")
        self.proc = subprocess.Popen(
            ["node", REA_ENTRY, "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
            bufsize=1,
            env=env,
        )
        self.next_id = 0
        self.initialize()

    def request(self, method: str, params: dict, timeout: float = 3600) -> dict:
        self.next_id += 1
        message = {"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()
        assert self.proc.stdout is not None
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("rea mcp closed the stream")
            try:
                answer = json.loads(line)
            except json.JSONDecodeError:
                continue
            if answer.get("id") == self.next_id:
                return answer

    def initialize(self) -> None:
        self.request(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "ibcmd-rs-rea", "version": "1.0.0"},
            },
        )
        assert self.proc.stdin is not None
        self.proc.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"
        )
        self.proc.stdin.flush()

    def tool(self, name: str, arguments: dict) -> dict:
        answer = self.request("tools/call", {"name": name, "arguments": arguments})
        if "error" in answer:
            raise RuntimeError(f"{name}: {answer['error']}")
        result = answer.get("result", {})
        blocks = result.get("content") or []
        text = "\n".join(block.get("text", "") for block in blocks if block.get("type") == "text")
        try:
            payload = json.loads(text)
        except json.JSONDecodeError:
            return {"raw": text}
        if isinstance(payload, dict) and "error" in payload and "code" in payload["error"]:
            raise RuntimeError(f"{name}: {payload['error'].get('message')}")
        return payload

    def close(self) -> None:
        self.proc.kill()


def pe_layout(path: Path) -> dict:
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    sections_count = struct.unpack_from("<H", data, pe + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    magic = struct.unpack_from("<H", data, optional)[0]
    image_base = struct.unpack_from("<Q" if magic == 0x20B else "<I", data, optional + 24)[0]
    sections = []
    table = optional + optional_size
    for index in range(sections_count):
        offset = table + index * 40
        name = data[offset : offset + 8].rstrip(b"\0").decode("latin1")
        virtual_size, virtual_address, raw_size, raw_pointer = struct.unpack_from(
            "<IIII", data, offset + 8
        )
        sections.append(
            {
                "name": name,
                "va": virtual_address,
                "vsize": virtual_size,
                "raw_size": raw_size,
                "raw": raw_pointer,
            }
        )
    pdata = next((s for s in sections if s["name"] == ".pdata"), None)
    ranges: list[tuple[int, int]] = []
    if pdata:
        for index in range(pdata["raw_size"] // 12):
            offset = pdata["raw"] + index * 12
            begin, end = struct.unpack_from("<II", data, offset)
            ranges.append((begin, end))
    return {"image_base": image_base, "sections": sections, "pdata": ranges, "data": data}


def function_entry(layout: dict, rva: int) -> int | None:
    for begin, end in layout["pdata"]:
        if begin <= rva < end:
            return begin
    return None


def file_offset(layout: dict, rva: int) -> int | None:
    for section in layout["sections"]:
        if section["va"] <= rva < section["va"] + max(section["vsize"], section["raw_size"]):
            return section["raw"] + (rva - section["va"])
    return None


def result_value(payload: dict) -> list:
    value = payload.get("normalized_result")
    if value is None:
        raw = payload.get("raw_result")
        value = raw.get("value") if isinstance(raw, dict) else raw
    return value if isinstance(value, list) else []


def decompile_text(payload: dict) -> str:
    value = payload.get("normalized_result")
    if value is None:
        raw = payload.get("raw_result")
        value = raw.get("value") if isinstance(raw, dict) else raw
    if isinstance(value, dict):
        value = value.get("value")
    return value if isinstance(value, str) else ""


def parse_attributes(code: str, element: str) -> list[dict]:
    """Достаёт последовательность (id, default) из тела писателя.

    Учитываются только атрибуты того объекта, который создал элемент:
    вложенные вызовы для дочерних узлов имеют другой получатель и
    отбрасываются.
    """
    lines = code.splitlines()
    receiver = None
    for line in lines:
        created = ELEMENT_CREATE.search(line)
        if created and created.group(2) == element:
            receiver = created.group(1)
            break

    rows: list[dict] = []
    for index, line in enumerate(lines):
        if receiver is not None:
            call = ATTRIBUTE_LINE.search(line)
            if not call or call.group(1) != receiver:
                continue
            identifier = int(call.group(2), 16)
        else:
            call = ATTRIBUTE_LINE.search(line)
            if not call:
                continue
            identifier = int(call.group(2), 16)
        default = None
        for back in range(max(0, index - 6), index):
            check = DEFAULT_CHECK.search(lines[back])
            if check:
                default = int(check.group(1), 0)
        rows.append({"id": f"0x{identifier:x}", "skip_if": default})
    return rows


def extract(dll: Path, elements: list[str]) -> dict:
    layout = pe_layout(dll)
    image_base = layout["image_base"]
    digest = hashlib.sha256(dll.read_bytes()).hexdigest()

    client = Mcp()
    try:
        client.tool("open_binary", {"path": str(dll)})
        result = {"platform": {"dll": str(dll), "sha256": digest}, "elements": []}

        for element in elements:
            found = client.tool("search_strings", {"pattern": element, "mode": "literal"})
            addresses = [
                item["address"]
                for item in result_value(found)
                if item.get("value") == element
            ]
            if not addresses:
                print(f"{element}: строка не найдена", file=sys.stderr)
                continue

            entries: list[int] = []
            for address in addresses:
                refs = client.tool("xrefs", {"address": address})
                for ref in result_value(refs):
                    rva = int(ref, 16) - image_base
                    entry = function_entry(layout, rva)
                    if entry is not None and entry not in entries:
                        entries.append(entry)

            candidates = []
            for entry in entries[:6]:
                offset = file_offset(layout, entry)
                if offset is None:
                    continue
                payload = client.tool(
                    "procedure_pseudo_code", {"procedure": f"file-offset:0x{offset:x}"}
                )
                code = decompile_text(payload)
                if f'L"{element}"' not in code:
                    continue
                attributes = parse_attributes(code, element)
                if not attributes:
                    continue
                candidates.append(
                    {
                        "element": element,
                        "entry": f"0x{image_base + entry:x}",
                        "attributes": attributes,
                    }
                )

            if not candidates:
                print(f"{element}: писатель не найден", file=sys.stderr)
                continue
            writer = max(candidates, key=lambda item: len(item["attributes"]))
            result["elements"].append(writer)
            print(
                f"{element}: 0x{int(writer['entry'], 16):x}, атрибутов {len(writer['attributes'])}",
                file=sys.stderr,
            )
        return result
    finally:
        client.close()


def render_markdown(rules: dict) -> str:
    lines = [
        f"# Правила записи атрибутов форм: {Path(rules['platform']['dll']).parent.parent.name}",
        "",
        f"`{rules['platform']['dll']}`",
        f"SHA-256 `{rules['platform']['sha256']}`",
        "",
    ]
    for element in rules["elements"]:
        lines.append(f"## {element['element']}")
        lines.append("")
        lines.append(f"Функция `{element['entry']}`")
        lines.append("")
        lines.append("| # | ID | пропуск, если значение равно |")
        lines.append("|---|---|---|")
        for index, attribute in enumerate(element["attributes"], 1):
            skip = "—" if attribute["skip_if"] is None else str(attribute["skip_if"])
            lines.append(f"| {index} | `{attribute['id']}` | {skip} |")
        lines.append("")
    return "\n".join(lines)


def version_key(path: Path) -> tuple[int, ...]:
    parts = re.findall(r"\d+", path.stem)
    return tuple(int(part) for part in parts) if parts else (0,)


def render_matrix(files: list[Path]) -> str:
    ordered = sorted(files, key=version_key)
    rules = [(path.stem, json.loads(path.read_text(encoding="utf-8"))) for path in ordered]
    elements: list[str] = []
    for _, ruleset in rules:
        for item in ruleset["elements"]:
            if item["element"] not in elements:
                elements.append(item["element"])

    lines = ["# Матрица правил записи атрибутов форм", ""]
    lines.append("| Платформа | " + " | ".join(elements) + " |")
    lines.append("|---|" + "---|" * len(elements))
    for name, ruleset in rules:
        by_name = {item["element"]: item for item in ruleset["elements"]}
        cells = [
            str(len(by_name[element]["attributes"])) if element in by_name else "—"
            for element in elements
        ]
        lines.append("| " + name + " | " + " | ".join(cells) + " |")
    lines.append("")

    lines.append("## Изменения между соседними версиями")
    lines.append("")
    for index in range(1, len(rules)):
        previous_name, previous = rules[index - 1]
        current_name, current = rules[index]
        lines.append(f"### {previous_name} → {current_name}")
        lines.append("")
        previous_by_name = {item["element"]: item for item in previous["elements"]}
        for item in current["elements"]:
            old = previous_by_name.get(item["element"])
            if old is None:
                lines.append(f"- {item['element']}: появился ({len(item['attributes'])} атрибутов)")
                continue
            old_ids = [a["id"] for a in old["attributes"]]
            new_ids = [a["id"] for a in item["attributes"]]
            if old_ids == new_ids and old["attributes"] == item["attributes"]:
                continue
            added = [x for x in new_ids if x not in old_ids]
            removed = [x for x in old_ids if x not in new_ids]
            details = []
            if added:
                details.append("добавлены " + ", ".join(added))
            if removed:
                details.append("удалены " + ", ".join(removed))
            if old_ids != new_ids and not added and not removed:
                details.append("изменился порядок")
            if not details:
                details.append("изменились умолчания")
            lines.append(f"- {item['element']}: " + "; ".join(details))
        lines.append("")
    return "\n".join(lines)


def compare(left: dict, right: dict) -> str:
    lines = [
        "# Различия правил записи атрибутов форм",
        "",
        f"Слева: `{left['platform']['dll']}`",
        f"Справа: `{right['platform']['dll']}`",
        "",
    ]
    left_by_name = {item["element"]: item for item in left["elements"]}
    right_by_name = {item["element"]: item for item in right["elements"]}
    for name in sorted(set(left_by_name) | set(right_by_name)):
        old = left_by_name.get(name)
        new = right_by_name.get(name)
        if old is None or new is None:
            lines.append(f"- **{name}**: есть только в одной версии")
            continue
        old_ids = [a["id"] for a in old["attributes"]]
        new_ids = [a["id"] for a in new["attributes"]]
        if old_ids == new_ids and old["attributes"] == new["attributes"]:
            lines.append(f"- {name}: без изменений ({len(old_ids)} атрибутов)")
            continue
        added = [x for x in new_ids if x not in old_ids]
        removed = [x for x in old_ids if x not in new_ids]
        lines.append(
            f"- **{name}**: порядок изменился; добавлены {added or '—'}, удалены {removed or '—'}"
        )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    extract_parser = sub.add_parser("extract")
    extract_parser.add_argument("--dll", required=True, type=Path)
    extract_parser.add_argument("--elements", nargs="+", required=True)
    extract_parser.add_argument("--out", required=True, type=Path)
    extract_parser.add_argument("--markdown", type=Path)

    compare_parser = sub.add_parser("compare")
    compare_parser.add_argument("--left", required=True, type=Path)
    compare_parser.add_argument("--right", required=True, type=Path)
    compare_parser.add_argument("--out", required=True, type=Path)

    matrix_parser = sub.add_parser("matrix")
    matrix_parser.add_argument("--dir", required=True, type=Path)
    matrix_parser.add_argument("--out", required=True, type=Path)

    args = parser.parse_args()

    if args.command == "matrix":
        files = sorted(args.dir.glob("*.json"))
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(render_matrix(files), encoding="utf-8")
        print(f"записано {args.out} ({len(files)} версий)")
        return 0

    if args.command == "extract":
        rules = extract(args.dll, args.elements)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(rules, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if args.markdown:
            args.markdown.write_text(render_markdown(rules), encoding="utf-8")
        print(f"записано {args.out}")
        return 0

    left = json.loads(args.left.read_text(encoding="utf-8"))
    right = json.loads(args.right.read_text(encoding="utf-8"))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(compare(left, right), encoding="utf-8")
    print(f"записано {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

