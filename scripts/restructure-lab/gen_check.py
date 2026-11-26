"""Generate the main-table field list of catalogs and documents from the XML metadata + DBNames and compare
it with the real DBSchema entries (research: is the metadata -> DBSchema mapping reproducible?).
usage: python gen_check.py <snapshot-label>"""
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
from md_types_check import NS, TREE, tag

DBN = None
COMMON_NUMS = set()


def by_uuid(label):
    t = zlib.decompress(lab.row(label, "Params", "DBNames"), -15)
    maxn, count, ents = names.parse_dbnames(t)
    d = collections.defaultdict(dict)
    for u, k, n in ents:
        d[u][k] = n
    return d


def type_entries(props, nm):
    """XML <Type> -> list of DBSchema type-entry tuples (tag, a, b, refname, k [, six])."""
    te = props.find("md:Type", NS)
    kids = list(te)
    types = [c for c in kids if tag(c) in ("Type", "TypeSet")]
    out = []
    prim = []
    refs = []
    for c in types:
        txt = (c.text or "").strip()
        if tag(c) == "TypeSet":
            return None      # defined types / characteristics: needs the resolved type set
        if txt == "xs:boolean":
            prim.append(("L", 0, 0, "", 0))
        elif txt == "xs:string":
            sq = te.find("v8:StringQualifiers", NS)
            ln = int(sq.find("v8:Length", NS).text)
            var = sq.find("v8:AllowedLength", NS).text == "Variable"
            prim.append(("S", (0x80000000 | ln) if var else ln, 0, "", 0))
        elif txt == "xs:decimal":
            nq = te.find("v8:NumberQualifiers", NS)
            prim.append(("N", int(nq.find("v8:Digits", NS).text), int(nq.find("v8:FractionDigits", NS).text), "", 0))
        elif txt == "xs:dateTime":
            prim.append(("T", 0, 0, "", 0))
        elif txt == "v8:ValueStorage":
            prim.append(("B", 0x80000000, 0, "", 0))
        elif txt == "v8:UUID":
            prim.append(("B", 16, 0, "", 0))
        elif txt.startswith("cfg:"):
            m = re.match(r"cfg:(\w+?)Ref\.(.+)$", txt)
            if not m:
                return None
            refs.append((m.group(1), m.group(2)))
        else:
            return None
    if len(prim) + len(refs) == 1:
        if prim:
            return [prim[0]]
        kind, name = refs[0]
        return [("R", 0, 0, ("REF", kind, name), 3)]
    # composite type: E + primitive parts (in the fixed order L N T S) + R (untyped)
    order = {"L": 0, "N": 1, "T": 2, "S": 3, "B": 4}
    parts = [("E", 0, 0, "", 0)] + sorted(prim, key=lambda x: order[x[0]])
    if refs:
        parts.append(("R", 0, 0, "", 4))
    return parts


def main():
    label = sys.argv[1]
    db = by_uuid(label)
    nmx = names.dumpinfo_names(os.path.join(TREE, "ConfigDumpInfo.xml"))
    for u, kv in db.items():
        if "Fld" in kv and nmx.get(u, "").startswith("CommonAttribute."):
            COMMON_NUMS.add(kv["Fld"])
    print("common attribute fields:", sorted(COMMON_NUMS))
    tables = {D.s(t[0]): t for t in D.parse_schema(lab.dbschema(label))}
    # uuid of every named object for reference resolution: ConfigDumpInfo
    stats = collections.Counter()
    bad = []
    for d, root_tag, kind in (("Catalogs", "Catalog", "Reference"), ("Documents", "Document", "Document")):
        for f in sorted(glob.glob(os.path.join(TREE, d, "*.xml"))):
            obj = ET.parse(f).getroot()[0]
            ouuid = obj.get("uuid")
            num = db[ouuid].get(kind)
            t = tables.get("%s%d" % (kind, num)) if num else None
            if t is None:
                stats["no_table"] += 1
                continue
            props = obj.find("md:Properties", NS)
            hier = (props.findtext("md:Hierarchical", namespaces=NS) or "false") == "true"
            htype = props.findtext("md:HierarchyType", namespaces=NS)
            gen = []      # (name, flag, [entries])
            def add(name, flag, ents):
                gen.append((name, flag, ents))
            add("ID", 0, [("R", 0, 0, "%s%d" % (kind, num), 2)])
            add("Version", 0, [("V", 0, 0, "", 0)])
            add("Marked", 0, [("L", 0, 0, "", 0)])
            if kind == "Reference":
                add("PredefinedID", 0, [("B", 16, 0, "", 0)])
                owners = props.find("md:Owners", NS)
                if owners is not None and len(owners):
                    stats["skip_owners"] += 1
                    continue
                if hier:
                    add("ParentID", 0, [("R", 0, 0, "%s%d" % (kind, num), 3)])
                    if htype == "HierarchyFoldersAndItems":
                        add("Folder", 0, [("L", 0, 0, "", 0)])
                cl = int(props.findtext("md:CodeLength", "0", NS))
                if cl > 0:
                    ct = props.findtext("md:CodeType", namespaces=NS)
                    al = props.findtext("md:CodeAllowedLength", namespaces=NS)
                    if ct == "String":
                        add("Code", 0, [("S", (0x80000000 | cl) if al == "Variable" else cl, 0, "", 0)])
                    else:
                        add("Code", 0, [("N", cl, 0, "", 0)])
                dl = int(props.findtext("md:DescriptionLength", "0", NS))
                if dl > 0:
                    add("Description", 0, [("S", 0x80000000 | dl, 0, "", 0)])
            else:
                add("Date_Time", 0, [("T", 0, 0, "", 0)])
                if (props.findtext("md:NumberPeriodicity", namespaces=NS) or "Nonperiodical") != "Nonperiodical":
                    add("NumberPrefix", 0, [("T", 0, 0, "", 0)])
                nt = props.findtext("md:NumberType", namespaces=NS)
                nl = int(props.findtext("md:NumberLength", "0", NS))
                al = props.findtext("md:NumberAllowedLength", namespaces=NS)
                if nt == "String":
                    add("Number", 0, [("S", (0x80000000 | nl) if al == "Variable" else nl, 0, "", 0)])
                else:
                    add("Number", 0, [("N", nl, 0, "", 0)])
                add("Posted", 0, [("L", 0, 0, "", 0)])
            co = obj.find("md:ChildObjects", NS)
            unsupported = False
            if co is not None:
                for el in co:
                    if tag(el) != "Attribute":
                        continue
                    ap = el.find("md:Properties", NS)
                    fld = db[el.get("uuid")].get("Fld")
                    ents = type_entries(ap, None)
                    if ents is None or fld is None:
                        unsupported = True
                        break
                    use = ap.findtext("md:Use", namespaces=NS)
                    nullable = 1 if (kind == "Reference" and hier and htype == "HierarchyFoldersAndItems" and use == "ForItem") else 0
                    add("Fld%d" % fld, nullable, ents)
            if unsupported:
                stats["skip_unsupported_type"] += 1
                continue
            # compare with the real entry (without trailing separator fields, refs to types resolved loosely)
            real = [(D.s(fl[0]), n_(fl[1]), [tuple(D.s(x) for x in ty[:5]) for ty in fl[2][1:]]) for fl in t[4][1:]]
            sep_names = {x for grp in D.sep_fields(t[9]) for x in grp} | {x for grp in D.sep_fields(t[10]) for x in grp}
            sep_names |= {"Fld%d" % n for n in COMMON_NUMS}
            real = [r for r in real if r[0] not in sep_names]

            def norm_gen(g):
                out = []
                for name, flag, ents in g:
                    es = []
                    for e in ents:
                        if isinstance(e[3], tuple):        # a typed reference: compare only the shape
                            es.append(("R", "0", "0", "*", "3"))
                        elif e[0] == "R" and e[3]:
                            es.append(("R", "0", "0", "*", str(e[4])))
                        else:
                            es.append(tuple(str(x) for x in e[:5]))
                    out.append((name, flag, es))
                return out

            def norm_real(rl):
                out = []
                for name, flag, ents in rl:
                    es = []
                    for e in ents:
                        if e[0] == "R" and e[3]:
                            es.append(("R", "0", "0", "*", e[4]))
                        else:
                            es.append(tuple(e))
                    out.append((name, flag, es))
                return out

            g, r = norm_gen(gen), norm_real(real)
            if g == r:
                stats["main_fields_match"] += 1
            else:
                stats["main_fields_mismatch"] += 1
                bad.append((os.path.basename(f), [x for x in g if x not in r][:3], [x for x in r if x not in g][:3]))
    print(dict(stats))
    for b in bad[:12]:
        print(b)


def n_(x):
    return int(D.s(x))


if __name__ == "__main__":
    main()
