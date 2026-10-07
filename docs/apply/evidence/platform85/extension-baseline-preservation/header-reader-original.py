"""Read-only header proof for ONE exact owned COPY_ONLY preservation backup."""
import datetime, json, pathlib, sys

DB = 'ibcmd_rs_05_p85_w3_ext_version_native_20261001'
BACKUP = pathlib.Path('F:/ibcmd/lab/05/wave3/platform85/baselines/extension-settled-A-20261002.bak')
OUTPUT = pathlib.Path('F:/ibcmd/lab/05/wave3/platform85/extension-baseline-preservation-v5/backup-header.json')

def verify(rows):
    if len(rows) != 1:
        raise ValueError('exactly one backup set required')
    row = rows[0]
    for key, expected in [('DatabaseName', DB), ('BackupType', 1),
                          ('IsCopyOnly', True), ('HasBackupChecksums', True),
                          ('Position', 1), ('IsDamaged', False)]:
        if key not in row or row[key] != expected:
            raise ValueError('backup header differs: ' + key)
    return row

def self_test():
    good = dict(DatabaseName=DB, BackupType=1, IsCopyOnly=True,
                HasBackupChecksums=True, Position=1, IsDamaged=False)
    verify([good])
    for key in good:
        for broken in ({**good, key: None}, {k:v for k,v in good.items() if k != key}):
            try: verify([broken])
            except ValueError: pass
            else: raise AssertionError(key)
    for bad in ([], [good, good], [{**good, 'DatabaseName':'foreign'}],
                [{**good, 'IsCopyOnly':False}], [{**good, 'BackupType':5}]):
        try: verify(bad)
        except ValueError: pass
        else: raise AssertionError('invalid header accepted')
    print('PASS header positive+17 negative checks; no DB/process action')

if __name__ == '__main__':
    if sys.argv[1:] == ['--self-test']:
        self_test()
    else:
        import pyodbc
        backup, output = map(pathlib.Path, sys.argv[1:3])
        if backup != BACKUP or output != OUTPUT or not backup.is_file() or output.exists():
            raise ValueError('exact fresh owned backup/header output required')
        cn = pyodbc.connect('DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=master;Trusted_Connection=yes;TrustServerCertificate=yes', timeout=10, autocommit=True)
        cn.timeout = 20
        try:
            cur = cn.cursor()
            cur.execute("RESTORE HEADERONLY FROM DISK=N'" + str(backup).replace("'", "''") + "'")
            names = [column[0] for column in cur.description]
            rows = [dict(zip(names,row)) for row in cur.fetchall()]
            verify(rows)
            with output.open('x',encoding='utf-8') as f:
                json.dump(rows, f, indent=2, default=lambda value: value.isoformat() if isinstance(value,datetime.datetime) else str(value))
                f.write('\n')
        finally:
            cn.close()
