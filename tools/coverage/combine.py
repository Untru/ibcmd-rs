#!/usr/bin/env python3
"""Builds the combined coverage configuration of one platform and proves it.

    combine.py --ibcmd <ibcmd> --platform <build> --xml-version <version> \
        --root tests/fixtures/coverage/<platform> --work <scratch>

Every platform-proven variant of every configuration set under `--root` goes
into one configuration (names are unique within a class, and the classes
differ). The platform imports it, exports it, saves it as `.cf`, loads that
`.cf` into a new infobase and exports again; the exports must match. The
fixture lands in `<root>/combined/`: `configuration.cf.b64`, `native/` and a
manifest that names, per object, its files, and per source set, the digest of
its manifest, so the offline test can tell a combined set that is out of date.
"""

import argparse
import base64
import datetime
import json
import pathlib
import re
import shutil
import subprocess
import sys

HERE = pathlib.Path(__file__).parent
sys.path.insert(0, str(HERE))
from generate import rename  # noqa: E402
from native_round_trip import (  # noqa: E402
    create_from_tree, export, first_error, prune_references, run_ibcmd, sha256, tree_files,
)  # noqa: E402


# Names of registers of every kind share one namespace (a document's register
# records list them together); all but information registers get a suffix.
# Session parameters and functional options share another.
SHARED_NAMESPACE_SUFFIX = {"AccumulationRegisters": "_Acc", "AccountingRegisters": "_Bk", "CalculationRegisters": "_Calc",
                           "FunctionalOptions": "_FO"}


def configuration_sets(root):
    for manifest in sorted(root.glob("*/manifest.json")):
        data = json.loads(manifest.read_text(encoding="utf-8"))
        if data.get("kind", "configuration") == "configuration":
            yield manifest.parent, data


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ibcmd", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--xml-version", required=True)
    parser.add_argument("--root", required=True, type=pathlib.Path)
    parser.add_argument("--work", required=True, type=pathlib.Path)
    parser.add_argument("--locale", default="ru_RU")
    args = parser.parse_args()

    work = args.work
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    tree = work / "combined"
    sources, objects, aux = [], [], []
    renamed, references = {}, {}
    for set_dir, manifest in configuration_sets(args.root):
        generated = work / "gen" / set_dir.name
        subprocess.run([sys.executable, "-I", str(HERE / "generate.py"), str(set_dir), str(generated)], check=True,
                       capture_output=True)
        if not tree.exists():
            # The configuration root and the languages only; every auxiliary
            # object comes from a generated tree below.
            tree.mkdir()
            shutil.copyfile(set_dir / "input" / "base" / "Configuration.xml", tree / "Configuration.xml")
            shutil.copytree(set_dir / "input" / "base" / "Languages", tree / "Languages")
            root_text = (tree / "Configuration.xml").read_text(encoding="utf-8-sig")
            kept = [line for line in root_text.split("\n")
                    if not re.fullmatch(r"\t\t\t<(\w+)>[^<]+</\1>", line) or "<Language>" in line]
            (tree / "Configuration.xml").write_text("\ufeff" + "\n".join(kept), encoding="utf-8")
        # The set's auxiliary objects (a recorder, a handler module…) come along;
        # the same one in two sets must be the same file.
        # From the generated tree, not input/base: there a recorder already lists
        # every register of the set.
        for source in sorted(generated.iterdir()):
            if not source.is_dir() or source.name in ("Languages", manifest["collection"]):
                continue
            for item in sorted(source.glob("*.xml")):
                target = tree / source.name / item.name
                if target.exists():
                    if target.read_bytes() != item.read_bytes():
                        raise SystemExit(f"two sets disagree on {source.name}/{item.name}")
                    continue
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(item, target)
                if (source / item.stem).is_dir():
                    shutil.copytree(source / item.stem, tree / source.name / item.stem)
                text = item.read_text(encoding="utf-8-sig")
                aux.append((source.name, item.stem, re.search(r"<MetaDataObject[^>]*>\s*<(\w+) uuid=", text).group(1)))
        collection = manifest["collection"]
        proven = [v["name"] for v in manifest["variants"]
                  if v["status"] == "platform_proven" and v["name"] != "configuration"]
        suffix = SHARED_NAMESPACE_SUFFIX.get(collection, "")
        for name in proven:
            source = generated / collection / f"{name}.xml"
            text = source.read_text(encoding="utf-8-sig")
            tag = re.search(r"<MetaDataObject[^>]*>\s*<(\w+) uuid=", text).group(1)
            new_name = f"{name}{suffix}"
            target = tree / collection / f"{new_name}.xml"
            target.parent.mkdir(parents=True, exist_ok=True)
            # Variant names repeat across classes, so are their identifiers:
            # reissue them with the class in the seed.
            target.write_text("\ufeff" + rename(text, name, new_name, seed=f"{collection}/{name}"), encoding="utf-8")
            if (generated / collection / name).is_dir():
                shutil.copytree(generated / collection / name, tree / collection / new_name)
            objects.append((collection, new_name, tag))
            if new_name != name:
                renamed[f"{collection}/{name}"] = new_name
                references[f">{tag}.{name}<"] = f">{tag}.{new_name}<"
        sources.append({"set": manifest["set"], "path": set_dir.name,
                        "manifest_sha256": sha256((set_dir / "manifest.json").read_bytes())})

    configuration = tree / "Configuration.xml"
    text = configuration.read_text(encoding="utf-8-sig")
    anchor = "\t\t</ChildObjects>"
    listed = set(re.findall(r"\n\t\t\t<(\w+)>([^<]+)</\1>", text))
    children = "".join(
        f"\t\t\t<{tag}>{name}</{tag}>\n" for _, name, tag in aux + objects if (tag, name) not in listed
    )
    configuration.write_text("\ufeff" + text.replace(anchor, children + anchor), encoding="utf-8")
    # The recorders name the registers that were renamed.
    for path in tree.rglob("*.xml"):
        text = path.read_text(encoding="utf-8-sig")
        changed = text
        for old, new in references.items():
            changed = changed.replace(old, new)
        if changed != text:
            path.write_text("\ufeff" + changed, encoding="utf-8")
    prune_references(tree)

    ok, output = create_from_tree(args, work, tree, work / "ib1")
    if not ok:
        raise SystemExit(f"the platform refuses the combined configuration: {first_error(output)}")
    export(args, work, work / "ib1", work / "round1")
    cf = work / "combined.cf"
    ok, output = run_ibcmd(args, args.ibcmd, work, "infobase", "config", "save", f"--db-path={work / 'ib1'}", str(cf))
    if not ok:
        raise SystemExit(f"config save: {first_error(output)}")
    ok, output = run_ibcmd(args, args.ibcmd, work, "infobase", "create", f"--db-path={work / 'ib2'}",
                           "--create-database", f"--locale={args.locale}", f"--load={cf}", "--apply", "--force")
    if not ok:
        raise SystemExit(f"load of the saved .cf: {first_error(output)}")
    export(args, work, work / "ib2", work / "round2")
    round1, round2 = tree_files(work / "round1"), tree_files(work / "round2")
    if round1 != round2:
        raise SystemExit("native rounds of the combined configuration differ")

    out = args.root / "combined"
    # The ratchet outlives a rebuild: what was equal must stay equal.
    baseline = out / "baseline.json"
    kept_baseline = baseline.read_bytes() if baseline.is_file() else None
    if out.exists():
        shutil.rmtree(out)
    if kept_baseline is not None:
        out.mkdir(parents=True)
        baseline.write_bytes(kept_baseline)
    for path, data in round2.items():
        (out / "native" / path).parent.mkdir(parents=True, exist_ok=True)
        (out / "native" / path).write_bytes(data)
    cf_bytes = cf.read_bytes()
    encoded = base64.encodebytes(cf_bytes)
    (out / "configuration.cf.b64").write_bytes(encoded)

    groups = {}
    for path, data in round2.items():
        parts = path.split("/")
        owner = (f"{parts[0]}/{parts[1].removesuffix('.xml')}"
                 if len(parts) > 1 and any(parts[0] == c for c, _, _ in objects) else "configuration")
        groups.setdefault(owner, []).append({"path": path, "size": len(data), "sha256": sha256(data)})
    manifest = {
        "schema_version": 1,
        "kind": "combined",
        "set": f"{args.platform}-xml-{args.xml_version}-combined",
        "evidence": {
            "status": "platform_proven",
            "created_on": datetime.date.today().isoformat(),
            "platform_version": args.platform,
            "source_version": args.xml_version,
            "ibcmd_sha256": sha256(pathlib.Path(args.ibcmd).read_bytes()),
            "database_kind": "isolated_file_infobase",
            "rounds": "import -> export (1) -> config save -> load -> export (2); identical outside ConfigDumpInfo.xml",
        },
        "inputs_sha256": sha256(b""),
        "sources": sources,
        "renamed": renamed,
        "configuration_cf": {"path": "configuration.cf.b64", "decoded_size": len(cf_bytes),
                             "decoded_sha256": sha256(cf_bytes), "encoded_sha256": sha256(encoded)},
        "variants": [{"name": owner, "status": "platform_proven", "files": files}
                     for owner, files in sorted(groups.items())],
    }
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"combined: {len(objects)} objects from {len(sources)} sets, {len(round2)} native files", file=sys.stderr)


if __name__ == "__main__":
    main()
