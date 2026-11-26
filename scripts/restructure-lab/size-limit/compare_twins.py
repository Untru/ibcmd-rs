"""Checks 3 and 4 of the twin protocol for two ibcmd_rs_04_ui_* twins (SELECT only): the rows of the rebuilt table and
of Config, EXCEPT both ways (every column but the row version; Config with Creation and Modified).

  python compare_twins.py <db A> <db B> [--tables _Reference20,_Reference20_VT155,_Reference20_VT159]
"""
import argparse
import contextlib

import pyodbc

pyodbc.pooling = False


def connect(db):
    return pyodbc.connect('DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;Trusted_Connection=yes;'
                          'TrustServerCertificate=yes' % db, autocommit=True, timeout=30)


def count(cur, sql):
    cur.execute(sql)
    return cur.fetchone()[0]


def main():
    p = argparse.ArgumentParser()
    p.add_argument('a')
    p.add_argument('b')
    p.add_argument('--tables', default='_Reference20,_Reference20_VT155,_Reference20_VT159')
    args = p.parse_args()
    with contextlib.closing(connect('master')) as c:
        cur = c.cursor()
        cur.execute('SET QUOTED_IDENTIFIER ON')
        cur.execute('SET ANSI_NULLS ON')
        cur.execute('SET ANSI_WARNINGS ON')
        cur.execute('SET ARITHABORT ON')
        # Config: every column, both ways
        cols = 'FileName, PartNo, Creation, Modified, Attributes, DataSize, BinaryData'
        rows = count(cur, 'SELECT COUNT(*) FROM [%s].dbo.Config' % args.a), count(cur, 'SELECT COUNT(*) FROM [%s].dbo.Config' % args.b)
        ab = count(cur, 'SELECT COUNT(*) FROM (SELECT %s FROM [%s].dbo.Config EXCEPT SELECT %s FROM [%s].dbo.Config) d' % (cols, args.a, cols, args.b))
        ba = count(cur, 'SELECT COUNT(*) FROM (SELECT %s FROM [%s].dbo.Config EXCEPT SELECT %s FROM [%s].dbo.Config) d' % (cols, args.b, cols, args.a))
        print('Config rows %d / %d, only in A %d, only in B %d' % (rows[0], rows[1], ab, ba))
        for table in [t for t in args.tables.split(',') if t]:
            names = [r[0] for r in cur.execute(
                "SELECT c.name FROM [%s].sys.columns c JOIN [%s].sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'[%s].dbo.%s') "
                "AND t.name <> 'timestamp' ORDER BY c.column_id" % (args.a, args.a, args.a, table)).fetchall()]
            col_list = ', '.join('[%s]' % n for n in names)
            n_a = count(cur, 'SELECT COUNT_BIG(*) FROM [%s].dbo.[%s]' % (args.a, table))
            n_b = count(cur, 'SELECT COUNT_BIG(*) FROM [%s].dbo.[%s]' % (args.b, table))
            ab = count(cur, 'SELECT COUNT_BIG(*) FROM (SELECT %s FROM [%s].dbo.[%s] EXCEPT SELECT %s FROM [%s].dbo.[%s]) d' % (col_list, args.a, table, col_list, args.b, table))
            ba = count(cur, 'SELECT COUNT_BIG(*) FROM (SELECT %s FROM [%s].dbo.[%s] EXCEPT SELECT %s FROM [%s].dbo.[%s]) d' % (col_list, args.b, table, col_list, args.a, table))
            print('%-22s rows %d / %d, %d columns, only in A %d, only in B %d' % (table, n_a, n_b, len(names), ab, ba))


if __name__ == '__main__':
    main()
