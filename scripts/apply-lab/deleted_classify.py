#!/usr/bin/env python3
"""Classify the names of a stage's `deleted` row against the Config of the same database (read-only).

usage: deleted_classify.py <database>
For every listed name: whether Config has it, the object kind/name when it is a descriptor, whether ConfigSave has it.
"""
import collections
import re
import sys
import zlib

import pyodbc

sys.stdout.reconfigure(encoding='utf-8')
db = sys.argv[1]
assert db.startswith(('ibcmd_rs_04_', 'ibcmd_rs_05_')), db
cur = pyodbc.connect(
    'DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;Trusted_Connection=yes;TrustServerCertificate=yes' % db,
    autocommit=True, timeout=0).cursor()
cur.execute("SELECT BinaryData FROM dbo.ConfigSave WHERE FileName = N'deleted' AND PartNo = 0")
text = zlib.decompress(bytes(cur.fetchone()[0]), -15).decode('utf-8-sig')
head, _, rest = text.partition(',')
names = re.findall(r'"([^"]+)",(\d+)', rest)
print('count', head, 'parsed', len(names))
cur.execute("SELECT FileName, PartNo, DataSize FROM dbo.Config")
config = {}
for name, part, size in cur.fetchall():
    config.setdefault(name.lower(), []).append((part, size))
cur.execute("SELECT FileName FROM dbo.ConfigSave")
staged = {r[0].lower() for r in cur.fetchall()}
kinds = collections.Counter()
rows = []
for name, flag in names:
    low = name.lower()
    inconfig = low in config
    instage = low in staged
    label = ''
    if inconfig and re.fullmatch(r'[0-9a-f-]{36}', low):
        cur.execute('SELECT BinaryData FROM dbo.Config WHERE FileName = ? AND PartNo = 0', name)
        try:
            body = zlib.decompress(bytes(cur.fetchone()[0]), -15).decode('utf-8-sig')
            m = re.search(r'"([^"]{2,80})"', body)
            label = (body[:60].replace('\n', ' ') if not m else m.group(1))
        except Exception:
            label = '(not deflate)'
    kind = ('overlay' if '_dynupdate_' in low or low in ('dynamicallyupdated',) or low.startswith('versions_dynupdate') else
            ('body' if re.fullmatch(r'[0-9a-f-]{36}\.\d+', low) else ('descriptor' if re.fullmatch(r'[0-9a-f-]{36}', low) else 'other')))
    kinds[(kind, 'inConfig' if inconfig else 'NOT in Config', 'inStage' if instage else 'notStaged')] += 1
    rows.append((name, kind, inconfig, instage, label))
for k, v in sorted(kinds.items()):
    print(k, v)
print('--- first descriptors')
shown = 0
for name, kind, inconfig, instage, label in rows:
    if kind in ('descriptor', 'body') and shown < 40:
        print(name, kind, 'inConfig' if inconfig else 'absent', label[:70])
        shown += 1
