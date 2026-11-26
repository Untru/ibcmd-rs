"""Offline tests of the kit's SQL text handling.  Run: python -m unittest discover -s scripts/apply-trace/tests"""

import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import apply_trace_common as common  # noqa: E402
import trace_report as tr  # noqa: E402

UPDATE_PARAMS = (
    "exec sp_executesql N'UPDATE Params SET Modified = @P1, DataSize = @P2, BinaryData = @P3 WHERE FileName = @P4 AND PartNo = @P5',"
    "N'@P1 datetime2(3),@P2 numeric(10),@P3 varbinary(max),@P4 nvarchar(4000),@P5 int',"
    "'4026-08-29 11:45:31.1341325312',9,0xEFBBBF7B307D,N'54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui',0"
)
INSERT_VALUES = (
    "exec sp_executesql N'INSERT INTO Config (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) "
    "VALUES (@P1, @P2, @P3, @P4, @P5, @P6, @P7)',N'@P1 nvarchar(4000),@P2 datetime2(3),@P3 datetime2(3),@P4 int,"
    "@P5 numeric(10),@P6 varbinary(8000),@P7 int',N'commit','4026-08-29 11:45:31.2','4026-08-29 11:45:31.2',0,0,0x,0"
)
COPY = (
    "exec sp_executesql N'INSERT Config SELECT @P1, Creation, Modified, Attributes, DataSize, BinaryData, PartNo FROM ConfigSave WHERE FileName = @P2',"
    "N'@P1 nvarchar(4000),@P2 nvarchar(4000)',N'root.new',N'root'"
)
DELETE_EXISTS = (
    "exec sp_executesql N'DELETE FROM Config WHERE FileName = @P1 AND EXISTS(SELECT 1 FROM ConfigSave WHERE FileName = @P2)',"
    "N'@P1 nvarchar(4000),@P2 nvarchar(4000)',N'versions.new',N'versions'"
)
RENAME = (
    "exec sp_executesql N'UPDATE config SET FileName = @P1 WHERE FileName = @P2',N'@P1 nvarchar(4000),@P2 nvarchar(4000)',"
    "N'versions_dynupdate_8c2ac6ff-7309-4025-93f6-264cbb068d62',N'versions.new'"
)
NAMED = (
    "exec sp_executesql N'INSERT INTO zz (FileName, PartNo) VALUES (@P1, @P5)',N'@P1 nvarchar(4000),@P5 int',@P1=N'it''s',@P5=0"
)


class ParseRpc(unittest.TestCase):
    def test_positional(self):
        proc, sql, params = tr.parse_rpc(UPDATE_PARAMS)
        self.assertEqual(proc, "sp_executesql")
        self.assertTrue(sql.startswith("UPDATE Params SET"))
        self.assertEqual([p.name for p in params], ["@P1", "@P2", "@P3", "@P4", "@P5"])
        self.assertEqual(params[2].kind, "hex")
        self.assertEqual(params[2].nbytes(), 6)
        self.assertEqual(params[3].value, "54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui")
        self.assertEqual(params[3].type, "nvarchar(4000)")

    def test_named_and_quotes(self):
        _, _, params = tr.parse_rpc(NAMED)
        self.assertEqual(params[0].value, "it's")
        self.assertEqual(params[1].value, "0")

    def test_types_with_commas(self):
        text = "exec sp_executesql N'SELECT @P1',N'@P1 numeric(38,8),@P2 int',1.5,2"
        _, _, params = tr.parse_rpc(text)
        self.assertEqual([p.type for p in params], ["numeric(38,8)", "int"])
        self.assertEqual(params[0].value, "1.5")

    def test_prepared_statement_is_read_like_sp_executesql(self):
        text = ("declare @p1 int\nset @p1=1\ndeclare @p4 int\nset @p4=0\n"
                "exec sp_prepexec @p1 output,N'@P1 int OUTPUT,@P2 nvarchar(1035),@P3 nvarchar(128)',"
                "N'exec @P1 = sp_rename @P2, @P3, ''object''',@p4 output,N'_ConfigChngRNG',N'_ConfigChngR'\nselect @p1, @p4")
        proc, sql, params = tr.parse_rpc(text)
        self.assertEqual(proc, "sp_executesql")
        self.assertEqual(sql, "exec @P1 = sp_rename @P2, @P3, 'object'")
        self.assertEqual([(p.name, p.value) for p in params[1:]], [("@P2", "_ConfigChngRNG"), ("@P3", "_ConfigChngR")])
        ops = tr.statement_facts(sql)
        self.assertEqual([v for v, _, _ in ops], ["RENAME"])
        self.assertEqual(tr.inline_params(ops[0][2], params), "exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object'")


class Normalize(unittest.TestCase):
    def test_literals(self):
        n = tr.normalize_sql("DELETE FROM Config WHERE FileName = 'commit' AND PartNo = 0 AND X IN (1, 2, 3)  SELECT TOP 1 FileName FROM Config")
        self.assertEqual(n, "DELETE FROM Config WHERE FileName = '?' AND PartNo = ? AND X IN (?...) SELECT TOP ? FileName FROM Config")

    def test_identifiers_with_digits_survive(self):
        self.assertIn("_Reference123", tr.normalize_sql("SELECT * FROM _Reference123 T1 WHERE T1._Fld456 = @P1"))
        self.assertIn("@P1", tr.normalize_sql("SELECT * FROM _Reference123 T1 WHERE T1._Fld456 = @P1"))


class Facts(unittest.TestCase):
    def test_verbs_and_tables(self):
        ops = tr.statement_facts("DELETE FROM ConfigSave WHERE FileName LIKE '%.new'\nSELECT TOP 1 FileName FROM ConfigSave WHERE FileName LIKE '%.new'")
        self.assertEqual([(v, t) for v, t, _ in ops], [("DELETE", "ConfigSave"), ("SELECT", "ConfigSave")])

    def test_begin_batch(self):
        ops = tr.statement_facts("SET TRANSACTION ISOLATION LEVEL READ COMMITTED BEGIN TRANSACTION")
        self.assertEqual([v for v, _, _ in ops], ["BEGIN"])

    def test_ddl(self):
        ops = tr.statement_facts("ALTER TABLE [dbo].[_Reference123] ADD _Fld999 nvarchar(25) NOT NULL DEFAULT N''")
        self.assertEqual([(v, t) for v, t, _ in ops], [("ALTER TABLE", "_Reference123")])
        self.assertEqual(tr.verb_of(ops), "ALTER TABLE")

    def test_temp_table_is_not_service(self):
        self.assertFalse(tr.is_service_table("#tt1"))
        self.assertTrue(tr.is_service_table("Config"))
        self.assertTrue(tr.is_service_table("_ConfigChngR"))
        self.assertFalse(tr.is_service_table("_Reference123"))

    def test_update_from_alias(self):
        ops = tr.statement_facts("UPDATE T2 SET _MessageNo = CAST(NULL AS NUMERIC(38,8)) FROM dbo._ConfigChngR T2 WHERE T2._IDRRef IN (SELECT T3.RS_FIELD FROM #tt2 T3)")
        self.assertEqual([(v, t) for v, t, _ in ops], [("UPDATE", "_ConfigChngR")])

    def test_update_with_set_on_the_next_line(self):
        ops = tr.statement_facts("UPDATE IBVersion\nSET IBVersion = 7, PlatformVersionReq = 80313 ")
        self.assertEqual([(v, t) for v, t, _ in ops], [("UPDATE", "IBVersion")])

    def test_table_hint_line_is_not_a_new_statement(self):
        ops = tr.statement_facts("SELECT TOP 1 ID FROM v8users\nWITH(NOLOCK)\nWHERE EAuth IS NULL")
        self.assertEqual([(v, t) for v, t, _ in ops], [("SELECT", "v8users")])


class Dml(unittest.TestCase):
    def _detail(self, text):
        _, sql, params = tr.parse_rpc(text)
        ops = tr.statement_facts(sql)
        op, table, stext = ops[0]
        return op, table, tr.dml_detail(op, table, stext, params)

    def test_update_with_payload(self):
        op, table, d = self._detail(UPDATE_PARAMS)
        self.assertEqual((op, table), ("UPDATE", "Params"))
        self.assertEqual(d["name"], "54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui")
        self.assertEqual(d["partno"], "0")
        self.assertEqual(d["size"], 9)
        self.assertEqual(d["payload"].nbytes(), 6)

    def test_insert_values(self):
        op, table, d = self._detail(INSERT_VALUES)
        self.assertEqual((op, table, d["name"], d["size"]), ("INSERT", "Config", "commit", 0))
        self.assertEqual(d["payload"].nbytes(), 0)

    def test_copy(self):
        op, table, d = self._detail(COPY)
        self.assertEqual((op, table, d["name"], d["src"]), ("INSERT", "Config", "root.new", "ConfigSave:root"))

    def test_delete_exists(self):
        op, table, d = self._detail(DELETE_EXISTS)
        self.assertEqual((op, table, d["name"], d["src"]), ("DELETE", "Config", "versions.new", "ConfigSave:versions"))

    def test_rename(self):
        op, table, d = self._detail(RENAME)
        self.assertEqual((op, table), ("UPDATE", "Config"))
        self.assertEqual(d["name"], "versions.new")
        self.assertTrue(d["name2"].startswith("versions_dynupdate_"))

    def test_delete_like_and_literal(self):
        ops = tr.statement_facts("DELETE FROM ConfigSave WHERE FileName LIKE '%.new'")
        d = tr.dml_detail(ops[0][0], ops[0][1], ops[0][2], [])
        self.assertEqual(d["name"], "LIKE %.new")
        ops = tr.statement_facts("DELETE FROM Config WHERE FileName = 'dynamicCommit'")
        d = tr.dml_detail(ops[0][0], ops[0][1], ops[0][2], [])
        self.assertEqual(d["name"], "dynamicCommit")

    def test_delete_with_an_exclusion_names_the_first_row(self):
        # the platform sends `... WHERE FileName = 'a' AND FileName <> 'b'`: the row is `a`
        sql = "DELETE FROM Params WHERE FileName = 'DBNames.New' AND FileName <> 'Params/DBNamesVersion-DBNames'"
        ops = tr.statement_facts(sql)
        d = tr.dml_detail(ops[0][0], ops[0][1], ops[0][2], [])
        self.assertEqual(d["name"], "DBNames.New")
        self.assertIn("Params/DBNamesVersion-DBNames", d["note"])


class Shapes(unittest.TestCase):
    def test_name_shape(self):
        g = "8c2ac6ff-7309-4025-93f6-264cbb068d62"
        self.assertEqual(common.name_shape(f"{g}_dynupdate_{g}.0"), "<guid>_dynupdate_<guid>.<n>")
        self.assertEqual(common.name_shape(f"{g}.0.new"), "<guid>.<n>.new")
        self.assertEqual(common.name_shape("root.new"), "root.new")
        self.assertEqual(common.name_shape("versions_dynupdate_" + g), "versions_dynupdate_<guid>")

    def test_tsv_roundtrip(self):
        s = "a\tb\r\nc\\d"
        self.assertEqual(common.tsv_unescape(common.tsv_escape(s)), s)

    def test_versions_codec(self):
        text = '{1,2,"",848a0a59-8803-445f-b89d-3cfae4f98bbd,"root",66193438-abc5-410b-a1f1-a204102d1a62,"version",30424fa4-6704-4ce4-807c-b62386988f97}'
        header, mapping = common.parse_versions(text)
        self.assertEqual(header[3], "848a0a59-8803-445f-b89d-3cfae4f98bbd")
        self.assertEqual(sorted(mapping), ["root", "version"])

    def test_inflate(self):
        import zlib
        c = zlib.compressobj(9, zlib.DEFLATED, -15)
        raw = c.compress(b"{0,2,abc}") + c.flush()
        d = common.decode_payload(raw)
        self.assertEqual((d["enc"], d["kind"], d["text"]), ("deflate", "v8text", "{0,2,abc}"))
        d = common.decode_payload(b"\xef\xbb\xbf{1,1,x}")
        self.assertEqual((d["enc"], d["kind"], d["text"]), ("raw", "v8text", "{1,1,x}"))


def event_line(**kw):
    cols = ["seq", "ts", "ev", "spid", "xid", "dur_us", "cpu_us", "reads", "writes", "rows", "result", "tstate", "ttype", "tid",
            "app", "dbname", "err_no", "err_sev", "err_msg", "stmt_chars", "stmt"]
    row = {c: "" for c in cols}
    row.update({k: str(v) for k, v in kw.items()})
    if not row["stmt_chars"] and row["stmt"]:
        row["stmt_chars"] = str(len(row["stmt"]))
    return "\t".join(common.tsv_escape(row[c]) for c in cols) + "\n"


class EndToEnd(unittest.TestCase):
    def test_small_trace(self):
        rows = [
            event_line(seq=1, ts="2026-09-29T06:00:00.0000000", ev="sql_transaction", spid=50, xid=0, tstate="Begin", ttype="User", tid=900),
            event_line(seq=2, ts="2026-09-29T06:00:00.0100000", ev="sql_batch_completed", spid=50, xid=900, dur_us=500, result="OK",
                       stmt="SET TRANSACTION ISOLATION LEVEL READ COMMITTED BEGIN TRANSACTION"),
            event_line(seq=3, ts="2026-09-29T06:00:00.0200000", ev="rpc_completed", spid=50, xid=900, dur_us=1200, rows=1, result="OK", stmt=UPDATE_PARAMS),
            event_line(seq=4, ts="2026-09-29T06:00:00.0300000", ev="rpc_completed", spid=50, xid=900, dur_us=800, rows=1, result="OK", stmt=COPY),
            event_line(seq=5, ts="2026-09-29T06:00:00.0310000", ev="sql_transaction", spid=50, xid=900, tstate="Commit", ttype="User", tid=900, dur_us=31000),
            event_line(seq=6, ts="2026-09-29T06:00:00.0320000", ev="sql_batch_completed", spid=50, xid=0, dur_us=100, result="OK", stmt="COMMIT TRANSACTION"),
            event_line(seq=7, ts="2026-09-29T06:00:01.0000000", ev="sql_batch_completed", spid=50, xid=0, dur_us=90000, result="OK",
                       stmt="ALTER TABLE dbo._Reference12 ADD _Fld99 nvarchar(25) NULL"),
            event_line(seq=8, ts="2026-09-29T06:00:01.5000000", ev="rpc_completed", spid=50, xid=0, dur_us=1000, rows=3, result="OK", stmt=RENAME),
        ]
        with tempfile.TemporaryDirectory() as d:
            ev = os.path.join(d, "events.tsv")
            with open(ev, "w", encoding="utf-8", newline="\n") as fh:
                fh.write("seq\tts\tev\tspid\txid\tdur_us\tcpu_us\treads\twrites\trows\tresult\ttstate\tttype\ttid\tapp\tdbname\terr_no\terr_sev\terr_msg\tstmt_chars\tstmt\n")
                fh.writelines(rows)
            out = os.path.join(d, "out")
            self.assertEqual(tr.main(["--events", ev, "--out", out]), 0)
            for name in ("summary.md", "groups.md", "timeline.md", "service-writes.tsv", "ddl.sql", "transactions.tsv"):
                self.assertTrue(os.path.isfile(os.path.join(out, name)), name)
            sw = list(common.read_tsv(os.path.join(out, "service-writes.tsv")))
            self.assertEqual([(r["op"], r["table"], r["name"]) for r in sw],
                             [("UPDATE", "Params", "54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui"), ("INSERT", "Config", "root.new"),
                              ("UPDATE", "Config", "versions.new")])
            self.assertEqual(sw[0]["tx"], "900")
            self.assertEqual(sw[2]["tx"], "0")
            tx = list(common.read_tsv(os.path.join(out, "transactions.tsv")))
            self.assertEqual(len(tx), 1)
            self.assertEqual(tx[0]["state"], "Commit")
            self.assertEqual(tx[0]["statements"], "4")   # begin batch, two writes, commit batch
            with open(os.path.join(out, "ddl.sql"), encoding="utf-8") as fh:
                ddl = fh.read()
            self.assertIn("ALTER TABLE dbo._Reference12 ADD _Fld99 nvarchar(25) NULL", ddl)


XE_BATCH = (
    '<event name="sql_batch_completed" package="sqlserver" timestamp="2026-09-29T07:19:57.519Z"><data name="cpu_time"><value>0</value></data>'
    '<data name="duration"><value>14741</value></data><data name="logical_reads"><value>179</value></data><data name="writes"><value>1</value></data>'
    '<data name="row_count"><value>0</value></data><data name="result"><value>0</value><text><![CDATA[OK]]></text></data>'
    '<data name="batch_text"><value><![CDATA[CREATE TABLE dbo.zz (a int)]]></value></data>'
    '<action name="database_id" package="sqlserver"><value>149</value></action>'
    '<action name="client_app_name" package="sqlserver"><value><![CDATA[xe_test]]></value></action>'
    '<action name="transaction_id" package="sqlserver"><value>0</value></action>'
    '<action name="session_id" package="sqlserver"><value>130</value></action>'
    '<action name="event_sequence" package="package0"><value>4</value></action></event>')
XE_TRAN = (
    '<event name="sql_transaction" package="sqlserver" timestamp="2026-09-29T07:19:57.519Z"><data name="duration"><value>14209</value></data>'
    '<data name="transaction_state"><value>1</value><text><![CDATA[Commit]]></text></data>'
    '<data name="transaction_type"><value>1</value><text><![CDATA[User]]></text></data>'
    '<data name="transaction_id"><value>3048973961</value></data>'
    '<action name="transaction_id" package="sqlserver"><value>3048973961</value></action>'
    '<action name="session_id" package="sqlserver"><value>130</value></action>'
    '<action name="event_sequence" package="package0"><value>3</value></action></event>')


class XmlConversion(unittest.TestCase):
    def test_convert(self):
        import gzip
        with tempfile.TemporaryDirectory() as d:
            xml = os.path.join(d, "events.xml.gz")
            with gzip.open(xml, "wt", encoding="utf-8", newline="") as fh:
                fh.write("2026-09-29T07:19:57.5190000\t" + XE_TRAN + "\x1e" + "2026-09-29T07:19:57.5200000\t" + XE_BATCH + "\x1e")
            out = os.path.join(d, "out")
            self.assertEqual(tr.main(["--xml", xml, "--out", out]), 0)
            rows = list(common.read_tsv(os.path.join(out, "events.tsv.gz")))
            self.assertEqual(len(rows), 2)
            tran, batch = rows
            self.assertEqual((tran["ev"], tran["tstate"], tran["ttype"], tran["tid"], tran["spid"], tran["seq"]), ("sql_transaction", "Commit", "User", "3048973961", "130", "3"))
            self.assertEqual((batch["ev"], batch["stmt"], batch["dur_us"], batch["app"], batch["result"]), ("sql_batch_completed", "CREATE TABLE dbo.zz (a int)", "14741", "xe_test", "OK"))
            self.assertEqual(batch["ts"], "2026-09-29T07:19:57.5200000")
            with open(os.path.join(out, "ddl.sql"), encoding="utf-8") as fh:
                self.assertIn("CREATE TABLE dbo.zz (a int)", fh.read())

    def test_focus(self):
        ev = [tr.Ev() for _ in range(3)]
        for i, e in enumerate(ev):
            e.spid, e.kind, e.dbname, e.stmt = (10 + i), "batch", ("other" if i else "target_db"), ""
        ev[2].dbname, ev[2].stmt = "master", "CREATE DATABASE [Target_DB]"
        kept, dropped = tr.focus_events(ev, "target_db")
        self.assertEqual([e.spid for e in kept], [10, 12])
        self.assertEqual(dropped, 1)


if __name__ == "__main__":
    unittest.main()
