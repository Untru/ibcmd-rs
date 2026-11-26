"""Two stored Config rows of two lab databases, inflated and compared as brace text (issue #395).

  python row_diff.py <db-a> <db-b> <row name> [--table Config] [--table-b ConfigSave] [--limit 12]

Lists where the two texts differ: a token-level diff (the text cut at `{`, `}` and `,`), each difference with the
nesting path of the token in A. Rows are put together from their parts and inflated (raw deflate) when they inflate.
Read-only; lab databases only.
"""
import argparse
import difflib
import re
import sys
import zlib

import pyodbc

CONN = ("DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;"
        "Trusted_Connection=yes;TrustServerCertificate=yes")


def row(db, table, name):
    if not db.startswith("ibcmd_rs_04_"):
        raise SystemExit("lab databases only: " + db)
    con = pyodbc.connect(CONN % db, autocommit=True)
    cur = con.cursor()
    cur.execute("SELECT PartNo, BinaryData FROM %s WHERE FileName = ? ORDER BY PartNo" % table, name)
    parts = [bytes(data) for _, data in cur.fetchall()]
    con.close()
    if not parts:
        return None
    raw = b"".join(parts)
    try:
        raw = zlib.decompress(raw, -15)
    except zlib.error:
        pass
    return raw.decode("utf-8-sig", errors="replace")


TOKEN = re.compile(r'"(?:[^"]|"")*"|[{},]|[^{},"]+')


def tokens(text):
    out, path, depth, index = [], [], 0, [0]
    for token in TOKEN.findall(text):
        if token == "{":
            path.append(index[-1])
            index.append(0)
            depth += 1
        elif token == "}":
            if len(index) > 1:
                index.pop()
            if path:
                path.pop()
            index[-1] += 1
            depth -= 1
        elif token == ",":
            index[-1] += 1
        else:
            out.append((".".join(map(str, path + [index[-1]])), token))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("name")
    ap.add_argument("--table", default="Config")
    ap.add_argument("--table-b", default="")
    ap.add_argument("--limit", type=int, default=12)
    args = ap.parse_args()
    a, b = row(args.a, args.table, args.name), row(args.b, args.table_b or args.table, args.name)
    if a is None or b is None:
        print("missing in", args.a if a is None else args.b)
        return
    print("A %d chars, B %d chars, equal: %s" % (len(a), len(b), a == b))
    ta, tb = tokens(a), tokens(b)
    matcher = difflib.SequenceMatcher(None, [t[1] for t in ta], [t[1] for t in tb], autojunk=False)
    shown = 0
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            continue
        shown += 1
        if shown > args.limit:
            print("... more")
            break
        where = ta[i1][0] if i1 < len(ta) else "end"
        print("%s at %s: A %r  B %r" % (tag, where, " ".join(t[1] for t in ta[i1:i2])[:200],
                                        " ".join(t[1] for t in tb[j1:j2])[:200]))


if __name__ == "__main__":
    sys.exit(main())
