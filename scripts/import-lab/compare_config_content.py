"""Config rows of two lab databases compared (issue #388, #395: the check 4 of the S1 acceptance).

  python compare_config_content.py <db-a> <db-b> [--out <file>] [--list 40]

rcheck's compare_config.ps1 compares the stored bytes (and the times), which is right for twins that start from the very
same ConfigSave. Here each row is put together from its parts and put in one of four classes:
  identical   the stored bytes are the same
  same-text   the stored bytes differ (another deflate stream) and the inflated text is the same
  different   the inflated text differs (a parent configuration is deflated twice and compared as stored)
  only-in-A / only-in-B
The two twins of a case are not the same bytes for the rows the stage carries: the platform's deflate is not this
program's, and it writes a descriptor in its newest record format. Every row the stage leaves alone is `identical`.
Read-only; lab databases only.
"""
import argparse
import hashlib
import sys
import zlib

import pyodbc

CONN = ("DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;"
        "Trusted_Connection=yes;TrustServerCertificate=yes")


def rows_of(db):
    if not db.startswith("ibcmd_rs_04_"):
        raise SystemExit("lab databases only: " + db)
    con = pyodbc.connect(CONN % db, autocommit=True)
    cur = con.cursor()
    cur.execute("SELECT FileName, PartNo, BinaryData FROM Config ORDER BY FileName, PartNo")
    out = {}
    name, parts = None, []

    def flush():
        if name is None:
            return
        raw = b"".join(parts)
        try:
            plain = zlib.decompress(raw, -15)
        except zlib.error:
            plain = raw
        out[name] = (len(raw), hashlib.sha256(raw).hexdigest(), len(plain), hashlib.sha256(plain).hexdigest())

    while True:
        batch = cur.fetchmany(200)
        if not batch:
            break
        for file_name, _part, data in batch:
            if file_name != name:
                flush()
                name, parts = file_name, []
            parts.append(bytes(data))
    flush()
    con.close()
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--out", default="")
    ap.add_argument("--list", type=int, default=40)
    args = ap.parse_args()
    a, b = rows_of(args.a), rows_of(args.b)
    only_a = sorted(set(a) - set(b))
    only_b = sorted(set(b) - set(a))
    both = sorted(set(a) & set(b))
    identical = [n for n in both if a[n][:2] == b[n][:2]]
    same_text = [n for n in both if a[n][:2] != b[n][:2] and a[n][2:] == b[n][2:]]
    different = [n for n in both if a[n][2:] != b[n][2:]]
    lines = ["rows: %s %d, %s %d" % (args.a, len(a), args.b, len(b)),
             "identical %d, same-text %d, different %d, only-in-A %d, only-in-B %d"
             % (len(identical), len(same_text), len(different), len(only_a), len(only_b))]
    lines.append("== only in A (%d)" % len(only_a))
    lines += ["  " + n for n in only_a[:args.list]]
    lines.append("== only in B (%d)" % len(only_b))
    lines += ["  " + n for n in only_b[:args.list]]
    lines.append("== same text, other stored bytes (%d)" % len(same_text))
    lines += ["  %s  A %d B %d bytes" % (n, a[n][0], b[n][0]) for n in same_text[:args.list]]
    lines.append("== other text (%d)" % len(different))
    lines += ["  %s  A %d chars  B %d chars" % (n, a[n][2], b[n][2]) for n in different[:args.list]]
    text = "\n".join(lines)
    print(text)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(text + "\n")


if __name__ == "__main__":
    sys.exit(main())
