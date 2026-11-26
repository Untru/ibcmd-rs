"""S1-J (#406): measurement kit of the size limit of the own restructuring.

Lab only: writes only to databases named ibcmd_rs_04_ui_* (the twins of the track "ui"); everything else is read.
Python 3.13 with pyodbc and "ODBC Driver 18 for SQL Server"; SQL Server with Windows authentication.

  python s1j.py stats     --db <db>                       rows and bytes of the three tables of the a2 catalog
  python s1j.py presize   --db <db> --data-gb 8 --log-gb 20
  python s1j.py recovery  --db <db> --model FULL|SIMPLE   (FULL takes a full backup to NUL to start the log chain)
  python s1j.py resetlog  --db <db>                       CHECKPOINT (SIMPLE) or BACKUP LOG to NUL (FULL)
  python s1j.py grow      --db <db> --shape narrow|wide|lob --rows N [--batch 500000]
  python s1j.py trial     --db <db> --exe <ibcmd-rs.exe> --tag <name> [--mode trial|rehearse] [--max-seconds 3600]
  python s1j.py poll      ...                             (internal: the sampler `trial` starts)

`trial` runs the tool with a sampler in its own process. The sampler is connected to `master`, not to the twin
(the tool refuses to run when another session is connected to the database), it stops when the stop file
appears and at the latest after --max-seconds, and `trial` checks that it is gone.
"""
import argparse
import contextlib
import csv
import json
import os
import subprocess
import sys
import time

import pyodbc

# a closed connection must really go: the tool refuses a database that another session holds
pyodbc.pooling = False

TABLES = ['_Reference20', '_Reference20_VT155', '_Reference20_VT159']
RUNS = os.path.join(os.environ.get('S1J_LAB', r'F:\ibcmd\lab\04\ui-codec\s1j'), 'runs')


def connect(db, autocommit=True):
    if not db.lower().startswith('ibcmd_rs_04_ui_') and db.lower() != 'master':
        raise SystemExit('lab databases only: ibcmd_rs_04_ui_* (got %s)' % db)
    return pyodbc.connect(
        'DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;Trusted_Connection=yes;'
        'TrustServerCertificate=yes' % db, autocommit=autocommit, timeout=30)


def read_connect(db):
    """A read connection for the stats of any database (SELECT only)."""
    return pyodbc.connect(
        'DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;Trusted_Connection=yes;'
        'TrustServerCertificate=yes' % db, autocommit=True, timeout=30)


SIZE_SQL = """
SELECT o.name,
  SUM(CASE WHEN ps.index_id IN (0,1) THEN ps.row_count ELSE 0 END),
  SUM(CASE WHEN ps.index_id IN (0,1) THEN ps.used_page_count ELSE 0 END) * 8192,
  SUM(CASE WHEN ps.index_id > 1 THEN ps.used_page_count ELSE 0 END) * 8192
FROM sys.dm_db_partition_stats ps JOIN sys.objects o ON o.object_id = ps.object_id
WHERE o.type = 'U' AND o.name IN (%s) GROUP BY o.name
"""


def table_stats(db, tables=TABLES):
    with contextlib.closing(read_connect(db)) as c:
        marks = ','.join("N'%s'" % t for t in tables)
        rows = c.cursor().execute(SIZE_SQL % marks).fetchall()
    out = {name: dict(rows=int(r), data_bytes=int(d), index_bytes=int(i)) for name, r, d, i in rows}
    for t in tables:
        out.setdefault(t, dict(rows=0, data_bytes=0, index_bytes=0))
    return out


def cmd_stats(a):
    stats = table_stats(a.db)
    print(json.dumps(stats, indent=1))
    print('total rows %d, data %.1f MB, index %.1f MB' % (
        sum(v['rows'] for v in stats.values()), sum(v['data_bytes'] for v in stats.values()) / 1048576,
        sum(v['index_bytes'] for v in stats.values()) / 1048576))


def files_of(c, db):
    return c.cursor().execute(
        "SELECT name, type_desc FROM sys.master_files WHERE database_id = DB_ID(?)", db).fetchall()


def guard(db):
    if not db.lower().startswith('ibcmd_rs_04_ui_'):
        raise SystemExit('lab databases only: ibcmd_rs_04_ui_* (got %s)' % db)


def cmd_presize(a):
    guard(a.db)
    with contextlib.closing(connect('master')) as c:
        for name, kind in files_of(c, a.db):
            gb = a.log_gb if kind == 'LOG' else a.data_gb
            c.cursor().execute(
                "ALTER DATABASE [%s] MODIFY FILE (NAME = N'%s', SIZE = %dMB, FILEGROWTH = 1024MB)" % (a.db, name, int(gb * 1024)))
            print('presized', name, kind, gb, 'GB')


def cmd_recovery(a):
    guard(a.db)
    with contextlib.closing(connect('master')) as c:
        cur = c.cursor()
        cur.execute("ALTER DATABASE [%s] SET RECOVERY %s" % (a.db, a.model))
        if a.model == 'FULL':
            # a regular (not COPY_ONLY) full backup starts the log chain: the FULL model is then in force
            cur.execute("BACKUP DATABASE [%s] TO DISK = N'NUL' WITH STATS = 100" % a.db)
            while cur.nextset():
                pass
    print('recovery', a.model)


def cmd_resetlog(a):
    guard(a.db)
    with contextlib.closing(connect('master')) as c:
        model = c.cursor().execute("SELECT recovery_model_desc FROM sys.databases WHERE name = ?", a.db).fetchone()[0]
    if model == 'FULL':
        with contextlib.closing(connect('master')) as c:
            cur = c.cursor()
            try:
                cur.execute("BACKUP LOG [%s] TO DISK = N'NUL'" % a.db)
                while cur.nextset():
                    pass
            except pyodbc.Error as error:
                print('log backup skipped:', str(error)[:100])
    with contextlib.closing(connect(a.db)) as c:
        c.cursor().execute('CHECKPOINT')
    print('log reset', model)


GROW = {
    # main table: a narrow row (about 300 bytes), six indexes
    'narrow': """
INSERT INTO dbo._Reference20 (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description,
                              _Fld151, _Fld152, _Fld153, _Fld154, _Fld949RRef, _Fld6357RRef, _Fld2683)
SELECT TOP (?) CONVERT(binary(16), NEWID()), 0x00, 0x00000000000000000000000000000000, 0x00000000000000000000000000000000, 0x01,
       RIGHT(N'000000000' + CONVERT(nvarchar(20), ? + ROW_NUMBER() OVER (ORDER BY (SELECT NULL))), 9),
       LEFT(CONVERT(nvarchar(36), NEWID()) + N' ' + REPLICATE(N'x', 60), 150),
       0x00, 0x00, 0x00, 0x00, NULL, NULL, (SELECT TOP 1 _Fld2683 FROM dbo._Reference20)
FROM sys.all_columns a CROSS JOIN sys.all_columns b
""",
    # a tabular section with a thin row (about 100 bytes, the clustered index only)
    'thin': """
INSERT INTO dbo._Reference20_VT155 (_Reference20_IDRRef, _Fld2683, _KeyField, _LineNo156, _Fld157RRef, _Fld158_TYPE, _Fld158_L,
       _Fld158_N, _Fld158_T, _Fld158_S, _Fld158_RTRef, _Fld158_RRRef, _Fld1473)
SELECT TOP (?) (SELECT TOP 1 _IDRRef FROM dbo._Reference20 ORDER BY _IDRRef), (SELECT TOP 1 _Fld2683 FROM dbo._Reference20),
       CONVERT(binary(4), ? + ROW_NUMBER() OVER (ORDER BY (SELECT NULL))), 1, 0x00000000000000000000000000000000, 0x00, 0x00,
       0, '2001-01-01', N'', 0x00000000, 0x00000000000000000000000000000000, N''
FROM sys.all_columns a CROSS JOIN sys.all_columns b
""",
    # a tabular section: a wide row (about 2 KB of strings)
    'wide': """
INSERT INTO dbo._Reference20_VT159 (_Reference20_IDRRef, _Fld2683, _KeyField, _LineNo160, _Fld161RRef, _Fld162RRef,
       _Fld163, _Fld164, _Fld165, _Fld166, _Fld167, _Fld168, _Fld169, _Fld170, _Fld171, _Fld5024RRef, _Fld6753)
SELECT TOP (?) (SELECT TOP 1 _IDRRef FROM dbo._Reference20 ORDER BY _IDRRef), (SELECT TOP 1 _Fld2683 FROM dbo._Reference20),
       CONVERT(binary(4), ? + ROW_NUMBER() OVER (ORDER BY (SELECT NULL))), 1, CONVERT(binary(16), NEWID()), CONVERT(binary(16), NEWID()),
       REPLICATE(N'a', 500), N'', REPLICATE(N'b', 100), REPLICATE(N'c', 50), REPLICATE(N'd', 50), REPLICATE(N'e', 100),
       REPLICATE(N'f', 100), REPLICATE(N'g', 20), REPLICATE(N'h', 20), CONVERT(binary(16), NEWID()), N''
FROM sys.all_columns a CROSS JOIN sys.all_columns b
""",
    # a tabular section with two nvarchar(max) values of 20 KB (LOB pages)
    'lob': """
INSERT INTO dbo._Reference20_VT159 (_Reference20_IDRRef, _Fld2683, _KeyField, _LineNo160, _Fld161RRef, _Fld162RRef,
       _Fld163, _Fld164, _Fld165, _Fld166, _Fld167, _Fld168, _Fld169, _Fld170, _Fld171, _Fld5024RRef, _Fld6753)
SELECT TOP (?) (SELECT TOP 1 _IDRRef FROM dbo._Reference20 ORDER BY _IDRRef), (SELECT TOP 1 _Fld2683 FROM dbo._Reference20),
       CONVERT(binary(4), ? + ROW_NUMBER() OVER (ORDER BY (SELECT NULL))), 1, CONVERT(binary(16), NEWID()), CONVERT(binary(16), NEWID()),
       REPLICATE(N'a', 100), REPLICATE(CONVERT(nvarchar(max), N'l'), 10000), N'b', N'c', N'd', N'e', N'f', N'g', N'h',
       CONVERT(binary(16), NEWID()), REPLICATE(CONVERT(nvarchar(max), N'm'), 10000)
FROM sys.all_columns a CROSS JOIN sys.all_columns b
""",
}
GROW_TABLE = {'narrow': '_Reference20', 'thin': '_Reference20_VT155', 'wide': '_Reference20_VT159', 'lob': '_Reference20_VT159'}


def cmd_grow(a):
    guard(a.db)
    sql = GROW[a.shape]
    table = GROW_TABLE[a.shape]
    start = table_stats(a.db)[table]['rows'] + 1000
    done = 0
    t0 = time.time()
    with contextlib.closing(connect(a.db)) as c:
        cur = c.cursor()
        while done < a.rows:
            n = min(a.batch, a.rows - done)
            cur.execute(sql, n, start + done)
            done += n
            if done % (a.batch * 4) == 0 or done == a.rows:
                cur.execute('CHECKPOINT')
                print('  %s +%d rows (%.0fs)' % (table, done, time.time() - t0), flush=True)
    print('grown', table, a.rows, 'rows in %.0fs' % (time.time() - t0))


def cmd_rung(a):
    """Grow one shape to a target size (in SIMPLE, batched), then one trial under each recovery model."""
    guard(a.db)
    table = GROW_TABLE[a.shape]
    have = table_stats(a.db)[table]['rows']
    if a.rows > have:
        cmd_recovery(argparse.Namespace(db=a.db, model='SIMPLE'))
        cmd_grow(argparse.Namespace(db=a.db, shape=a.shape, rows=a.rows - have, batch=a.batch))
    stats = table_stats(a.db)
    print('rung %s: %s' % (a.tag, {t: (v['rows'], v['data_bytes'] // 1048576, v['index_bytes'] // 1048576) for t, v in stats.items()}), flush=True)
    for model in a.models.split(','):
        cmd_recovery(argparse.Namespace(db=a.db, model=model))
        cmd_resetlog(argparse.Namespace(db=a.db))
        for repeat in range(a.repeat):
            cmd_trial(argparse.Namespace(db=a.db, exe=a.exe, tag='%s_%s%s' % (a.tag, model.lower(), '' if repeat == 0 else '_r%d' % (repeat + 1)),
                                         mode='trial', max_seconds=a.max_seconds, interval=0.25))
            cmd_resetlog(argparse.Namespace(db=a.db))


# ---- the sampler -------------------------------------------------------------------------------------------

POLL_FIELDS = ['t', 'tx_log_used', 'tx_log_reserved', 'log_size_mb', 'log_used_pct', 'log_written', 'data_written',
               'tempdb_mb', 'command', 'statement']


def cmd_poll(a):
    deadline = time.time() + a.max_seconds
    c = connect('master')
    cur = c.cursor()
    t0 = time.time()
    with open(a.out, 'w', newline='', encoding='utf-8') as fh:
        w = csv.writer(fh)
        w.writerow(POLL_FIELDS)
        while time.time() < deadline and not os.path.exists(a.stop_file):
            try:
                tx = cur.execute(
                    "SELECT ISNULL(MAX(database_transaction_log_bytes_used),0), ISNULL(MAX(database_transaction_log_bytes_reserved),0) "
                    "FROM sys.dm_tran_database_transactions WHERE database_id = DB_ID(?)", a.db).fetchone()
                io = cur.execute(
                    "SELECT SUM(CASE WHEN mf.type = 1 THEN vfs.num_of_bytes_written ELSE 0 END), "
                    "SUM(CASE WHEN mf.type = 0 THEN vfs.num_of_bytes_written ELSE 0 END), "
                    "SUM(CASE WHEN mf.type = 1 THEN mf.size ELSE 0 END) * 8 / 1024 "
                    "FROM sys.dm_io_virtual_file_stats(DB_ID(?), NULL) vfs JOIN sys.master_files mf "
                    "ON mf.database_id = vfs.database_id AND mf.file_id = vfs.file_id", a.db).fetchone()
                temp = cur.execute(
                    "SELECT SUM(allocated_extent_page_count) * 8 / 1024 FROM tempdb.sys.dm_db_file_space_usage").fetchone()
                pct = None
                for row in cur.execute('DBCC SQLPERF(LOGSPACE)').fetchall():
                    if row[0].lower() == a.db.lower():
                        pct = float(row[2])
                req = cur.execute(
                    "SELECT TOP 1 r.command, LEFT(REPLACE(REPLACE(t.text, CHAR(13), ' '), CHAR(10), ' '), 90) "
                    "FROM sys.dm_exec_requests r CROSS APPLY sys.dm_exec_sql_text(r.sql_handle) t "
                    "WHERE r.database_id = DB_ID(?) AND r.session_id <> @@SPID", a.db).fetchone()
                w.writerow([round(time.time() - t0, 3), tx[0], tx[1], io[2], pct, io[0], io[1], temp[0],
                            req[0] if req else '', req[1] if req else ''])
                fh.flush()
            except pyodbc.Error as error:
                w.writerow([round(time.time() - t0, 3), 'error', str(error)[:80]])
            time.sleep(a.interval)


# ---- the calibration on a real table (no staged change needed) -----------------------------------------------

def cmd_big(a):
    """The biggest tables of a database by rows and bytes (SELECT only)."""
    with contextlib.closing(read_connect(a.db)) as c:
        rows = c.cursor().execute(
            "SELECT TOP (?) o.name, SUM(CASE WHEN ps.index_id IN (0,1) THEN ps.row_count ELSE 0 END) AS r, "
            "SUM(CASE WHEN ps.index_id IN (0,1) THEN CONVERT(bigint, ps.used_page_count) ELSE 0 END)*8192 AS d, "
            "SUM(CASE WHEN ps.index_id > 1 THEN CONVERT(bigint, ps.used_page_count) ELSE 0 END)*8192 AS i, "
            "MAX(CASE WHEN ps.index_id IN (0,1) THEN ps.lob_used_page_count ELSE 0 END) * 8192 AS lob "
            "FROM sys.dm_db_partition_stats ps JOIN sys.objects o ON o.object_id = ps.object_id "
            "WHERE o.type = 'U' AND o.name LIKE ? GROUP BY o.name ORDER BY SUM(CASE WHEN ps.index_id IN (0,1) THEN CONVERT(bigint, ps.used_page_count) ELSE 0 END) "
            "+ SUM(CASE WHEN ps.index_id > 1 THEN CONVERT(bigint, ps.used_page_count) ELSE 0 END) DESC", a.top, a.like).fetchall()
    for name, r, d, i, lob in rows:
        print('%-28s rows %10d data %8.1f MB idx %8.1f MB lob %8.1f MB  2d+i %8.1f MB' % (name, r, d / 1e6, i / 1e6, lob / 1e6, (2 * d + i) / 1e6))


def index_ddl(cur, table, new):
    out = []
    for idx in cur.execute("SELECT index_id, name, is_unique, type FROM sys.indexes WHERE object_id = OBJECT_ID(?) AND index_id > 0 AND type IN (1, 2) ORDER BY index_id", 'dbo.' + table).fetchall():
        cols = cur.execute(
            "SELECT c.name, ic.is_descending_key, ic.is_included_column FROM sys.index_columns ic JOIN sys.columns c "
            "ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = OBJECT_ID(?) AND ic.index_id = ? "
            "ORDER BY ic.is_included_column, ic.key_ordinal", 'dbo.' + table, idx.index_id).fetchall()
        keys = ', '.join('[%s]%s' % (n, ' DESC' if d else '') for n, d, inc in cols if not inc)
        incl = ', '.join('[%s]' % n for n, d, inc in cols if inc)
        out.append('CREATE %s%s INDEX [%sNG] ON dbo.[%sNG] (%s)%s WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0)' % (
            'UNIQUE ' if idx.is_unique else '', 'CLUSTERED' if idx.type == 1 else 'NONCLUSTERED', idx.name, table, keys,
            ' INCLUDE (%s)' % incl if incl else ''))
    return out


def table_stats_inside(cur, table):
    r = cur.execute("SELECT SUM(CASE WHEN index_id IN (0,1) THEN row_count ELSE 0 END), SUM(CASE WHEN index_id IN (0,1) THEN CONVERT(bigint, used_page_count) ELSE 0 END) * 8192, "
                    "SUM(CASE WHEN index_id > 1 THEN CONVERT(bigint, used_page_count) ELSE 0 END) * 8192 FROM sys.dm_db_partition_stats WHERE object_id = OBJECT_ID(?)", 'dbo.' + table).fetchone()
    return dict(rows=int(r[0] or 0), data_bytes=int(r[1] or 0), index_bytes=int(r[2] or 0))


def cmd_probe(a):
    """One rolled-back rebuild of a real table, in the shape of the plan: an empty heap of the same columns,
    INSERT ... WITH(TABLOCK) SELECT, the indexes built after the load; the log of the transaction is read before
    the ROLLBACK. Needs no staged change."""
    guard(a.db)
    os.makedirs(RUNS, exist_ok=True)
    out = os.path.join(RUNS, a.tag)
    csv_path, stop = out + '.csv', out + '.stop'
    before = table_stats(a.db, [a.table])
    poller = subprocess.Popen([sys.executable, os.path.abspath(__file__), 'poll', '--db', a.db, '--out', csv_path, '--stop-file', stop,
                               '--max-seconds', str(a.max_seconds + 120), '--interval', '0.25'])
    time.sleep(1.5)
    result = dict(tag=a.tag, db=a.db, table=a.table, before=before[a.table])
    with contextlib.closing(connect(a.db, autocommit=False)) as c:
        cur = c.cursor()
        result['recovery'] = cur.execute("SELECT recovery_model_desc FROM sys.databases WHERE name = DB_NAME()").fetchone()[0]
        cols = [r[0] for r in cur.execute(
            "SELECT c.name FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(?) "
            "AND t.name <> 'timestamp' AND c.is_computed = 0 ORDER BY c.column_id", 'dbo.' + a.table).fetchall()]
        col_list = ', '.join('[%s]' % n for n in cols)
        ddl = index_ddl(cur, a.table, '')
        phases = {}

        def scalar(sql):
            return int(cur.execute(sql).fetchone()[0])
        used_sql = ("SELECT ISNULL(MAX(database_transaction_log_bytes_used), 0) FROM sys.dm_tran_database_transactions "
                    "WHERE transaction_id = CURRENT_TRANSACTION_ID() AND database_id = DB_ID()")
        reserved_sql = used_sql.replace('log_bytes_used', 'log_bytes_reserved')
        cur.execute("SET XACT_ABORT ON; SET LOCK_TIMEOUT 60000")
        t0 = time.time()
        cur.execute("SELECT TOP 0 %s INTO dbo.[%sNG] FROM dbo.[%s]" % (col_list, a.table, a.table))
        phases['create'] = round(time.time() - t0, 1)
        t1 = time.time()
        cur.execute("INSERT INTO dbo.[%sNG] WITH (TABLOCK) (%s) SELECT %s FROM dbo.[%s] WITH (NOLOCK)" % (a.table, col_list, col_list, a.table))
        phases['load'] = round(time.time() - t1, 1)
        result['log_after_load'] = scalar(used_sql)
        t2 = time.time()
        for statement in ddl:
            cur.execute(statement)
        phases['indexes'] = round(time.time() - t2, 1)
        result['log_after_indexes'] = scalar(used_sql)
        result['reserved'] = scalar(reserved_sql)
        result['built'] = table_stats_inside(cur, a.table + 'NG')
        t3 = time.time()
        c.rollback()
        phases['rollback'] = round(time.time() - t3, 1)
        result['phases'] = phases
        result['indexes_built'] = len(ddl)
    open(stop, 'w').write('stop')
    try:
        poller.wait(timeout=20)
    except subprocess.TimeoutExpired:
        poller.kill()
        poller.wait(timeout=10)
    result['poller_alive'] = poller.poll() is None
    os.remove(stop)
    d, i = result['before']['data_bytes'], result['before']['index_bytes']
    result['model_2d_plus_i'] = 2 * d + i
    result['measured_over_model'] = round(result['log_after_indexes'] / max(2 * d + i, 1), 3)
    json.dump(result, open(out + '.probe.json', 'w'), indent=1)
    print(json.dumps(result, indent=1))
    if result['poller_alive']:
        raise SystemExit('the poller survived')


def cmd_fingerprint(a):
    """What a refused or rehearsed run must not change: the service tables, the schema, the tables (SELECT only)."""
    queries = {
        'ConfigSave': "SELECT COUNT(*), ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0), ISNULL(CHECKSUM_AGG(BINARY_CHECKSUM(FileName, PartNo, DataSize)), 0) FROM dbo.ConfigSave",
        'Config': "SELECT COUNT(*), ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0), ISNULL(CHECKSUM_AGG(BINARY_CHECKSUM(FileName, PartNo, DataSize)), 0) FROM dbo.Config",
        'Params': "SELECT COUNT(*), ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0), ISNULL(CHECKSUM_AGG(BINARY_CHECKSUM(FileName, PartNo, DataSize)), 0) FROM dbo.Params",
        'Files': "SELECT COUNT(*), ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0), ISNULL(CHECKSUM_AGG(BINARY_CHECKSUM(FileName, PartNo, DataSize)), 0) FROM dbo.Files",
        'SchemaStorage': "SELECT COUNT(*), MAX(Status), CONVERT(varchar(64), HASHBYTES('SHA2_256', MAX(CurrentSchema)), 2) FROM dbo.SchemaStorage WHERE SchemaID = 0",
        'DBSchema': "SELECT COUNT(*), 0, CONVERT(varchar(64), HASHBYTES('SHA2_256', MAX(SerializedData)), 2) FROM dbo.DBSchema",
        'DBNames': "SELECT COUNT(*), 0, CONVERT(varchar(64), HASHBYTES('SHA2_256', MAX(BinaryData)), 2) FROM dbo.Params WHERE FileName = N'DBNames'",
        'tables': "SELECT COUNT(*), SUM(CASE WHEN name LIKE N'%NG' THEN 1 ELSE 0 END), CONVERT(varchar(30), MAX(modify_date), 126) FROM sys.tables",
        '_Reference20': "SELECT (SELECT SUM(row_count) FROM sys.dm_db_partition_stats WHERE object_id = OBJECT_ID(N'dbo._Reference20') AND index_id IN (0,1)), "
                        "(SELECT COUNT(*) FROM sys.columns WHERE object_id = OBJECT_ID(N'dbo._Reference20')), "
                        "CONVERT(varchar(30), (SELECT create_date FROM sys.tables WHERE name = N'_Reference20'), 126)",
    }
    out = {}
    with contextlib.closing(read_connect(a.db)) as c:
        cur = c.cursor()
        for name, sql in queries.items():
            out[name] = [str(v) for v in cur.execute(sql).fetchone()]
    text = json.dumps(out, indent=1, sort_keys=True)
    if a.out:
        open(a.out, 'w').write(text)
    print(text)


# ---- one run -----------------------------------------------------------------------------------------------

def cmd_trial(a):
    guard(a.db)
    os.makedirs(RUNS, exist_ok=True)
    out = os.path.join(RUNS, a.tag)
    csv_path, json_path, stop = out + '.csv', out + '.report.json', out + '.stop'
    if os.path.exists(stop):
        os.remove(stop)
    with contextlib.closing(connect('master')) as c:
        model = c.cursor().execute("SELECT recovery_model_desc FROM sys.databases WHERE name = ?", a.db).fetchone()[0]
    before = table_stats(a.db)
    poller = subprocess.Popen(
        [sys.executable, os.path.abspath(__file__), 'poll', '--db', a.db, '--out', csv_path, '--stop-file', stop,
         '--max-seconds', str(a.max_seconds + 120), '--interval', str(a.interval)])
    time.sleep(1.5)
    args = [a.exe, 'mssql-restructure', '--database', a.db, '--report', json_path]
    if a.mode == 'trial':
        # the tool's own pool holds a second session on the database, which its session check counts
        args += ['--trial', '--skip-session-check']
    elif a.mode == 'rehearse':
        args += ['--through-apply', '--rehearse', '--i-have-a-backup']
    elif a.mode == 'apply':
        # the standalone rebuild, committed
        args += ['--skip-session-check']
    else:
        raise SystemExit('mode: trial, rehearse or apply')
    t0 = time.time()
    try:
        proc = subprocess.run(args, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=a.max_seconds)
        rc, err = proc.returncode, proc.stderr[-500:]
    except subprocess.TimeoutExpired:
        rc, err = 'timeout', ''
    elapsed = time.time() - t0
    open(stop, 'w').write('stop')
    try:
        poller.wait(timeout=20)
    except subprocess.TimeoutExpired:
        poller.kill()
        poller.wait(timeout=10)
    alive = poller.poll() is None
    os.remove(stop)
    summary = summarize(csv_path, json_path)
    summary.update(tag=a.tag, db=a.db, recovery=model, mode=a.mode, exit=rc, elapsed_s=round(elapsed, 1),
                   stderr=err, before=before, poller_alive=alive)
    json.dump(summary, open(out + '.summary.json', 'w'), indent=1)
    print(json.dumps({k: summary[k] for k in summary if k not in ('before', 'phases', 'steps')}, indent=1))
    if alive:
        raise SystemExit('the poller survived')


def summarize(csv_path, json_path):
    rows = []
    with open(csv_path, encoding='utf-8') as fh:
        for r in csv.DictReader(fh):
            if r['tx_log_used'] == 'error' or r['tx_log_used'] == '':
                continue
            rows.append(r)
    def col(name, kind=float):
        return [kind(r[name]) for r in rows if r[name] not in ('', 'None')]
    out = dict(samples=len(rows))
    if rows:
        # the structure work ends where the promotion of the staged rows into Config begins; the rollback of a
        # trial writes compensation records, which the transaction's counter includes
        def first(pred):
            for i, r in enumerate(rows):
                if pred(r):
                    return i
            return len(rows)
        cut_config = first(lambda r: 'dbo.Config c' in r['statement'] or 'FROM dbo.Config c' in r['statement'])
        cut_rollback = first(lambda r: r['command'].startswith('ROLLBACK') or r['statement'].startswith('ROLLBACK'))
        used_all = [float(r['tx_log_used']) for r in rows]
        res_all = [float(r['tx_log_reserved']) for r in rows]
        def peak(values, upto):
            return int(max(values[:upto])) if upto > 0 else 0
        out.update(
            structure_log_used=peak(used_all, cut_config), structure_log_reserved=peak(res_all, cut_config),
            forward_log_used=peak(used_all, min(cut_rollback, len(rows))), forward_log_reserved=peak(res_all, min(cut_rollback, len(rows))),
            peak_all_log_used=int(max(used_all)), rollback_added=int(max(used_all) - peak(used_all, min(cut_rollback, len(rows)))),
            t_structure_end=float(rows[min(cut_config, len(rows) - 1)]['t']), t_rollback_start=float(rows[min(cut_rollback, len(rows) - 1)]['t']),
            t_end=float(rows[-1]['t']))
        used, reserved = col('tx_log_used'), col('tx_log_reserved')
        out.update(peak_tx_log_used=int(max(used)), peak_tx_log_reserved=int(max(reserved)),
                   peak_used_plus_reserved=int(max(u + v for u, v in zip(used, reserved))),
                   log_file_mb_start=float(rows[0]['log_size_mb']), log_file_mb_end=float(rows[-1]['log_size_mb']),
                   log_written_bytes=int(float(rows[-1]['log_written']) - float(rows[0]['log_written'])),
                   data_written_bytes=int(float(rows[-1]['data_written']) - float(rows[0]['data_written'])),
                   tempdb_peak_mb=int(max(col('tempdb_mb'))), tempdb_start_mb=int(float(rows[0]['tempdb_mb'])),
                   log_pct_peak=max(col('log_used_pct')) if col('log_used_pct') else None)
    try:
        report = json.load(open(json_path, encoding='utf-8'))
        execution = report.get('execution') or {}
        steps = execution.get('steps', [])
        phases = {}
        for s in steps:
            phases[s['phase']] = phases.get(s['phase'], 0) + s['milliseconds']
        out.update(phases=phases, tool_ms=execution.get('milliseconds'), tables=[t['table'] for o in report.get('objects', []) for t in o.get('tables', [])])
    except (OSError, ValueError):
        out.update(phases={}, tool_ms=None)
    return out


def main():
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest='cmd', required=True)
    for name in ('stats', 'presize', 'recovery', 'resetlog', 'grow', 'trial', 'poll', 'rung', 'big', 'probe', 'fingerprint'):
        s = sub.add_parser(name)
        s.add_argument('--db', required=True)
        if name == 'presize':
            s.add_argument('--data-gb', type=float, required=True)
            s.add_argument('--log-gb', type=float, required=True)
        if name == 'recovery':
            s.add_argument('--model', choices=['FULL', 'SIMPLE'], required=True)
        if name == 'grow':
            s.add_argument('--shape', choices=list(GROW), required=True)
            s.add_argument('--rows', type=int, required=True)
            s.add_argument('--batch', type=int, default=500000)
        if name == 'fingerprint':
            s.add_argument('--out', default='')
        if name == 'big':
            s.add_argument('--top', type=int, default=15)
            s.add_argument('--like', default='[_]%')
        if name == 'probe':
            s.add_argument('--table', required=True)
            s.add_argument('--tag', required=True)
            s.add_argument('--max-seconds', type=int, default=3600)
        if name == 'rung':
            s.add_argument('--shape', choices=list(GROW), default='narrow')
            s.add_argument('--rows', type=int, required=True)
            s.add_argument('--batch', type=int, default=500000)
            s.add_argument('--exe', required=True)
            s.add_argument('--tag', required=True)
            s.add_argument('--models', default='SIMPLE,FULL')
            s.add_argument('--repeat', type=int, default=1)
            s.add_argument('--max-seconds', type=int, default=3600)
        if name == 'trial':
            s.add_argument('--exe', required=True)
            s.add_argument('--tag', required=True)
            s.add_argument('--mode', default='trial')
            s.add_argument('--max-seconds', type=int, default=3600)
            s.add_argument('--interval', type=float, default=0.25)
        if name == 'poll':
            s.add_argument('--out', required=True)
            s.add_argument('--stop-file', required=True)
            s.add_argument('--max-seconds', type=int, required=True)
            s.add_argument('--interval', type=float, default=0.25)
    a = p.parse_args()
    {'stats': cmd_stats, 'presize': cmd_presize, 'recovery': cmd_recovery, 'resetlog': cmd_resetlog,
     'grow': cmd_grow, 'trial': cmd_trial, 'poll': cmd_poll, 'rung': cmd_rung, 'big': cmd_big, 'probe': cmd_probe, 'fingerprint': cmd_fingerprint}[a.cmd](a)


if __name__ == '__main__':
    main()
