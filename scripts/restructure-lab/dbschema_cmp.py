"""DBSchema of two snapshots (any two databases) entry by entry.

usage: python dbschema_cmp.py <db1> <label1> <db2> <label2>
Prints the tables whose entry differs, the table order differences and DBNames (text) equality.
"""
import sys
import zlib

import bracefmt as bf
import lab


def entries(db, label):
    doc = bf.parse(lab.dbschema(label, db))
    tables = doc[1][1:]
    return {str(t[0]): t for t in tables}, [str(t[0]) for t in tables]


def names_text(db, label):
    return zlib.decompress(lab.row(label, 'Params', 'DBNames', db=db), -15)


def main():
    db1, l1, db2, l2 = sys.argv[1:5]
    e1, o1 = entries(db1, l1)
    e2, o2 = entries(db2, l2)
    print('tables: %d / %d' % (len(e1), len(e2)))
    only1 = [k for k in o1 if k not in e2]
    only2 = [k for k in o2 if k not in e1]
    print('only in 1:', only1, ' only in 2:', only2)
    differing = [k for k in o1 if k in e2 and bf.dumps(e1[k]) != bf.dumps(e2[k])]
    print('entries that differ (%d): %s' % (len(differing), differing))
    common1 = [k for k in o1 if k in e2]
    common2 = [k for k in o2 if k in e1]
    print('order of the common entries is the same:', common1 == common2)
    if common1 != common2:
        moved = [k for k, a, b in zip(o1, o1, o2) if k != b][:20]
        print('first positions that differ (1 vs 2):', [(o1[i], o2[i]) for i in range(min(len(o1), len(o2))) if o1[i] != o2[i]][:12])
    n1 = names_text(db1, l1)
    n2 = names_text(db2, l2)
    print('DBNames text equal:', n1 == n2, len(n1), len(n2))


main()
