"""The order of the tables in the DBSchema of a lab database (uha_order_proof.ps1).

usage: python schema_order_dump.py <db> <out file>            one table name per line
       python schema_order_dump.py --compare <before> <after> [<rcheck log with the platform's positions>]
The comparison finds the tables the apply moved (the smallest tail of `after` such that the rest is `before` without them), prints where they sit
and where ConfigChngR sits, and, with rcheck's log (`b1_schema_order.txt`: "Reference226  pos in 1: 16851  pos in 2: 4803", 1 = the platform's twin), whether
each of them is where the platform put it.
"""
import io
import os
import re
import sys

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def dump(db, out):
    import bracefmt as bf
    from db import rows

    text = bytes(rows(db, "SELECT SerializedData FROM DBSchema")[0][0])
    doc = bf.parse(text)
    names = [str(t[0]) for t in doc[1][1:]]
    with open(out, "w", encoding="utf-8") as f:
        f.write("\n".join(names) + "\n")
    print("%d tables -> %s" % (len(names), out))


def load(path):
    with open(path, encoding="utf-8") as f:
        return [line.strip() for line in f if line.strip()]


def compare(before_path, after_path, log_path):
    before, after = load(before_path), load(after_path)
    print("tables: %d before, %d after" % (len(before), len(after)))
    tail = None
    for i in range(1, 200):
        moved = set(after[-i:])
        if after[:-i] == [t for t in before if t not in moved]:
            tail = after[-i:]
            break
    if tail is None:
        print("the apply moved more than the last 199 tables, or changed the order of the others")
        return 1
    start = len(after) - len(tail)
    print("moved to the end: %d tables at positions %d-%d: %s" % (len(tail), start, len(after) - 1, ", ".join(tail)))
    print("ConfigChngR: position %d before, %d after (of %d)" % (before.index("ConfigChngR"), after.index("ConfigChngR"), len(after)))
    print("the order of the other tables is the stored one: True")
    if log_path:
        expected = {}
        with open(log_path, encoding="utf-8") as f:
            for line in f:
                m = re.match(r"^(\S+)\s+pos in 1: (\d+)\s+pos in 2: ", line)
                if m:
                    expected[m.group(1)] = int(m.group(2))
        if not expected or set(tail) != set(expected):
            print("missing native positions, or moved-table set differs from the native log")
            return 1
        bad = [(name, want, after.index(name) if name in after else None) for name, want in expected.items() if not name in after or after.index(name) != want]
        print("the platform's positions (rcheck's twin, %d tables): %s" % (len(expected), "all equal" if not bad else "DIFFERENT %s" % bad))
        return 0 if not bad else 1
    return 0


def main():
    if sys.argv[1] == "--compare":
        raise SystemExit(compare(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None))
    dump(sys.argv[1], sys.argv[2])


main()
