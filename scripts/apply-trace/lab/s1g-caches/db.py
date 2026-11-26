"""Small pyodbc helpers for the ddl track lab (read-mostly).

Every function takes the database name; writes are only ever done by callers
against ibcmd_rs_04_ddl_* databases.
"""
import os
import sys
import pyodbc

CONN = ("DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;"
        "Trusted_Connection=yes;TrustServerCertificate=yes")


def connect(db, autocommit=True):
    return pyodbc.connect(CONN % db, autocommit=autocommit)


def rows(db, sql, *params):
    with connect(db) as c:
        cur = c.cursor()
        cur.execute(sql, *params)
        if cur.description is None:
            return []
        return cur.fetchall()


def blob(db, sql, *params):
    """First column of the first row as bytes (or None)."""
    r = rows(db, sql, *params)
    return bytes(r[0][0]) if r and r[0][0] is not None else None


def dump_blob(db, sql, path, *params):
    b = blob(db, sql, *params)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'wb') as f:
        f.write(b or b'')
    return len(b or b'')


if __name__ == '__main__':
    db = sys.argv[1]
    for r in rows(db, sys.argv[2]):
        print('|'.join('' if v is None else str(v) for v in r))
