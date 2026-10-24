#!/usr/bin/env python3
"""Creates the committed inputs of a new coverage set.

    new_set.py --ibcmd <ibcmd> --collection Documents --corpus <native tree> \
        --corpus-name <name> --set tests/fixtures/coverage/<platform>/<set> --work <scratch dir>

1. A new empty infobase, exported natively: the base configuration.
2. A minimal seed named `CovTemplate`: name, empty synonym and comment, the
   generated types an object of the same class in the corpus declares for
   itself (fresh identifiers), and what `recipes.json` says the platform
   requires of the class, with the auxiliary objects that needs. The platform
   imports and exports it; the export minus `CovTemplate` is `input/base/`,
   `CovTemplate` is `input/template.xml`.
3. `input/values.json`: `observe_values.py` over the corpus.

Then `generate.py` and `native_round_trip.py` prove the set.
"""

import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys
import uuid

HERE = pathlib.Path(__file__).parent
sys.path.insert(0, str(HERE))
from generate import NAMESPACE  # noqa: E402
from native_round_trip import create_from_tree, export, first_error, run_ibcmd  # noqa: E402

TEMPLATE = "CovTemplate"


def own_generated_types(text):
    """`(prefix, category)` of the generated types the object declares for
    itself: `<prefix>.<object>`, not those of its tabular sections
    (`<prefix>.<object>.<section>`)."""
    return re.findall(r'<xr:GeneratedType name="(\w+)\.[^".]+" category="(\w+)"', text)


def seed_object(corpus_text, head, name, recipe):
    """A minimal object of the corpus object's class, as XML, under `head`:
    the target platform's own `MetaDataObject` line (the corpus may be of
    another platform and format version). Returns (tag, xml)."""
    tag = recipe.get("tag") or re.search(r"<MetaDataObject[^>]*>\s*<(\w+) uuid=", corpus_text).group(1)
    generated = recipe.get("generated") or own_generated_types(corpus_text)
    ids = (str(uuid.uuid5(NAMESPACE, f"{name}-{tag}/{index}")) for index in range(100))
    # The same envelope as the corpus object: generated types and a children
    # list only where its class has them.
    internal = ""
    head_xml = recipe.get("internal_head", "")
    if generated or head_xml:
        types = "".join(
            f'\t\t\t<xr:GeneratedType name="{prefix}.{name}" category="{category}">\n'
            f"\t\t\t\t<xr:TypeId>{next(ids)}</xr:TypeId>\n"
            f"\t\t\t\t<xr:ValueId>{next(ids)}</xr:ValueId>\n"
            "\t\t\t</xr:GeneratedType>\n"
            for prefix, category in generated
        )
        internal = f"\t\t<InternalInfo>\n{{head}}{types}\t\t</InternalInfo>\n"

    def placeholders(text):
        return re.sub(r"\{uuid:(\d+)\}", lambda m: str(uuid.uuid5(NAMESPACE, f"{name}-{tag}/child/{m.group(1)}")), text)

    internal = internal.replace("{head}", placeholders(head_xml))
    properties = placeholders(recipe.get("properties", ""))
    children_xml = placeholders(recipe.get("children", ""))
    if children_xml:
        children = f"\t\t<ChildObjects>\n{children_xml}\t\t</ChildObjects>\n"
    elif re.search(r"<ChildObjects\s*/?>", corpus_text) or recipe.get("child_objects"):
        children = "\t\t<ChildObjects/>\n"
    else:
        children = ""
    return tag, (
        f'\ufeff<?xml version="1.0" encoding="UTF-8"?>\n{head}\n'
        f'\t<{tag} uuid="{next(ids)}">\n{internal}'
        f"\t\t<Properties>\n\t\t\t<Name>{name}</Name>\n\t\t\t<Synonym/>\n\t\t\t<Comment/>\n{properties}"
        f"\t\t</Properties>\n{children}\t</{tag}>\n</MetaDataObject>"
    )


def corpus_text(corpus, collection, recipe):
    objects = sorted((corpus / collection).glob("*.xml"))
    if objects:
        return objects[0].read_text(encoding="utf-8-sig")
    if recipe.get("tag"):
        return ""
    raise SystemExit(f"the corpus has no {collection}, and recipes.json gives no tag for it")


def required(recipes, collection):
    """Auxiliary objects the class needs, dependencies first."""
    out = []

    def visit(key):
        for dependency in recipes["aux"][key].get("requires", []):
            visit(dependency)
        if key not in out:
            out.append(key)

    for key in recipes["classes"].get(collection, {}).get("requires", []):
        visit(key)
    return out


def add_child(configuration, tag, name):
    anchor = "\t\t</ChildObjects>"
    return configuration.replace(anchor, f"\t\t\t<{tag}>{name}</{tag}>\n{anchor}", 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--ibcmd", required=True)
    parser.add_argument("--collection", required=True)
    parser.add_argument("--corpus", required=True, type=pathlib.Path)
    parser.add_argument("--corpus-name", required=True)
    parser.add_argument("--set", required=True, type=pathlib.Path)
    parser.add_argument("--work", required=True, type=pathlib.Path)
    parser.add_argument("--recipes", type=pathlib.Path, default=HERE / "recipes.json")
    parser.add_argument("--locale", default="ru_RU")
    args = parser.parse_args()

    recipes = json.loads(args.recipes.read_text(encoding="utf-8"))
    recipe = recipes["classes"].get(args.collection, {})
    work = args.work
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    # 1. An empty configuration, as the platform writes it.
    ok, output = run_ibcmd(
        args, args.ibcmd, work, "infobase", "create", f"--db-path={work / 'empty'}", "--create-database",
        f"--locale={args.locale}",
    )
    if not ok:
        raise SystemExit(f"empty infobase: {first_error(output)}")
    export(args, work, work / "empty", work / "empty-xml")
    tree = work / "seed"
    shutil.copytree(work / "empty-xml", tree, ignore=shutil.ignore_patterns("ConfigDumpInfo.xml"))
    configuration = (tree / "Configuration.xml").read_text(encoding="utf-8-sig")
    head = configuration.split("\n")[1]

    # 2. The seed, its auxiliary objects, and what the platform makes of them.
    for key in required(recipes, args.collection):
        collection, name = key.split("/")
        aux = recipes["aux"][key]
        tag, xml = seed_object(corpus_text(args.corpus, collection, aux), head, name, aux)
        (tree / collection).mkdir(exist_ok=True)
        (tree / collection / f"{name}.xml").write_text(xml, encoding="utf-8")
        for relative, text in aux.get("files", {}).items():
            target = tree / collection / name / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding="utf-8", newline="")
        configuration = add_child(configuration, tag, name)
    tag, xml = seed_object(corpus_text(args.corpus, args.collection, recipe), head, TEMPLATE, recipe)
    (tree / args.collection).mkdir(exist_ok=True)
    (tree / args.collection / f"{TEMPLATE}.xml").write_text(xml, encoding="utf-8")
    (tree / "Configuration.xml").write_text("\ufeff" + add_child(configuration, tag, TEMPLATE), encoding="utf-8")
    ok, output = create_from_tree(args, work, tree, work / "seeded")
    if not ok:
        raise SystemExit(f"the platform refuses a minimal {tag}: {first_error(output)}")
    export(args, work, work / "seeded", work / "seeded-xml")

    inputs = args.set / "input"
    if inputs.exists():
        shutil.rmtree(inputs)
    inputs.mkdir(parents=True)
    exported = work / "seeded-xml" / args.collection / f"{TEMPLATE}.xml"
    if not exported.is_file():
        raise SystemExit(f"the platform accepts a {tag} but does not export it: the class is not kept by this version")
    shutil.copyfile(exported, inputs / "template.xml")
    shutil.copytree(work / "seeded-xml", inputs / "base", ignore=shutil.ignore_patterns("ConfigDumpInfo.xml"))
    (inputs / "base" / args.collection / f"{TEMPLATE}.xml").unlink()
    shutil.rmtree(inputs / "base" / args.collection / TEMPLATE, ignore_errors=True)
    if not any((inputs / "base" / args.collection).iterdir()):
        (inputs / "base" / args.collection).rmdir()
    base_configuration = inputs / "base" / "Configuration.xml"
    text = base_configuration.read_text(encoding="utf-8-sig")
    line = f"\t\t\t<{tag}>{TEMPLATE}</{tag}>\n"
    if text.count(line) != 1:
        raise SystemExit("seeded export: the template is not listed once in Configuration.xml")
    base_configuration.write_text("\ufeff" + text.replace(line, ""), encoding="utf-8")

    # 3. The value space, observed on the class itself or on the class the
    # recipe names (the source then says so).
    observed = recipe.get("values_from", args.collection)
    corpus_name = args.corpus_name if observed == args.collection else f"{args.corpus_name}/{observed}"
    subprocess.run(
        [sys.executable, "-I", str(HERE / "observe_values.py"), "--template", str(inputs / "template.xml"),
         "--corpus", str(args.corpus), "--collection", observed, "--corpus-name", corpus_name,
         "--output", str(inputs / "values.json")],
        check=True,
    )
    if observed != args.collection:
        space = json.loads((inputs / "values.json").read_text(encoding="utf-8"))
        space["collection"] = args.collection
        (inputs / "values.json").write_text(json.dumps(space, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if recipe.get("recorder"):
        # The register's recorder lists every register of the set; generate.py
        # extends it to the variants.
        space = json.loads((inputs / "values.json").read_text(encoding="utf-8"))
        collection, name = recipe["recorder"].split("/")
        space["recorder"] = {"path": f"{collection}/{name}.xml", "kind": tag}
        (inputs / "values.json").write_text(json.dumps(space, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
