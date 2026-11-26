#!/usr/bin/env python3
"""The change-register rows of the removed form and template of #393 in one of MY databases (read or set up).

usage: reg_state.py <db> show
       reg_state.py <db> setup       (message numbers and missing rows for the removed objects, see SETUP)
"""
import sys
import uuid

import pyodbc

sys.stdout.reconfigure(encoding='utf-8')
db = sys.argv[1]
assert db.startswith('ibcmd_rs_04_apply_'), db
FORM = '8a7546f4-bfc9-4732-bf60-43a41e2c8753'
TEMPLATE = '0384ec55-dc18-40ae-ae83-7defc28d8f3f'
cur = pyodbc.connect(
    'DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;Trusted_Connection=yes;TrustServerCertificate=yes' % db,
    autocommit=True, timeout=0).cursor()

# (object, node prefix) -> what to do: an int sets _MessageNo, 'delete' takes the row (and its file list) away
SETUP = {
    (FORM, '0190A8667F21FDE411E728F60896C4FA'): 7,
    (FORM, '8792107B44A2802511E919964197A943'): 'delete',
    (FORM, 'B9E29B2BD879468F49D49084E3E554B1'): 0,
    (TEMPLATE, '0190A8667F21FDE411E728F60896C4FA'): 7,
    (TEMPLATE, '8792107B44A2802511E919964197A943'): 'delete',
}


def object_bytes(text):
    return uuid.UUID(text).bytes_le


def show():
    for name, text in (('form', FORM), ('template', TEMPLATE)):
        cur.execute(
            'SELECT CONVERT(varchar(34), r._NodeRRef, 2), r._MessageNo, CONVERT(varchar(34), r._IDRRef, 2), '
            '(SELECT COUNT(*) FROM dbo._ConfigChngR_ExtProps e WHERE e._ConfigChngR_IDRRef = r._IDRRef) '
            'FROM dbo._ConfigChngR r WHERE r._MDObjID = ? ORDER BY 1', object_bytes(text))
        rows = cur.fetchall()
        print(name, text[:8], [(n[:8], m, f) for n, m, _, f in rows])


def setup():
    for (text, node), action in SETUP.items():
        key = object_bytes(text)
        node_bytes = bytes.fromhex(node)
        if action == 'delete':
            cur.execute('DELETE e FROM dbo._ConfigChngR_ExtProps e JOIN dbo._ConfigChngR r ON r._IDRRef = e._ConfigChngR_IDRRef '
                        'WHERE r._MDObjID = ? AND r._NodeRRef = ?', key, node_bytes)
            cur.execute('DELETE FROM dbo._ConfigChngR WHERE _MDObjID = ? AND _NodeRRef = ?', key, node_bytes)
        else:
            cur.execute('UPDATE dbo._ConfigChngR SET _MessageNo = ? WHERE _MDObjID = ? AND _NodeRRef = ?', action, key, node_bytes)
    print('set up')


{'show': show, 'setup': setup}[sys.argv[2]]()
