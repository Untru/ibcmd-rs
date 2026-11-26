"""Diff two snapshots made by snapshot.py.

usage: python snapdiff.py <db-before> <label-before> <db-after> <label-after> [--max N]
"""
import json
import os
import sys

from lab import ROOT


def load(db, label):
    d = os.path.join(ROOT, "snap", db, label)
    schema = {}
    cur = None
    if os.path.exists(os.path.join(d, "schema.txt")):
        with open(os.path.join(d, "schema.txt"), encoding="utf-8") as f:
            for line in f:
                line = line.rstrip("\n")
                if line.startswith("T "):
                    cur = line[2:]
                    schema[cur] = []
                else:
                    schema[cur].append(line.strip())
    tables = {}
    if os.path.exists(os.path.join(d, "tables.tsv")):
        with open(os.path.join(d, "tables.tsv"), encoding="utf-8") as f:
            for line in f:
                a = line.rstrip("\n").split("\t")
                tables[a[0]] = (int(a[1]), a[2])
    with open(os.path.join(d, "svc.json"), encoding="utf-8") as f:
        svc = json.load(f)
    return schema, tables, svc


def main():
    db1, l1, db2, l2 = sys.argv[1:5]
    mx = int(sys.argv[sys.argv.index("--max") + 1]) if "--max" in sys.argv else 60
    s1, t1, v1 = load(db1, l1)
    s2, t2, v2 = load(db2, l2)
    print("== schema: tables added / removed / changed")
    add = sorted(set(s2) - set(s1))
    rem = sorted(set(s1) - set(s2))
    chg = sorted(t for t in set(s1) & set(s2) if s1[t] != s2[t])
    print("added (%d): %s" % (len(add), ", ".join(add[:mx])))
    print("removed (%d): %s" % (len(rem), ", ".join(rem[:mx])))
    print("changed (%d):" % len(chg))
    for t in chg[:mx]:
        a, b = s1[t], s2[t]
        print("  T %s" % t)
        for x in a:
            if x not in b:
                print("     - %s" % x)
        for x in b:
            if x not in a:
                print("     + %s" % x)
    for t in add[:mx]:
        print("  NEW T %s" % t)
        for x in s2[t]:
            print("     + %s" % x)
    print("== data: tables whose rowcount/checksum changed")
    dchg = []
    for t in sorted(set(t1) | set(t2)):
        if t1.get(t) != t2.get(t):
            dchg.append((t, t1.get(t), t2.get(t)))
    print("(%d)" % len(dchg))
    for t, a, b in dchg[:mx]:
        print("  %s: %s -> %s" % (t, a, b))
    print("== service blobs")
    print("DBSchema same:", v1["DBSchema"] == v2["DBSchema"], v1["DBSchema"], v2["DBSchema"])
    for a, b in zip(v1["SchemaStorage"], v2["SchemaStorage"]):
        print("SchemaStorage id=%s status %s->%s cur same=%s ngc %s->%s ngd %s->%s" % (
            a["id"], a["status"], b["status"], a["cur"] == b["cur"], a["ngc"][:8], b["ngc"][:8],
            a["ngd"][:8], b["ngd"][:8]))
    print("IBVersion", v1["IBVersion"], v2["IBVersion"])
    for tname in ("Params", "Config", "ConfigSave", "ConfigCAS", "ConfigCASSave", "Files"):
        i1 = {(r["name"], r["part"]): r for r in v1[tname]}
        i2 = {(r["name"], r["part"]): r for r in v2[tname]}
        added = sorted(set(i2) - set(i1))
        removed = sorted(set(i1) - set(i2))
        changed = sorted(k for k in set(i1) & set(i2) if i1[k]["sha"] != i2[k]["sha"])
        meta = sorted(k for k in set(i1) & set(i2) if i1[k]["sha"] == i2[k]["sha"] and
                      (i1[k]["created"], i1[k]["modified"], i1[k]["attr"]) != (i2[k]["created"], i2[k]["modified"], i2[k]["attr"]))
        print("%s: rows %d -> %d; added %d removed %d changed %d meta-only %d" % (
            tname, len(i1), len(i2), len(added), len(removed), len(changed), len(meta)))
        for k in added[:mx]:
            print("    + %s [%d] %d bytes" % (k[0], k[1], i2[k]["len"]))
        for k in removed[:mx]:
            print("    - %s [%d] %d bytes" % (k[0], k[1], i1[k]["len"]))
        for k in changed[:mx]:
            print("    ~ %s [%d] %d -> %d bytes  %s -> %s" % (k[0], k[1], i1[k]["len"], i2[k]["len"],
                                                            i1[k]["sha"][:8], i2[k]["sha"][:8]))
        for k in meta[:10]:
            print("    m %s [%d] %s -> %s" % (k[0], k[1], i1[k]["modified"], i2[k]["modified"]))


if __name__ == "__main__":
    main()
