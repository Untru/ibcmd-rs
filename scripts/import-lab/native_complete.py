"""Is the ConfigSave a native `config import` staged complete? (issue #388)

  python native_complete.py <db>      exit 0 = complete, 1 = not

The lab rule (README, 2026-09-29 14:15): a native import is complete when ConfigSave has `versions`, has no
`commit` or `*.new` row, and every entry of the staged `versions` that differs from Config's has its row. The row
count is not a criterion (a complete БСП stage is ~9 618 rows in one clone, 9 842 in another). Read-only.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rowdiff  # noqa: E402

SERVICE = {"root", "version", "versions", "deleted"}


def entries(row):
    text = rowdiff.inflate(row["raw"]).decode("utf-8-sig", "replace")
    return dict(re.findall(r'"([^"]+)",([0-9a-f]{8}-[0-9a-f-]{27})', text))


def main():
    db = sys.argv[1]
    save = rowdiff.load(db, "ConfigSave")
    config = rowdiff.load(db, "Config")
    if "versions" not in save:
        print("incomplete: ConfigSave has no `versions` (%d rows)" % len(save))
        return 1
    leftovers = sorted(n for n in save if n == "commit" or n.endswith(".new"))
    if leftovers:
        print("incomplete: rows %s" % leftovers[:5])
        return 1
    staged = entries(save["versions"])
    stored = entries(config["versions"]) if "versions" in config else {}
    missing = sorted(n for n, u in staged.items() if stored.get(n) != u and n not in save and n not in SERVICE)
    if missing:
        print("incomplete: %d entries of `versions` differ from Config's and have no row, e.g. %s" % (len(missing), missing[:3]))
        return 1
    print("complete: %d rows (%d part rows), %d versions entries, %d differ from Config's" % (
        len(save), sum(e["parts"] for e in save.values()), len(staged),
        sum(1 for n, u in staged.items() if stored.get(n) != u)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
