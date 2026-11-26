"""Snapshot of an infobase database for the ddl track (read-only).

usage: python snapshot.py <db> <label>
  writes F:\\ibcmd\\lab\\04\\restructure\\snap\\<db>\\<label>\\
    schema.txt      canonical text of every user table: columns, indexes, constraints
    tables.tsv      table, rows, checksum_agg(binary_checksum(*))   (data change detector)
    svc.json        DBSchema / SchemaStorage / Params / Config / ConfigSave / ConfigCAS(Save) /
                    Files / IBVersion inventories (name, part, size, sha256, dates)
  and adds blobs of the service tables to the content-addressed store
    F:\\ibcmd\\lab\\04\\restructure\\blobs\\<sha256>
"""
import hashlib
import json
import os
import sys

import db as dbm

from lab import ROOT
BLOBS = os.path.join(ROOT, "blobs")

SCHEMA_SQL = """
SELECT t.object_id, t.name AS tname, c.column_id, c.name AS cname, ty.name AS tyname,
       c.max_length, c.precision, c.scale, c.is_nullable, c.is_identity, c.is_computed,
       dc.name AS dcname, dc.definition AS dcdef, c.collation_name
FROM sys.tables t
JOIN sys.columns c ON c.object_id = t.object_id
JOIN sys.types ty ON ty.user_type_id = c.user_type_id
LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id
ORDER BY t.name, c.column_id
"""
INDEX_SQL = """
SELECT t.name AS tname, i.name AS iname, i.index_id, i.type_desc, i.is_unique, i.is_primary_key,
       i.is_unique_constraint, i.filter_definition, i.fill_factor, i.has_filter, i.ignore_dup_key,
       ic.key_ordinal, ic.is_descending_key, ic.is_included_column, c.name AS cname
FROM sys.tables t
JOIN sys.indexes i ON i.object_id = t.object_id AND i.index_id > 0
JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id
JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id
ORDER BY t.name, i.index_id, ic.is_included_column, ic.key_ordinal, ic.index_column_id
"""
STATS_SQL = """
SELECT t.name, SUM(p.rows) FROM sys.tables t
JOIN sys.partitions p ON p.object_id = t.object_id AND p.index_id IN (0, 1)
GROUP BY t.name ORDER BY t.name
"""


def coltype(r):
    n = r.tyname
    if n in ("nvarchar", "nchar"):
        ln = "max" if r.max_length == -1 else str(r.max_length // 2)
        return "%s(%s)" % (n, ln)
    if n in ("varchar", "char", "varbinary", "binary"):
        ln = "max" if r.max_length == -1 else str(r.max_length)
        return "%s(%s)" % (n, ln)
    if n in ("numeric", "decimal"):
        return "%s(%d,%d)" % (n, r.precision, r.scale)
    if n in ("datetime2", "datetimeoffset", "time"):
        return "%s(%d)" % (n, r.scale)
    return n


def put_blob(b):
    h = hashlib.sha256(b).hexdigest()
    p = os.path.join(BLOBS, h)
    if not os.path.exists(p):
        with open(p, "wb") as f:
            f.write(b)
    return h


def schema_text(D):
    out = []
    cols = dbm.rows(D, SCHEMA_SQL)
    idx = dbm.rows(D, INDEX_SQL)
    bytab = {}
    for r in cols:
        bytab.setdefault(r.tname, {"c": [], "i": {}})["c"].append(r)
    for r in idx:
        bytab.setdefault(r.tname, {"c": [], "i": {}})["i"].setdefault(
            (r.index_id, r.iname), []).append(r)
    for tname in sorted(bytab):
        out.append("T %s" % tname)
        for r in bytab[tname]["c"]:
            s = "  C %s %s %s" % (r.cname, coltype(r), "NULL" if r.is_nullable else "NOT NULL")
            if r.is_identity:
                s += " IDENTITY"
            if r.is_computed:
                s += " COMPUTED"
            if r.dcdef is not None:
                s += " DEFAULT %s" % r.dcdef
            if r.collation_name and r.tyname in ("nvarchar", "nchar", "varchar", "char"):
                s += " COLLATE %s" % r.collation_name
            out.append(s)
        for (iid, iname), lst in sorted(bytab[tname]["i"].items()):
            f = lst[0]
            keys = ",".join(("%s%s" % (x.cname, " DESC" if x.is_descending_key else ""))
                            for x in lst if not x.is_included_column)
            inc = ",".join(x.cname for x in lst if x.is_included_column)
            flags = []
            if f.is_primary_key:
                flags.append("PK")
            if f.is_unique:
                flags.append("UNIQUE")
            if f.is_unique_constraint:
                flags.append("UQCONS")
            if f.has_filter:
                flags.append("FILTER %s" % f.filter_definition)
            if f.fill_factor:
                flags.append("FF=%d" % f.fill_factor)
            out.append("  I %s %s [%s] (%s)%s" % (iname, f.type_desc, " ".join(flags), keys,
                                                   (" INCLUDE (%s)" % inc) if inc else ""))
    return "\n".join(out) + "\n"


def inventory(D, table, cols_extra=""):
    inv = []
    with dbm.connect(D) as c:
        cur = c.cursor()
        cur.execute("SELECT FileName, PartNo, Attributes, DataSize, Creation, Modified, BinaryData "
                    "FROM [%s] ORDER BY FileName, PartNo" % table)
        while True:
            batch = cur.fetchmany(200)
            if not batch:
                break
            for r in batch:
                b = bytes(r.BinaryData) if r.BinaryData is not None else b""
                inv.append({"name": r.FileName, "part": r.PartNo, "attr": r.Attributes,
                            "size": r.DataSize, "len": len(b),
                            "created": str(r.Creation), "modified": str(r.Modified),
                            "sha": put_blob(b)})
    return inv


def main():
    D, label = sys.argv[1], sys.argv[2]
    light = "--light" in sys.argv          # service tables only: no schema dump, no checksums
    out = os.path.join(ROOT, "snap", D, label)
    os.makedirs(out, exist_ok=True)
    os.makedirs(BLOBS, exist_ok=True)
    tables = []
    if not light:
        with open(os.path.join(out, "schema.txt"), "w", encoding="utf-8", newline="\n") as f:
            f.write(schema_text(D))
        # data change detector
        stats = dbm.rows(D, STATS_SQL)
        tables = [r[0] for r in stats]
        lines = []
        import concurrent.futures as cf
        import threading
        local = threading.local()

        def one(item):
            name, rows = item
            if not hasattr(local, "c"):
                local.c = dbm.connect(D)
            try:
                cur = local.c.cursor()
                cur.execute("SELECT CHECKSUM_AGG(BINARY_CHECKSUM(*)) FROM [%s]" % name)
                cs = cur.fetchone()[0]
            except Exception:  # e.g. types that BINARY_CHECKSUM cannot take
                cs = "ERR"
            return "%s\t%s\t%s" % (name, rows, cs)

        with cf.ThreadPoolExecutor(6) as ex:
            lines = list(ex.map(one, stats))
        with open(os.path.join(out, "tables.tsv"), "w", encoding="utf-8", newline="\n") as f:
            f.write("\n".join(lines) + "\n")
    svc = {}
    r = dbm.rows(D, "SELECT SerializedData FROM DBSchema")
    svc["DBSchema"] = [put_blob(bytes(x[0])) for x in r]
    r = dbm.rows(D, "SELECT SchemaID, Status, CurrentSchema, NewGenCreated, NewGenDropped FROM SchemaStorage ORDER BY SchemaID")
    svc["SchemaStorage"] = [{"id": x[0], "status": x[1], "cur": put_blob(bytes(x[2])),
                             "ngc": put_blob(bytes(x[3])), "ngd": put_blob(bytes(x[4]))} for x in r]
    r = dbm.rows(D, "SELECT IBVersion, PlatformVersionReq FROM IBVersion")
    svc["IBVersion"] = [list(x) for x in r]
    for t in ("Params", "Config", "ConfigSave", "ConfigCAS", "ConfigCASSave", "Files"):
        svc[t] = inventory(D, t)
    with open(os.path.join(out, "svc.json"), "w", encoding="utf-8") as f:
        json.dump(svc, f, ensure_ascii=False, indent=0)
    print("snapshot %s/%s: %d tables, Config=%d ConfigSave=%d Params=%d" % (
        D, label, len(tables), len(svc["Config"]), len(svc["ConfigSave"]), len(svc["Params"])))


if __name__ == "__main__":
    main()
