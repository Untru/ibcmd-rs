#!/usr/bin/env python3
"""Compare the writes of several captured runs, family by family.

    python compare_traces.py --capture E01=<dir> --capture c1=<dir> ... [--out matrix.md]

A capture dir is a folder made by capture.ps1 / trace.ps1 (it holds `service-writes.tsv`,
or a `trace\\service-writes.tsv`).  A *family* is table + operation + the shape of the row
name (`<guid>.<n>.new`, `<guid>.sinew -> <guid>.si`, ...).  A cell is the number of write
statements, the rows they affected and the payload bytes visible in the trace:
`14 st / 14 rows / 2.3 KB`.  An empty cell: the run did not write that family.

Families are listed in the order of the runs: a family unknown so far is put right after the
family that preceded it in its own run, so the list reads like the phases of the operation.
The DDL statements of each run are counted by verb at the end.
"""

import argparse
import collections
import os
import re
import sys

from apply_trace_common import FILE_TABLES, name_shape, read_tsv


def find_writes(path):
    for cand in (os.path.join(path, "service-writes.tsv"), os.path.join(path, "trace", "service-writes.tsv")):
        if os.path.exists(cand):
            return cand
    raise SystemExit(f"no service-writes.tsv in {path}")


def family(row):
    table = row["table"]
    name = row["name"]
    shape = ""
    if table in FILE_TABLES and name:
        shape = ("LIKE " + name_shape(name[5:])) if name.startswith("LIKE ") else name_shape(name)
        if row.get("name2"):
            shape += " -> " + name_shape(row["name2"])
        if row.get("src"):
            shape += " <- " + row["src"].split(":")[0]
    return (table, row["op"], shape)


def to_int(text):
    try:
        return int(text)
    except (TypeError, ValueError):
        return 0


def load(path):
    """-> (ordered list of families, {family: [statements, rows, bytes]})."""
    order = []
    cells = collections.OrderedDict()
    for row in read_tsv(find_writes(path)):
        fam = family(row)
        cell = cells.get(fam)
        if cell is None:
            cell = cells[fam] = [0, 0, 0]
            order.append(fam)
        cell[0] += 1
        cell[1] += to_int(row.get("rows"))
        cell[2] += to_int(row.get("payload_bytes"))
    return order, cells


def merge_order(orders):
    merged = []
    for order in orders:
        prev = None
        for fam in order:
            if fam not in merged:
                merged.insert(merged.index(prev) + 1 if prev in merged else len(merged), fam)
            prev = fam
    return merged


def human(n):
    if n < 1024:
        return f"{n} B"
    if n < 1024 * 1024:
        return f"{n / 1024:.1f} KB"
    return f"{n / 1024 / 1024:.1f} MB"


def cell_text(cell):
    if not cell:
        return ""
    text = f"{cell[0]} st / {cell[1]} rows"
    if cell[2]:
        text += f" / {human(cell[2])}"
    return text


DDL_VERBS = re.compile(r"^\s*(create\s+(?:unique\s+)?(?:clustered\s+)?(?:nonclustered\s+)?(?:table|index|function|procedure|view)|alter\s+(?:table|index|database)|drop\s+(?:table|index|function|procedure|view)|truncate\s+table|exec(?:ute)?\s+sp_rename)", re.I)


def ddl_counts(path):
    cand = None
    for c in (os.path.join(path, "ddl.sql"), os.path.join(path, "trace", "ddl.sql")):
        if os.path.exists(c):
            cand = c
            break
    counts = collections.Counter()
    if not cand:
        return counts
    with open(cand, "r", encoding="utf-8") as fh:
        text = fh.read()
    for block in re.split(r"^-- seq=.*$", text, flags=re.M)[1:]:
        for stmt in re.split(r"^GO\s*$", block, flags=re.M):
            body = "\n".join(line for line in stmt.splitlines() if line.strip() and not line.startswith("--")).strip()
            if not body:
                continue
            m = DDL_VERBS.match(body)
            if m:
                counts[re.sub(r"\s+", " ", m.group(1).upper())] += 1
    return counts


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--capture", action="append", required=True, metavar="LABEL=DIR")
    ap.add_argument("--out", help="write the Markdown here (default: stdout)")
    ap.add_argument("--skip-config-copies", action="store_true",
                    help="leave out the Config rows that are copies of staged rows (<guid>[.n].new): they scale with the staged rows")
    args = ap.parse_args()

    runs = []
    for spec in args.capture:
        label, _, path = spec.partition("=")
        if not path:
            raise SystemExit(f"--capture wants LABEL=DIR, got {spec!r}")
        order, cells = load(path)
        runs.append((label, path, order, cells))

    families = merge_order([r[2] for r in runs])
    if args.skip_config_copies:
        families = [f for f in families if not (f[0] == "Config" and re.search(r"<guid>(\.<n>)?\.new( <- ConfigSave)?$", f[2]))]

    lines = []
    lines.append("| table | op | row name | " + " | ".join(r[0] for r in runs) + " |")
    lines.append("|---|---|---|" + "|".join("---" for _ in runs) + "|")
    for fam in families:
        table, op, shape = fam
        cells = [cell_text(r[3].get(fam)) for r in runs]
        lines.append(f"| {table} | {op} | {('`' + shape + '`') if shape else ''} | " + " | ".join(cells) + " |")
    lines.append("")
    lines.append("DDL statements by verb:")
    lines.append("")
    all_counts = [ddl_counts(r[1]) for r in runs]
    verbs = sorted({v for c in all_counts for v in c})
    lines.append("| verb | " + " | ".join(r[0] for r in runs) + " |")
    lines.append("|---|" + "|".join("---" for _ in runs) + "|")
    for v in verbs:
        lines.append(f"| {v} | " + " | ".join(str(c.get(v, "")) for c in all_counts) + " |")
    text = "\n".join(lines) + "\n"
    if args.out:
        with open(args.out, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)
    else:
        sys.stdout.write(text)


if __name__ == "__main__":
    main()
