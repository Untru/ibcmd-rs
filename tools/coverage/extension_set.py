#!/usr/bin/env python3
"""Builds and proves a coverage set of configuration extensions.

    extension_set.py --ibcmd <ibcmd> --catalogs-set tests/fixtures/coverage/<platform>/catalogs \
        --set tests/fixtures/coverage/<platform>/extensions/<set> --work <scratch> \
        --platform <build> --xml-version 2.21

The base configuration is the catalogs set's empty configuration plus one
catalog `CovBase` (its template). `CovTemplate` is the extension the platform
creates (`extension create`, purpose add-on, prefix `Cov_`). Variants, each its
own extension over a fresh copy of the base: every boolean property of the
extension flipped and every other purpose; an own catalog; the base catalog
adopted; and the base catalog adopted with control of one scalar property. Per
variant the platform imports the extension, exports it, saves it as `.cfe`,
loads that `.cfe` into another fresh base and exports again; the exports must
match. Committed per variant: `variants/<name>.cfe.b64` and `native/<name>/`.
"""

import argparse
import base64
import datetime
import json
import pathlib
import re
import shutil
import sys

HERE = pathlib.Path(__file__).parent
sys.path.insert(0, str(HERE))
import uuid  # noqa: E402

from generate import NAMESPACE, UUID, rename, set_property  # noqa: E402
from native_round_trip import first_error, inputs_digest, run_ibcmd, sha256, tree_files  # noqa: E402

TEMPLATE = "CovTemplate"
BASE_CATALOG = "CovBase"
PREFIX = "Cov_"
# `ibcmd extension create --purpose` spellings, by the XML value they write.
PURPOSES = {"Customization": "customization", "AddOn": "add-on", "Patch": "patch"}


def add_child(configuration, tag, name):
    anchor = "\t\t</ChildObjects>"
    if "<ChildObjects/>" in configuration:
        configuration = configuration.replace("<ChildObjects/>", f"<ChildObjects>\n{anchor}")
    return configuration.replace(anchor, f"\t\t\t<{tag}>{name}</{tag}>\n{anchor}", 1)


def adopted(base_catalog, controlled):
    """The base catalog as an adopted object: its own generated types, the
    base uuid as the extended object, and the controlled properties."""
    base_uuid = re.search(r'<Catalog uuid="([^"]+)">', base_catalog).group(1)
    internal = re.search(r"\t\t<InternalInfo>.*?</InternalInfo>\n", base_catalog, re.S).group(0)
    # The adopted object has identifiers of its own; the base object is named
    # only by `ExtendedConfigurationObject`.
    seen = {}

    def fresh(match):
        seen.setdefault(match.group(0), str(uuid.uuid5(NAMESPACE, f"adopted-{BASE_CATALOG}/{len(seen)}")))
        return seen[match.group(0)]

    internal = UUID.sub(fresh, internal)
    own_uuid = str(uuid.uuid5(NAMESPACE, f"adopted-{BASE_CATALOG}/object"))
    head = base_catalog.split("\n")[1]
    props = "".join(f"\t\t\t{element}\n" for element in controlled)
    return (
        f'<?xml version="1.0" encoding="UTF-8"?>\n{head}\n\t<Catalog uuid="{own_uuid}">\n{internal}'
        f"\t\t<Properties>\n\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\n"
        f"\t\t\t<Name>{BASE_CATALOG}</Name>\n\t\t\t<Comment/>\n"
        f"\t\t\t<ExtendedConfigurationObject>{base_uuid}</ExtendedConfigurationObject>\n{props}"
        f"\t\t</Properties>\n\t\t<ChildObjects/>\n\t</Catalog>\n</MetaDataObject>"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ibcmd", required=True)
    parser.add_argument("--catalogs-set", required=True, type=pathlib.Path)
    parser.add_argument("--set", required=True, type=pathlib.Path)
    parser.add_argument("--work", required=True, type=pathlib.Path)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--xml-version", required=True)
    parser.add_argument("--locale", default="ru_RU")
    parser.add_argument("--combined-of", type=pathlib.Path,
                        help="a proven extension set whose objects all go into one CovCombined extension")
    args = parser.parse_args()

    work = args.work
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    inputs = args.set / "input"
    if inputs.exists():
        shutil.rmtree(inputs)
    inputs.mkdir(parents=True)

    # The base configuration: an empty one plus CovBase.
    source = args.catalogs_set / "input"
    catalog_template = (source / "template.xml").read_text(encoding="utf-8-sig")
    base_catalog = rename(catalog_template, "CovTemplate", BASE_CATALOG)
    shutil.copytree(source / "base", inputs / "base")
    (inputs / "base" / "Catalogs").mkdir()
    (inputs / "base" / "Catalogs" / f"{BASE_CATALOG}.xml").write_text("\ufeff" + base_catalog, encoding="utf-8")
    configuration = inputs / "base" / "Configuration.xml"
    configuration.write_text(
        "\ufeff" + add_child(configuration.read_text(encoding="utf-8-sig"), "Catalog", BASE_CATALOG),
        encoding="utf-8")
    ok, output = run_ibcmd(args, args.ibcmd, work, "infobase", "create", f"--db-path={work / 'base-ib'}",
                           "--create-database", f"--locale={args.locale}", f"--import={inputs / 'base'}",
                           "--apply", "--force")
    if not ok:
        raise SystemExit(f"base configuration: {first_error(output)}")

    def fresh_base(target):
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(work / "base-ib", target)
        return target

    def create_extension(infobase, name, purpose):
        ok, output = run_ibcmd(args, args.ibcmd, work, "extension", "create", f"--db-path={infobase}",
                               f"--name={name}", f"--name-prefix={PREFIX}", f"--purpose={purpose}")
        if not ok:
            raise SystemExit(f"extension create {name}: {first_error(output)}")

    def export(infobase, name, target):
        if target.exists():
            shutil.rmtree(target)
        ok, output = run_ibcmd(args, args.ibcmd, work, "config", "export", f"--db-path={infobase}",
                               f"--extension={name}", str(target))
        if not ok:
            raise SystemExit(f"export of extension {name}: {first_error(output)}")

    # The template: the extension the platform creates.
    ib = fresh_base(work / "template-ib")
    create_extension(ib, TEMPLATE, "add-on")
    export(ib, TEMPLATE, work / "template")
    shutil.copytree(work / "template", inputs / "template", ignore=shutil.ignore_patterns("ConfigDumpInfo.xml"))
    template_cfg = (inputs / "template" / "Configuration.xml").read_text(encoding="utf-8-sig")

    def variant_tree(name, configuration_xml, extra):
        tree = work / "src" / name
        if tree.exists():
            shutil.rmtree(tree)
        shutil.copytree(inputs / "template", tree)
        (tree / "Configuration.xml").write_text("\ufeff" + rename(configuration_xml, TEMPLATE, name),
                                                encoding="utf-8")
        for relative, text in extra.items():
            (tree / relative).parent.mkdir(parents=True, exist_ok=True)
            (tree / relative).write_text("\ufeff" + text, encoding="utf-8")
        return tree

    variants = [(TEMPLATE, template_cfg, {}, "add-on", {"property": None})]
    purpose = re.search(r"<ConfigurationExtensionPurpose>(\w+)</", template_cfg)
    for prop, value in re.findall(r"\n\t\t\t<(\w+)>(true|false)</\1>", template_cfg):
        flipped = "false" if value == "true" else "true"
        variants.append((f"CovExt{prop}_{flipped}", set_property(template_cfg, prop, "boolean", flipped), {},
                         "add-on", {"property": prop, "value": flipped, "source": "boolean"}))
    if purpose:
        for xml_value, cli in PURPOSES.items():
            if xml_value != purpose.group(1):
                variants.append((f"CovExtPurpose_{xml_value}",
                                 set_property(template_cfg, "ConfigurationExtensionPurpose", "enum", xml_value), {},
                                 cli, {"property": "ConfigurationExtensionPurpose", "value": xml_value,
                                       "source": "declared:ibcmd extension create --purpose"}))
    own = rename(catalog_template, "CovTemplate", f"{PREFIX}Own")
    variants.append(("CovExtOwnCatalog", add_child(template_cfg, "Catalog", f"{PREFIX}Own"),
                     {f"Catalogs/{PREFIX}Own.xml": own}, "add-on", {"property": "OwnObject", "value": "Catalog"}))
    variants.append(("CovExtAdoptedCatalog", add_child(template_cfg, "Catalog", BASE_CATALOG),
                     {f"Catalogs/{BASE_CATALOG}.xml": adopted(base_catalog, [])}, "add-on",
                     {"property": "AdoptedObject", "value": "Catalog"}))
    for prop, value in re.findall(r"\n\t\t\t<(\w+)>([A-Za-z0-9]+)</\1>", base_catalog):
        if prop in ("Name",):
            continue
        variants.append((f"CovExtControl_{prop}", add_child(template_cfg, "Catalog", BASE_CATALOG),
                         {f"Catalogs/{BASE_CATALOG}.xml": adopted(base_catalog, [f"<{prop}>{value}</{prop}>"])},
                         "add-on", {"property": "ControlledProperty", "value": prop, "source": "template"}))

    sources = []
    if args.combined_of:
        # One extension with every object the source set proves one by one:
        # the own catalog and the adopted catalog with all its controlled
        # properties. Properties of the extension itself do not combine (an
        # extension has one purpose); they stay separate extensions.
        source_manifest = json.loads((args.combined_of / "manifest.json").read_text(encoding="utf-8"))
        proven = {v["name"]: v for v in source_manifest["variants"] if v["status"] == "platform_proven"}
        values = dict(re.findall(r"\n\t\t\t<(\w+)>([A-Za-z0-9]+)</\1>", base_catalog))
        controlled = [f"<{v['value']}>{values[v['value']]}</{v['value']}>" for v in proven.values()
                      if v.get("property") == "ControlledProperty"]
        configuration_xml = template_cfg
        extra = {}
        if "CovExtOwnCatalog" in proven:
            configuration_xml = add_child(configuration_xml, "Catalog", f"{PREFIX}Own")
            extra[f"Catalogs/{PREFIX}Own.xml"] = own
        if "CovExtAdoptedCatalog" in proven:
            configuration_xml = add_child(configuration_xml, "Catalog", BASE_CATALOG)
            extra[f"Catalogs/{BASE_CATALOG}.xml"] = adopted(base_catalog, controlled)
        variants = [("CovCombined", configuration_xml, extra, "add-on",
                     {"property": "Combined", "value": args.combined_of.name, "source": "combined"})]
        sources = [{"set": source_manifest["set"], "path": args.combined_of.name,
                    "manifest_sha256": sha256((args.combined_of / "manifest.json").read_bytes())}]
    (inputs / "values.json").write_text(json.dumps(
        {"schema_version": 1, "kind": "extension", "sources": sources, "variants": [v[0] for v in variants]},
        ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for sub in ("variants", "native"):
        if (args.set / sub).exists():
            shutil.rmtree(args.set / sub)

    records = []
    for name, configuration_xml, extra, cli_purpose, meta in variants:
        record = {"name": name, **meta}
        tree = variant_tree(name, configuration_xml, extra)
        ib = fresh_base(work / "ib")
        create_extension(ib, name, cli_purpose)
        ok, output = run_ibcmd(args, args.ibcmd, work, "config", "import", f"--db-path={ib}",
                               f"--extension={name}", str(tree))
        if not ok:
            records.append({**record, "status": "platform_refused", "message": first_error(output), "files": []})
            print(f"  refused {name}: {records[-1]['message']}", file=sys.stderr)
            continue
        export(ib, name, work / "round1" / name)
        cfe = work / "cfe" / f"{name}.cfe"
        cfe.parent.mkdir(parents=True, exist_ok=True)
        ok, output = run_ibcmd(args, args.ibcmd, work, "config", "save", f"--db-path={ib}",
                               f"--extension={name}", str(cfe))
        if not ok:
            raise SystemExit(f"{name}: config save --extension: {first_error(output)}")
        ib2 = fresh_base(work / "ib2")
        create_extension(ib2, name, cli_purpose)
        ok, output = run_ibcmd(args, args.ibcmd, work, "config", "load", f"--db-path={ib2}",
                               f"--extension={name}", str(cfe))
        if not ok:
            raise SystemExit(f"{name}: config load --extension: {first_error(output)}")
        export(ib2, name, work / "round2" / name)
        round1, round2 = tree_files(work / "round1" / name), tree_files(work / "round2" / name)
        if round1 != round2:
            raise SystemExit(f"{name}: native rounds differ")
        for path, data in round2.items():
            target = args.set / "native" / name / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        data = cfe.read_bytes()
        (args.set / "variants").mkdir(exist_ok=True)
        (args.set / "variants" / f"{name}.cfe.b64").write_bytes(base64.encodebytes(data))
        records.append({**record, "status": "platform_proven",
                        "file": {"path": f"variants/{name}.cfe.b64", "decoded_sha256": sha256(data)},
                        "files": [{"path": f"{name}/{p}", "size": len(d), "sha256": sha256(d)}
                                  for p, d in round2.items()]})

    manifest = {
        "schema_version": 1,
        "kind": "extension",
        "set": f"{args.platform}-xml-{args.xml_version}-{args.set.parent.name}-{args.set.name}",
        "evidence": {
            "status": "platform_proven",
            "created_on": datetime.date.today().isoformat(),
            "platform_version": args.platform,
            "source_version": args.xml_version,
            "ibcmd_sha256": sha256(pathlib.Path(args.ibcmd).read_bytes()),
            "database_kind": "isolated_file_infobase",
            "rounds": "extension create -> config import --extension -> export (1) -> config save --extension"
                      " -> extension create + config load --extension in a fresh base -> export (2); identical"
                      " outside ConfigDumpInfo.xml",
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
