"""What differs in the Params `*.si` cache rows between two snapshots (ddl track, read-only).

usage: python si_diff.py <db> <label> <db> <label> [--rows <name-prefix> ...] [--max-lines 40]

The rows are raw-deflate compressed brace text. For every `*.si` row that differs after inflating,
prints the sizes and a line diff (the text is CRLF-separated). The XDTO model row
(`ea13a2c9-0c2f-40fa-b855-710387e3271d.si`) carries its XML as a base64 block: that XML is decoded and
diffed instead of the base64 lines. Snapshots come from `snapshot.py` (svc.json + the blob store).
"""
import argparse
import base64
import difflib
import hashlib
import json
import os
import re
import sys
import zlib

ROOT = os.environ.get("DDL_LAB", r"F:\ibcmd\lab\04\restructure")
XDTO_ROW = "ea13a2c9-0c2f-40fa-b855-710387e3271d.si"


def load(db, label):
    with open(os.path.join(ROOT, "snap", db, label, "svc.json"), encoding="utf-8") as f:
        return json.load(f)


def inflated(entry):
    with open(os.path.join(ROOT, "blobs", entry["sha"]), "rb") as f:
        return zlib.decompress(f.read(), -15).decode("utf-8-sig")


def params(svc):
    return {r["name"]: r for r in svc["Params"] if r["part"] == 0}


def xml_of(text):
    k = text.index("#base64:") + len("#base64:")
    b64 = text[k:text.index("}", k)]
    return base64.b64decode(re.sub(r"\s", "", b64)).decode("utf-8-sig")


def show_diff(a_lines, b_lines, limit):
    diff = [x for x in difflib.unified_diff(a_lines, b_lines, "before", "after", lineterm="", n=0) if not x.startswith("@@")]
    print("  diff lines:", len(diff))
    for x in diff[:limit]:
        print("  " + x[:300])
    if len(diff) > limit:
        print("  ...")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("db_a")
    ap.add_argument("label_a")
    ap.add_argument("db_b")
    ap.add_argument("label_b")
    ap.add_argument("--rows", nargs="*", default=[])
    ap.add_argument("--max-lines", type=int, default=40)
    args = ap.parse_args()
    a, b = params(load(args.db_a, args.label_a)), params(load(args.db_b, args.label_b))
    print(f"== {args.db_a}/{args.label_a}  ->  {args.db_b}/{args.label_b}")
    names = sorted(n for n in set(a) | set(b) if n.endswith(".si") and (not args.rows or any(n.startswith(p) for p in args.rows)))
    same = 0
    for name in names:
        if name not in a or name not in b:
            print(f"-- {name}: only in {'after' if name in b else 'before'}")
            continue
        ta, tb = inflated(a[name]), inflated(b[name])
        if ta == tb:
            same += 1
            continue
        print(f"-- {name}: raw {a[name]['len']} -> {b[name]['len']} bytes, text {len(ta)} -> {len(tb)} chars, "
              f"sha256 {hashlib.sha256(ta.encode()).hexdigest()[:12]} -> {hashlib.sha256(tb.encode()).hexdigest()[:12]}")
        if name == XDTO_ROW:
            print("  (the base64 block decoded: the XDTO model XML)")
            show_diff(xml_of(ta).split("\r\n"), xml_of(tb).split("\r\n"), args.max_lines)
        else:
            show_diff(ta.split("\r\n"), tb.split("\r\n"), args.max_lines)
    print(f"{same} of {len(names)} `.si` rows have the same text (compared after inflate)")


if __name__ == "__main__":
    sys.exit(main())
