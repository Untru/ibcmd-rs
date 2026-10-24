#!/usr/bin/env python3
"""Generate unmeasured chart OBJECT inputs from pinned public structural fixtures.

These are standalone XML fragments, not complete 1C configurations or native
expected outputs. --check validates in memory; --out must name a NEW directory.
No native process, database, network, or private input is accessed.
"""
import argparse
import copy
import hashlib
import itertools
import json
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

CHART = "http://v8.1c.ru/8.2/data/chart"
CORE = "http://v8.1c.ru/8.1/data/core"
UI = "http://v8.1c.ru/8.1/data/ui"
XSI = "http://www.w3.org/2001/XMLSchema-instance"
SOURCES = {
    "empty-extended-scales-object.xml": "45df7b93ccca3f942b91574545b8a97c788f1391d03d618cac42521725c039f3",
    "empty-no-extended-scales-object.xml": "5ce3f968df903898e27acfb4e1dacbda9a7fb754926e9883ef86c9f700bf0b99",
    "one-real-series-extended-scales-object.xml": "0561d8da1a0e72f6adb522d1e89aaf9061a2f5ab84f40480e2191675d8d2d12e",
}
for prefix, uri in (("d3p1", CHART), ("v8", CORE), ("v8ui", UI), ("xsi", XSI)):
    ET.register_namespace(prefix, uri)


def cq(name):
    return "{" + CHART + "}" + name


def required(parent, path):
    found = parent.findall(path)
    if len(found) != 1:
        raise ValueError("source structure changed")
    return found[0]


def load_sources():
    directory = Path(__file__).resolve().parents[1] / "native-evidence/8.3.27.2214/moxel-chart-series-count-zero/native"
    result = {}
    for name, digest in SOURCES.items():
        raw = (directory / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != digest:
            raise ValueError("source fingerprint changed")
        # Native object fragments inherit these namespaces from the spreadsheet.
        wrapper = ('<seed xmlns:xsi="' + XSI + '" xmlns:v8="' + CORE
                   + '" xmlns:v8ui="' + UI + '">').encode() + raw + b"</seed>"
        parsed = ET.fromstring(wrapper)
        if len(parsed) != 1 or parsed[0].tag != "object":
            raise ValueError("source root changed")
        obj = parsed[0]
        # Replace every localized content value, including the public scaffold's
        # unrelated title. No source title is propagated into generated inputs.
        for content in obj.iter("{" + CORE + "}content"):
            content.text = "ChartSeedTitle"
        target = required(obj, cq("realExSeriesData"))
        content = required(target, cq("text") + "/{" + CORE + "}item/{" + CORE + "}content")
        content.text = "Сводная"  # Approved public automatic-name literal.
        result[name] = obj
    return result


def canonical(node):
    # Ignore formatting only for generator invariant checks, NEVER parity.
    return (node.tag, tuple(sorted(node.attrib.items())), (node.text or "").strip(),
            tuple(canonical(child) for child in node))


def inputs():
    sources = load_sources()
    real = copy.deepcopy(required(sources["one-real-series-extended-scales-object.xml"], cq("realSeriesData")))
    output = {}
    records = []
    for changed, scales, count in itertools.product((False, True), (False, True), (0, 1)):
        source = "empty-extended-scales-object.xml" if scales else "empty-no-extended-scales-object.xml"
        obj = copy.deepcopy(sources[source])
        required(obj, cq("realSeriesCount")).text = str(count)
        if count:
            obj.insert(list(obj).index(required(obj, cq("realExSeriesData"))), copy.deepcopy(real))
        target = required(obj, cq("realExSeriesData"))
        required(target, cq("strIsChanged")).text = str(changed).lower()
        key = f"changed-{int(changed)}_scales-{int(scales)}_real-{count}"
        output[key] = obj
        records.append({"id": key, "extra_series_changed": changed,
                        "source_scales_profile": "extended" if scales else "without_extended",
                        "real_series_count": count, "expected_native": None})
    # Literal control: differs from exactly one factorial case only in content.
    literal = copy.deepcopy(output["changed-1_scales-0_real-1"])
    required(required(literal, cq("realExSeriesData")),
             cq("text") + "/{" + CORE + "}item/{" + CORE + "}content").text = "OwnedUserSeries"
    output["user-literal-control"] = literal
    records.append({"id": "user-literal-control", "paired_with": "changed-1_scales-0_real-1", "expected_native": None})
    for index in (1, 2):
        key = f"baseline-repeat-{index}"
        output[key] = copy.deepcopy(output["changed-0_scales-0_real-1"])
        records.append({"id": key, "paired_with": "changed-0_scales-0_real-1", "expected_native": None})
    # Each changed flag/count edge must differ only in its declared source field
    # or coherent count+record group. Scales are a multi-field source profile.
    for scales, count in itertools.product((False, True), (0, 1)):
        a = copy.deepcopy(output[f"changed-0_scales-{int(scales)}_real-{count}"])
        required(required(a, cq("realExSeriesData")), cq("strIsChanged")).text = "true"
        assert canonical(a) == canonical(output[f"changed-1_scales-{int(scales)}_real-{count}"])
    for changed, scales in itertools.product((False, True), (False, True)):
        a = copy.deepcopy(output[f"changed-{int(changed)}_scales-{int(scales)}_real-1"])
        a.remove(required(a, cq("realSeriesData")))
        required(a, cq("realSeriesCount")).text = "0"
        assert canonical(a) == canonical(output[f"changed-{int(changed)}_scales-{int(scales)}_real-0"])
    for changed, count in itertools.product((False, True), (0, 1)):
        a = copy.deepcopy(output[f"changed-{int(changed)}_scales-1_real-{count}"])
        b = output[f"changed-{int(changed)}_scales-0_real-{count}"]
        for name in ("pointsScale", "valuesScale", "colorPaletteDescription"):
            a.remove(required(a, cq(name)))
            assert b.find(cq(name)) is None
        for name in ("xLabelsOrientation", "paletteKind"):
            required(a, cq(name)).text = required(b, cq(name)).text
        assert canonical(a) == canonical(b)
    control = copy.deepcopy(literal)
    required(required(control, cq("realExSeriesData")),
             cq("text") + "/{" + CORE + "}item/{" + CORE + "}content").text = "Сводная"
    assert canonical(control) == canonical(output["changed-1_scales-0_real-1"])
    for index in (1, 2):
        assert canonical(output[f"baseline-repeat-{index}"]) == canonical(output["changed-0_scales-0_real-1"])
    encoded = {}
    for key, obj in output.items():
        count = int(required(obj, cq("realSeriesCount")).text)
        assert len(obj.findall(cq("realSeriesData"))) == count
        assert len(obj.findall(cq("realExSeriesData"))) == 1
        raw = ET.tostring(obj, encoding="utf-8", xml_declaration=True)
        assert canonical(ET.fromstring(raw)) == canonical(obj)
        encoded[key + ".xml"] = raw + b"\n"
    manifest = {"status": "unmeasured_source_inputs", "kind": "chart_object_fragment",
                "source_sha256": SOURCES, "cases": records,
                "files": {name: {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
                          for name, raw in encoded.items()}}
    return encoded, manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--out", type=Path)
    args = parser.parse_args()
    files, manifest = inputs()
    if args.out is not None:
        if not args.out.is_absolute():
            raise ValueError("output must be absolute")
        # Refuse existing paths, including symlinks; never overwrite or delete.
        args.out.mkdir(parents=False, exist_ok=False)
        for name, raw in files.items():
            with (args.out / name).open("xb") as stream:
                stream.write(raw)
        with (args.out / "manifest.json").open("x", encoding="utf-8") as stream:
            json.dump(manifest, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
    print(json.dumps({"status": "design_inputs_validated", "cases": len(files),
                      "native_runs": 0, "expected_outputs": 0}, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, ET.ParseError, AssertionError):
        print("chart_seed_validation_failed", file=sys.stderr)
        sys.exit(1)
