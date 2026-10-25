#!/usr/bin/env python3
"""Generates the XML tree of a coverage set from its committed inputs.

    generate.py <set dir> <output tree>

`<set dir>/input/` holds `base/` (a minimal configuration tree), `template.xml`
(a natively exported object) and `values.json` (the value space). Every value
of every property becomes one variant: a copy of the template in which only that
property, the object's name and its identifiers differ. Identifiers are UUIDv5
of the variant name, so the same inputs give a byte-identical tree.
"""

import json
import pathlib
import re
import shutil
import sys
import uuid

# Fixed: changing it renames every identifier of every committed set.
NAMESPACE = uuid.UUID("6f1d3f2e-9b7a-4c55-9a0e-2c4f6d8b1a01")
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
LOCALIZED = (
    "\n\t\t\t\t<v8:item>\n\t\t\t\t\t<v8:lang>ru</v8:lang>\n"
    "\t\t\t\t\t<v8:content>{value}</v8:content>\n\t\t\t\t</v8:item>\n\t\t\t"
)


def variant_name(prop, value):
    """`Cov<property>_<value>`, a valid 1C identifier."""
    tail = re.sub(r"[^A-Za-z0-9]", "_", value) or "Empty"
    return f"Cov{prop}_{tail}"[:80]


def combination_name(combination):
    """The combination's own `name`, or `Cov<property>_<value>__<property>_<value>…`."""
    if combination.get("name"):
        return combination["name"]
    parts = [variant_name(prop, value).removeprefix("Cov") for prop, value in combination["set"].items()]
    return ("Cov" + "__".join(parts))[:80]


def set_property(xml, prop, kind, value):
    """The template with one property's element replaced; refuses when the
    element is missing or occurs more than once. Kind `xml` replaces a nested
    element's whole content (`""` empties it)."""
    # Only the object's own properties: a dimension or an attribute in its
    # children may carry a property of the same name.
    own_end = xml.find("\n\t\t<ChildObjects")
    head, tail = (xml, "") if own_end < 0 else (xml[:own_end], xml[own_end:])
    content = ".*?" if kind == "xml" else "[^<]*"
    pattern = re.compile(rf'<{re.escape(prop)}(?P<attrs>\s[^<>]*?)?(?:/>|>{content}</{re.escape(prop)}>)', re.S)
    matches = list(pattern.finditer(head))
    if len(matches) != 1:
        raise SystemExit(f"template: property {prop} is not exactly one element")
    attrs = matches[0].group("attrs") or ""
    if kind == "xml":
        element = f"<{prop}{attrs}>{value}</{prop}>" if value else f"<{prop}{attrs}/>"
        return pattern.sub(lambda _: element, head) + tail
    if kind == "localized":
        element = f"<{prop}{attrs}>" + LOCALIZED.format(value=value) + f"</{prop}>"
    else:
        element = f"<{prop}{attrs}>{value}</{prop}>"
    return pattern.sub(lambda _: element, head) + tail


CLASS_ID = re.compile(r"<xr:ClassId>([0-9a-f-]{36})</xr:ClassId>")


def input_file(inputs, relative):
    """Fixture files are local, regular inputs, never paths outside input/."""
    relative = pathlib.PurePosixPath(relative)
    if relative.is_absolute() or ".." in relative.parts or "\\" in str(relative) or ":" in str(relative):
        raise SystemExit(f"unsafe coverage input path: {relative}")
    path = inputs.joinpath(*relative.parts)
    if not path.is_file() or path.is_symlink() or not path.resolve().is_relative_to(inputs.resolve()):
        raise SystemExit(f"coverage input is not a regular local file: {relative}")
    return path


def combination_template(inputs, base, combination):
    """Structural variants may extend the same named metadata template."""
    source = combination.get("template")
    if source is None:
        return base
    xml = input_file(inputs, source).read_text(encoding="utf-8-sig")

    def identity(text):
        kind = re.search(r"<MetaDataObject[^>]*>\s*<(\w+)\s", text)
        name = re.search(r"<Name>([^<]+)</Name>", text)
        return (kind.group(1), name.group(1)) if kind and name else None

    if identity(base) is None or identity(xml) != identity(base):
        raise SystemExit("a structural template must retain the metadata class and template name")
    return xml


def write_body_files(inputs, output, files, template_name, name, collection):
    """A structural variant owns its descriptor and its associated source files.

    Values name committed inputs, not XML copied out of an application. This
    keeps generation deterministic and makes the existing input digest cover
    the form/template/module body as well as its descriptor.
    """
    for relative, source in files.items():
        target_relative = pathlib.PurePosixPath(relative)
        if (target_relative.is_absolute() or ".." in target_relative.parts
                or "\\" in relative or ":" in relative):
            raise SystemExit(f"unsafe coverage output path: {relative}")
        target = output / collection / name / pathlib.Path(*target_relative.parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(source, dict):
            if set(source) != {"path", "binary"} or source["binary"] is not True:
                raise SystemExit("binary fixture source requires path and binary=true")
            target.write_bytes(input_file(inputs, source["path"]).read_bytes())
        else:
            data = input_file(inputs, source).read_text(encoding="utf-8-sig")
            data = rename(data, template_name, name, seed=f"{collection}/{name}/{relative}")
            target.write_text("\ufeff" + data, encoding="utf-8")


def rename(xml, template_name, name, seed=None):
    """Gives the copy its own name and its own identifiers, derived from `seed`
    (the name by default). Class ids are constants of the platform, not of the
    object, and stay."""
    xml = xml.replace(f">{template_name}<", f">{name}<").replace(f".{template_name}", f".{name}")
    constants = set(CLASS_ID.findall(xml))
    seen = {}

    def fresh(match):
        old = match.group(0)
        if old in constants:
            return old
        if old not in seen:
            seen[old] = str(uuid.uuid5(NAMESPACE, f"{seed or name}/{len(seen)}"))
        return seen[old]

    return UUID.sub(fresh, xml)


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    set_dir, output = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    inputs = set_dir / "input"
    space = json.loads((inputs / "values.json").read_text(encoding="utf-8"))
    template = (inputs / "template.xml").read_text(encoding="utf-8-sig")
    template_name = re.search(r"<Name>([^<]+)</Name>", template).group(1)
    collection = space["collection"]
    kind_tag = re.search(r"<MetaDataObject[^>]*>\s*<(\w+) ", template).group(1)

    if output.exists():
        shutil.rmtree(output)
    shutil.copytree(inputs / "base", output)
    (output / collection).mkdir()

    variants = [(template_name, template)]
    for prop in space["properties"]:
        for entry in prop["values"]:
            name = variant_name(prop["name"], entry["value"])
            xml = set_property(template, prop["name"], prop["kind"], entry["value"])
            variants.append((name, rename(xml, template_name, name)))
    # Values that only work together: one variant sets them all.
    kinds = {prop["name"]: prop["kind"] for prop in space["properties"]}
    for combination in space.get("combinations", []):
        values = combination["set"]
        name = combination_name(combination)
        xml = combination_template(inputs, template, combination)
        for prop, value in values.items():
            xml = set_property(xml, prop, combination.get("kinds", {}).get(prop, kinds.get(prop, "enum")), value)
        variants.append((name, rename(xml, template_name, name)))
    names = [name for name, _ in variants]
    if len(set(names)) != len(names):
        raise SystemExit("two variants share a name")

    for name, xml in variants:
        (output / collection / f"{name}.xml").write_text("\ufeff" + xml, encoding="utf-8")
        files = dict(space.get("template_files", {}))
        for combination in space.get("combinations", []):
            if combination_name(combination) == name:
                files.update(combination.get("files", {}))
        write_body_files(inputs, output, files, template_name, name, collection)
    recorder = space.get("recorder")
    if recorder:
        # A register needs a document that records into it: the base's recorder
        # lists the template; it lists every variant as well.
        path = output / recorder["path"]
        text = path.read_text(encoding="utf-8-sig")
        item = f'<xr:Item xsi:type="xr:MDObjectRef">{recorder["kind"]}.{template_name}</xr:Item>'
        line = next((line for line in text.split("\n") if line.strip() == item), None)
        if line is None:
            raise SystemExit(f"{recorder['path']}: the recorder does not list {recorder['kind']}.{template_name}")
        indent = line[: len(line) - len(line.lstrip())]
        listed = "\n".join(f'{indent}<xr:Item xsi:type="xr:MDObjectRef">{recorder["kind"]}.{name}</xr:Item>'
                           for name in names)
        path.write_text("\ufeff" + text.replace(line, listed, 1), encoding="utf-8")
    configuration = output / "Configuration.xml"
    text = configuration.read_text(encoding="utf-8-sig")
    children = "".join(f"\t\t\t<{kind_tag}>{name}</{kind_tag}>\n" for name in names)
    anchor = "\t\t</ChildObjects>"
    if text.count(anchor) != 1:
        raise SystemExit("base: Configuration.xml has no single ChildObjects list")
    configuration.write_text("\ufeff" + text.replace(anchor, children + anchor), encoding="utf-8")
    print(f"{len(variants)} objects ({len(variants) - 1} variants) in {output}", file=sys.stderr)


if __name__ == "__main__":
    main()
