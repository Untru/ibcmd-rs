#!/usr/bin/env python3
"""Print the `deleted` row of the ConfigSave of a lab database (read-only; any ibcmd_rs_04_*/05_* database may be READ).

usage: deleted_row.py <database> [--names]
"""
import sys
import zlib

import pyodbc

sys.stdout.reconfigure(encoding='utf-8')
db = sys.argv[1]
assert db.startswith(('ibcmd_rs_04_', 'ibcmd_rs_05_')), db
cur = pyodbc.connect(
    'DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;Trusted_Connection=yes;TrustServerCertificate=yes;ApplicationIntent=ReadOnly' % db,
    autocommit=True, timeout=0).cursor()
cur.execute("SELECT PartNo, DataSize, BinaryData FROM dbo.ConfigSave WHERE FileName = N'deleted' ORDER BY PartNo")
rows = cur.fetchall()
print('rows', len(rows))
for part, size, data in rows:
    text = zlib.decompress(bytes(data), -15).decode('utf-8-sig')
    print('part', part, 'size', size, 'text length', len(text))
    print(text[:3000] if '--names' not in sys.argv else '\n'.join(text.replace('","', '"\n"').split('\n')))
cur.execute('SELECT COUNT(*) FROM dbo.ConfigSave')
print('ConfigSave rows:', cur.fetchone()[0])
