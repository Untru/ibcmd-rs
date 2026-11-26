"""Compact story of an XE capture: statement shapes per session in order of first appearance.

usage: python xe_shapes.py <events.jsonl> [--session N] [--events e1,e2] [--min-count 1] [--width 400]
Literals are normalised (hex -> 0x#, uuid -> <uuid>, numbers -> #, 'strings' -> 's').
"""
import argparse
import collections
import json
import re
import sys

HEX = re.compile(r"0x[0-9A-Fa-f]+(?:<\d+ hex>)?")
UUID = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
NUM = re.compile(r"(?<![\w@#.])-?\d+(?:\.\d+)?(?![\w])")
STR = re.compile(r"N?'(?:[^']|'')*'")


EXEC_SQL = re.compile(r"(?is)^\s*exec\s+sp_executesql\s+N'((?:[^']|'')*)'")
PREPEXEC = re.compile(r"(?is)exec\s+sp_prepexec\s+@p1\s+output\s*,\s*N'(?:[^']|'')*'\s*,\s*N'((?:[^']|'')*)'")


def inner_sql(t):
    """the SQL text inside sp_executesql / sp_prepexec calls, else the text itself"""
    m = EXEC_SQL.match(t or "")
    if m:
        return m.group(1).replace("''", "'")
    m = PREPEXEC.search(t or "")
    if m:
        return m.group(1).replace("''", "'")
    return t


def norm(t):
    t = inner_sql(t)
    t = re.sub(r"\s+", " ", t or "").strip()
    t = HEX.sub("0x#", t)
    t = STR.sub("'s'", t)
    t = UUID.sub("<uuid>", t)
    t = NUM.sub("#", t)
    return t


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("--session", action="append")
    ap.add_argument("--events-only", default="")
    ap.add_argument("--min-count", type=int, default=1)
    ap.add_argument("--width", type=int, default=300)
    ap.add_argument("--skip-select", action="store_true")
    a = ap.parse_args()
    evs = set(a.events_only.split(",")) if a.events_only else None
    shapes = collections.OrderedDict()
    for line in open(a.events, encoding="utf-8"):
        e = json.loads(line)
        if a.session and e.get("sid") not in a.session:
            continue
        if evs and e["ev"] not in evs:
            continue
        t = e.get("text")
        if t is None:
            continue
        s = norm(t)
        if a.skip_select and re.match(r"(?is)(\(@[^)]*\))?\s*select", s):
            continue
        key = (e.get("sid"), e["ev"], s)
        if key not in shapes:
            shapes[key] = [0, e["t"], e["t"]]
        shapes[key][0] += 1
        shapes[key][2] = e["t"]
    for (sid, ev, s), (n, t0, t1) in shapes.items():
        if n < a.min_count:
            continue
        print("%s %s-%s %-22s x%-6d %s" % (sid, t0[11:23], t1[11:23], ev, n, s[:a.width]))


if __name__ == "__main__":
    main()
