#!/usr/bin/env python3
"""Turn the events.tsv written by trace.ps1 into a bounded report.

Inputs : events.tsv (one line per XE event), trace-meta.json (optional)
Outputs (all in --out):
  summary.md          headline numbers, writes per table, DDL summary, errors
  groups.md/.tsv      normalized statement groups, in order of first appearance
  timeline.md         run-length-collapsed statement timeline with transactions
  service-writes.tsv  every write to a service table, in order (FileName, PartNo,
                      size, payload sha256, rows, transaction)
  payloads.jsonl      decoded content of small written file rows
  data-writes.tsv     writes to object (data) tables, aggregated per table/op
  ddl.sql             every non-temporary DDL statement, verbatim, in order
  transactions.tsv    user transactions: begin, end, state, duration, statements
  slowest.tsv         the slowest statements
  sessions.tsv        sessions and client applications

Only the Python standard library is used.
"""

import argparse
import collections
import json
import os
import re
import sys
import calendar
import gzip
import hashlib
import shutil
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from apply_trace_common import (  # noqa: E402
    FILE_TABLES, canonical_table, decode_payload, name_shape, one_line, read_tsv, sha256_hex,
    tsv_escape,
)

WRITE_OPS = ("INSERT", "INSERT BULK", "UPDATE", "DELETE", "MERGE", "SELECT INTO")
SUMMARY_SHAPE_ROWS = 120
MAX_TIMELINE_RUNS = 400
MAX_GROUPS_IN_MD = 200
MAX_SERVICE_WRITE_LINES = 200000
MAX_PAYLOADS = 400
MAX_PAYLOAD_TEXT = 64 * 1024
SQL_RPC_TRUNCATION = 1_999_000   # SQL Server cuts rpc_completed.statement at 2,000,000 chars
SERVICE_TABLES_EXTRA = {
    "DBSchema", "SchemaStorage", "IBVersion", "v8users", "_ConfigChngR", "_ConfigChngR_ExtProps",
    "_ExtensionsInfo", "_ExtensionsRestruct", "_ExtsChngR", "_ExtsChngR_ExtProps", "_SystemSettings",
    "_CommonSettings", "_InternalSettings", "_YearOffset", "_UsersWorkHistory", "BinaryData",
}


# ---------------------------------------------------------------------------
# events
# ---------------------------------------------------------------------------

class Ev:
    __slots__ = ("idx", "seq", "us", "ev", "kind", "spid", "xid", "dur", "cpu", "reads", "writes", "rows",
                 "result", "tstate", "ttype", "tid", "app", "dbname", "err_no", "err_sev", "err_msg",
                 "stmt_chars", "stmt", "start_us")


def to_int(v):
    if v is None or v == "":
        return None
    try:
        return int(v)
    except ValueError:
        return None


def parse_ts(s):
    """ISO 8601 UTC with up to 7 fractional digits -> microseconds since the epoch."""
    m = re.match(r"(\d{4})-(\d\d)-(\d\d)[T ](\d\d):(\d\d):(\d\d)(?:\.(\d+))?", s or "")
    if not m:
        return 0
    y, mo, d, h, mi, sec = (int(m.group(i)) for i in range(1, 7))
    frac = (m.group(7) or "0")[:6].ljust(6, "0")
    return calendar.timegm((y, mo, d, h, mi, sec)) * 1_000_000 + int(frac)


KIND_OF = {
    "rpc_completed": "rpc", "sql_batch_completed": "batch", "sql_statement_completed": "stmt",
    "sql_transaction": "utran", "begin_tran_completed": "tbegin", "commit_tran_completed": "tcommit",
    "rollback_tran_completed": "trollback", "error_reported": "error",
}


def load_events(path):
    events = []
    for idx, row in enumerate(read_tsv(path)):
        e = Ev()
        e.idx = idx
        e.seq = to_int(row.get("seq"))
        e.us = parse_ts(row.get("ts"))
        e.ev = row.get("ev", "")
        e.kind = KIND_OF.get(e.ev, e.ev)
        e.spid = to_int(row.get("spid"))
        e.xid = to_int(row.get("xid")) or 0
        e.dur = to_int(row.get("dur_us")) or 0
        e.cpu = to_int(row.get("cpu_us")) or 0
        e.reads = to_int(row.get("reads")) or 0
        e.writes = to_int(row.get("writes")) or 0
        e.rows = to_int(row.get("rows"))
        e.result = row.get("result", "")
        e.tstate = row.get("tstate", "")
        e.ttype = row.get("ttype", "")
        e.tid = to_int(row.get("tid")) or 0
        e.app = row.get("app", "")
        e.dbname = row.get("dbname", "")
        e.err_no = to_int(row.get("err_no"))
        e.err_sev = to_int(row.get("err_sev"))
        e.err_msg = row.get("err_msg", "")
        e.stmt_chars = to_int(row.get("stmt_chars")) or 0
        e.stmt = row.get("stmt", "")
        e.start_us = e.us - e.dur
        events.append(e)
    events.sort(key=lambda e: (e.seq if e.seq is not None else 1 << 62, e.idx))
    return events


EVENT_COLUMNS = ["seq", "ts", "ev", "spid", "xid", "dur_us", "cpu_us", "reads", "writes", "rows", "result", "tstate", "ttype", "tid",
                 "app", "dbname", "err_no", "err_sev", "err_msg", "stmt_chars", "stmt"]
_BAD_XML = re.compile("[\x00-\x08\x0b\x0c\x0e-\x1f]")
RECORD_SEPARATOR = "\x1e"


def _xe_records(path):
    """Records of events.xml.gz: `<timestamp_utc><TAB><event xml>` separated by U+001E."""
    with gzip.open(path, "rt", encoding="utf-8", newline="") as fh:
        buf = ""
        while True:
            chunk = fh.read(1 << 22)
            if not chunk:
                break
            buf += chunk
            parts = buf.split(RECORD_SEPARATOR)
            buf = parts.pop()
            for part in parts:
                yield part
        if buf.strip():
            yield buf


def _event_row(ts, xml_text):
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        root = ET.fromstring(_BAD_XML.sub("?", xml_text))
    data = {d.get("name"): d for d in root.findall("data")}
    act = {a.get("name"): a for a in root.findall("action")}

    def val(el):
        if el is None:
            return ""
        v = el.find("value")
        return (v.text or "") if v is not None else ""

    def txt(el):
        if el is None:
            return ""
        t = el.find("text")
        return (t.text or "") if t is not None else ""

    stmt = val(data.get("statement")) or val(data.get("batch_text")) or val(act.get("sql_text"))
    return {
        "seq": val(act.get("event_sequence")), "ts": ts, "ev": root.get("name", ""), "spid": val(act.get("session_id")),
        "xid": val(act.get("transaction_id")), "dur_us": val(data.get("duration")), "cpu_us": val(data.get("cpu_time")),
        "reads": val(data.get("logical_reads")), "writes": val(data.get("writes")), "rows": val(data.get("row_count")),
        "result": txt(data.get("result")), "tstate": txt(data.get("transaction_state")), "ttype": txt(data.get("transaction_type")),
        "tid": val(data.get("transaction_id")), "app": val(act.get("client_app_name")), "dbname": val(act.get("database_name")),
        "err_no": val(data.get("error_number")), "err_sev": val(data.get("severity")), "err_msg": val(data.get("message"))[:2000],
        "stmt_chars": str(len(stmt)), "stmt": stmt,
    }


def convert_xml_to_tsv(xml_gz, tsv_path, max_stmt_chars):
    """events.xml.gz (raw XE events written by trace.ps1) -> events.tsv; returns the number of events."""
    n = 0
    bad = 0
    with open(tsv_path, "w", encoding="utf-8", newline="\n") as out:
        out.write("\t".join(EVENT_COLUMNS) + "\n")
        for rec in _xe_records(xml_gz):
            ts, _, xml_text = rec.partition("\t")
            try:
                row = _event_row(ts, xml_text)
            except ET.ParseError:
                bad += 1
                continue
            if len(row["stmt"]) > max_stmt_chars:
                row["stmt"] = row["stmt"][:max_stmt_chars]
            out.write("\t".join(tsv_escape(row[c]) for c in EVENT_COLUMNS) + "\n")
            n += 1
    if bad:
        print(f"warning: {bad} events could not be parsed", file=sys.stderr)
    return n


def focus_events(events, database):
    """Keep only the sessions that touched `database` (their database context or their text).

    For traces taken without the database filter (trace.ps1 -AllDatabases): a session is
    kept as a whole when any of its statements ran in the database or names it.
    Returns (events, number of sessions dropped).
    """
    if not database:
        return events, 0
    name = database.lower()
    keep = set()
    seen = set()
    for e in events:
        if e.spid is None:
            continue
        seen.add(e.spid)
        if e.dbname.lower() == name or (e.kind in ("rpc", "batch", "stmt") and name in e.stmt.lower()):
            keep.add(e.spid)
    return [e for e in events if e.spid in keep], len(seen - keep)


# ---------------------------------------------------------------------------
# SQL text: literals, sp_executesql, normalization
# ---------------------------------------------------------------------------

_ARG_TOKEN = re.compile(
    r"(?P<str>N?'(?:[^']|'')*')|(?P<hex>0x[0-9A-Fa-f]*)|(?P<comma>,)|(?P<lp>\()|(?P<rp>\))|(?P<ws>\s+)|(?P<other>[^,'()\s]+)",
    re.S,
)


def split_args(s):
    """Split a comma-separated argument list at top level (quotes and parentheses aware)."""
    args = []
    cur = []
    depth = 0
    for m in _ARG_TOKEN.finditer(s):
        kind = m.lastgroup
        tok = m.group()
        if kind == "comma" and depth == 0:
            args.append("".join(cur).strip())
            cur = []
            continue
        if kind == "lp":
            depth += 1
        elif kind == "rp":
            depth = max(0, depth - 1)
        cur.append(tok)
    tail = "".join(cur).strip()
    if tail or args:
        args.append(tail)
    return args


class Param:
    __slots__ = ("name", "type", "kind", "value")

    def __init__(self, name, type_, kind, value):
        self.name = name
        self.type = type_
        self.kind = kind      # str | hex | num | null | raw
        self.value = value    # str, hex digits, number text, None

    def nbytes(self):
        return len(self.value) // 2 if self.kind == "hex" else None

    def text(self, limit=60):
        if self.kind == "str":
            v = self.value if len(self.value) <= limit else self.value[: limit - 3] + "..."
            return repr(v)
        if self.kind == "hex":
            h = self.value
            return "0x" + h[:16] + ("..." if len(h) > 16 else "") + f"({len(h) // 2}B)"
        if self.kind == "null":
            return "NULL"
        return str(self.value)


def lit_value(tok):
    t = tok.strip()
    if t[:1] in ("N", "n") and t[1:2] == "'" or t[:1] == "'":
        body = t[t.index("'") + 1: -1]
        return "str", body.replace("''", "'")
    if t[:2].lower() == "0x":
        return "hex", t[2:]
    if t.upper() == "NULL":
        return "null", None
    if re.match(r"^[-+]?\d+(\.\d+)?([eE][-+]?\d+)?$", t):
        return "num", t
    return "raw", t


_EXEC_RE = re.compile(r"^\s*exec(?:ute)?\s+(?P<proc>sp_\w+)\s*(?P<rest>.*)$", re.I | re.S)
# the driver's prepared statement: `declare @p1 int ... exec sp_prepexec @p1 output,N'<decl>',N'<sql>',<values>` + `select @p1, ...`
_PREPEXEC_RE = re.compile(r"\bexec(?:ute)?\s+sp_prepexec\s+(?P<rest>.*?)\s*(?:\r?\n\s*select\s+@p1\b[^\r\n]*)?\s*$", re.I | re.S)


def parse_rpc(text):
    """-> (proc, sql, params) for sp_executesql-like calls, or (proc, None, []) otherwise.

    `sp_prepexec` (prepare + execute) is reported as `sp_executesql`: same SQL, same parameters."""
    pm = _PREPEXEC_RE.search(text or "")
    if pm:
        pargs = split_args(pm.group("rest"))
        if len(pargs) >= 3:
            # (@handle OUTPUT, <decl>, <sql>, <values>) -> (<sql>, <decl>, <values>)
            return _sp_executesql_args("sp_executesql", [pargs[2], pargs[1]] + pargs[3:])
    m = _EXEC_RE.match(text or "")
    if not m:
        return None, None, []
    proc = m.group("proc").lower()
    if proc != "sp_executesql":
        return proc, None, []
    return _sp_executesql_args(proc, split_args(m.group("rest")))


def _sp_executesql_args(proc, args):
    if not args:
        return proc, None, []
    k0, v0 = lit_value(args[0])
    sql = v0 if k0 == "str" else None
    decl = []
    if len(args) > 1:
        k1, v1 = lit_value(args[1])
        if k1 == "str":
            for d in split_args(v1):
                dm = re.match(r"(@\w+)\s+(.*)$", d.strip(), re.S)
                if dm:
                    decl.append((dm.group(1), dm.group(2).strip()))
    values = args[2:]
    params = []
    positional = [v for v in values if not re.match(r"^@\w+\s*=", v)]
    by_name = len(values) > 0 and len(positional) == 0
    for i, v in enumerate(values):
        name = None
        if by_name or re.match(r"^@\w+\s*=", v):
            nm = re.match(r"^(@\w+)\s*=\s*(.*)$", v, re.S)
            if nm:
                name, v = nm.group(1), nm.group(2)
        if name is None and i < len(decl):
            name = decl[i][0]
        typ = next((t for n, t in decl if n == name), "")
        kind, val = lit_value(v)
        params.append(Param(name, typ, kind, val))
    return proc, sql, params


_STR_LIT = re.compile(r"N?'(?:[^']|'')*'")
_HEX_LIT = re.compile(r"0x[0-9A-Fa-f]+")
_NUM_LIT = re.compile(r"(?<![\w@#\[\]\.$])[-+]?\d+(?:\.\d+)?(?![\w\]])")
_IN_LIST = re.compile(r"\(\s*\?(?:\s*,\s*\?)+\s*\)")
_WS = re.compile(r"\s+")


def normalize_sql(sql):
    """Collapse whitespace and replace literals so that equal statements share a key."""
    s = _STR_LIT.sub("'?'", sql)
    s = _HEX_LIT.sub("0x?", s)
    s = _NUM_LIT.sub("?", s)
    s = _IN_LIST.sub("(?...)", s)
    return _WS.sub(" ", s).strip()


# --- statement structure ----------------------------------------------------

_TBL = r'((?:\[?[\w#]+\]?\.)*\[?[#\w]+\]?)'
_RE_INSERT = re.compile(r"\bINSERT\s+(?:INTO\s+)?" + _TBL, re.I)
_RE_UPDATE = re.compile(r"\bUPDATE\s+(?:TOP\s*\(\s*\d+\s*\)\s*)?" + _TBL + r"\s+SET\b", re.I)
_RE_DELETE = re.compile(r"\bDELETE\s+(?:TOP\s*\(\s*\d+\s*\)\s*)?(?:FROM\s+)?" + _TBL, re.I)
_RE_MERGE = re.compile(r"\bMERGE\s+(?:INTO\s+)?" + _TBL, re.I)
_RE_TRUNC = re.compile(r"\bTRUNCATE\s+TABLE\s+" + _TBL, re.I)
_RE_DDL = re.compile(
    r"\b(CREATE|ALTER|DROP)\s+(?:UNIQUE\s+)?(?:(?:NON)?CLUSTERED\s+)?(TABLE|INDEX|VIEW|PROCEDURE|PROC|FUNCTION|TRIGGER|SCHEMA|SEQUENCE|STATISTICS|TYPE|DATABASE|LOGIN|USER)\s+(?:IF\s+(?:NOT\s+)?EXISTS\s+)?" + _TBL,
    re.I)
_RE_SPRENAME = re.compile(r"\bsp_rename\b", re.I)
_RE_FROM = re.compile(r"\b(?:FROM|JOIN)\s+" + _TBL, re.I)
_RE_SELECT_INTO = re.compile(r"\bINTO\s+(#" + r"[\w#]+)\s+FROM\b", re.I)


def clean_table(raw):
    t = re.sub(r'[\[\]"]', "", raw)
    if t.startswith("#"):
        return t
    return canonical_table(t)


def is_temp(t):
    return t.startswith("#") or t.startswith("@")


def is_service_table(t):
    if is_temp(t):
        return False
    return t in FILE_TABLES or t in SERVICE_TABLES_EXTRA or not re.search(r"\d", t)


def split_statements(sql):
    """Split a batch into statements: at `;` and at lines that start a new statement (depth 0)."""
    stmts = []
    cur = []
    depth = 0
    starts = re.compile(r"^\s*(INSERT|UPDATE|DELETE|MERGE|TRUNCATE|SELECT|CREATE|ALTER|DROP|EXEC|EXECUTE|DECLARE|IF|BEGIN|COMMIT|ROLLBACK|SET|USE|WITH|DBCC)\b", re.I)
    for line in re.split(r"(?<=;)|\r?\n", sql):
        if line is None:
            continue
        starts_new = bool(starts.match(line)) and depth == 0 and cur
        if starts_new:
            first = starts.match(line).group(1).upper()
            joined = " ".join(cur)
            # INSERT ... <newline> SELECT and SET ... <newline> BEGIN are one statement
            if first == "SELECT" and re.match(r"^\s*INSERT\b", joined, re.I) and not re.search(r"\bSELECT\b|\bVALUES\b", joined, re.I):
                starts_new = False
            if first == "BEGIN" and re.match(r"^\s*SET\b", joined, re.I):
                starts_new = False
            # UPDATE t <newline> SET a = b: the SET belongs to the UPDATE
            if first == "SET" and re.match(r"^\s*UPDATE\b", joined, re.I) and not re.search(r"\bSET\b", joined, re.I):
                starts_new = False
            # a WITH that is a table hint (FROM t <newline> WITH (NOLOCK)) is not a CTE
            if first == "WITH" and not re.match(r"^\s*WITH\s+[\w\[\]]+\s*(\(|AS\b)", line, re.I):
                starts_new = False
        if starts_new:
            stmts.append(" ".join(cur))
            cur = []
        cur.append(line)
        # depth outside quotes (cheap: strip literals first)
        bare = _STR_LIT.sub("", line)
        depth += bare.count("(") - bare.count(")")
        if depth < 0:
            depth = 0
        if line.rstrip().endswith(";") and depth == 0:
            stmts.append(" ".join(cur))
            cur = []
    if cur:
        stmts.append(" ".join(cur))
    return [s.strip() for s in stmts if s.strip()]


def statement_facts(sql):
    """Facts about a statement text: ordered (verb, table) pairs, read tables, flags."""
    ops = []
    for s in split_statements(sql):
        m = _RE_DDL.search(s)
        if m and re.match(r"^\s*(CREATE|ALTER|DROP)\b", s, re.I):
            ops.append((f"{m.group(1).upper()} {m.group(2).upper()}", clean_table(m.group(3)), s))
            continue
        # IF OBJECT_ID(...) IS NULL BEGIN EXEC('CREATE FUNCTION ...') END and the like
        mo = re.search(r"\bEXEC(?:UTE)?\s*\(\s*N?'\s*(CREATE|ALTER)\s+(FUNCTION|PROCEDURE|PROC|VIEW|TRIGGER)\s+([\w\.\[\]]+)", s, re.I)
        if mo:
            ops.append((f"{mo.group(1).upper()} {mo.group(2).upper()}", clean_table(mo.group(3)), s))
            continue
        if re.match(r"^\s*TRUNCATE\b", s, re.I):
            m = _RE_TRUNC.search(s)
            if m:
                ops.append(("TRUNCATE", clean_table(m.group(1)), s))
                continue
        if re.match(r"^\s*(?:EXEC|EXECUTE)\s+(?:dbo\.)?sp_rename\b", s, re.I) or _RE_SPRENAME.search(s[:60]):
            ops.append(("RENAME", "", s))
            continue
        mb = re.match(r"^\s*insert\s+bulk\s+" + _TBL, s, re.I)
        if mb:
            ops.append(("INSERT BULK", clean_table(mb.group(1)), s))
            continue
        for verb, rx in (("INSERT", _RE_INSERT), ("UPDATE", _RE_UPDATE), ("DELETE", _RE_DELETE), ("MERGE", _RE_MERGE)):
            if re.match(r"^\s*(?:SET\s+[^;]*?\s+)?" + verb + r"\b", s, re.I) or re.match(r"^\s*" + verb + r"\b", s, re.I):
                m = rx.search(s)
                if m:
                    target = m.group(1)
                    # UPDATE T2 SET ... FROM dbo._ConfigChngR T2: the target is an alias
                    if verb in ("UPDATE", "DELETE") and "." not in target and not target.startswith("#"):
                        am = re.search(r"\b(?:FROM|JOIN)\s+" + _TBL + r"\s+(?:AS\s+)?" + re.escape(target) + r"\b", s, re.I)
                        if am:
                            target = am.group(1)
                    ops.append((verb, clean_table(target), s))
                    break
        else:
            if re.match(r"^\s*(?:SET\s+[^;]*?\s+)?BEGIN\s+TRAN", s, re.I):
                ops.append(("BEGIN", "", s))
            elif re.match(r"^\s*COMMIT\b", s, re.I):
                ops.append(("COMMIT", "", s))
            elif re.match(r"^\s*ROLLBACK\b", s, re.I):
                ops.append(("ROLLBACK", "", s))
            elif re.match(r"^\s*(?:WITH|SELECT)\b", s, re.I):
                si = _RE_SELECT_INTO.search(s)
                fm = _RE_FROM.search(s)
                ops.append(("SELECT INTO" if si else "SELECT", clean_table(si.group(1)) if si else (clean_table(fm.group(1)) if fm else ""), s))
            else:
                first = re.match(r"^\s*(\w+)", s)
                ops.append(((first.group(1).upper() if first else "?"), "", s))
    return ops


def verb_of(ops):
    """The most significant verb of a batch: DDL > write > select > control."""
    rank = {"DDL": 4, "W": 3, "S": 2, "C": 1}

    def r(v):
        if v.split()[0] in ("CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME"):
            return rank["DDL"]
        if v in WRITE_OPS:
            return rank["W"]
        if v == "SELECT":
            return rank["S"]
        return rank["C"]
    best = None
    for v, _t, _s in ops:
        if best is None or r(v) > r(best):
            best = v
    return best or "?"


# --- DML detail for file tables ---------------------------------------------

def _where_conds(where):
    out = []
    for part in re.split(r"\s+AND\s+(?![^()]*\))", where, flags=re.I):
        part = part.strip()
        m = re.match(r"^\(?\s*\[?(\w+)\]?\s*(=|<>|!=|LIKE|IN)\s*(.+?)\)?$", part, re.I | re.S)
        if m:
            out.append((m.group(1).lower(), m.group(2).upper(), m.group(3).strip()))
        elif re.match(r"^EXISTS\b", part, re.I):
            em = re.search(r"FROM\s+" + _TBL + r"\s+WHERE\s+\[?(\w+)\]?\s*=\s*(\S+?)\)", part, re.I)
            out.append(("exists", "EXISTS", (clean_table(em.group(1)), em.group(2).lower(), em.group(3)) if em else part))
    return out


def resolve(expr, params):
    """A @Pn / literal expression -> Param (or None)."""
    if isinstance(expr, tuple):
        return None
    e = expr.strip()
    if e.startswith("@"):
        for p in params:
            if p.name == e:
                return p
        return None
    k, v = lit_value(e)
    return Param(None, "", k, v)


def dml_detail(op, table, stmt, params):
    """Extract FileName / PartNo / payload of a write to a file table."""
    d = {"name": "", "name2": "", "partno": "", "size": None, "payload": None, "src": "", "note": ""}
    s = stmt.strip()
    try:
        if op == "INSERT":
            m = re.match(r"^\s*INSERT\s+(?:INTO\s+)?\S+\s*\((?P<cols>[^)]*)\)\s*VALUES\s*\((?P<vals>.*)\)\s*;?\s*$", s, re.I | re.S)
            if m:
                cols = [c.strip().strip("[]").lower() for c in split_args(m.group("cols"))]
                vals = split_args(m.group("vals"))
                row = dict(zip(cols, vals))
                p = resolve(row.get("filename", ""), params)
                d["name"] = p.value if p and p.kind == "str" else ""
                pn = resolve(row.get("partno", ""), params)
                d["partno"] = pn.value if pn else ""
                ps = resolve(row.get("datasize", ""), params)
                d["size"] = int(float(ps.value)) if ps and ps.kind == "num" else None
                pb = resolve(row.get("binarydata", ""), params)
                if pb and pb.kind == "hex":
                    d["payload"] = pb
                return d
            m = re.match(r"^\s*INSERT\s+(?:INTO\s+)?\S+\s*(?:\((?P<cols>[^)]*)\))?\s*(?:WITH\s*\([^)]*\)\s*)?SELECT\s+(?P<sel>.*?)\s+FROM\s+(?P<src>\S+)(?P<rest>.*)$", s, re.I | re.S)
            if m:
                sel = split_args(m.group("sel"))
                cols = [c.strip().strip("[]").lower() for c in split_args(m.group("cols"))] if m.group("cols") else ["filename", "creation", "modified", "attributes", "datasize", "binarydata", "partno"]
                row = dict(zip(cols, sel))
                first = resolve(row.get("filename", ""), params)
                d["name"] = first.value if first and first.kind == "str" else (row.get("filename") or "")
                srcname = ""
                wm = re.search(r"WHERE\s+(.*)$", m.group("rest"), re.I | re.S)
                if wm:
                    for c, o, v in _where_conds(wm.group(1)):
                        if c == "filename":
                            pv = resolve(v, params)
                            srcname = pv.value if pv and pv.kind == "str" else v
                d["src"] = f"{clean_table(m.group('src'))}:{srcname}"
                d["note"] = "copy"
                return d
        elif op == "UPDATE":
            m = re.match(r"^\s*UPDATE\s+\S+\s+SET\s+(?P<sets>.*?)\s+WHERE\s+(?P<where>.*)$", s, re.I | re.S)
            if m:
                sets = {}
                for a in split_args(m.group("sets")):
                    am = re.match(r"^\[?(\w+)\]?\s*=\s*(.*)$", a, re.S)
                    if am:
                        sets[am.group(1).lower()] = am.group(2).strip()
                conds = _where_conds(m.group("where"))
                for c, o, v in conds:
                    if c == "filename" and o in ("<>", "!="):
                        continue   # an exclusion (`AND FileName <> 'x'`), not the row that is written
                    if c == "filename":
                        pv = resolve(v, params)
                        d["name"] = pv.value if pv and pv.kind == "str" else v
                    elif c == "partno":
                        pv = resolve(v, params)
                        d["partno"] = (pv.value if pv else v)
                if "filename" in sets:
                    pv = resolve(sets["filename"], params)
                    d["name2"] = pv.value if pv and pv.kind == "str" else sets["filename"]
                    d["note"] = "rename (name -> name2)"
                ps = resolve(sets.get("datasize", ""), params) if "datasize" in sets else None
                if ps and ps.kind == "num":
                    d["size"] = int(float(ps.value))
                pb = resolve(sets.get("binarydata", ""), params) if "binarydata" in sets else None
                if pb and pb.kind == "hex":
                    d["payload"] = pb
                return d
        elif op == "DELETE":
            m = re.match(r"^\s*DELETE\s+(?:FROM\s+)?\S+(?:\s+WHERE\s+(?P<where>.*))?$", s, re.I | re.S)
            if m and m.group("where"):
                for c, o, v in _where_conds(m.group("where")):
                    if c == "filename" and o in ("<>", "!="):
                        # `DELETE ... WHERE FileName = 'a' AND FileName <> 'b'`: the row is `a`; `b` only excludes
                        pv = resolve(v, params)
                        d["note"] = "excluding " + (pv.value if pv and pv.kind == "str" else v)
                        continue
                    if c == "filename":
                        pv = resolve(v, params)
                        val = pv.value if pv and pv.kind == "str" else v
                        d["name"] = ("LIKE " + val) if o == "LIKE" else val
                    elif c == "partno":
                        pv = resolve(v, params)
                        d["partno"] = f"{o}{pv.value if pv else v}"
                    elif c == "exists" and isinstance(v, tuple):
                        pv = resolve(v[2], params)
                        d["src"] = f"{v[0]}:{pv.value if pv and pv.kind == 'str' else v[2]}"
                        d["note"] = "delete if the other row exists"
                return d
    except Exception as exc:  # a parsing miss must never stop the report
        d["note"] = f"parse error: {exc}"
    return d


# ---------------------------------------------------------------------------
# analysis
# ---------------------------------------------------------------------------

class Stmt:
    """One statement-completing event with everything derived from its text."""
    __slots__ = ("ev", "proc", "sql", "params", "norm", "ops", "verb", "group", "tx", "cut", "trunc")


class Group:
    def __init__(self, gid, kind, norm):
        self.id = gid
        self.kind = kind
        self.norm = norm
        self.count = 0
        self.dur = 0
        self.rows = 0
        self.first_seq = None
        self.first_us = None
        self.last_us = None
        self.spids = set()
        self.verb = ""
        self.tables = []
        self.samples = []
        self.shapes = collections.defaultdict(collections.Counter)
        self.errors = 0


def inline_params(text, params):
    """Put the values of @Pn parameters into a (short) statement: `exec sp_rename @P2, @P3` -> `exec sp_rename 'a', 'b'`."""
    def sub(m):
        for p in params:
            if p.name == m.group(0) and p.kind in ("str", "num"):
                return "'" + p.value.replace("'", "''") + "'" if p.kind == "str" else str(p.value)
        return m.group(0)
    out = re.sub(r"@P\d+\b", sub, text)
    return re.sub(r"^exec\s+@P1\s*=\s*", "exec ", out, flags=re.I)


def analyze(events):
    stmts = []
    groups = collections.OrderedDict()
    for e in events:
        if e.kind not in ("rpc", "batch"):
            continue
        s = Stmt()
        s.ev = e
        s.proc, s.sql, s.params = (None, None, [])
        # the platform sends `exec sp_executesql N'...',N'...',<values>` both as RPC and as batch text
        if e.kind == "rpc" or _EXEC_RE.match(e.stmt or "") or _PREPEXEC_RE.search(e.stmt or ""):
            s.proc, s.sql, s.params = parse_rpc(e.stmt)
        text = s.sql if s.sql is not None else e.stmt
        s.cut = e.stmt_chars > len(e.stmt)
        s.trunc = e.kind == "rpc" and e.stmt_chars >= SQL_RPC_TRUNCATION
        s.sql = text
        s.ops = statement_facts(text) if text else []
        if s.params and any(op == "RENAME" for op, _t, _s in s.ops):
            s.ops = [(op, t, inline_params(st, s.params) if op == "RENAME" else st) for op, t, st in s.ops]
        s.verb = verb_of(s.ops)
        if s.proc and s.proc != "sp_executesql":
            s.norm = f"exec {s.proc} ..."
            s.verb = s.proc.upper()
        else:
            s.norm = normalize_sql(text)
        s.tx = None
        key = (e.kind, s.norm)
        g = groups.get(key)
        if g is None:
            g = Group(len(groups) + 1, e.kind, s.norm)
            g.first_seq = e.seq
            g.first_us = e.start_us
            g.verb = s.verb
            tabs = []
            for v, t, _ in s.ops:
                if t and t not in tabs:
                    tabs.append(t)
            g.tables = tabs
            groups[key] = g
        g.count += 1
        g.dur += e.dur
        g.rows += e.rows or 0
        g.last_us = e.us
        g.spids.add(e.spid)
        if e.result and e.result != "OK":
            g.errors += 1
        for p in s.params:
            if p.kind == "str":
                sh = name_shape(p.value)
                if len(g.shapes[p.name]) < 40 or sh in g.shapes[p.name]:
                    g.shapes[p.name][sh] += 1
        if len(g.samples) < 3:
            g.samples.append(", ".join(f"{p.name}={p.text()}" for p in s.params) if s.params else "")
        s.group = g
        stmts.append(s)
    return stmts, list(groups.values())


class Tx:
    def __init__(self, spid, tid, begin_seq, begin_us):
        self.spid = spid
        self.tid = tid
        self.begin_seq = begin_seq
        self.begin_us = begin_us
        self.end_seq = None
        self.end_us = None
        self.state = "open"
        self.dur = None
        self.stmts = []
        self.savepoints = 0


def build_transactions(events, stmts):
    txs = collections.OrderedDict()
    for e in events:
        if e.kind == "utran" and e.ttype == "User":
            key = (e.spid, e.tid)
            if e.tstate == "Begin":
                txs[key] = Tx(e.spid, e.tid, e.seq, e.us)
            elif e.tstate in ("Commit", "Rollback"):
                t = txs.get(key)
                if t is not None:
                    t.end_seq, t.end_us, t.state, t.dur = e.seq, e.us, e.tstate, e.dur
            elif e.tstate == "Savepoint":
                t = txs.get(key)
                if t is not None:
                    t.savepoints += 1
    by_spid = collections.defaultdict(list)
    for s in stmts:
        by_spid[s.ev.spid].append(s)
    for s in stmts:
        t = txs.get((s.ev.spid, s.ev.xid)) if s.ev.xid else None
        if t is not None:
            s.tx = t
            t.stmts.append(s)
    # the batch that carries BEGIN TRANSACTION / COMMIT / ROLLBACK completes after the sql_transaction event
    for t in txs.values():
        lst = by_spid.get(t.spid, [])
        for s in lst:
            if s.tx is None and s.ev.seq is not None and t.begin_seq is not None and s.ev.seq > t.begin_seq \
                    and any(v == "BEGIN" for v, _, _ in s.ops):
                s.tx = t
                t.stmts.insert(0, s)
                break
        if t.end_seq is not None:
            for s in lst:
                if s.tx is None and s.ev.seq is not None and s.ev.seq > t.end_seq \
                        and any(v in ("COMMIT", "ROLLBACK") for v, _, _ in s.ops):
                    s.tx = t
                    t.stmts.append(s)
                    break
    return list(txs.values())


def collect_writes(stmts, t0_us):
    """Every write statement as a record; file-table writes carry FileName/PartNo/payload."""
    recs = []
    last_delete = {}   # spid -> (table, name) of the last "DELETE ... PartNo <> 0": the row the next UPDATE writes
    for s in stmts:
        e = s.ev
        for op, table, stext in s.ops:
            if op not in WRITE_OPS and op != "TRUNCATE":
                continue
            r = {"seq": e.seq, "t": (e.start_us - t0_us) / 1e6, "spid": e.spid, "xid": (s.tx.tid if s.tx else 0),
                 "op": op, "table": table, "rows": e.rows if len(s.ops) == 1 else None, "dur": e.dur,
                 "group": s.group.id, "name": "", "name2": "", "partno": "", "size": None, "payload": None,
                 "src": "", "note": "", "stmt": s, "temp": is_temp(table), "service": is_service_table(table)}
            if table in FILE_TABLES and op in ("INSERT", "UPDATE", "DELETE"):
                d = dml_detail(op, table, stext, s.params)
                r.update(d)
                # a parameter after the 1,000,000-byte payload is cut off by SQL Server: take the row name
                # from the DELETE of the other parts that the platform sends right before (same session)
                if r["name"].startswith("@P") and (s.trunc or s.cut):
                    prev = last_delete.get(e.spid)
                    if op == "UPDATE" and prev and prev[0] == table:
                        r["name"] = prev[1]
                        r["note"] = (r["note"] + "; " if r["note"] else "") + "name taken from the preceding DELETE (parameter cut off by SQL Server)"
                    else:
                        r["name"] = "<row name cut off by SQL Server>"
                if op == "DELETE" and r["partno"] == "<>0":
                    last_delete[e.spid] = (table, r["name"])
            recs.append(r)
    return recs


def fmt_dur(us):
    if us >= 1_000_000:
        return f"{us / 1e6:.2f} s"
    return f"{us / 1000:.1f} ms"


def fmt_sessions(spids):
    s = sorted(x for x in spids if x is not None)
    return str(s) if len(s) <= 6 else f"{len(s)} sessions"


def rel(us, t0):
    sec = (us - t0) / 1e6
    return f"+{int(sec // 60):02d}:{sec % 60:06.3f}"


# ---------------------------------------------------------------------------
# outputs
# ---------------------------------------------------------------------------

def write_groups(out, groups, t0):
    with open(os.path.join(out, "groups.tsv"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("id\tkind\tverb\ttables\tcount\ttotal_ms\tmax_note\trows\tfirst_seq\tfirst_t\tspids\tnormalized_sql\n")
        for g in groups:
            fh.write("\t".join([
                f"G{g.id:04d}", g.kind, g.verb, ",".join(g.tables), str(g.count), f"{g.dur / 1000:.1f}", "",
                str(g.rows), str(g.first_seq), f"{(g.first_us - t0) / 1e6:.3f}",
                ",".join(str(x) for x in sorted(x for x in g.spids if x is not None)), tsv_escape(g.norm[:4000]),
            ]) + "\n")
    lines = ["# Statement groups (order of first appearance)", "",
             f"{len(groups)} groups; the first {min(len(groups), MAX_GROUPS_IN_MD)} are listed here, all are in groups.tsv.", ""]
    for g in groups[:MAX_GROUPS_IN_MD]:
        lines.append(f"### G{g.id:04d}  x{g.count}  {g.verb}  {','.join(g.tables)}")
        lines.append(f"first {rel(g.first_us, t0)} (seq {g.first_seq}), total {fmt_dur(g.dur)}, rows {g.rows}, sessions {fmt_sessions(g.spids)}"
                     + (f", errors {g.errors}" if g.errors else ""))
        lines.append("```sql")
        lines.append(g.norm[:1500] + (" ..." if len(g.norm) > 1500 else ""))
        lines.append("```")
        if g.shapes:
            for pname, ctr in g.shapes.items():
                top = ", ".join(f"`{k}` x{v}" for k, v in ctr.most_common(8))
                lines.append(f"- {pname} shapes: {top}" + (" ..." if len(ctr) > 8 else ""))
        for smp in g.samples[:2]:
            if smp:
                lines.append(f"- e.g. {smp[:300]}")
        lines.append("")
    with open(os.path.join(out, "groups.md"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(lines))


def coarsen(runs, limit):
    """Merge consecutive runs into blocks of about equal statement count so that about `limit` remain.

    Transaction / error markers stay as separate lines while they are few (at most half of the
    limit); beyond that they are folded into the blocks.  Linear time.
    """
    if len(runs) <= limit:
        return runs
    markers = sum(1 for r in runs if r["marker"])
    keep_markers = markers <= limit // 2
    total = sum(r["count"] for r in runs)
    budget = max(1, limit - (markers if keep_markers else 0))
    thresh = max(1, total // budget + 1)
    out = []
    acc = None

    def flush():
        nonlocal acc
        if acc is not None:
            out.append(acc)
            acc = None

    for r in runs:
        if r["marker"]:
            if keep_markers:
                flush()
                out.append(r)
            continue
        if acc is None:
            acc = {"marker": None, "count": 0, "dur": 0, "rows": 0, "start": r["start"], "end": r["end"], "spid": r["spid"],
                   "groups": collections.Counter(), "first_seq": r["first_seq"], "mixed": True}
        acc["count"] += r["count"]
        acc["dur"] += r["dur"]
        acc["rows"] += r["rows"]
        acc["end"] = r["end"]
        if acc["spid"] != r["spid"]:
            acc["spid"] = "*"
        acc["groups"].update(r["groups"])
        if acc["count"] >= thresh:
            flush()
    flush()
    return out


def write_timeline(out, stmts, txs, events, t0):
    items = []
    for s in stmts:
        items.append((s.ev.seq if s.ev.seq is not None else 0, "stmt", s))
    for t in txs:
        if t.begin_seq is not None:
            items.append((t.begin_seq, "begin", t))
        if t.end_seq is not None:
            items.append((t.end_seq, "end", t))
    for e in events:
        if e.kind == "error":
            items.append((e.seq if e.seq is not None else 0, "error", e))
    items.sort(key=lambda x: (x[0], {"begin": 0, "stmt": 1, "end": 2, "error": 3}[x[1]]))
    runs = []
    for seq, kind, obj in items:
        if kind == "stmt":
            s = obj
            e = s.ev
            last = runs[-1] if runs else None
            if last and not last["marker"] and not last.get("mixed") and last["group"] == s.group.id and last["spid"] == e.spid:
                last["count"] += 1
                last["dur"] += e.dur
                last["rows"] += e.rows or 0
                last["end"] = e.us
                last["groups"][s.group.id] += 1
            else:
                runs.append({"marker": None, "group": s.group.id, "count": 1, "dur": e.dur, "rows": e.rows or 0,
                             "start": e.start_us, "end": e.us, "spid": e.spid, "groups": collections.Counter({s.group.id: 1}),
                             "first_seq": e.seq})
        elif kind == "begin":
            runs.append({"marker": f"BEGIN  tx {obj.tid} (spid {obj.spid})", "count": 0, "dur": 0, "rows": 0, "start": obj.begin_us,
                         "end": obj.begin_us, "spid": obj.spid, "groups": collections.Counter(), "first_seq": seq})
        elif kind == "end":
            runs.append({"marker": f"{obj.state.upper()}  tx {obj.tid} (spid {obj.spid}), {len(obj.stmts)} statements, {fmt_dur(obj.dur or 0)}",
                         "count": 0, "dur": 0, "rows": 0, "start": obj.end_us, "end": obj.end_us, "spid": obj.spid,
                         "groups": collections.Counter(), "first_seq": seq})
        else:
            runs.append({"marker": f"ERROR {obj.err_no} sev {obj.err_sev}: {one_line(obj.err_msg, 160)} (spid {obj.spid})",
                         "count": 0, "dur": 0, "rows": 0, "start": obj.us, "end": obj.us, "spid": obj.spid,
                         "groups": collections.Counter(), "first_seq": seq})
    # transaction begin markers of the same transaction can arrive out of order across sessions; keep as is
    total_runs = len(runs)
    runs = coarsen(runs, MAX_TIMELINE_RUNS)
    gid_norm = {}
    for s in stmts:
        gid_norm.setdefault(s.group.id, s.group)
    lines = ["# Timeline", "",
             f"{len(stmts)} statements in {total_runs} runs" + (f" (coarsened to {len(runs)})" if len(runs) < total_runs else ""), "",
             "| time | spid | n | duration | rows | what |", "|---|---|---|---|---|---|"]
    for r in runs:
        if r["marker"]:
            lines.append(f"| {rel(r['start'], t0)} | {r['spid']} |  |  |  | **{r['marker']}** |")
        elif r.get("mixed"):
            top = ", ".join(f"G{gid:04d} x{n}" for gid, n in r["groups"].most_common(4))
            lines.append(f"| {rel(r['start'], t0)} | {r['spid']} | {r['count']} | {fmt_dur(r['dur'])} | {r['rows']} | mixed: {top} |")
        else:
            g = gid_norm[r["group"]]
            lines.append(f"| {rel(r['start'], t0)} | {r['spid']} | {r['count']} | {fmt_dur(r['dur'])} | {r['rows']} | G{g.id:04d} {g.verb} {','.join(g.tables)}: `{one_line(g.norm, 110).replace('|', chr(0x2502))}` |")
    with open(os.path.join(out, "timeline.md"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(lines) + "\n")


def write_service_writes(out, recs, t0):
    header = ["seq", "t_s", "spid", "tx", "op", "table", "name", "name2", "partno", "rows", "size", "payload_bytes",
              "payload_sha256", "group", "src", "note"]
    payloads = []
    seen_payload = set()
    n = 0
    dropped = 0
    with open(os.path.join(out, "service-writes.tsv"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\t".join(header) + "\n")
        for r in recs:
            if r["temp"] or not r["service"]:
                continue
            if n >= MAX_SERVICE_WRITE_LINES:
                dropped += 1
                continue
            pl = r["payload"]
            plen = pl.nbytes() if pl is not None else ""
            psha = ""
            # the statement text is cut by SQL Server (2,000,000 chars) or by the kit (-MaxStatementKB):
            # the payload is then incomplete, which the declared size shows
            cutoff = r["stmt"].trunc or r["stmt"].cut
            if pl is not None:
                if cutoff and (r["size"] is None or plen < r["size"]):
                    psha = f"(incomplete: {plen} of {r['size']} bytes visible)"
                else:
                    data = bytes.fromhex(pl.value) if len(pl.value) % 2 == 0 else b""
                    psha = sha256_hex(data)
                    key = (r["table"], r["name"], psha)
                    if len(payloads) < MAX_PAYLOADS and key not in seen_payload and r["table"] in FILE_TABLES:
                        seen_payload.add(key)
                        dec = decode_payload(data)
                        payloads.append({
                            "seq": r["seq"], "table": r["table"], "name": r["name"], "partno": r["partno"],
                            "stored_bytes": len(data), "sha256": psha, "enc": dec["enc"], "kind": dec["kind"],
                            "decoded_bytes": len(dec["data"]),
                            "text": dec["text"] if dec["text"] is not None and len(dec["text"]) <= MAX_PAYLOAD_TEXT else None,
                            "head_hex": dec["data"][:48].hex() if dec["text"] is None else None,
                        })
            fh.write("\t".join(tsv_escape(x) for x in [
                r["seq"], f"{r['t']:.3f}", r["spid"], r["xid"], r["op"], r["table"], r["name"], r["name2"], r["partno"],
                "" if r["rows"] is None else r["rows"], "" if r["size"] is None else r["size"], plen, psha, f"G{r['group']:04d}",
                r["src"], r["note"]]) + "\n")
            n += 1
        if dropped:
            fh.write(f"# {dropped} more lines not written (limit {MAX_SERVICE_WRITE_LINES})\n")
    with open(os.path.join(out, "payloads.jsonl"), "w", encoding="utf-8", newline="\n") as fh:
        for p in payloads:
            fh.write(json.dumps(p, ensure_ascii=False) + "\n")
    return n, len(payloads)


def write_data_writes(out, recs):
    agg = collections.OrderedDict()
    for r in recs:
        if r["temp"] or r["service"] or not r["table"]:
            continue
        k = (r["table"], r["op"])
        a = agg.setdefault(k, {"stmts": 0, "rows": 0, "dur": 0, "first": r["seq"], "last": r["seq"], "sample": one_line(r["stmt"].norm, 200)})
        a["stmts"] += 1
        a["rows"] += r["rows"] or 0
        a["dur"] += r["dur"]
        a["last"] = r["seq"]
    with open(os.path.join(out, "data-writes.tsv"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("table\top\tstatements\trows\ttotal_ms\tfirst_seq\tlast_seq\tsample\n")
        for (t, op), a in agg.items():
            fh.write("\t".join(tsv_escape(x) for x in [t, op, a["stmts"], a["rows"], f"{a['dur'] / 1000:.1f}", a["first"], a["last"], a["sample"]]) + "\n")
    return agg


def write_ddl(out, stmts, t0):
    n = 0
    temp_ddl = 0
    with open(os.path.join(out, "ddl.sql"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("-- DDL of the traced command, verbatim, in order (temporary-table DDL is only counted).\n")
        for s in stmts:
            ddl = [(v, t) for v, t, _ in s.ops if v.split()[0] in ("CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME")]
            if not ddl:
                continue
            if all(is_temp(t) for _, t in ddl if t) and all(t for _, t in ddl):
                temp_ddl += 1
                continue
            e = s.ev
            fh.write(f"\n-- seq={e.seq} t={rel(e.start_us, t0)} spid={e.spid} tx={s.tx.tid if s.tx else 0} dur={fmt_dur(e.dur)} rows={e.rows}"
                     f"{' [text cut]' if s.cut else ''}\n")
            text = e.stmt if e.kind == "batch" else e.stmt
            renames = [st for v, _t, st in s.ops if v == "RENAME"]
            if renames and all(v == "RENAME" for v, _ in ddl) and e.kind == "rpc":
                # a prepared `exec @P1 = sp_rename @P2, @P3, ...`: show it with the values in place
                text = "-- prepared statement, values inlined\n" + "\n".join(renames)
            if len(text) > 200000:
                text = text[:200000] + "\n-- ... (cut at 200000 chars)"
            fh.write(text.rstrip() + "\n" + ("GO\n" if e.kind == "batch" else ""))
            n += 1
        fh.write(f"\n-- {n} DDL statements written, {temp_ddl} temporary-table DDL statements not listed\n")
    return n, temp_ddl


def write_transactions(out, txs, t0):
    with open(os.path.join(out, "transactions.tsv"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("tx\tspid\tbegin_seq\tbegin_t\tend_seq\tstate\tduration_ms\tstatements\twrites\tservice_write_tables\tsavepoints\tfirst_statement\n")
        for t in txs:
            wr = 0
            tabs = collections.Counter()
            for s in t.stmts:
                for op, tab, _ in s.ops:
                    if op in (WRITE_OPS + ("TRUNCATE",)):
                        wr += 1
                        if tab and not is_temp(tab) and is_service_table(tab):
                            tabs[tab] += 1
            first = ""
            for s in t.stmts:
                if not any(v in ("BEGIN",) for v, _, _ in s.ops):
                    first = one_line(s.norm, 100)
                    break
            fh.write("\t".join(tsv_escape(x) for x in [
                t.tid, t.spid, t.begin_seq, f"{(t.begin_us - t0) / 1e6:.3f}", t.end_seq if t.end_seq is not None else "", t.state,
                f"{(t.dur or 0) / 1000:.1f}", len(t.stmts), wr, ",".join(f"{k}:{v}" for k, v in tabs.most_common()), t.savepoints, first]) + "\n")


def write_phases(out, stmts, recs, txs, t0):
    """write-phases.md: only what changes the database (DML on non-temporary tables, DDL), grouped
    into blocks of consecutive writes to the same table, in the order they happened.

    A block lists what was done to the table (operation and row-name shape with counts), how many
    user transactions ran inside it and the first and last row name.  Reads, temporary-table
    writes and transaction control statements are left out (see timeline.md)."""
    items = []
    for r in recs:
        if r["temp"]:
            continue
        shape = ""
        if r["table"] in FILE_TABLES and r["name"]:
            n = r["name"]
            shape = ("LIKE " + name_shape(n[5:])) if n.startswith("LIKE ") else name_shape(n)
            if r["name2"]:
                shape += " -> " + name_shape(r["name2"])
            if r["src"]:
                shape += " <- " + r["src"].split(":")[0]
        items.append((r["seq"] or 0, 1, r["op"], r["table"], shape, r["spid"], r["rows"] or 0, r["name"] or "", r["dur"], r["stmt"].ev.start_us, r["stmt"].ev.us))
    for s in stmts:
        for op, table, stext in s.ops:
            if op.split()[0] in ("CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME") and not is_temp(table):
                shape = one_line(stext, 90) if op == "RENAME" or table == "" else ""
                items.append((s.ev.seq or 0, 1, op, table, shape, s.ev.spid, 0, "", s.ev.dur, s.ev.start_us, s.ev.us))
    tx_marks = []
    for t in txs:
        if t.begin_seq is not None:
            tx_marks.append((t.begin_seq, "BEGIN", t.begin_us))
        if t.end_seq is not None:
            tx_marks.append((t.end_seq, t.state.upper(), t.end_us))
    items.sort(key=lambda x: x[0])
    tx_marks.sort()

    blocks = []
    gap_us = 5_000_000
    for seq, _o, op, table, shape, spid, rows, name, dur, us0, us1 in items:
        b = blocks[-1] if blocks else None
        if b is None or b["table"] != table or us0 - b["end"] > gap_us:
            b = {"table": table, "first_seq": seq, "last_seq": seq, "start": us0, "end": us1, "spids": set(), "n": 0, "rows": 0, "dur": 0,
                 "ops": collections.OrderedDict(), "first": name, "last": name, "tx": collections.Counter(), "mixed": False}
            blocks.append(b)
        b["last_seq"] = seq
        b["end"] = max(b["end"], us1)
        b["spids"].add(spid)
        b["n"] += 1
        b["rows"] += rows
        b["dur"] += dur
        o = b["ops"].setdefault((op, shape), [0, 0])
        o[0] += 1
        o[1] += rows
        if name:
            if not b["first"]:
                b["first"] = name
            b["last"] = name
    # transaction boundaries per block (by event sequence)
    if blocks:
        starts = [b["first_seq"] for b in blocks]
        import bisect
        for seq, kind, _us in tx_marks:
            i = bisect.bisect_right(starts, seq) - 1
            if i >= 0 and seq <= blocks[i]["last_seq"] + 3:
                blocks[i]["tx"][kind] += 1
            elif i + 1 < len(blocks) and blocks[i + 1]["first_seq"] - seq <= 3:
                blocks[i + 1]["tx"][kind] += 1
    total_blocks = len(blocks)
    limit = 400
    thresh = 3
    while len(blocks) > limit and thresh < 1 << 20:
        merged = []
        acc = None
        for b in blocks:
            if b["n"] < thresh:
                if acc is None:
                    acc = {"table": "(several)", "first_seq": b["first_seq"], "last_seq": b["last_seq"], "start": b["start"], "end": b["end"], "spids": set(),
                           "n": 0, "rows": 0, "dur": 0, "ops": collections.OrderedDict(), "first": "", "last": "", "tx": collections.Counter(), "mixed": True, "tables": collections.Counter()}
                acc["last_seq"] = b["last_seq"]
                acc["end"] = max(acc["end"], b["end"])
                acc["spids"] |= b["spids"]
                acc["n"] += b["n"]
                acc["rows"] += b["rows"]
                acc["dur"] += b["dur"]
                acc["tx"].update(b["tx"])
                acc.setdefault("tables", collections.Counter())[b["table"]] += b["n"]
            else:
                if acc is not None:
                    merged.append(acc)
                    acc = None
                merged.append(b)
        if acc is not None:
            merged.append(acc)
        if len(merged) == len(blocks):
            thresh *= 2
        blocks = merged
        thresh *= 2

    L = ["# Write phases", "",
         f"{len(items)} writes/DDL statements in {total_blocks} blocks" + (f" (small blocks merged to {len(blocks)})" if len(blocks) < total_blocks else "")
         + ". A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).",
         "", "| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |",
         "|---|---|---|---|---|---|---|---|---|"]
    for i, b in enumerate(blocks, 1):
        sp = sorted(x for x in b["spids"] if x is not None)
        sess = str(sp[0]) if len(sp) == 1 else f"{len(sp)} sess."
        tx = ", ".join(f"{k.lower()} {v}" for k, v in sorted(b["tx"].items())) if b["tx"] else ""
        if b["mixed"]:
            what = "; ".join(f"{t} x{n}" for t, n in b.get("tables", {}).most_common(6))
        else:
            parts = [f"{op}{(' ' + shape) if shape else ''} x{cnt[0]}" for (op, shape), cnt in list(b["ops"].items())[:8]]
            what = "; ".join(parts) + (f"; ... +{len(b['ops']) - 8} more" if len(b["ops"]) > 8 else "")
        names = ""
        if b["first"]:
            names = f"`{one_line(b['first'], 50)}`" + (f" .. `{one_line(b['last'], 50)}`" if b["last"] != b["first"] else "")
        L.append(f"| {i} | {rel(b['start'], t0)} .. {rel(b['end'], t0)} | {sess} | {b['table']} | {b['n']} | {b['rows']} | {tx} | {what.replace('|', chr(0x2502))} | {names} |")
    with open(os.path.join(out, "write-phases.md"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(L) + "\n")
def write_slowest(out, stmts, t0, top=60):
    ranked = sorted(stmts, key=lambda s: -s.ev.dur)[:top]
    with open(os.path.join(out, "slowest.tsv"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("seq\tt\tspid\tduration_ms\tcpu_ms\treads\twrites\trows\tgroup\tnormalized_sql\n")
        for s in ranked:
            e = s.ev
            fh.write("\t".join(tsv_escape(x) for x in [
                e.seq, rel(e.start_us, t0), e.spid, f"{e.dur / 1000:.1f}", f"{e.cpu / 1000:.1f}", e.reads, e.writes,
                "" if e.rows is None else e.rows, f"G{s.group.id:04d}", one_line(s.norm, 300)]) + "\n")


def write_sessions(out, stmts):
    sess = collections.OrderedDict()
    for s in stmts:
        e = s.ev
        d = sess.setdefault(e.spid, {"app": e.app, "db": e.dbname, "n": 0, "dur": 0, "first": e.start_us, "last": e.us})
        d["n"] += 1
        d["dur"] += e.dur
        d["last"] = max(d["last"], e.us)
        if not d["app"] and e.app:
            d["app"] = e.app
    with open(os.path.join(out, "sessions.tsv"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("spid\tapp\tdatabase\tstatements\ttotal_statement_ms\tfirst_utc_us\tlast_utc_us\n")
        for spid, d in sess.items():
            fh.write("\t".join(tsv_escape(x) for x in [spid, d["app"], d["db"], d["n"], f"{d['dur'] / 1000:.1f}", d["first"], d["last"]]) + "\n")
    return sess


def write_summary(out, meta, events, stmts, groups, txs, recs, agg, ddl_counts, sessions, t0, t1, payload_count, sw_lines):
    L = []
    add = L.append
    add(f"# Trace report: {meta.get('tag', '')}")
    add("")
    add(f"- database: `{meta.get('database', '?')}` on `{meta.get('server', '?')}`, {meta.get('sql_version', '')}")
    add(f"- session: `{meta.get('session', '?')}`, predicate `{meta.get('predicate', '?')}`"
        + (f"; sessions kept only when they touched `{meta['focus_db']}` ({meta.get('sessions_dropped_by_focus', 0)} other sessions dropped)" if meta.get("focus_db") else ""))
    add(f"- command: `{one_line(str(meta.get('command', '')), 400)}`")
    add(f"- exit code {meta.get('exit_code', '?')}, command {meta.get('command_seconds', '?')} s; trace window {(t1 - t0) / 1e6:.1f} s "
        f"(first statement start to last statement end)")
    dropped = meta.get("dropped_events")
    add(f"- events {len(events)}; dropped by XE: {dropped if dropped is not None else '?'}"
        + (" **(events were lost: the report is incomplete)**" if dropped else "")
        + (f"; {meta['events_before_start_dropped']} older events in the file (an earlier run) were ignored" if meta.get("events_before_start_dropped") else ""))
    cut = sum(1 for s in stmts if s.cut)
    trunc = sum(1 for s in stmts if s.trunc)
    if cut or trunc:
        add(f"- statements cut by the kit (over {meta.get('max_statement_kb', '?')} KB): {cut}; "
            f"rpc statements truncated by SQL Server at 2,000,000 chars (binary parameter over ~1,000,000 bytes): {trunc}")
    add("")
    add("## Volume")
    add("")
    by_verb = collections.Counter(s.verb for s in stmts)
    add(f"- {len(stmts)} statement events ({sum(1 for s in stmts if s.ev.kind == 'rpc')} rpc, {sum(1 for s in stmts if s.ev.kind == 'batch')} batch), "
        f"{len(groups)} normalized groups, total statement time {fmt_dur(sum(s.ev.dur for s in stmts))}")
    add("- by verb: " + ", ".join(f"{v} {n}" for v, n in by_verb.most_common()))
    committed = sum(1 for t in txs if t.state == "Commit")
    rolled = sum(1 for t in txs if t.state == "Rollback")
    openx = sum(1 for t in txs if t.state == "open")
    auto = sum(1 for s in stmts if s.tx is None and not any(v in ("BEGIN", "COMMIT", "ROLLBACK") for v, _, _ in s.ops))
    add(f"- user transactions {len(txs)} (commit {committed}, rollback {rolled}, unfinished {openx}); statements outside a user transaction {auto}")
    add("- sessions: " + "; ".join(f"spid {k} `{v['app'] or '-'}` {v['n']} statements" for k, v in list(sessions.items())[:12]))
    errs = [e for e in events if e.kind == "error"]
    failed = [s for s in stmts if s.ev.result and s.ev.result != "OK"]
    add(f"- SQL errors reported {len(errs)}, statements with a non-OK result {len(failed)}")
    add("")
    add("## Writes to service tables (name shapes)")
    add("")
    agg_sw = collections.OrderedDict()
    for r in recs:
        if r["temp"] or not r["service"]:
            continue
        key = (r["table"], r["op"], name_shape(r["name"]) if r["name"] and r["table"] in FILE_TABLES else "")
        a = agg_sw.setdefault(key, {"n": 0, "rows": 0, "first": r["seq"], "t": r["t"], "bytes": 0})
        a["n"] += 1
        a["rows"] += r["rows"] or 0
        a["bytes"] += (r["size"] or 0)
    add("| first seq | table | op | name shape | statements | rows | declared bytes |")
    add("|---|---|---|---|---|---|---|")
    for (t, op, sh), a in list(agg_sw.items())[:SUMMARY_SHAPE_ROWS]:
        add(f"| {a['first']} | {t} | {op} | `{sh}` | {a['n']} | {a['rows']} | {a['bytes']} |")
    if len(agg_sw) > SUMMARY_SHAPE_ROWS:
        add(f"| ... | | | {len(agg_sw) - SUMMARY_SHAPE_ROWS} more shapes, see write-phases.md and service-writes.tsv | | | |")
    add("")
    add(f"Every write in order: `service-writes.tsv` ({sw_lines} lines), decoded small payloads: `payloads.jsonl` ({payload_count}).")
    add("")
    add("## Writes to object (data) tables")
    add("")
    if agg:
        add("| table | op | statements | rows | total ms |")
        add("|---|---|---|---|---|")
        for (t, op), a in list(agg.items())[:60]:
            add(f"| {t} | {op} | {a['stmts']} | {a['rows']} | {a['dur'] / 1000:.1f} |")
        if len(agg) > 60:
            add(f"... {len(agg) - 60} more in data-writes.tsv")
    else:
        add("none")
    add("")
    add("## DDL")
    add("")
    ddl_kinds = collections.Counter()
    for s in stmts:
        for v, t, _ in s.ops:
            if v.split()[0] in ("CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME") and not is_temp(t):
                ddl_kinds[(v, t)] += 1
    add(f"{ddl_counts[0]} DDL statements (verbatim in `ddl.sql`), {ddl_counts[1]} temporary-table DDL statements not listed.")
    for (v, t), n in ddl_kinds.most_common(40):
        add(f"- {v} `{t}` x{n}")
    add("")
    if errs or failed:
        add("## Errors")
        add("")
        for e in errs[:30]:
            add(f"- seq {e.seq} spid {e.spid}: error {e.err_no} severity {e.err_sev}: {one_line(e.err_msg, 300)}")
        for s in failed[:30]:
            add(f"- seq {s.ev.seq} spid {s.ev.spid} result {s.ev.result}: `{one_line(s.norm, 200)}`")
        add("")
    add("## Transactions")
    add("")
    add("| tx | spid | begin | duration | state | statements | writes to service tables |")
    add("|---|---|---|---|---|---|---|")
    for t in txs[:60]:
        tabs = collections.Counter()
        for s in t.stmts:
            for op, tab, _ in s.ops:
                if op in (WRITE_OPS + ("TRUNCATE",)) and tab and not is_temp(tab) and is_service_table(tab):
                    tabs[tab] += 1
        add(f"| {t.tid} | {t.spid} | {rel(t.begin_us, t0)} | {fmt_dur(t.dur or 0)} | {t.state} | {len(t.stmts)} | "
            + ", ".join(f"{k} {v}" for k, v in tabs.most_common()) + " |")
    if len(txs) > 60:
        add(f"... {len(txs) - 60} more in transactions.tsv")
    add("")
    add("## Slowest statements")
    add("")
    for s in sorted(stmts, key=lambda s: -s.ev.dur)[:10]:
        add(f"- {fmt_dur(s.ev.dur)} at {rel(s.ev.start_us, t0)} (seq {s.ev.seq}, G{s.group.id:04d}): `{one_line(s.norm, 160)}`")
    add("")
    add("## Files")
    add("")
    add("`groups.md` `groups.tsv` `timeline.md` `service-writes.tsv` `payloads.jsonl` `data-writes.tsv` `ddl.sql` "
        "`transactions.tsv` `slowest.tsv` `sessions.tsv` `events.tsv` `trace-meta.json` `session.sql` `command.log`")
    with open(os.path.join(out, "summary.md"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(L) + "\n")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--events", default="", help="events.tsv")
    ap.add_argument("--xml", default="", help="events.xml.gz (raw XE events); converted to <out>/events.tsv first")
    ap.add_argument("--max-statement-kb", type=int, default=1024)
    ap.add_argument("--keep-intermediate", action="store_true", help="retain parsed events.tsv after compression (no file cleanup)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--meta", default="")
    ap.add_argument("--focus-db", default="", help="keep only the sessions that touched this database (traces taken with -AllDatabases)")
    ap.add_argument("--since", default="", help="drop events older than this UTC time (ISO 8601): the start of the traced command")
    args = ap.parse_args(argv)
    meta = {}
    if args.meta and os.path.isfile(args.meta):
        with open(args.meta, "r", encoding="utf-8-sig") as fh:
            meta = json.load(fh)
    os.makedirs(args.out, exist_ok=True)
    events_path = args.events
    if args.xml:
        events_path = os.path.join(args.out, "events.tsv")
        count = convert_xml_to_tsv(args.xml, events_path, args.max_statement_kb * 1024)
        print(f"{count} events converted from {os.path.basename(args.xml)}")
    if not events_path:
        raise SystemExit("give --events events.tsv or --xml events.xml.gz")
    events = load_events(events_path)
    since = args.since or meta.get("started_utc", "")
    if since:
        cut = parse_ts(since) - 1_000_000
        older = sum(1 for e in events if e.us < cut)
        if older:
            events = [e for e in events if e.us >= cut]
            meta["events_before_start_dropped"] = older
            print(f"dropped {older} events older than the start of the command ({since})")
    events, dropped_sessions = focus_events(events, args.focus_db)
    if args.focus_db:
        meta["focus_db"] = args.focus_db
        meta["sessions_dropped_by_focus"] = dropped_sessions
    stmts, groups = analyze(events)
    txs = build_transactions(events, stmts)
    if stmts:
        t0 = min(s.ev.start_us for s in stmts)
        t1 = max(s.ev.us for s in stmts)
    else:
        t0 = t1 = 0
    recs = collect_writes(stmts, t0)
    write_groups(args.out, groups, t0)
    write_timeline(args.out, stmts, txs, events, t0)
    sw_lines, payload_count = write_service_writes(args.out, recs, t0)
    agg = write_data_writes(args.out, recs)
    ddl_counts = write_ddl(args.out, stmts, t0)
    write_transactions(args.out, txs, t0)
    write_phases(args.out, stmts, recs, txs, t0)
    write_slowest(args.out, stmts, t0)
    sessions = write_sessions(args.out, stmts)
    write_summary(args.out, meta, events, stmts, groups, txs, recs, agg, ddl_counts, sessions, t0, t1, payload_count, sw_lines)
    if args.xml:
        # the parsed events stay for later questions, compressed
        with open(events_path, "rb") as src, gzip.open(events_path + ".gz", "wb", compresslevel=6) as dst:
            shutil.copyfileobj(src, dst)
        if not args.keep_intermediate:
            os.remove(events_path)
    print(f"report: {len(events)} events, {len(stmts)} statements, {len(groups)} groups, {len(txs)} transactions -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
