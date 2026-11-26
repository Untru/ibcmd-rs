#!/usr/bin/env python3
"""Small helpers for stages in MY disposable databases (ibcmd_rs_04_apply_*).

  stage_tools.py copy <src_db> <dst_db>            ConfigSave of src -> ConfigSave of dst (dst emptied first)
  stage_tools.py fingerprint <db>                  row count, bytes and a digest of ConfigSave
  stage_tools.py add-deleted <db> <text>           add a `deleted` row whose text is <text> (BOM added), raw-deflated,
                                                   Creation/Modified as the databases' own rows (copied from `versions`)
  stage_tools.py drop-deleted <db>                 remove the `deleted` row
  stage_tools.py overlay-names <db>                the dynamic-update names of Config, as a `deleted` list text
  stage_tools.py unstage <db> <row>                take the row out of ConfigSave and give its `versions` entry back the id Config has
  stage_tools.py stage-all <db>                    ConfigSave := every published Config row (no _dynupdate_ aliases, no marker), `versions` with a new generation:
                                                   the worst case for the size of one apply (the importer stages nearly all rows of a tree)
  stage_tools.py rebase-versions <db>              staged `versions`: names the dynamic overlay changed but the stage left alone
                                                   take the overlay's version id (what an importer that reads the effective row writes)
"""
import hashlib
import re
import sys
import zlib

import pyodbc


def connect(db):
    assert re.fullmatch(r'ibcmd_rs_04_apply_[a-z0-9_]+', db) or db.startswith('ibcmd_rs_04_'), db
    return pyodbc.connect(
        'DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;'
        'Trusted_Connection=yes;TrustServerCertificate=yes' % db,
        autocommit=True,
    )


def mine(db):
    assert re.fullmatch(r'ibcmd_rs_04_apply_[a-z0-9_]+', db), 'writes only to my databases: %s' % db


def fingerprint(db):
    c = connect(db).cursor()
    c.execute('SELECT COUNT(*), ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0) FROM dbo.ConfigSave')
    rows, size = c.fetchone()
    c.execute("SELECT CONVERT(varchar(64), HASHBYTES('SHA2_256', "
              "(SELECT CONVERT(varchar(max), FileName) + '|' + CONVERT(varchar(10), PartNo) + '|' + CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2) + ';' "
              "FROM dbo.ConfigSave ORDER BY FileName, PartNo FOR XML PATH(''), TYPE).value('.', 'varchar(max)')), 2)")
    return rows, size, c.fetchone()[0]


def main():
    cmd = sys.argv[1]
    if cmd == 'copy':
        src, dst = sys.argv[2], sys.argv[3]
        mine(dst)
        c = connect(dst).cursor()
        c.execute('DELETE FROM dbo.ConfigSave')
        c.execute('INSERT dbo.ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) '
                  'SELECT FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo FROM [%s].dbo.ConfigSave' % src)
        a, b = fingerprint(src), fingerprint(dst)
        print('src', a, '\ndst', b, '\nequal:', a == b)
        sys.exit(0 if a == b else 1)
    if cmd == 'fingerprint':
        print(fingerprint(sys.argv[2]))
    elif cmd == 'add-deleted':
        db, text = sys.argv[2], sys.argv[3]
        mine(db)
        raw = ('﻿' + text).encode('utf-8')
        co = zlib.compressobj(9, zlib.DEFLATED, -15)
        stored = co.compress(raw) + co.flush()
        c = connect(db).cursor()
        c.execute("DELETE FROM dbo.ConfigSave WHERE FileName = N'deleted'")
        c.execute("INSERT dbo.ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) "
                  "SELECT TOP (1) N'deleted', Creation, Modified, 0, ?, ?, 0 FROM dbo.ConfigSave WHERE FileName = N'versions'",
                  len(stored), pyodbc.Binary(stored))
        print('deleted row added:', len(stored), 'bytes stored for', repr(text[:80]))
    elif cmd == 'drop-deleted':
        mine(sys.argv[2])
        connect(sys.argv[2]).cursor().execute("DELETE FROM dbo.ConfigSave WHERE FileName = N'deleted'")
    elif cmd == 'overlay-names':
        c = connect(sys.argv[2]).cursor()
        c.execute("SELECT FileName FROM dbo.Config WHERE FileName = N'DynamicallyUpdated' OR FileName LIKE N'%\\_dynupdate\\_%' ESCAPE N'\\' ORDER BY FileName")
        names = [r[0] for r in c.fetchall()]
        print('%d,%s' % (len(names), ','.join('"%s",0' % n for n in names)))
    elif cmd == 'unstage':
        db, name = sys.argv[2], sys.argv[3]
        mine(db)
        c = connect(db).cursor()

        def stored_text(table, row):
            c.execute('SELECT BinaryData FROM dbo.%s WHERE FileName = ? AND PartNo = 0' % table, row)
            got = c.fetchone()
            return None if got is None else zlib.decompress(bytes(got[0]), -15).decode('utf-8')

        pair = re.compile(r'"' + re.escape(name) + r'",([0-9a-f]{8}-[0-9a-f-]{27})')
        active = pair.search(stored_text('Config', 'versions'))
        staged = stored_text('ConfigSave', 'versions')
        assert active and pair.search(staged), 'the row is not in versions'
        new_text = pair.sub('"%s",%s' % (name, active.group(1)), staged, count=1)
        co = zlib.compressobj(9, zlib.DEFLATED, -15)
        stored = co.compress(new_text.encode('utf-8')) + co.flush()
        c.execute("UPDATE dbo.ConfigSave SET DataSize = ?, BinaryData = ? WHERE FileName = N'versions' AND PartNo = 0", len(stored), pyodbc.Binary(stored))
        c.execute('DELETE FROM dbo.ConfigSave WHERE FileName = ?', name)
        print('unstaged', name, '; its versions entry is back to', active.group(1))
    elif cmd == 'stage-all':
        import uuid as _uuid
        db = sys.argv[2]
        mine(db)
        c = connect(db).cursor()
        c.execute('DELETE FROM dbo.ConfigSave')
        c.execute("INSERT dbo.ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) "
                  "SELECT FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo FROM dbo.Config "
                  "WHERE FileName NOT LIKE N'%!_dynupdate!_%' ESCAPE N'!' AND FileName <> N'DynamicallyUpdated'")
        c.execute("SELECT COUNT(*), SUM(CONVERT(bigint, DATALENGTH(BinaryData))) FROM dbo.ConfigSave")
        rows, size = c.fetchone()
        c.execute("SELECT BinaryData FROM dbo.ConfigSave WHERE FileName = N'versions' AND PartNo = 0")
        text = zlib.decompress(bytes(c.fetchone()[0]), -15).decode('utf-8')
        head = re.match(r'(﻿?\{1,\d+,"",)([0-9a-f]{8}-[0-9a-f-]{27})', text)
        assert head, text[:60]
        new_text = head.group(1) + str(_uuid.uuid4()) + text[head.end():]
        co = zlib.compressobj(9, zlib.DEFLATED, -15)
        stored = co.compress(new_text.encode('utf-8')) + co.flush()
        c.execute("UPDATE dbo.ConfigSave SET DataSize = ?, BinaryData = ? WHERE FileName = N'versions' AND PartNo = 0", len(stored), pyodbc.Binary(stored))
        print('staged %d rows, %d bytes; versions %d -> %d bytes stored, new generation' % (rows, size, len(text), len(stored)))
    elif cmd == 'rebase-versions':
        db = sys.argv[2]
        mine(db)
        c = connect(db).cursor()

        def text_of(table, name):
            c.execute('SELECT BinaryData FROM dbo.%s WHERE FileName = ? AND PartNo = 0' % table, name)
            row = c.fetchone()
            return None if row is None else zlib.decompress(bytes(row[0]), -15).decode('utf-8')

        c.execute("SELECT FileName FROM dbo.Config WHERE FileName LIKE N'versions!_dynupdate!_%' ESCAPE N'!'")
        overlays = [r[0] for r in c.fetchall()]
        assert len(overlays) == 1, overlays
        pair = re.compile(r'"([^"]*)",([0-9a-f]{8}-[0-9a-f-]{27})')
        plain = dict(pair.findall(text_of('Config', 'versions')))
        overlay = dict(pair.findall(text_of('Config', overlays[0])))
        staged_text = text_of('ConfigSave', 'versions')
        staged = dict(pair.findall(staged_text))
        changed = 0

        def swap(match):
            nonlocal changed
            name, guid = match.group(1), match.group(2)
            if name in overlay and overlay[name] != plain.get(name) and staged.get(name) == plain.get(name):
                changed += 1
                return '"%s",%s' % (name, overlay[name])
            return match.group(0)

        new_text = pair.sub(swap, staged_text)
        co = zlib.compressobj(9, zlib.DEFLATED, -15)
        stored = co.compress(new_text.encode('utf-8')) + co.flush()
        c.execute("UPDATE dbo.ConfigSave SET DataSize = ?, BinaryData = ? WHERE FileName = N'versions' AND PartNo = 0",
                  len(stored), pyodbc.Binary(stored))
        print('versions rebased on', overlays[0], ':', changed, 'entries changed')
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
