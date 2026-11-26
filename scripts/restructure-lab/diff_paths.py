"""The files a `source-diff` report lists as not unchanged, against the ones that are expected to differ.

usage: python diff_paths.py <source-diff report.json> [<list of expected paths, one per line> [<export of our twin> <edited forms dir>]]
With the last two: the forms of our twin's export are also compared, byte for byte, with the edited forms that were imported.
The route through our import stages the whole tree, the forms without the fields bound to removed attributes (forms.txt of the case) among it;
the twin the platform staged with the descriptors only keeps the original forms. So the exports of the two differ in exactly those forms and in
`ConfigDumpInfo.xml` (the versions of the changed forms). Prints the differing paths and whether they are exactly the expected ones.
"""
import json
import os
import sys

BOM = bytes([0xEF, 0xBB, 0xBF])


def main():
    report = json.load(open(sys.argv[1], encoding="utf-8"))
    expected = set()
    if len(sys.argv) > 2 and os.path.exists(sys.argv[2]):
        with open(sys.argv[2], encoding="utf-8") as f:
            expected = {line.strip() for line in f if line.strip()}
    expected.add("ConfigDumpInfo.xml")
    differ = sorted(x["path"] for x in report["differences"] if x["status"] != "unchanged")
    extra = [p for p in differ if p not in expected]
    missing = sorted(p for p in expected if p != "ConfigDumpInfo.xml" and p not in differ)
    print("%d file(s) differ; exactly the edited forms and ConfigDumpInfo.xml: %s" % (len(differ), not extra and not missing))
    for p in extra:
        print("  unexpected:", p)
    for p in missing:
        print("  edited but equal:", p)
    valid = not extra and not missing
    if len(sys.argv) > 4:
        same = 0
        for rel in sorted(p for p in expected if p != "ConfigDumpInfo.xml"):
            with open(os.path.join(sys.argv[3], rel), "rb") as a, open(os.path.join(sys.argv[4], rel), "rb") as b:
                if a.read().lstrip(BOM) == b.read().lstrip(BOM):
                    same += 1
                else:
                    print("  the export differs from the imported form:", rel)
        print("%d of %d edited forms come back from the twin's export byte for byte" % (same, len(expected) - 1))
        valid = valid and same == len(expected) - 1
    return 0 if valid else 1


raise SystemExit(main())
