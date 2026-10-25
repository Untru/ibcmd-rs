#!/usr/bin/env python3
"""Builds the value space of a coverage set from a native export.

For every scalar property of the template object it collects the distinct
values that objects of the same class carry in a native XML tree, and writes
`values.json`: only property names, values and the corpus name, never the
objects themselves (openspec/changes/add-coverage-configuration).

    observe_values.py --template input/template.xml --corpus <native tree> \
        --collection Catalogs --corpus-name servicedesk-bsp-demo-8.5.1.1150 \
        --output input/values.json
"""

import argparse
import json
import pathlib
import re
import sys
import xml.etree.ElementTree as ET

MD = "{http://v8.1c.ru/8.3/MDClasses}"
NUMBER = re.compile(r"^\d+$")
IDENTIFIER = re.compile(r"^[A-Za-z][A-Za-z0-9_]*$")
# Numbers vary without limit; a few observed ones cover the encodings.
MAX_NUMBERS = 4


def properties(path):
    """`name -> (kind, value)` of the scalar properties of one object file."""
    root = ET.parse(path).getroot()
    obj = next(iter(root))
    props = obj.find(f"{MD}Properties")
    out = {}
    for prop in props:
        name = prop.tag.removeprefix(MD)
        if len(prop):
            # Localized strings and reference lists: not scalar.
            kind = "localized" if prop.find("{http://v8.1c.ru/8.1/data/core}item") is not None else "nested"
            out[name] = (kind, None)
            continue
        text = (prop.text or "").strip()
        if text in ("true", "false"):
            out[name] = ("boolean", text)
        elif NUMBER.match(text):
            out[name] = ("number", text)
        elif IDENTIFIER.match(text):
            out[name] = ("enum", text)
        elif text == "":
            out[name] = ("empty", "")
        else:
            out[name] = ("other", text)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--template", required=True, type=pathlib.Path)
    parser.add_argument("--corpus", required=True, type=pathlib.Path)
    parser.add_argument("--collection", required=True)
    parser.add_argument("--corpus-name", required=True)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()

    template = properties(args.template)
    observed = {}
    files = sorted((args.corpus / args.collection).glob("*.xml"))
    for path in files:
        for name, (kind, value) in properties(path).items():
            observed.setdefault(name, {}).setdefault(kind, set()).add(value)

    space = []
    for name, (kind, own) in template.items():
        if name in ("Name", "Synonym", "Comment"):
            continue
        entry = {"name": name, "kind": kind, "template": own, "values": []}
        if kind == "boolean":
            entry["values"] = [{"value": "false" if own == "true" else "true", "source": "boolean"}]
        elif kind in ("enum", "number"):
            seen = sorted(v for v in observed.get(name, {}).get(kind, set()) if v != own)
            if kind == "number":
                seen = sorted(seen, key=int)[:MAX_NUMBERS]
            entry["values"] = [{"value": v, "source": f"observed:{args.corpus_name}"} for v in seen]
        elif kind == "empty" and "localized" in observed.get(name, {}):
            # A presentation text: the generator writes its own sample.
            entry["kind"] = "localized"
            entry["values"] = [{"value": "Cov", "source": "generated"}]
        else:
            entry["skip"] = "reference or structured property; needs other objects"
        space.append(entry)

    document = {
        "schema_version": 1,
        "collection": args.collection,
        "corpus": args.corpus_name,
        "corpus_objects": len(files),
        "properties": space,
    }
    args.output.write_text(json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    variants = sum(len(p["values"]) for p in space)
    print(f"{len(space)} properties, {variants} variants, from {len(files)} objects", file=sys.stderr)


if __name__ == "__main__":
    main()
