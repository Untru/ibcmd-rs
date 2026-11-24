"""Check 8 of docs/apply/restructuring.md 12.6 for twins staged SEPARATELY: the native export of both, compared file by file.

usage: python check8.py [--exact-summary] <export of the native twin> <export of the own twin>

For strict check 8 evidence, --exact-summary includes the raw ConfigDumpInfo.xml,
prints only counts and exits nonzero on missing roots, unreadable files or any difference.
Run it only after both native exports have exited successfully into fresh directories.

`ConfigDumpInfo.xml` prints a `configVersion` for every object; the version of a changed object is the generation the
staging (an import) gave it, random per import. So two twins staged apart can never agree on those attributes; this
compares every file byte for byte, and `ConfigDumpInfo.xml` with the `configVersion` values taken out, and lists the objects
whose `configVersion` differs (they should be exactly the changed ones).
"""
import argparse
import hashlib
import os
import re
import sys


def raise_walk_error(error):
    raise error


def files(root):
    out = {}
    for base, _, names in os.walk(root, onerror=raise_walk_error):
        for name in names:
            path = os.path.join(base, name)
            out[os.path.relpath(path, root)] = path
    return out


def sha(path):
    with open(path, "rb") as f:
        digest = hashlib.sha256()
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
        return digest.digest()


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--exact-summary", action="store_true")
parser.add_argument("native")
parser.add_argument("own")
args = parser.parse_args()

try:
    if args.exact_summary:
        for root in (args.native, args.own):
            if not all(os.path.isfile(os.path.join(root, name))
                       for name in ("Configuration.xml", "ConfigDumpInfo.xml")):
                sys.exit("8. incomplete export: missing configuration or dump information")
        if os.path.samefile(args.native, args.own):
            sys.exit("8. comparison requires separate exports")

    nat, own = files(args.native), files(args.own)
    only_nat, only_own = sorted(set(nat) - set(own)), sorted(set(own) - set(nat))
    different = [rel for rel in sorted(set(nat) & set(own)) if sha(nat[rel]) != sha(own[rel])]
except OSError:
    if args.exact_summary:
        sys.exit("8. unable to read both exports completely")
    raise
info = "ConfigDumpInfo.xml"
if args.exact_summary:
    print(f"8. exact export: native {len(nat)}, own {len(own)}, only native {len(only_nat)}, "
          f"only own {len(only_own)}, different {len(different)}, "
          f"ConfigDumpInfo.xml differs {int(info in different)}")
    sys.exit(int(bool(only_nat or only_own or different)))

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
