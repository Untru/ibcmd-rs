#!/usr/bin/env python3
"""Proves a generated coverage set on the native platform and writes its fixture.

    native_round_trip.py --ibcmd <ibcmd> --platform <build> --xml-version 2.21 \
        --set <set dir> --tree <generated tree> --work <scratch dir>

1. Imports the tree into a new file infobase. When the platform refuses it, every
   variant is imported alone next to the template to find the refused ones,
   which are recorded with the platform's first message and left out.
2. Exports the accepted set (round 1), saves it as `.cf`, loads that `.cf` into
   another new infobase and exports again (round 2). The rounds must match
   outside `ConfigDumpInfo.xml`, or nothing is written.
3. Writes `configuration.cf.b64`, `native/` (round 2 without
   `ConfigDumpInfo.xml`) and `manifest.json` into the set directory.

Only the generated objects reach the fixture. Works on Linux and Windows.
"""

import argparse
import base64
import datetime
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import unicodedata

DUMP_INFO = "ConfigDumpInfo.xml"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def run_ibcmd(args, ibcmd, work, *command):
    """Runs native ibcmd with its own work directory (a shared default one is
    locked by any other running ibcmd). Returns (ok, output)."""
    env = dict(os.environ, LANG="ru_RU.UTF-8")
    full = [ibcmd, *command, f"--data={work / 'ibcmd-data'}"]
    done = subprocess.run(full, capture_output=True, text=True, env=env, errors="replace")
    output = (done.stdout + done.stderr).strip()
    return done.returncode == 0, output


def first_error(output):
    """The platform's most specific error line: the last `[ERROR]` line (the
    first one only says that import found critical errors)."""
    errors = [line for line in output.splitlines() if line.startswith("[ERROR]")]
    lines = errors or [line for line in output.splitlines() if line.strip() and not line.startswith("[INFO]")]
    return (lines[-1] if lines else output.splitlines()[-1] if output else "").strip()[:300]


def objects_of(tree, collection):
    return sorted(path.stem for path in (tree / collection).glob("*.xml"))


MD_REF = re.compile(r'^\s*<xr:Item xsi:type="xr:MDObjectRef">(\w+)\.(\w+)</xr:Item>\s*$')


def prune_references(tree):
    """Drops `<xr:Item>` references to objects the tree does not hold (a
    recorder listing a register left out), so one refused variant does not
    take the rest of the set with it."""
    configuration = (tree / "Configuration.xml").read_text(encoding="utf-8-sig")
    present = set(re.findall(r"\n\t\t\t<(\w+)>([^<]+)</\1>", configuration))
    for path in tree.rglob("*.xml"):
        if path.name == "Configuration.xml":
            continue
        text = path.read_text(encoding="utf-8-sig")
        lines = text.split("\n")
        kept = [line for line in lines if not ((m := MD_REF.match(line)) and m.groups() not in present
                                                and m.group(1)[0].isupper() and m.group(1) in KINDS)]
        if len(kept) != len(lines):
            path.write_text("\ufeff" + "\n".join(kept), encoding="utf-8")


# Metadata reference kinds a recorder or a list can name.
KINDS = {"AccumulationRegister", "AccountingRegister", "CalculationRegister", "InformationRegister", "Document",
         "Catalog", "BusinessProcess", "Task", "ChartOfAccounts", "ChartOfCalculationTypes",
         "ChartOfCharacteristicTypes", "ExchangePlan", "Constant", "Sequence", "DocumentJournal"}


def subset(tree, collection, keep, target):
    """A copy of `tree` with only those objects of the set's class named in
    `keep`; objects of other classes (the base's auxiliary objects) stay."""
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(tree, target)
    any_object = next((target / collection).glob("*.xml"))
    tag = re.search(r"<MetaDataObject[^>]*>\s*<(\w+) uuid=", any_object.read_text(encoding="utf-8-sig")).group(1)
    for name in objects_of(target, collection):
        if name not in keep:
            (target / collection / f"{name}.xml").unlink()
            shutil.rmtree(target / collection / name, ignore_errors=True)
    configuration = target / "Configuration.xml"
    text = configuration.read_text(encoding="utf-8-sig")
    lines = [
        line
        for line in text.split("\n")
        if not (m := re.fullmatch(rf"\t\t\t<{tag}>([^<]+)</{tag}>", line)) or m.group(1) in keep
    ]
    configuration.write_text("\ufeff" + "\n".join(lines), encoding="utf-8")
    prune_references(target)
    return target


def create_from_tree(args, work, tree, infobase):
    if infobase.exists():
        shutil.rmtree(infobase)
    return run_ibcmd(
        args, args.ibcmd, work, "infobase", "create", f"--db-path={infobase}",
        "--create-database", f"--locale={args.locale}", f"--import={tree}", "--apply", "--force",
    )


def export(args, work, infobase, target):
    if target.exists():
        shutil.rmtree(target)
    ok, output = run_ibcmd(args, args.ibcmd, work, "infobase", "config", "export", f"--db-path={infobase}", str(target))
    if not ok:
        raise SystemExit(f"native export failed: {first_error(output)}")


def tree_files(root):
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file() and path.name != DUMP_INFO
    }


def inputs_digest(set_dir):
    """sha256 over `path NUL bytes` of every input file, ordered by the composed
    relative path; `tests/coverage_corpus.rs` computes the same."""
    files = sorted(
        (unicodedata.normalize("NFC", path.relative_to(set_dir).as_posix()), path.read_bytes())
        for path in (set_dir / "input").rglob("*")
        if path.is_file()
    )
    digest = hashlib.sha256()
    for relative, data in files:
        digest.update(relative.encode() + b"\0" + data)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ibcmd", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--xml-version", required=True)
    parser.add_argument("--set", required=True, type=pathlib.Path)
    parser.add_argument("--tree", required=True, type=pathlib.Path)
    parser.add_argument("--work", required=True, type=pathlib.Path)
    parser.add_argument("--locale", default="ru_RU")
    args = parser.parse_args()

    space = json.loads((args.set / "input" / "values.json").read_text(encoding="utf-8"))
    collection = space["collection"]
    template_name = re.search(
        r"<Name>([^<]+)</Name>", (args.set / "input" / "template.xml").read_text(encoding="utf-8-sig")
    ).group(1)
    work = args.work
    work.mkdir(parents=True, exist_ok=True)

    names = objects_of(args.tree, collection)
    refused = {}
    ok, output = create_from_tree(args, work, args.tree, work / "ib1")
    if not ok:
        print(f"set refused ({first_error(output)}); checking variants one by one", file=sys.stderr)
        for name in names:
            if name == template_name:
                continue
            alone = subset(args.tree, collection, {template_name, name}, work / "alone")
            single_ok, single_output = create_from_tree(args, work, alone, work / "ib-alone")
            if not single_ok:
                refused[name] = first_error(single_output)
                print(f"  refused {name}: {refused[name]}", file=sys.stderr)
        accepted = subset(args.tree, collection, set(names) - set(refused), work / "accepted")
        ok, output = create_from_tree(args, work, accepted, work / "ib1")
        if not ok:
            raise SystemExit(f"accepted variants still refused together: {first_error(output)}")

    export(args, work, work / "ib1", work / "round1")
    cf = work / "set.cf"
    ok, output = run_ibcmd(args, args.ibcmd, work, "infobase", "config", "save", f"--db-path={work / 'ib1'}", str(cf))
    if not ok:
        raise SystemExit(f"native config save failed: {first_error(output)}")
    if (work / "ib2").exists():
        shutil.rmtree(work / "ib2")
    ok, output = run_ibcmd(
        args, args.ibcmd, work, "infobase", "create", f"--db-path={work / 'ib2'}", "--create-database",
        f"--locale={args.locale}", f"--load={cf}", "--apply", "--force",
    )
    if not ok:
        raise SystemExit(f"native load of the saved .cf failed: {first_error(output)}")
    export(args, work, work / "ib2", work / "round2")
    round1, round2 = tree_files(work / "round1"), tree_files(work / "round2")
    if round1 != round2:
        changed = sorted(set(round1) ^ set(round2) | {p for p in round1 if round1.get(p) != round2.get(p)})
        raise SystemExit(f"native rounds differ: {changed[:5]}")

    native = args.set / "native"
    if native.exists():
        shutil.rmtree(native)
    for path, data in round2.items():
        target = native / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    cf_bytes = cf.read_bytes()
    encoded = base64.encodebytes(cf_bytes)
    (args.set / "configuration.cf.b64").write_bytes(encoded)

    by_variant = {}
    for path, data in round2.items():
        parts = path.split("/")
        owner = parts[1].removesuffix(".xml") if parts[0] == collection and len(parts) > 1 else "configuration"
        by_variant.setdefault(owner, []).append({"path": path, "size": len(data), "sha256": sha256(data)})
    variants = [{"name": "configuration", "status": "platform_proven", "files": by_variant.pop("configuration", [])}]
    variants.append({"name": template_name, "property": None, "status": "platform_proven",
                     "files": by_variant.pop(template_name, [])})
    for prop in space["properties"]:
        for entry in prop["values"]:
            name = generate_name(prop["name"], entry["value"])
            record = {"name": name, "property": prop["name"], "kind": prop["kind"],
                      "value": entry["value"], "source": entry["source"]}
            if name in refused:
                record.update(status="platform_refused", message=refused[name], files=[])
            else:
                record.update(status="platform_proven", files=by_variant.pop(name, []))
            variants.append(record)
    sys.path.insert(0, str(pathlib.Path(__file__).parent))
    from generate import combination_name

    for combination in space.get("combinations", []):
        name = combination_name(combination)
        record = {"name": name, "property": sorted(combination["set"]), "kind": "combination",
                  "value": combination["set"], "source": combination.get("source", "combination")}
        if name in refused:
            record.update(status="platform_refused", message=refused[name], files=[])
        else:
            record.update(status="platform_proven", files=by_variant.pop(name, []))
        variants.append(record)
    if by_variant:
        raise SystemExit(f"native files of no known variant: {sorted(by_variant)[:5]}")

    manifest = {
        "schema_version": 1,
        "set": f"{args.platform}-xml-{args.xml_version}-{collection.lower()}",
        "collection": collection,
        "evidence": {
            "status": "platform_proven",
            "created_on": datetime.date.today().isoformat(),
            "platform_version": args.platform,
            "source_version": args.xml_version,
            "ibcmd_sha256": sha256(pathlib.Path(args.ibcmd).read_bytes()),
            "database_kind": "isolated_file_infobase",
            "rounds": "import -> export (1) -> config save -> load -> export (2); identical outside ConfigDumpInfo.xml",
        },
        "inputs_sha256": inputs_digest(args.set),
        "configuration_cf": {
            "path": "configuration.cf.b64",
            "decoded_size": len(cf_bytes),
            "decoded_sha256": sha256(cf_bytes),
            "encoded_sha256": sha256(encoded),
        },
        "variants": variants,
    }
    (args.set / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    proven = sum(v["status"] == "platform_proven" for v in variants) - 2
    print(f"{proven} variants proven, {len(refused)} refused by the platform", file=sys.stderr)


def generate_name(prop, value):
    # The generator's naming rule, kept in one place.
    sys.path.insert(0, str(pathlib.Path(__file__).parent))
    from generate import variant_name

    return variant_name(prop, value)


if __name__ == "__main__":
    main()
