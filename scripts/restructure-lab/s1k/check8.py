"""Check 8 of docs/apply/restructuring.md 12.6 for twins staged SEPARATELY: the native export of both, compared file by file.

usage: python check8.py <export of the native twin> <export of the own twin>

`ConfigDumpInfo.xml` prints a `configVersion` for every object; the version of a changed object is the generation the
staging (an import) gave it, random per import. So two twins staged apart can never agree on those attributes; this
compares every file byte for byte, and `ConfigDumpInfo.xml` with the `configVersion` values taken out, and lists the objects
whose `configVersion` differs (they should be exactly the changed ones).
"""
import hashlib
import os
import re
import sys


def files(root):
    out = {}
    for base, _, names in os.walk(root):
        for name in names:
            path = os.path.join(base, name)
            out[os.path.relpath(path, root)] = path
    return out


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


nat, own = files(sys.argv[1]), files(sys.argv[2])
only_nat, only_own = sorted(set(nat) - set(own)), sorted(set(own) - set(nat))
different = [rel for rel in sorted(set(nat) & set(own)) if sha(nat[rel]) != sha(own[rel])]
info = "ConfigDumpInfo.xml"
version = re.compile(r'(<Metadata name="([^"]*)" id="[^"]*") configVersion="[0-9a-f]*"')
detail = ""
if info in different:
    a = open(nat[info], encoding="utf-8-sig").read()
    b = open(own[info], encoding="utf-8-sig").read()
    blank = lambda t: version.sub(r"\1", t)
    by_name = lambda t: dict((m.group(2), m.group(0)) for m in version.finditer(t))
    va, vb = by_name(a), by_name(b)
    moved = sorted(name for name in va if va[name] != vb.get(name))
    if blank(a) == blank(b):
        different.remove(info)
        detail = "; ConfigDumpInfo.xml equal but the configVersion of %d object(s): %s" % (len(moved), ", ".join(moved[:8]) + (" ..." if len(moved) > 8 else ""))
    else:
        detail = "; ConfigDumpInfo.xml differs beyond configVersion (%d objects with another version)" % len(moved)
total = len(set(nat) | set(own))
print("8. native export of both: %d files; only native %d, only own %d, different %d%s"
      % (total, len(only_nat), len(only_own), len(different), detail))
for rel in different[:10]:
    print("8.   different:", rel)
for rel in (only_nat + only_own)[:10]:
    print("8.   only in one:", rel)
