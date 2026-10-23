#!/usr/bin/env python3
"""Сопоставляет числовые ID атрибутов платформы с именами свойств из EDT.

Писатели платформы (см. tools/rea/form_writer_rules.py) задают порядок
атрибутов числовыми ID. EDT-плагин com._1c.g5.v8.dt.form.export.xml задаёт тот
же порядок списком EMF-свойств и учитывает версию платформы. Списки совпадают
по длине, поэтому позиция в списке EDT даёт имя для ID.

Метод разбирает исходник ExtInfoWriter.java (декомпилированный CFR), вычисляет
условия `version.compareTo(Version.X) >= 0` для целевой версии и выводит
порядок свойств. Затем склеивает его с JSON правил нативного писателя.

Пример:
    python tools/rea/edt_feature_order.py map \
        --java C:/Tools/edt-form-export/src/.../ExtInfoWriter.java \
        --native docs/evidence/rea-form-writers-20261022/8.5.1.1529.json \
        --method getUsualGroupExtInfoFeatures --version 8.5.1 \
        --out docs/evidence/rea-form-writers-20261022/id-name-8.5.1.json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ADD = re.compile(r"builder\.add\(\(Object\)FormPackage\.Literals\.([A-Z0-9_]+)\)")
COND = re.compile(
    r"version\.compareTo\(Version\.V(\d+)_(\d+)_(\d+)\)\s*(>=|<=|==|>|<)\s*0"
)
COND_METHOD = re.compile(
    r"version\.(isLessThan|isGreaterThan|isLessOrEqual|isGreaterOrEqual|"
    r"isLessThanOrEqual|isGreaterThanOrEqual|isEqual)\(Version\.V(\d+)_(\d+)_(\d+)\)"
)


def method_body(source: str, name: str) -> list[str]:
    lines = source.splitlines()
    start = None
    for index, line in enumerate(lines):
        if f"{name}(Version version)" in line:
            start = index
            break
    if start is None:
        raise ValueError(f"метод {name} не найден")
    depth = 0
    started = False
    body: list[str] = []
    for line in lines[start:]:
        body.append(line)
        depth += line.count("{") - line.count("}")
        if "{" in line:
            started = True
        if started and depth == 0:
            break
    return body


def evaluate(condition: str, version: tuple[int, int, int]) -> bool:
    match = COND.search(condition)
    if match:
        target = (int(match.group(1)), int(match.group(2)), int(match.group(3)))
        operator = match.group(4)
        return {
            ">=": version >= target,
            "<=": version <= target,
            "==": version == target,
            ">": version > target,
            "<": version < target,
        }[operator]

    method = COND_METHOD.search(condition)
    if not method:
        raise ValueError(f"неизвестное условие: {condition}")
    target = (int(method.group(2)), int(method.group(3)), int(method.group(4)))
    return {
        "isLessThan": version < target,
        "isGreaterThan": version > target,
        "isLessOrEqual": version <= target,
        "isLessThanOrEqual": version <= target,
        "isGreaterOrEqual": version >= target,
        "isGreaterThanOrEqual": version >= target,
        "isEqual": version == target,
    }[method.group(1)]


def feature_order(source: str, method: str, version: tuple[int, int, int]) -> list[str]:
    body = method_body(source, method)
    stack: list[dict] = []
    order: list[str] = []

    def active() -> bool:
        for frame in stack:
            if frame["branch"] != frame["positive"]:
                return False
        return True

    for raw in body:
        line = raw.strip()
        if line.startswith("if (") and line.endswith("{"):
            stack.append({"positive": evaluate(line, version), "branch": True})
            continue
        if line == "} else {":
            stack[-1]["branch"] = False
            continue
        if line.startswith("} else if (") and line.endswith("{"):
            stack[-1]["branch"] = False
            stack.append({"positive": evaluate(line, version), "branch": True})
            continue
        if line == "}":
            if stack:
                stack.pop()
            continue
        call = ADD.search(line)
        if call and active():
            order.append(call.group(1))
    return order


def to_xml_name(feature: str) -> str:
    parts = feature.split("__")[-1]
    tokens = [token for token in parts.split("_") if token]
    tokens[-1] = re.sub(r"\d+$", "", tokens[-1]) or tokens[-1]
    head = tokens[0].lower()
    tail = "".join(token[:1].upper() + token[1:].lower() for token in tokens[1:])
    return head + tail


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java", required=True, type=Path)
    parser.add_argument("--native", required=True, type=Path)
    parser.add_argument("--method", required=True)
    parser.add_argument("--element", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--drop-tail", type=int, default=0, help="отбросить последние свойства-контейнеры")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    version = tuple(int(part) for part in args.version.split(".")[:3])
    features = feature_order(args.java.read_text(encoding="utf-8"), args.method, version)
    if args.drop_tail:
        features = features[: -args.drop_tail]

    native = json.loads(args.native.read_text(encoding="utf-8"))
    element = next((item for item in native["elements"] if item["element"] == args.element), None)
    if element is None:
        print(f"элемент {args.element} не найден в {args.native}", file=sys.stderr)
        return 1

    attributes = element["attributes"]
    if len(features) != len(attributes):
        print(
            f"РАСХОЖДЕНИЕ: EDT {len(features)} свойств, нативных ID {len(attributes)}",
            file=sys.stderr,
        )
        return 2

    mapping = [
        {
            "id": attribute["id"],
            "skip_if": attribute["skip_if"],
            "feature": feature,
            "xml": to_xml_name(feature),
        }
        for attribute, feature in zip(attributes, features)
    ]
    result = {
        "platform": args.version,
        "element": args.element,
        "source_java": str(args.java),
        "mapping": mapping,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print(f"{args.element}: {len(mapping)} свойств сопоставлено")
    for row in mapping[:8]:
        print(f"  {row['id']:>6}  {row['feature']:<45} {row['xml']}")
    print(f"записано {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())


