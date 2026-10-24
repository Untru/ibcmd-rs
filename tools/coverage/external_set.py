#!/usr/bin/env python3
"""Builds and proves a coverage set of external data processors or reports.

    external_set.py --ibcmd <ibcmd> --context-db <empty file infobase> --kind epf \
        --set tests/fixtures/coverage/<platform>/external/<set> --work <scratch> \
        [--attributes-donor <file.epf>] --platform <build> --xml-version 2.21

Every variant is its own file. `CovTemplate` is the object the platform writes
from a minimal seed; each attribute of the donor (a committed clean-room
fixture with one attribute per platform type) becomes a variant holding that
attribute alone. For each variant the platform builds the file from its root
XML (`config import --out`), exports it, builds it again from that export and
exports again; the two exports must be identical. Committed per variant: the
second build (`variants/<name>.<kind>.b64`) and the second export
(`native/<name>.xml`, `native/<name>/`).
"""

import argparse
import base64
import datetime
import hashlib
import json
import pathlib
import re
import shutil
import sys
import uuid

HERE = pathlib.Path(__file__).parent
sys.path.insert(0, str(HERE))
from generate import NAMESPACE, rename  # noqa: E402
from native_round_trip import first_error, inputs_digest, run_ibcmd, sha256  # noqa: E402

TEMPLATE = "CovTemplate"
ROOTS = {
    "epf": ("ExternalDataProcessor", "c3831ec8-d8d5-4f93-8a22-f9bfae07327f",
            "ExternalDataProcessorObject", ["DefaultForm", "AuxiliaryForm"]),
    "erf": ("ExternalReport", "e41aff26-25cf-4bb6-b6c1-3f478a75f374", "ExternalReportObject",
            ["DefaultForm", "AuxiliaryForm", "MainDataCompositionSchema", "DefaultSettingsForm",
             "AuxiliarySettingsForm", "DefaultVariantForm", "AuxiliaryVariantForm", "VariantsStorage"]),
}
HEAD = (
    '<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" '
    'xmlns:app="http://v8.1c.ru/8.2/managed-application/core" '
    'xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" '
    'xmlns:cmi="http://v8.1c.ru/8.2/managed-application/cmi" '
    'xmlns:ent="http://v8.1c.ru/8.1/data/enterprise" '
    'xmlns:lf="http://v8.1c.ru/8.2/managed-application/logform" '
    'xmlns:style="http://v8.1c.ru/8.1/data/ui/style" '
    'xmlns:sys="http://v8.1c.ru/8.1/data/ui/fonts/system" '
    'xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" '
    'xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" '
    'xmlns:win="http://v8.1c.ru/8.1/data/ui/colors/windows" '
    'xmlns:xen="http://v8.1c.ru/8.3/xcf/enums" xmlns:xpr="http://v8.1c.ru/8.3/xcf/predef" '
    'xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" '
    'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="{version}">'
)


def seed(kind, version):
    tag, class_id, generated, references = ROOTS[kind]
    ids = (str(uuid.uuid5(NAMESPACE, f"{TEMPLATE}-{kind}/{index}")) for index in range(10))
    empty = "".join(f"\t\t\t<{name}/>\n" for name in references)
    return (
        f'\ufeff<?xml version="1.0" encoding="UTF-8"?>\n{HEAD.format(version=version)}\n'
        f'\t<{tag} uuid="{next(ids)}">\n\t\t<InternalInfo>\n'
        f"\t\t\t<xr:ContainedObject>\n\t\t\t\t<xr:ClassId>{class_id}</xr:ClassId>\n"
        f"\t\t\t\t<xr:ObjectId>{next(ids)}</xr:ObjectId>\n\t\t\t</xr:ContainedObject>\n"
        f'\t\t\t<xr:GeneratedType name="{generated}.{TEMPLATE}" category="Object">\n'
        f"\t\t\t\t<xr:TypeId>{next(ids)}</xr:TypeId>\n\t\t\t\t<xr:ValueId>{next(ids)}</xr:ValueId>\n"
        f"\t\t\t</xr:GeneratedType>\n\t\t</InternalInfo>\n\t\t<Properties>\n"
        f"\t\t\t<Name>{TEMPLATE}</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n{empty}"
        f"\t\t</Properties>\n\t\t<ChildObjects/>\n\t</{tag}>\n</MetaDataObject>"
    )


def build(args, work, root_xml, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    return run_ibcmd(args, args.ibcmd, work, "config", "import", f"--db-path={args.context_db}",
                     f"--out={target}", str(root_xml))


def dump(args, work, file, target_dir):
    """Exports `file` into `target_dir`; the root XML lands beside it."""
    if target_dir.exists():
        shutil.rmtree(target_dir)
    target_dir.with_suffix(".xml").unlink(missing_ok=True)
    ok, output = run_ibcmd(args, args.ibcmd, work, "config", "export", f"--db-path={args.context_db}",
                           f"--file={file}", str(target_dir))
    if not ok:
        raise SystemExit(f"native export of {file.name} failed: {first_error(output)}")


def object_files(parent, name):
    """`name.xml` and everything under `name/`, relative to `parent`."""
    files = {f"{name}.xml": (parent / f"{name}.xml").read_bytes()}
    if (parent / name).is_dir():
        for path in sorted((parent / name).rglob("*")):
            if path.is_file():
                files[path.relative_to(parent).as_posix()] = path.read_bytes()
    return files


def attributes(donor_root):
    """`(variant name, attribute XML)` for every attribute of the donor."""
    return attributes_of(donor_root.read_text(encoding="utf-8-sig"))


def attributes_of(text):
    out, names = [], set()
    for block in re.findall(r"\t\t\t<Attribute uuid=\"[^\"]+\">.*?\n\t\t\t</Attribute>", text, re.S):
        kind = re.search(r"<v8:(?:Type|TypeSet)>([^<]+)</v8:(?:Type|TypeSet)>", block)
        key = re.sub(r"[^A-Za-z0-9]", "_", kind.group(1) if kind else "untyped")
        name = f"CovAttr_{key}"[:80]
        while name in names:
            name += "_"
        names.add(name)
        out.append((name, block))
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ibcmd", required=True)
    parser.add_argument("--context-db", required=True)
    parser.add_argument("--kind", required=True, choices=sorted(ROOTS))
    parser.add_argument("--set", required=True, type=pathlib.Path)
    parser.add_argument("--work", required=True, type=pathlib.Path)
    parser.add_argument("--attributes-donor", type=pathlib.Path)
    parser.add_argument("--combined-of", type=pathlib.Path,
                        help="a proven set whose variants' attributes all go into one CovCombined file")
    parser.add_argument("--platform", required=True)
    parser.add_argument("--xml-version", required=True)
    args = parser.parse_args()

    work = args.work
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    inputs = args.set / "input"
    if inputs.exists():
        shutil.rmtree(inputs)
    inputs.mkdir(parents=True)

    # The template: a minimal seed the platform completes.
    (work / "seed").mkdir()
    (work / "seed" / f"{TEMPLATE}.xml").write_text(seed(args.kind, args.xml_version), encoding="utf-8")
    ok, output = build(args, work, work / "seed" / f"{TEMPLATE}.xml", work / f"{TEMPLATE}.{args.kind}")
    if not ok:
        raise SystemExit(f"the platform refuses a minimal {args.kind}: {first_error(output)}")
    dump(args, work, work / f"{TEMPLATE}.{args.kind}", work / "template" / TEMPLATE)
    template = (work / "template" / f"{TEMPLATE}.xml").read_text(encoding="utf-8-sig")
    (inputs / "template.xml").write_text("\ufeff" + template, encoding="utf-8")

    variants = [(TEMPLATE, template, {"property": None})]
    donor = None
    if args.attributes_donor:
        dump(args, work, args.attributes_donor, work / "donor" / "Donor")
        donor = {"path": args.attributes_donor.as_posix(), "sha256": sha256(args.attributes_donor.read_bytes())}
        if "<ChildObjects/>" not in template:
            raise SystemExit("template: no empty ChildObjects to fill")
        for name, block in attributes(work / "donor" / "Donor.xml"):
            xml = template.replace("<ChildObjects/>", f"<ChildObjects>\n{block}\n\t\t</ChildObjects>")
            variants.append((name, rename(xml, TEMPLATE, name), {"property": "Attribute", "value": name[8:],
                                                                   "source": "donor"}))
    sources = []
    if args.combined_of:
        # One file with every attribute of every proven variant of the source
        # set, as the platform wrote them; their names and ids are unique.
        source_manifest = json.loads((args.combined_of / "manifest.json").read_text(encoding="utf-8"))
        blocks = []
        for variant in source_manifest["variants"]:
            if variant["status"] != "platform_proven" or variant["name"] == TEMPLATE:
                continue
            root_xml = (args.combined_of / "native" / f"{variant['name']}.xml").read_text(encoding="utf-8-sig")
            blocks += [block for _, block in attributes_of(root_xml)]
        xml = template.replace("<ChildObjects/>", "<ChildObjects>\n" + "\n".join(blocks) + "\n\t\t</ChildObjects>")
        variants = [("CovCombined", rename(xml, TEMPLATE, "CovCombined"),
                     {"property": "Combined", "value": args.combined_of.name, "source": "combined"})]
        sources = [{"set": source_manifest["set"], "path": args.combined_of.name,
                    "manifest_sha256": sha256((args.combined_of / "manifest.json").read_bytes())}]
    (inputs / "values.json").write_text(json.dumps({
        "schema_version": 1, "kind": args.kind, "attributes_donor": donor, "sources": sources,
        "variants": [name for name, _, _ in variants],
    }, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    for sub in ("variants", "native"):
        if (args.set / sub).exists():
            shutil.rmtree(args.set / sub)
    records = []
    for name, xml, meta in variants:
        record = {"name": name, **meta}
        root = work / "src" / f"{name}.xml"
        root.parent.mkdir(parents=True, exist_ok=True)
        root.write_text("\ufeff" + xml, encoding="utf-8")
        first = work / "build1" / f"{name}.{args.kind}"
        ok, output = build(args, work, root, first)
        if not ok:
            records.append({**record, "status": "platform_refused", "message": first_error(output), "files": []})
            print(f"  refused {name}: {records[-1]['message']}", file=sys.stderr)
            continue
        dump(args, work, first, work / "round1" / name)
        second = work / "build2" / f"{name}.{args.kind}"
        ok, output = build(args, work, work / "round1" / f"{name}.xml", second)
        if not ok:
            raise SystemExit(f"{name}: the platform refuses its own export: {first_error(output)}")
        dump(args, work, second, work / "round2" / name)
        round1, round2 = object_files(work / "round1", name), object_files(work / "round2", name)
        if round1 != round2:
            raise SystemExit(f"{name}: native rounds differ")
        for path, data in round2.items():
            target = args.set / "native" / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        data = second.read_bytes()
        encoded = base64.encodebytes(data)
        (args.set / "variants").mkdir(exist_ok=True)
        (args.set / "variants" / f"{name}.{args.kind}.b64").write_bytes(encoded)
        records.append({**record, "status": "platform_proven",
                        "file": {"path": f"variants/{name}.{args.kind}.b64", "decoded_sha256": sha256(data)},
                        "files": [{"path": p, "size": len(d), "sha256": sha256(d)} for p, d in round2.items()]})

    manifest = {
        "schema_version": 1,
        "kind": "external",
        "set": f"{args.platform}-xml-{args.xml_version}-{args.set.parent.name}-{args.set.name}",
        "evidence": {
            "status": "platform_proven",
            "created_on": datetime.date.today().isoformat(),
            "platform_version": args.platform,
            "source_version": args.xml_version,
            "ibcmd_sha256": sha256(pathlib.Path(args.ibcmd).read_bytes()),
            "rounds": "config import --out -> config export --file (1) -> config import --out -> config export --file (2); identical",
        },
        "inputs_sha256": inputs_digest(args.set),
        "sources": sources,
        "variants": records,
    }
    (args.set / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                                            encoding="utf-8")
    proven = sum(r["status"] == "platform_proven" for r in records)
    print(f"{proven} variants proven, {len(records) - proven} refused by the platform", file=sys.stderr)


if __name__ == "__main__":
    main()
