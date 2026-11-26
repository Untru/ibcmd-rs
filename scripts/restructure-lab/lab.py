"""Helpers over the snapshot store (lab/04/restructure/snap + blobs).

Paths come from the environment so the scripts can be used on another lab folder:
  DDL_LAB   lab folder            (default F:\\ibcmd\\lab\\04\\restructure)
  DDL_DB    default database      (default ibcmd_rs_04_ddl_bsp8327_a)
  DDL_TREE  working XML tree      (default <lab>\\tree\\bsp8327)
  DDL_NATIVE_TREE  native export used for the reference metadata (ConfigDumpInfo.xml, XML objects)
"""
import json
import os
import zlib

ROOT = os.environ.get("DDL_LAB", r"F:\ibcmd\lab\04\restructure")
DB = os.environ.get("DDL_DB", "ibcmd_rs_04_ddl_bsp8327_a")
TREE = os.environ.get("DDL_TREE", os.path.join(ROOT, "tree", "bsp8327"))
NATIVE_TREE = os.environ.get(
    "DDL_NATIVE_TREE",
    r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native")


def svc(label, db=DB):
    with open(os.path.join(ROOT, "snap", db, label, "svc.json"), encoding="utf-8") as f:
        return json.load(f)


def inv(label, table, db=DB):
    return {(r["name"], r["part"]): r for r in svc(label, db)[table]}


def blob(sha):
    with open(os.path.join(ROOT, "blobs", sha), "rb") as f:
        return f.read()


def row(label, table, name, part=0, db=DB):
    r = inv(label, table, db).get((name, part))
    return blob(r["sha"]) if r else None


def inflate(b):
    """Config rows are raw deflate (v8 container payloads) - returns b itself when they are not."""
    try:
        return zlib.decompress(b, -15)
    except Exception:
        return b


def dbschema(label, db=DB):
    return blob(svc(label, db)["DBSchema"][0])


def schemastorage(label, db=DB):
    return svc(label, db)["SchemaStorage"]
