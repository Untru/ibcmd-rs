#!/usr/bin/env python3
"""Compare the change registers of two of MY databases: per (node, object) the message number and the ordered file list
(ids are left out: the native apply numbers the rows it inserts from a clock).

usage: reg_cmp.py <db_a> <db_b> [--show 8]
Read-only.
"""
import sys
import uuid

import pyodbc

sys.stdout.reconfigure(encoding='utf-8')
a, b = sys.argv[1], sys.argv[2]
show = int(sys.argv[sys.argv.index('--show') + 1]) if '--show' in sys.argv else 8
assert a.startswith('ibcmd_rs_04_apply_') and b.startswith('ibcmd_rs_04_apply_')


def load(db):
    cur = pyodbc.connect(
        'DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;Trusted_Connection=yes;TrustServerCertificate=yes' % db,
        autocommit=True, timeout=0).cursor()
    cur.execute('SELECT CONVERT(varchar(34), r._NodeRRef, 2), CONVERT(varchar(34), r._MDObjID, 2), CONVERT(varchar(40), r._MessageNo), '
                'CONVERT(varchar(34), r._IDRRef, 2) FROM dbo._ConfigChngR r')
    rows = {}
    ids = {}
    for node, obj, msg, ident in cur.fetchall():
        rows[(node, obj)] = [msg, []]
        ids[ident] = (node, obj)
    cur.execute('SELECT CONVERT(varchar(34), _ConfigChngR_IDRRef, 2), CONVERT(int, CONVERT(binary(4), _KeyField)), _FileName FROM dbo._ConfigChngR_ExtProps')
    for ident, key, name in cur.fetchall():
        rows[ids[ident]][1].append((key, name))
    for value in rows.values():
        value[1].sort()
    return rows


ra, rb = load(a), load(b)
print('rows', len(ra), len(rb))
only_a = sorted(set(ra) - set(rb))
only_b = sorted(set(rb) - set(ra))
print('only in A', len(only_a), 'only in B', len(only_b))
diff_msg = [k for k in set(ra) & set(rb) if ra[k][0] != rb[k][0]]
diff_files = [k for k in set(ra) & set(rb) if ra[k][1] != rb[k][1]]
print('message number differs', len(diff_msg), '| file list differs', len(diff_files))


def text(k):
    return '%s obj %s' % (k[0][:8], str(uuid.UUID(bytes_le=bytes.fromhex(k[1]))))


for k in diff_msg[:show]:
    print('  msg', text(k), ra[k][0], rb[k][0])
for k in diff_files[:show]:
    print('  files', text(k), ra[k][1], rb[k][1])
for k in only_a[:show]:
    print('  only A', text(k))
for k in only_b[:show]:
    print('  only B', text(k))
