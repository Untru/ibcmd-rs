"""Is database B (staged base-free/patch, then natively applied) the same as database A (staged by the
native import, then natively applied)?  (issue #388)

  python equiv_cmp.py <dbA> <dbB> [--show N]

Read-only. Compares, after both applies:
  * the table structure (columns, indexes: the ddl-track snapshot text),
  * DBNames (uuid, kind, number) and DBSchema,
  * the Params names,
  * the Config rows: names, and row content by rowdiff's classes.
"""
import argparse
import difflib
import os
import re
import sys
import zlib

os.environ.setdefault("DDL_LAB", r"F:\ibcmd\lab\04\import")
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "restructure-lab"))
sys.path.insert(0, HERE)

import db as dbm  # noqa: E402
import names as dbnames  # noqa: E402
import rowdiff  # noqa: E402
import snapshot  # noqa: E402


def params(db):
    out = {}
    for name, size, data in dbm.rows(db, "SELECT FileName, DataSize, BinaryData FROM Params ORDER BY FileName"):
        out[name] = bytes(data)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--show", type=int, default=30)
    args = ap.parse_args()

    sa, sb = snapshot.schema_text(args.a), snapshot.schema_text(args.b)
    if sa == sb:
        print("structure: identical (%d lines)" % sa.count("\n"))
    else:
        print("structure: DIFFERENT")
        for line in list(difflib.unified_diff(sa.splitlines(), sb.splitlines(), "A", "B", lineterm="", n=0))[: args.show]:
            print("  " + line)

    pa, pb = params(args.a), params(args.b)
    print("Params names: A %d, B %d, only A %s, only B %s" % (len(pa), len(pb), sorted(set(pa) - set(pb)), sorted(set(pb) - set(pa))))
    for name in ("DBNames",):
        if name in pa and name in pb:
            ta, tb = zlib.decompress(pa[name], -15), zlib.decompress(pb[name], -15)
            ma, ca, ea = dbnames.parse_dbnames(ta)
            mb, cb, eb = dbnames.parse_dbnames(tb)
            print("DBNames: header max %d/%d count %d/%d; entries equal as sets: %s; same order: %s" % (
                ma, mb, ca, cb, set(ea) == set(eb), ea == eb))
            for e in sorted(set(ea) - set(eb))[:10]:
                print("   only A", e)
            for e in sorted(set(eb) - set(ea))[:10]:
                print("   only B", e)
    dba = dbm.rows(args.a, "SELECT SerializedData FROM DBSchema")
    dbb = dbm.rows(args.b, "SELECT SerializedData FROM DBSchema")
    if dba and dbb:
        xa = zlib.decompress(bytes(dba[0][0]), -15) if bytes(dba[0][0])[:1] != b"\xef" else bytes(dba[0][0])
        xb = zlib.decompress(bytes(dbb[0][0]), -15) if bytes(dbb[0][0])[:1] != b"\xef" else bytes(dbb[0][0])
        print("DBSchema: equal bytes %s (%d / %d)" % (xa == xb, len(xa), len(xb)))

    left = rowdiff.load(args.a, "Config")
    right = rowdiff.load(args.b, "Config")
    cls, counts = rowdiff.compare(left, right, {})
    print("Config rows: A %d names, B %d names" % (len(left), len(right)))
    for k in ("identical_bytes", "identical_text", "layout_only", "container_headers", "different", "only_left", "only_right"):
        print("  %-18s %6d" % (k, counts.get(k, 0)))
    for k in ("only_left", "only_right"):
        items = [n for n, (c, _) in cls.items() if c == k]
        if items:
            print("  %s (A only)" % k if k == "only_left" else "  %s (B only)" % k, items[: args.show])


if __name__ == "__main__":
    main()
