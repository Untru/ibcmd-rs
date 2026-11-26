"""compare_twins.py <db1> <db2> [<table> ...]

EXCEPT in both directions over the given tables (all columns but the timestamp `_Version`), and over every table
of the extension schema (the X1 tables of SchemaStorage 1) plus the extensions' own tables.
"""
import re
import sys
import zlib

sys.path.insert(0, 'F:/ibcmd/src/ibcmd-rs-m-root/scripts/restructure-lab')
import db as dbmod  # noqa: E402

A, B = sys.argv[1], sys.argv[2]
extra = sys.argv[3:]


def columns(db, table):
    rows = dbmod.rows(db, "SELECT c.name, t.name FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id "
                          "WHERE c.object_id = OBJECT_ID(?) ORDER BY c.column_id", 'dbo.' + table)
    return [r[0] for r in rows if r[1].lower() != 'timestamp']


def count_except(left, right, table, cols):
    lst = ', '.join('[%s]' % c for c in cols)
    q = ("SELECT COUNT_BIG(*) FROM (SELECT %s FROM [%s].dbo.[%s] EXCEPT SELECT %s FROM [%s].dbo.[%s]) x"
         % (lst, left, table, lst, right, table))
    return dbmod.rows(left, q)[0][0]


schema = bytes(dbmod.rows(A, "SELECT CurrentSchema FROM SchemaStorage WHERE SchemaID = 1")[0][0]).decode('utf-8-sig')
x1 = sorted('_%sX1' % m.group(1) for m in re.finditer(r'\r\n\{"(\w+)","\w",\d+,"",', schema))
tables = list(extra) + x1 + ['_ExtensionsInfo', '_ExtensionsRestruct', '_ExtensionsRestructNGS']
bad = 0
checked = 0
for table in tables:
    cols = columns(A, table)
    if not cols:
        print('MISSING', table)
        bad += 1
        continue
    left = count_except(A, B, table, cols)
    right = count_except(B, A, table, cols)
    checked += 1
    if left or right:
        bad += 1
        print('DIFF %-40s %s-minus-%s=%d  %s-minus-%s=%d' % (table, 'A', 'B', left, 'B', 'A', right))
    elif table in extra:
        print('equal', table)
print('checked', checked, 'tables (X1 tables: %d), differing: %d' % (len(x1), bad))
