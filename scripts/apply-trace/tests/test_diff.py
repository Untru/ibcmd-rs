"""Offline test of diff.py on two tiny synthetic snapshots."""

import gzip
import json
import os
import sys
import tempfile
import unittest
import zlib

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import apply_trace_common as common  # noqa: E402
import diff as D  # noqa: E402

FILE_HEADER = ["FileName", "parts", "size", "stored", "sha256", "attributes", "creation", "modified", "enc", "decoded_size",
               "decoded_sha256", "kind", "blob", "preview", "gaps"]


def deflate(data):
    c = zlib.compressobj(9, zlib.DEFLATED, -15)
    return c.compress(data) + c.flush()


class Store:
    """A minimal blob store in the pack/index layout."""

    def __init__(self, path):
        self.path = path
        os.makedirs(path, exist_ok=True)
        self.pack = os.path.join(path, "pack-test.bin")
        self.lines = []
        self.fh = open(self.pack, "wb")

    def add(self, data):
        sha = common.sha256_hex(data)
        off = self.fh.tell()
        raw = gzip.compress(data)
        self.fh.write(raw)
        self.lines.append(f"{sha}\tpack-test.bin\t{off}\t{len(raw)}\t{len(data)}\n")
        return sha

    def close(self):
        self.fh.close()
        with open(os.path.join(self.path, "index.tsv"), "w", encoding="utf-8", newline="\n") as fh:
            fh.writelines(self.lines)


def file_row(store, name, stored):
    sha = store.add(stored)
    dec = common.decode_payload(stored)
    return {"FileName": name, "parts": "1", "size": str(len(stored)), "stored": str(len(stored)), "sha256": sha, "attributes": "0",
            "creation": "4026-01-01T00:00:00.0000000", "modified": "4026-01-01T00:00:00.0000000", "enc": dec["enc"],
            "decoded_size": str(len(dec["data"])), "decoded_sha256": common.sha256_hex(dec["data"]), "kind": dec["kind"], "blob": "Y",
            "preview": (dec["text"] or "")[:60], "gaps": ""}


def write_snapshot(path, tables, cols, file_rows, settings=None):
    os.makedirs(os.path.join(path, "rows"), exist_ok=True)
    common.write_tsv(os.path.join(path, "tables.tsv"), ["schema", "table", "class", "rows", "used_kb", "checksum_agg", "checksum_sum", "note"],
                     [{"schema": "dbo", "table": t, "class": c, "rows": r, "used_kb": "8", "checksum_agg": ck, "checksum_sum": ck, "note": ""} for t, c, r, ck in tables])
    common.write_tsv(os.path.join(path, "columns.tsv"), ["schema", "table", "ord", "column", "type", "nullable", "identity", "computed", "computed_def", "default_def", "collation"],
                     [{"schema": "dbo", "table": t, "ord": str(i + 1), "column": n, "type": ty, "nullable": "0", "identity": "0", "computed": "0", "computed_def": "", "default_def": "", "collation": ""}
                      for t, cl in cols.items() for i, (n, ty) in enumerate(cl)])
    common.write_tsv(os.path.join(path, "indexes.tsv"), ["schema", "table", "index", "type", "unique", "primary_key", "unique_constraint", "disabled", "fill_factor", "filter", "keys", "included", "compression"], [])
    common.write_tsv(os.path.join(path, "constraints.tsv"), ["kind", "schema", "table", "name", "definition"], [])
    common.write_tsv(os.path.join(path, "objects.tsv"), ["kind", "schema", "name", "definition_sha256"], [])
    common.write_tsv(os.path.join(path, "dbsettings.tsv"), ["key", "value"], [{"key": k, "value": v} for k, v in (settings or {}).items()])
    for t, rows in file_rows.items():
        common.write_tsv(os.path.join(path, "rows", t + ".tsv"), FILE_HEADER, rows)
    with open(os.path.join(path, "meta.json"), "w", encoding="utf-8") as fh:
        json.dump({"database": "x", "taken_local": "t", "year_offset": 2000, "service_tables": list(file_rows), "blob_store": os.path.dirname(path)}, fh)


class DiffTest(unittest.TestCase):
    def test_diff(self):
        with tempfile.TemporaryDirectory() as d:
            store = Store(os.path.join(d, "blobs"))
            g = "8c2ac6ff-7309-4025-93f6-264cbb068d62"
            a_params = [file_row(store, "DynamicallyUpdated", b"\xef\xbb\xbf{0,2,aaa,bbb}")]
            b_params = [file_row(store, "DynamicallyUpdated", b"\xef\xbb\xbf{0,2,bbb,ccc}"), file_row(store, "new.ui", b"abc")]
            v_a = '{1,2,"",848a0a59-8803-445f-b89d-3cfae4f98bbd,"root",66193438-abc5-410b-a1f1-a204102d1a62,"x",30424fa4-6704-4ce4-807c-b62386988f97}'
            v_b = '{1,2,"",06cb0442-0c47-4fad-986a-f08f28287c1b,"root",66193438-abc5-410b-a1f1-a204102d1a62,"x",aaaaaaaa-6704-4ce4-807c-b62386988f97}'
            a_cfg = [file_row(store, "versions", deflate(v_a.encode())), file_row(store, f"m_dynupdate_{g}", deflate(b"{1}"))]
            b_cfg = [file_row(store, "versions", deflate(v_b.encode()))]
            a = os.path.join(d, "a")
            b = os.path.join(d, "b")
            write_snapshot(a, [("Config", "service", 2, 1), ("Params", "service", 1, 2), ("_Reference1", "data", 5, 7)],
                           {"_Reference1": [("_IDRRef", "binary(16)")], "Params": [("FileName", "nvarchar(256)")]},
                           {"Config": a_cfg, "Params": a_params}, {"option:x": "1"})
            write_snapshot(b, [("Config", "service", 1, 3), ("Params", "service", 2, 4), ("_Reference1", "data", 6, 9), ("_Reference2", "data", 0, 0)],
                           {"_Reference1": [("_IDRRef", "binary(16)"), ("_Fld9", "nvarchar(25)")], "_Reference2": [("_IDRRef", "binary(16)")], "Params": [("FileName", "nvarchar(256)")]},
                           {"Config": b_cfg, "Params": b_params}, {"option:x": "2"})
            store.close()
            out = os.path.join(d, "out")
            self.assertEqual(D.main(["--before", a, "--after", b, "--out", out, "--blobs", os.path.join(d, "blobs")]), 0)
            with open(os.path.join(out, "diff.md"), encoding="utf-8") as fh:
                text = fh.read()
            with open(os.path.join(out, "diff.json"), encoding="utf-8") as fh:
                js = json.load(fh)
            self.assertIn("CREATED `dbo._Reference2`", text)
            self.assertIn("column added: _Fld9 nvarchar(25) NOT NULL", text)
            self.assertIn("setting `option:x`: 1 -> 2", text)
            self.assertEqual(js["service"]["Params"]["inserted"], ["new.ui"])
            self.assertEqual(js["service"]["Params"]["updated"], ["DynamicallyUpdated"])
            self.assertEqual(js["service"]["Config"]["deleted"], [f"m_dynupdate_{g}"])
            self.assertEqual(js["service"]["Config"]["updated"], ["versions"])
            # the content of updated rows: DynamicallyUpdated as text, versions as entries
            self.assertIn("value: {0,2,aaa,bbb}  ->  {0,2,bbb,ccc}", text)
            self.assertIn("header 1 2 \"\" 848a0a59-8803-445f-b89d-3cfae4f98bbd -> 1 2 \"\" 06cb0442-0c47-4fad-986a-f08f28287c1b", text)
            self.assertIn("changed 1, added 0, removed 0", text)
            self.assertEqual([c["table"] for c in js["data_tables"]["changed"]], ["dbo.Config", "dbo.Params", "dbo._Reference1"])


if __name__ == "__main__":
    unittest.main()
