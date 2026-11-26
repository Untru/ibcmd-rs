"""What a stage left in ConfigSave (issue #388, step 2): the `deleted` row, the dates, the parts, the row count.

    python configsave_summary.py <database> [--config-names] > summary.json

Read-only. Lab databases only (ibcmd_rs_04_import_*).
"""
import argparse
import json
import re
import sys
import zlib

import pyodbc


def inflate(blob):
    return zlib.decompress(bytes(blob), -15)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("database")
    ap.add_argument("--names", action="store_true", help="list every staged file name")
    args = ap.parse_args()
    if not re.fullmatch(r"ibcmd_rs_04_import_[a-z0-9_]+", args.database):
        sys.exit("lab databases only: ibcmd_rs_04_import_*")
    cn = pyodbc.connect(
        "DRIVER={ODBC Driver 17 for SQL Server};SERVER=localhost;DATABASE=%s;"
        "Trusted_Connection=yes;TrustServerCertificate=yes" % args.database
    )
    cur = cn.cursor()
    out = {"database": args.database}
    out["rows"] = cur.execute("SELECT COUNT(*) FROM ConfigSave").fetchone()[0]
    out["names"] = cur.execute("SELECT COUNT(DISTINCT FileName) FROM ConfigSave").fetchone()[0]
    out["parts_over_one"] = [
        {"file": r[0], "parts": r[1], "data_size": r[2]}
        for r in cur.execute(
            "SELECT FileName, COUNT(*), MAX(DataSize) FROM ConfigSave GROUP BY FileName HAVING COUNT(*) > 1"
        ).fetchall()
    ]
    years = cur.execute(
        "SELECT YEAR(Creation), COUNT(*) FROM ConfigSave GROUP BY YEAR(Creation) ORDER BY 1"
    ).fetchall()
    out["creation_years"] = {str(y): n for y, n in years}
    row = cur.execute("SELECT BinaryData FROM ConfigSave WHERE FileName = 'deleted' AND PartNo = 0").fetchone()
    if row is None:
        out["deleted"] = None
    else:
        text = inflate(row[0]).decode("utf-8-sig")
        names = re.findall(r'"([^"]+)",(\d)', text)
        out["deleted"] = {"text_head": text[:80], "count": int(text.split(",", 1)[0]), "names": names}
    ver = cur.execute("SELECT BinaryData FROM ConfigSave WHERE FileName = 'versions' AND PartNo = 0").fetchone()
    if ver is not None:
        vt = inflate(ver[0]).decode("utf-8-sig")
        m = re.match(r"\{1,(\d+),", vt)
        out["versions_count"] = int(m.group(1)) if m else None
        out["versions_entries"] = len(re.findall(r'"[^"]+",[0-9a-f-]{36}', vt))
    if args.names:
        out["file_names"] = [r[0] for r in cur.execute("SELECT DISTINCT FileName FROM ConfigSave ORDER BY 1").fetchall()]
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
