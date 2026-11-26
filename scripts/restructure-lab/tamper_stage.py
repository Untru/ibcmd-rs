"""Changes one staged row of a ddl-track clone, to make the refusals of the S1 gate testable on a real database
(12.6 check 11): the row is inflated, edited as text, deflated again and written back to ConfigSave.

usage:
  python tamper_stage.py <db> <FileName> replace <old> <new>   # text replace (once) in the inflated row
  python tamper_stage.py <db> <FileName> append <text>          # text appended to the inflated row
  python tamper_stage.py <db> <FileName> show                   # the inflated row on stdout

Only ibcmd_rs_04_ddl_* databases; only ConfigSave part 0. The row keeps its BOM and its line endings.
"""
import sys
import zlib

import db as dbh


def inflate(raw):
    return zlib.decompress(raw, -15)


def deflate(text):
    c = zlib.compressobj(9, zlib.DEFLATED, -15)
    return c.compress(text) + c.flush()


def main():
    database, name, mode = sys.argv[1:4]
    if not database.startswith("ibcmd_rs_04_ddl_"):
        raise SystemExit("lab databases only: ibcmd_rs_04_ddl_*")
    raw = dbh.blob(database, "SELECT BinaryData FROM dbo.ConfigSave WHERE FileName = ? AND PartNo = 0", name)
    if raw is None:
        raise SystemExit("ConfigSave has no row %s" % name)
    text = inflate(raw)
    if mode == "show":
        sys.stdout.buffer.write(text)
        return
    if mode == "replace":
        old, new = sys.argv[4].encode("utf-8"), sys.argv[5].encode("utf-8")
        if text.count(old) < 1:
            raise SystemExit("%r is not in the row" % sys.argv[4])
        text = text.replace(old, new, 1)
    elif mode == "append":
        text = text + sys.argv[4].encode("utf-8")
    else:
        raise SystemExit("unknown mode " + mode)
    new_raw = deflate(text)
    with dbh.connect(database) as c:
        cur = c.cursor()
        cur.execute("UPDATE dbo.ConfigSave SET BinaryData = ?, DataSize = ? WHERE FileName = ? AND PartNo = 0",
                    new_raw, len(new_raw), name)
        if cur.rowcount != 1:
            raise SystemExit("updated %d rows" % cur.rowcount)
    print("%s: %d -> %d bytes stored" % (name, len(raw), len(new_raw)))


if __name__ == "__main__":
    main()
