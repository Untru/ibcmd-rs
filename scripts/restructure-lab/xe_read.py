"""Read an Extended Events .xel file (via SQL Server) into events.jsonl + summaries.

usage: python xe_read.py <xel-glob> <outdir>
  events.jsonl   one JSON per event (long hex literals shortened)
  ddl.txt        every DDL-ish statement (CREATE/ALTER/DROP/TRUNCATE/sp_rename/EXEC of DDL)
  writes.txt     every DML statement that writes (INSERT/UPDATE/DELETE/MERGE/SELECT INTO) except bulk row noise, deduplicated by shape
  tx.txt         transaction boundaries (BEGIN/COMMIT/ROLLBACK/SAVE) with timestamps
  shapes.txt     statement shapes with counts (numbers/hex/strings normalised)
"""
import json
import os
import re
import sys
import xml.etree.ElementTree as ET

import db as dbm

HEX = re.compile(r"0x[0-9A-Fa-f]{64,}")
UUID_LIT = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")


def shorten(s):
    if s is None:
        return None
    s = HEX.sub(lambda m: "0x<%d hex>" % (len(m.group(0)) - 2), s)
    return s


def field(ev, name, kind="data"):
    for el in ev.findall(kind):
        if el.get("name") == name:
            v = el.find("value")
            if v is not None and v.text is not None:
                return v.text
            t = el.find("text")
            if t is not None and t.text is not None:
                return t.text
    return None


def main():
    pattern, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    fj = open(os.path.join(out, "events.jsonl"), "w", encoding="utf-8", newline="\n")
    n = 0
    with dbm.connect("master") as c:
        cur = c.cursor()
        cur.execute("SELECT object_name, timestamp_utc, event_data, file_offset "
                    "FROM sys.fn_xe_file_target_read_file(?, NULL, NULL, NULL) ORDER BY timestamp_utc, file_offset",
                    pattern)
        while True:
            batch = cur.fetchmany(2000)
            if not batch:
                break
            for name, ts, xml, off in batch:
                ev = ET.fromstring(xml)
                rec = {
                    "n": n, "t": ts.isoformat() if ts else None, "ev": name,
                    "sid": field(ev, "session_id", "action"),
                    "app": field(ev, "client_app_name", "action"),
                    "tx": field(ev, "transaction_id", "action"),
                }
                for k in ("duration", "row_count", "cpu_time", "logical_reads", "writes", "object_name",
                          "object_type", "ddl_phase", "object_id", "line_number", "offset", "offset_end"):
                    v = field(ev, k)
                    if v is not None:
                        rec[k] = v
                txt = field(ev, "statement") or field(ev, "batch_text") or field(ev, "sql_text", "action")
                sqlt = field(ev, "sql_text", "action")
                rec["text"] = shorten(txt)
                if sqlt and sqlt != txt:
                    rec["sql_text"] = shorten(sqlt)[:3000]
                fj.write(json.dumps(rec, ensure_ascii=False) + "\n")
                n += 1
    fj.close()
    print("events:", n)


if __name__ == "__main__":
    main()
