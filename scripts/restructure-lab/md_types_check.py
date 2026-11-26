"""How the metadata (the XML tree) determines the DBSchema fields of catalogs/documents/registers.

For every Attribute / Dimension / Resource of the objects in the reference tree: XML type descriptor ->
DBSchema field type entries (found through DBNames uuid -> FldNNN -> the table's field), tallied.
usage: python md_types_check.py <snapshot-label> [tree-root]
"""
import collections
import glob
import os
import re
import sys
import zlib
import xml.etree.ElementTree as ET

import bracefmt as bf
import dbschema as D
import lab
import names

NS = {
    "md": "http://v8.1c.ru/8.3/MDClasses",
    "v8": "http://v8.1c.ru/8.1/data/core",
    "xr": "http://v8.1c.ru/8.3/xcf/readable",
    "xsi": "http://www.w3.org/2001/XMLSchema-instance",
}
from lab import NATIVE_TREE as TREE

KINDS = {  # xml dir -> (root element, DBNames kind of the object)
    "Catalogs": ("Catalog", "Reference"),
    "Documents": ("Document", "Document"),
    "InformationRegisters": ("InformationRegister", "InfoRg"),
    "AccumulationRegisters": ("AccumulationRegister", "AccumRg"),
}


def tag(e):
    return e.tag.split("}")[1] if "}" in e.tag else e.tag


def type_key(type_el):
    """A compact, comparable description of the <Type> of an attribute."""
    if type_el is None:
        return "?"
    parts = []
    for c in type_el:
        t = tag(c)
        if t in ("Type", "TypeSet"):
            txt = (c.text or "").strip()
            # generic form: keep the qualifier-free type class, drop the object name
            m = re.match(r"cfg:(\w+?)(Ref|)\.(.+)$", txt)
            if m:
                txt = "cfg:%s%s.*" % (m.group(1), m.group(2)) if t == "Type" else "%s:cfg:%s" % (t, m.group(1))
            parts.append("%s=%s" % (t, txt))
        elif t == "StringQualifiers":
            ln = c.find("v8:Length", NS).text
            al = c.find("v8:AllowedLength", NS).text
            parts.append("S(%s,%s)" % (ln, al))
        elif t == "NumberQualifiers":
            d = c.find("v8:Digits", NS).text
            f = c.find("v8:FractionDigits", NS).text
            sgn = c.find("v8:AllowedSign", NS).text
            parts.append("N(%s,%s,%s)" % (d, f, sgn))
        elif t == "DateQualifiers":
            parts.append("D(%s)" % c.find("v8:DateFractions", NS).text)
        elif t == "BinaryDataQualifiers":
            parts.append("BIN")
        else:
            parts.append(t)
    return "|".join(parts)


def load_db(label):
    t = zlib.decompress(lab.row(label, "Params", "DBNames"), -15)
    maxn, count, ents = names.parse_dbnames(t)
    by_uuid = collections.defaultdict(list)
    for u, k, n in ents:
        by_uuid[u].append((k, n))
    tables = {D.s(x[0]): x for x in D.parse_schema(lab.dbschema(label))}
    return by_uuid, tables


def main():
    label = sys.argv[1]
    tree = sys.argv[2] if len(sys.argv) > 2 else TREE
    by_uuid, tables = load_db(label)
    tally = collections.defaultdict(collections.Counter)
    ex = {}
    nullable_tally = collections.Counter()
    problems = collections.Counter()
    for d, (root_tag, kind) in KINDS.items():
        for f in sorted(glob.glob(os.path.join(tree, d, "*.xml"))):
            try:
                root = ET.parse(f).getroot()
            except Exception as e:
                problems["xml"] += 1
                continue
            obj = root[0]
            ouuid = obj.get("uuid")
            tn = [(k, n) for k, n in by_uuid.get(ouuid, []) if k == kind]
            if not tn:
                problems["object_not_in_dbnames"] += 1
                continue
            table = tables.get("%s%d" % (kind, tn[0][1]))
            if table is None:
                problems["table_missing"] += 1
                continue
            hier = (obj.findtext("md:Properties/md:Hierarchical", namespaces=NS) or "false") == "true"
            if hier:
                hier = obj.findtext("md:Properties/md:HierarchyType", namespaces=NS) or "?"
            fields = {D.s(x[0]): x for x in table[4][1:]}
            co = obj.find("md:ChildObjects", NS)
            if co is None:
                continue
            for el in co:
                t = tag(el)
                if t not in ("Attribute", "Dimension", "Resource"):
                    continue
                auuid = el.get("uuid")
                fl = [(k, n) for k, n in by_uuid.get(auuid, []) if k == "Fld"]
                if not fl:
                    problems["attr_not_in_dbnames"] += 1
                    continue
                fld = fields.get("Fld%d" % fl[0][1])
                if fld is None:
                    problems["field_missing_in_table"] += 1
                    continue
                props = el.find("md:Properties", NS)
                tk = type_key(props.find("md:Type", NS))
                entries = "/".join("%s:%s:%s:%s:%s" % tuple(D.s(y) for y in x[:5]) if D.s(x[0]) in ("B", "L", "N", "T", "S", "E") else
                                   "R:%s:%s" % (("T" if not D.s(x[3]) else "ref"), D.s(x[4]))
                                   for x in fld[2][1:])
                # generalize numbers in the entries only where they carry the type qualifiers
                key = (t, tk)
                tally[key][entries] += 1
                ex.setdefault((key, entries), "%s.%s" % (os.path.basename(f), props.findtext("md:Name", namespaces=NS)))
                use = props.findtext("md:Use", namespaces=NS)
                nullable_tally[(d, hier if hier else "flat", use, D.s(fld[1]))] += 1
    print("problems", dict(problems))
    print("== type descriptor -> DBSchema type entries (distinct mappings per descriptor)")
    multi = 0
    for key, c in sorted(tally.items(), key=lambda kv: -sum(kv[1].values())):
        if len(c) > 1:
            multi += 1
        if sum(c.values()) >= 1:
            pass
    print("descriptors:", len(tally), "with more than one mapping:", multi)
    for key, c in sorted(tally.items(), key=lambda kv: -sum(kv[1].values()))[:45]:
        print(sum(c.values()), key, "->", dict(c) if len(c) <= 3 else "%d variants e.g. %s" % (len(c), list(c)[:3]))
    print("== nullable flag by (object kind, hierarchy, Use)")
    for k, v in sorted(nullable_tally.items(), key=lambda kv: str(kv[0])):
        print(v, k)


if __name__ == "__main__":
    main()
