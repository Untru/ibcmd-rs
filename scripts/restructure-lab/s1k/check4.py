"""Check 4 of docs/apply/restructuring.md 12.6, for twins that were staged SEPARATELY (native import on one, our import on the other).

usage: python check4.py <native db> <own db>

When the twins come from one backup of the staged state their Config rows are equal in every column (12.6 says so). Staged
apart, two imports differ in what an import writes by itself: the `Creation` / `Modified` of the rows it writes, and the
generation the stage carries (`versions`, `version`, `root`). This says, per row of Config, whether the bytes are equal,
equal after inflate, or different, whether only the dates differ, and lists the rows that differ in content.
"""
import os
import sys
import zlib

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import db as dbm  # noqa: E402


def inflate(raw):
    try:
        return zlib.decompress(raw, -15)
    except Exception:
        return raw


def load(db):
    rows = dbm.rows(db, "SELECT FileName, PartNo, Creation, Modified, Attributes, DataSize, BinaryData FROM Config")
    return {(r[0], r[1]): (r[2], r[3], r[4], r[5], bytes(r[6])) for r in rows}


nat, own = load(sys.argv[1]), load(sys.argv[2])
only_nat = sorted(set(nat) - set(own))
only_own = sorted(set(own) - set(nat))
same = dates_only = inflated_equal = different = 0
content = []
for key in sorted(set(nat) & set(own)):
    a, b = nat[key], own[key]
    if a[4] == b[4]:
        if a[:4] == b[:4]:
            same += 1
        else:
            dates_only += 1
    elif inflate(a[4]) == inflate(b[4]):
        inflated_equal += 1
    else:
        different += 1
        content.append(key[0] if key[1] == 0 else "%s[%d]" % key)
print("4. Config rows: native %d, own %d; only in native %d, only in own %d" % (len(nat), len(own), len(only_nat), len(only_own)))
print("4. common rows: identical in every column %d; identical bytes, other dates %d; equal after inflate %d; different content %d"
      % (same, dates_only, inflated_equal, different))
if content:
    print("4. rows that differ in content: " + ", ".join(content[:12]) + (" ..." if len(content) > 12 else ""))
if only_nat or only_own:
    print("4. only native: %s; only own: %s" % (only_nat[:5], only_own[:5]))
