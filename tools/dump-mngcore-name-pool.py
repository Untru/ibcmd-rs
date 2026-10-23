#!/usr/bin/env python3
"""Выгружает пул имён свойств из .data указанной DLL платформы 1С.

Платформа держит имена метаданных и свойств интернированными UTF-16-строками,
а в .data лежит массив указателей на них. Скрипт разбирает PE, проходит по
секции данных и восстанавливает пары "адрес указателя -> имя".

Это инвентарь, а не доказательство порядка: порядок записи в XML определяет
код писателя, а не раскладка пула. См. docs/rea/form-property-inventory.md.

Пример:
    python tools/dump-mngcore-name-pool.py \
        --dll "C:/Program Files/1cv8/8.5.1.1529/bin/mngcore.dll" \
        --out docs/rea/data/mngcore-8.5.1.1529-name-pool.tsv
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

PRINTABLE = set(range(0x20, 0x7F))


def parse_sections(data: bytes) -> tuple[int, list[dict]]:
    if data[:2] != b"MZ":
        raise ValueError("не PE-файл: нет сигнатуры MZ")
    pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe_offset : pe_offset + 4] != b"PE\0\0":
        raise ValueError("не PE-файл: нет сигнатуры PE")
    machine, count = struct.unpack_from("<HH", data, pe_offset + 4)
    optional_size = struct.unpack_from("<H", data, pe_offset + 20)[0]
    optional = pe_offset + 24
    magic = struct.unpack_from("<H", data, optional)[0]
    image_base = struct.unpack_from("<Q" if magic == 0x20B else "<I", data, optional + 24)[0]
    sections = []
    table = optional + optional_size
    for index in range(count):
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
                "raw_pointer": raw_pointer,
            }
        )
    return image_base, sections


def rva_to_offset(sections: list[dict], rva: int) -> int:
    for section in sections:
        span = max(section["vsize"], section["raw_size"])
        if section["va"] <= rva < section["va"] + span:
            return section["raw_pointer"] + (rva - section["va"])
    return -1


def read_utf16(data: bytes, offset: int, max_chars: int = 80) -> str | None:
    end = offset
    while end + 1 < len(data) and end - offset < max_chars * 2:
        if data[end] == 0 and data[end + 1] == 0:
            break
        if data[end + 1] != 0 or data[end] not in PRINTABLE:
            return None
        end += 2
    if end == offset:
        return None
    text = data[offset:end].decode("utf-16le")
    if not 2 <= len(text) <= max_chars:
        return None
    return text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dll", required=True, type=Path, help="PE-файл платформы")
    parser.add_argument("--out", required=True, type=Path, help="куда записать TSV")
    parser.add_argument(
        "--section",
        default=".data",
        help="секция с массивом указателей (по умолчанию .data)",
    )
    args = parser.parse_args()

    data = args.dll.read_bytes()
    image_base, sections = parse_sections(data)
    target = next((s for s in sections if s["name"] == args.section), None)
    if target is None:
        print(f"секция {args.section} не найдена", file=sys.stderr)
        return 1

    start = target["raw_pointer"]
    end = start + target["raw_size"]
    limit = image_base + 0x1000_0000
    rows: list[tuple[int, str]] = []
    seen: set[int] = set()

    for pointer_offset in range(start, end - 7, 8):
        value = struct.unpack_from("<Q", data, pointer_offset)[0]
        if not image_base < value < limit:
            continue
        if value in seen:
            continue
        text = read_utf16(data, rva_to_offset(sections, value - image_base))
        if text is None:
            continue
        seen.add(value)
        address = image_base + (pointer_offset - start + target["va"])
        rows.append((address, text))

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write("# address\tname\n")
        for address, text in rows:
            handle.write(f"0x{address:016x}\t{text}\n")

    unique = len({text for _, text in rows})
    print(f"{args.dll.name}: указателей {len(rows)}, уникальных имён {unique}")
    print(f"записано в {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
