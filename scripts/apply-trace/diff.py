#!/usr/bin/env python3
"""Compare two snapshots written by snapshot.ps1.

usage: diff.py --before <snapshot dir> --after <snapshot dir> [--out <dir>] [--blobs <blob store>]

Writes diff.md (for reading) and diff.json (for tools):
  * schema: tables created / dropped / altered (columns, indexes), other objects, database settings
  * service tables: rows inserted / updated / deleted with names, sizes and hashes; for updated file rows
    with stored content: what changed inside (module text, `versions` entries, {...} tokens)
  * data tables: row count and checksum changes

Only the Python standard library is used.
"""

import argparse
import collections
import difflib
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from apply_trace_common import (  # noqa: E402
    FILE_TABLES, decode_payload, load_blob, name_shape, one_line, parse_v8_container, parse_versions,
    read_tsv, sha256_hex, v8_lines, v8_tokens,
)

IGNORED_SETTINGS = {"option:name", "file:"}
# database-level values that change with any run
VOLATILE_SETTING_PREFIXES = ("file:",)


def load_snapshot(path):
    snap = {"path": path}
    with open(os.path.join(path, "meta.json"), "r", encoding="utf-8-sig") as fh:
        snap["meta"] = json.load(fh)
    snap["tables"] = {}
    for r in read_tsv(os.path.join(path, "tables.tsv")):
        snap["tables"][f"{r['schema']}.{r['table']}"] = r
    cols = collections.defaultdict(list)
    for r in read_tsv(os.path.join(path, "columns.tsv")):
        cols[f"{r['schema']}.{r['table']}"].append(
            (int(r["ord"]), r["column"], r["type"], r["nullable"], r["identity"], r["computed"], r["computed_def"], r["default_def"], r["collation"]))
    snap["columns"] = cols
    idx = collections.defaultdict(dict)
    for r in read_tsv(os.path.join(path, "indexes.tsv")):
        idx[f"{r['schema']}.{r['table']}"][r["index"]] = (r["type"], r["unique"], r["primary_key"], r["unique_constraint"], r["disabled"],
                                                          r["fill_factor"], r["filter"], r["keys"], r["included"], r["compression"])
    snap["indexes"] = idx
    snap["constraints"] = {(r["kind"], r["schema"], r["table"], r["name"]): r["definition"] for r in read_tsv(os.path.join(path, "constraints.tsv"))}
    snap["objects"] = {(r["kind"], r["schema"], r["name"]): r["definition_sha256"] for r in read_tsv(os.path.join(path, "objects.tsv"))}
    snap["settings"] = {r["key"]: r["value"] for r in read_tsv(os.path.join(path, "dbsettings.tsv"))}
    return snap


def load_rows(snap, table):
    p = os.path.join(snap["path"], "rows", table + ".tsv")
    if not os.path.isfile(p):
        return None
    rows = collections.OrderedDict()
    for r in read_tsv(p):
        key = r.get("FileName", r.get("key"))
        rows[key] = r
    return rows


COLUMN_FIELDS = ("ord", "column", "type", "nullable", "identity", "computed", "computed_def", "default_def", "collation")


def fmt_col(c):
    ord_, name, typ, nul, idn, cmp_, cdef, ddef, coll = c
    s = f"{name} {typ}{' NULL' if nul == '1' else ' NOT NULL'}"
    if idn == "1":
        s += " IDENTITY"
    if cmp_ == "1":
        s += f" AS {cdef}"
    if coll:
        s += f" COLLATE {coll}"
    if ddef:
        s += f" DEFAULT {ddef}"
    return s


def schema_diff(a, b):
    out = {"tables_created": [], "tables_dropped": [], "tables_altered": {}, "objects": [], "constraints": [], "settings": []}
    ta, tb = set(a["tables"]), set(b["tables"])
    out["tables_created"] = sorted(tb - ta)
    out["tables_dropped"] = sorted(ta - tb)
    for t in sorted(ta & tb):
        ca = {c[1]: c for c in a["columns"].get(t, [])}
        cb = {c[1]: c for c in b["columns"].get(t, [])}
        changes = []
        for name in cb:
            if name not in ca:
                changes.append(("column added", fmt_col(cb[name])))
        for name in ca:
            if name not in cb:
                changes.append(("column dropped", fmt_col(ca[name])))
        for name in ca:
            if name in cb and ca[name][2:] != cb[name][2:]:
                changes.append(("column changed", f"{fmt_col(ca[name])}  ->  {fmt_col(cb[name])}"))
        oa = [c[1] for c in sorted(ca.values())]
        ob = [c[1] for c in sorted(cb.values())]
        if [n for n in oa if n in cb] != [n for n in ob if n in ca]:
            changes.append(("column order changed", ",".join(ob)))
        ia, ib = a["indexes"].get(t, {}), b["indexes"].get(t, {})
        for n in ib:
            if n not in ia:
                changes.append(("index added", f"{n} {ib[n][0]} ({ib[n][7]}){' INCLUDE ' + ib[n][8] if ib[n][8] else ''}{' UNIQUE' if ib[n][1] == '1' else ''}"))
        for n in ia:
            if n not in ib:
                changes.append(("index dropped", f"{n} {ia[n][0]} ({ia[n][7]})"))
        for n in ia:
            if n in ib and ia[n] != ib[n]:
                changes.append(("index changed", f"{n}: {ia[n]} -> {ib[n]}"))
        if changes:
            out["tables_altered"][t] = changes
    for k in sorted(set(a["objects"]) | set(b["objects"])):
        if k not in a["objects"]:
            out["objects"].append(("added", k))
        elif k not in b["objects"]:
            out["objects"].append(("dropped", k))
        elif a["objects"][k] != b["objects"][k]:
            out["objects"].append(("changed", k))
    for k in sorted(set(a["constraints"]) | set(b["constraints"])):
        if k not in a["constraints"]:
            out["constraints"].append(("added", k, b["constraints"][k]))
        elif k not in b["constraints"]:
            out["constraints"].append(("dropped", k, a["constraints"][k]))
        elif a["constraints"][k] != b["constraints"][k]:
            out["constraints"].append(("changed", k, f"{a['constraints'][k]} -> {b['constraints'][k]}"))
    for k in sorted(set(a["settings"]) | set(b["settings"])):
        if k == "option:name" or k.startswith("file:"):
            continue
        if a["settings"].get(k) != b["settings"].get(k):
            out["settings"].append((k, a["settings"].get(k), b["settings"].get(k)))
    return out


def data_diff(a, b):
    out = {"changed": [], "unchanged": 0, "created": [], "dropped": []}
    for t in sorted(set(a["tables"]) & set(b["tables"])):
        ra, rb = a["tables"][t], b["tables"][t]
        same = ra["rows"] == rb["rows"] and ra["checksum_agg"] == rb["checksum_agg"] and ra["checksum_sum"] == rb["checksum_sum"]
        if same:
            out["unchanged"] += 1
        else:
            out["changed"].append({"table": t, "class": rb["class"], "rows_before": ra["rows"], "rows_after": rb["rows"],
                                   "checksum_changed": ra["checksum_agg"] != rb["checksum_agg"] or ra["checksum_sum"] != rb["checksum_sum"]})
    for t in sorted(set(b["tables"]) - set(a["tables"])):
        out["created"].append({"table": t, "rows": b["tables"][t]["rows"]})
    for t in sorted(set(a["tables"]) - set(b["tables"])):
        out["dropped"].append({"table": t, "rows": a["tables"][t]["rows"]})
    return out


# ---------------------------------------------------------------------------
# row content
# ---------------------------------------------------------------------------

def unified(a_lines, b_lines, limit, context=2):
    diff = list(difflib.unified_diff(a_lines, b_lines, lineterm="", n=context))
    body = [d for d in diff if not d.startswith(("---", "+++"))]
    if len(body) > limit:
        return body[:limit] + [f"... ({len(body) - limit} more diff lines)"]
    return body


def describe_content(stored, name):
    """Structured description of stored row bytes: enc, kind, container elements, text."""
    dec = decode_payload(stored)
    info = {"enc": dec["enc"], "kind": dec["kind"], "decoded": len(dec["data"]), "text": dec["text"], "elements": None}
    if dec["kind"] == "container":
        els = parse_v8_container(dec["data"])
        if els is not None:
            info["elements"] = [(n, len(body), sha256_hex(body)[:12], decode_payload(body)) for n, _h, body in els]
    return info


def content_diff(name, table, sa, sb, max_lines):
    """Lines describing what changed between two stored payloads (sa before, sb after)."""
    da, db = describe_content(sa, name), describe_content(sb, name)
    lines = []
    if da["enc"] != db["enc"]:
        lines.append(f"encoding {da['enc']} -> {db['enc']}")
    if da["elements"] is not None and db["elements"] is not None:
        ea = {n: (sz, sh, d) for n, sz, sh, d in da["elements"]}
        eb = {n: (sz, sh, d) for n, sz, sh, d in db["elements"]}
        for n in eb:
            if n not in ea:
                lines.append(f"container element added: {n} ({eb[n][0]} bytes)")
        for n in ea:
            if n not in eb:
                lines.append(f"container element removed: {n} ({ea[n][0]} bytes)")
        for n in ea:
            if n in eb and ea[n][1] != eb[n][1]:
                lines.append(f"container element `{n}` changed: {ea[n][0]} -> {eb[n][0]} bytes")
                ta, tb = ea[n][2]["text"], eb[n][2]["text"]
                if ta is not None and tb is not None:
                    lines += ["    " + x for x in unified(ta.splitlines(), tb.splitlines(), max_lines)]
                else:
                    lines.append(f"    binary element, sha {ea[n][1]} -> {eb[n][1]}")
        if not lines:
            lines.append("container elements are identical (only the compression or the container layout differs)")
        return lines
    ta, tb = da["text"], db["text"]
    if ta is not None and tb is not None:
        if name.startswith("versions"):
            pa, pb = parse_versions(ta), parse_versions(tb)
            if pa and pb:
                (ha, ma), (hb, mb) = pa, pb
                if ha != hb:
                    lines.append(f"header {' '.join(ha)} -> {' '.join(hb)}")
                changed = [k for k in mb if k in ma and ma[k] != mb[k]]
                added = [k for k in mb if k not in ma]
                removed = [k for k in ma if k not in mb]
                lines.append(f"entries: {len(ma)} -> {len(mb)}; changed {len(changed)}, added {len(added)}, removed {len(removed)}")
                for k in changed[:max_lines]:
                    lines.append(f"    ~ {k}: {ma[k]} -> {mb[k]}")
                for k in added[:max_lines]:
                    lines.append(f"    + {k}: {mb[k]}")
                for k in removed[:max_lines]:
                    lines.append(f"    - {k}: {ma[k]}")
                return lines
        if len(ta) <= 400 and len(tb) <= 400:
            lines.append(f"value: {one_line(ta, 400)}  ->  {one_line(tb, 400)}")
            return lines
        if ta.lstrip().startswith("{") and tb.lstrip().startswith("{"):
            la, lb = v8_lines(ta), v8_lines(tb)
            if max(len(la), len(lb)) > 300000:
                lines.append("text too large for a token diff")
            else:
                d = unified(la, lb, max_lines)
                lines += d if d else ["same tokens: only whitespace and line breaks differ"]
        else:
            lines += unified(ta.splitlines(), tb.splitlines(), max_lines)
        return lines
    lines.append(f"binary content: {da['decoded']} -> {db['decoded']} bytes")
    return lines


def text_equal(a, b):
    """Equal as text: same {...} tokens, or the same lines apart from CRLF and trailing blanks."""
    if a == b:
        return True
    ta = a.decode("utf-8-sig", errors="replace")
    tb = b.decode("utf-8-sig", errors="replace")
    if ta.lstrip().startswith("{") and tb.lstrip().startswith("{"):
        return v8_tokens(ta) == v8_tokens(tb)
    return ta.replace("\r\n", "\n").rstrip() == tb.replace("\r\n", "\n").rstrip()


def semantic_equal(sa, sb):
    """The stored bytes differ; do they mean the same (container elements / tokens equal)?"""
    da, db = decode_payload(sa), decode_payload(sb)
    if da["kind"] == "container" and db["kind"] == "container":
        ea, eb = parse_v8_container(da["data"]), parse_v8_container(db["data"])
        if ea is None or eb is None:
            return da["data"] == db["data"]
        ma = collections.OrderedDict((n, body) for n, _h, body in ea)
        mb = collections.OrderedDict((n, body) for n, _h, body in eb)
        if list(ma) != list(mb):
            return False
        return all(ma[n] == mb[n] or text_equal(ma[n], mb[n]) for n in ma)
    if da["kind"] in ("text", "v8text") and db["kind"] in ("text", "v8text"):
        return text_equal(da["data"], db["data"])
    return da["data"] == db["data"]


def classify_update(o, r, blobs):
    """recompressed | formatting | changed | unknown for a file row whose stored bytes changed."""
    if o["decoded_sha256"] and o["decoded_sha256"] == r["decoded_sha256"]:
        return "recompressed"
    if o["blob"] != "Y" or r["blob"] != "Y":
        return "unknown"
    sa, sb = load_blob(blobs, o["sha256"]), load_blob(blobs, r["sha256"])
    if sa is None or sb is None:
        return "unknown"
    return "formatting" if semantic_equal(sa, sb) else "changed"



def file_rows_diff(table, ra, rb, blobs, max_lines, content_budget):
    ins, dele, upd, meta_only = [], [], [], []
    unchanged = 0
    for k, r in rb.items():
        if k not in ra:
            ins.append(r)
    for k, r in ra.items():
        if k not in rb:
            dele.append(r)
    for k, r in rb.items():
        o = ra.get(k)
        if o is None:
            continue
        if o["sha256"] != r["sha256"]:
            upd.append((o, r))
        elif (o["creation"], o["modified"], o["attributes"]) != (r["creation"], r["modified"], r["attributes"]):
            meta_only.append((o, r))
        else:
            unchanged += 1
    return ins, dele, upd, meta_only, unchanged


def parse_summary(s):
    d = collections.OrderedDict()
    for part in re.split(r";\s+", s.strip().rstrip(";")):
        if "=" in part:
            k, v = part.split("=", 1)
            d[k] = v
    return d


def generic_rows_diff(ra, rb):
    ins = [r for k, r in rb.items() if k not in ra]
    dele = [r for k, r in ra.items() if k not in rb]
    upd = [(ra[k], r) for k, r in rb.items() if k in ra and ra[k]["row_sha256"] != r["row_sha256"]]
    unchanged = sum(1 for k, r in rb.items() if k in ra and ra[k]["row_sha256"] == r["row_sha256"])
    return ins, dele, upd, unchanged


def shape_counts(rows, key="FileName"):
    c = collections.Counter(name_shape(r[key]) for r in rows)
    return ", ".join(f"`{s}` x{n}" for s, n in c.most_common(12)) + (" ..." if len(c) > 12 else "")


def _chngr_key(sm):
    return (sm.get("_NodeTRef", ""), sm.get("_NodeRRef", ""), sm.get("_MDObjID", ""), sm.get("_ExtensionID", ""))


def changereg_diff(t, ra, rb, add, js, max_list):
    """_ConfigChngR / _ExtsChngR: the platform rebuilds these tables with new row ids, so compare by
    (node, object[, extension]) and report what really changed: rows and _MessageNo."""
    def load(rows):
        d = {}
        for r in rows.values():
            sm = parse_summary(r["summary"])
            d[_chngr_key(sm)] = (sm.get("_MessageNo", ""), sm.get("_IDRRef", ""))
        return d
    da, db = load(ra), load(rb)
    ins = [k for k in db if k not in da]
    dele = [k for k in da if k not in db]
    msg = [(k, da[k][0], db[k][0]) for k in db if k in da and da[k][0] != db[k][0]]
    regen = sum(1 for k in db if k in da and da[k][1] != db[k][1])
    add(f"### {t}: {len(ra)} -> {len(rb)} rows (compared by node and object, not by the row id)")
    add("")
    add(f"- rows only after: {len(ins)}; rows only before: {len(dele)}; `_MessageNo` changed: {len(msg)}; "
        f"row id `_IDRRef` differs in {regen} of {len(db) - len(ins)} common rows")
    for k, a_, b_ in msg[:max_list]:
        add(f"    - object {k[2]} node {k[0]}/{k[1][:12]}: _MessageNo {a_} -> {b_}")
    if len(msg) > max_list:
        add(f"    - ... {len(msg) - max_list} more")
    for k in ins[:max_list]:
        add(f"- + object {k[2]} node {k[0]}/{k[1][:12]} _MessageNo {db[k][0]}")
    for k in dele[:max_list]:
        add(f"- - object {k[2]} node {k[0]}/{k[1][:12]} _MessageNo {da[k][0]}")
    add("")
    js["service"][t] = {"inserted": len(ins), "deleted": len(dele), "message_no_changed": len(msg), "id_regenerated": regen}


CHANGE_REGISTERS = ("_ConfigChngR", "_ExtsChngR")



def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--before", required=True)
    ap.add_argument("--after", required=True)
    ap.add_argument("--out", default="")
    ap.add_argument("--blobs", default="")
    ap.add_argument("--max-list", type=int, default=30, help="rows listed per category and table")
    ap.add_argument("--max-lines", type=int, default=30, help="diff lines per changed row")
    ap.add_argument("--max-diff-rows", type=int, default=25, help="changed rows per table that get a content diff")
    args = ap.parse_args(argv)
    out = args.out or os.path.join(os.path.dirname(os.path.abspath(args.after)), "diff")
    os.makedirs(out, exist_ok=True)
    a, b = load_snapshot(args.before), load_snapshot(args.after)
    blobs = args.blobs or b["meta"].get("blob_store") or a["meta"].get("blob_store") or ""
    L = []
    add = L.append
    js = {"before": a["path"], "after": b["path"]}

    add("# Snapshot diff")
    add("")
    add(f"- before: `{a['path']}` ({a['meta']['database']}, {a['meta']['taken_local']})")
    add(f"- after: `{b['path']}` ({b['meta']['database']}, {b['meta']['taken_local']})")
    yo = b["meta"].get("year_offset")
    if yo:
        add(f"- date columns are stored with a year offset of {yo}: 4026-09-29 is 2026-09-29")
    add("")

    # ---- schema
    sd = schema_diff(a, b)
    js["schema"] = sd
    add("## Schema")
    add("")
    add(f"tables: {len(a['tables'])} -> {len(b['tables'])}; created {len(sd['tables_created'])}, dropped {len(sd['tables_dropped'])}, altered {len(sd['tables_altered'])}")
    add("")
    for t in sd["tables_created"][:args.max_list]:
        add(f"- CREATED `{t}` ({b['tables'][t]['rows']} rows): " + ", ".join(fmt_col(c) for c in b["columns"].get(t, [])[:40]))
        for n, ix in b["indexes"].get(t, {}).items():
            add(f"    - index {n}: {ix[0]}{' UNIQUE' if ix[1] == '1' else ''}{' PK' if ix[2] == '1' else ''} ({ix[7]}){' INCLUDE ' + ix[8] if ix[8] else ''}")
    for t in sd["tables_dropped"][:args.max_list]:
        add(f"- DROPPED `{t}` ({a['tables'][t]['rows']} rows)")
    for t, changes in list(sd["tables_altered"].items())[:args.max_list]:
        add(f"- ALTERED `{t}`")
        for kind, text in changes[:30]:
            add(f"    - {kind}: {text}")
    if len(sd["tables_created"]) > args.max_list or len(sd["tables_altered"]) > args.max_list:
        add(f"- ... lists cut at {args.max_list}; the full lists are in diff.json")
    for kind, k in sd["objects"]:
        add(f"- object {kind}: {k}")
    for kind, k, d in sd["constraints"]:
        add(f"- constraint {kind}: {k} {d}")
    for k, va, vb in sd["settings"]:
        add(f"- setting `{k}`: {va} -> {vb}")
    if not (sd["tables_created"] or sd["tables_dropped"] or sd["tables_altered"] or sd["objects"] or sd["constraints"] or sd["settings"]):
        add("no schema changes")
    add("")

    # ---- service tables
    add("## Service tables")
    add("")
    js["service"] = {}
    service_tables = [t for t in b["meta"].get("service_tables", []) if os.path.isfile(os.path.join(b["path"], "rows", t + ".tsv"))]
    for t in sorted(set(service_tables) | set(a["meta"].get("service_tables", [])), key=lambda x: (x not in FILE_TABLES, x)):
        ra, rb = load_rows(a, t), load_rows(b, t)
        if ra is None and rb is None:
            continue
        ra = ra or {}
        rb = rb or {}
        is_file = t in FILE_TABLES
        if is_file:
            ins, dele, upd, meta_only, unchanged = file_rows_diff(t, ra, rb, blobs, args.max_lines, 0)
            if not (ins or dele or upd or meta_only):
                js["service"][t] = {"unchanged": unchanged}
                continue
            add(f"### {t}: {len(ra)} -> {len(rb)} rows (unchanged {unchanged})")
            add("")
            entry = {"inserted": [r["FileName"] for r in ins], "deleted": [r["FileName"] for r in dele],
                     "updated": [o["FileName"] for o, _ in upd], "meta_only": [o["FileName"] for o, _ in meta_only], "unchanged": unchanged}
            js["service"][t] = entry
            if ins:
                add(f"**inserted {len(ins)}**: {shape_counts(ins)}")
                for r in ins[:args.max_list]:
                    add(f"- + `{r['FileName']}` size {r['size']} (stored {r['stored']}, {r['parts']} part{'s' if r['parts'] != '1' else ''}) sha {r['sha256'][:12]} {r['enc']} {r['kind']}"
                        + (f"  `{one_line(r['preview'], 100)}`" if r["preview"] else ""))
                if len(ins) > args.max_list:
                    add(f"- ... {len(ins) - args.max_list} more")
                add("")
            if dele:
                add(f"**deleted {len(dele)}**: {shape_counts(dele)}")
                for r in dele[:args.max_list]:
                    add(f"- - `{r['FileName']}` size {r['size']} sha {r['sha256'][:12]} {r['enc']} {r['kind']}"
                        + (f"  `{one_line(r['preview'], 100)}`" if r["preview"] else ""))
                if len(dele) > args.max_list:
                    add(f"- ... {len(dele) - args.max_list} more")
                add("")
            if upd:
                # classify every updated row: only recompressed / only formatting / really changed / content not kept
                cats = collections.OrderedDict((k, []) for k in ("recompressed", "formatting", "changed", "unknown"))
                for o, r in upd:
                    cats[classify_update(o, r, blobs)].append((o, r))
                entry["update_categories"] = {k: [o["FileName"] for o, _ in v] for k, v in cats.items()}
                add(f"**updated {len(upd)}**: {shape_counts([o for o, _ in upd])}")
                add(f"- only the compression differs (same decoded bytes): {len(cats['recompressed'])}")
                add(f"- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): {len(cats['formatting'])}")
                add(f"- content changed: {len(cats['changed'])}")
                add(f"- content not kept, cannot tell: {len(cats['unknown'])}")
                for cat in ("recompressed", "formatting", "unknown"):
                    if cats[cat]:
                        add(f"  - {cat}: {shape_counts([o for o, _ in cats[cat]])}; first: " + ", ".join(f"`{o['FileName']}`" for o, _ in cats[cat][:4]))
                shown_diffs = 0
                for o, r in cats["changed"][:args.max_list]:
                    line = f"- ~ `{r['FileName']}` size {o['size']} -> {r['size']} sha {o['sha256'][:12]} -> {r['sha256'][:12]}"
                    if o["modified"] != r["modified"]:
                        line += f"; modified {o['modified'][:19]} -> {r['modified'][:19]}"
                    add(line)
                    if shown_diffs < args.max_diff_rows:
                        sa, sb = load_blob(blobs, o["sha256"]), load_blob(blobs, r["sha256"])
                        for ln in content_diff(r["FileName"], t, sa, sb, args.max_lines):
                            add("    " + ln)
                        shown_diffs += 1
                if len(cats["changed"]) > args.max_list:
                    add(f"- ... {len(cats['changed']) - args.max_list} more changed rows in diff.json")
                add("")
            if meta_only:
                add(f"**same content, other metadata {len(meta_only)}**: {shape_counts([o for o, _ in meta_only])}")
                for o, r in meta_only[:args.max_list]:
                    add(f"- = `{r['FileName']}` creation {o['creation'][:19]} -> {r['creation'][:19]}, modified {o['modified'][:19]} -> {r['modified'][:19]}, attributes {o['attributes']} -> {r['attributes']}")
                add("")
        elif t in CHANGE_REGISTERS:
            if ra and rb and all(ra.get(k, {}).get("row_sha256") == v.get("row_sha256") for k, v in rb.items()) and len(ra) == len(rb):
                js["service"][t] = {"unchanged": len(rb)}
                continue
            changereg_diff(t, ra, rb, add, js, args.max_list)
        elif t.endswith("_ExtProps") and t[: -len("_ExtProps")] in CHANGE_REGISTERS:
            # rows are keyed by the row id of the register, which is regenerated: the count is what is comparable
            if len(ra) != len(rb):
                add(f"### {t}: {len(ra)} -> {len(rb)} rows")
                add("")
            js["service"][t] = {"rows_before": len(ra), "rows_after": len(rb)}
        else:
            ins, dele, upd, unchanged = generic_rows_diff(ra, rb)
            if not (ins or dele or upd):
                js["service"][t] = {"unchanged": unchanged}
                continue
            add(f"### {t}: {len(ra)} -> {len(rb)} rows (unchanged {unchanged})")
            add("")
            js["service"][t] = {"inserted": [r["key"] for r in ins], "deleted": [r["key"] for r in dele], "updated": [o["key"] for o, _ in upd], "unchanged": unchanged}
            for r in ins[:args.max_list]:
                add(f"- + `{r['key']}` {one_line(r['summary'], 240)}")
            if len(ins) > args.max_list:
                add(f"- ... {len(ins) - args.max_list} more inserted")
            for r in dele[:args.max_list]:
                add(f"- - `{r['key']}` {one_line(r['summary'], 240)}")
            if len(dele) > args.max_list:
                add(f"- ... {len(dele) - args.max_list} more deleted")
            for o, r in upd[:args.max_list]:
                so, sn = parse_summary(o["summary"]), parse_summary(r["summary"])
                ch = [f"{k}: {so.get(k, '-')} -> {sn.get(k, '-')}" for k in sn if so.get(k) != sn.get(k)]
                add(f"- ~ `{r['key']}` " + ("; ".join(ch) if ch else "content of a large column changed"))
                # a blob of a large column: show the change when both sides are kept
                for k in sn:
                    if so.get(k) != sn.get(k) and sn[k].endswith(" blob") and so.get(k, "").endswith(" blob"):
                        m_a, m_b = re.search(r"sha:([0-9a-f]+)", so[k]), re.search(r"sha:([0-9a-f]+)", sn[k])
                        add(f"    (column {k}: sha prefix {m_a.group(1) if m_a else '?'} -> {m_b.group(1) if m_b else '?'}; the blobs are in the store by full sha, see rows/{t}.tsv row_sha256)")
            if len(upd) > args.max_list:
                add(f"- ... {len(upd) - args.max_list} more updated")
            add("")

    # ---- data tables
    dd = data_diff(a, b)
    js["data_tables"] = dd
    add("## Data tables and counts")
    add("")
    add(f"unchanged tables: {dd['unchanged']}; changed: {len(dd['changed'])}; created: {len(dd['created'])}; dropped: {len(dd['dropped'])}")
    add("")
    if dd["changed"]:
        add("| table | class | rows before | rows after | checksum |")
        add("|---|---|---|---|---|")
        for c in dd["changed"][:args.max_list * 2]:
            add(f"| {c['table']} | {c['class']} | {c['rows_before']} | {c['rows_after']} | {'changed' if c['checksum_changed'] else 'same'} |")
        if len(dd["changed"]) > args.max_list * 2:
            add(f"... {len(dd['changed']) - args.max_list * 2} more in diff.json")
        add("")
    for c in dd["created"][:args.max_list]:
        add(f"- created table {c['table']} ({c['rows']} rows)")
    for c in dd["dropped"][:args.max_list]:
        add(f"- dropped table {c['table']} ({c['rows']} rows)")

    with open(os.path.join(out, "diff.md"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(L) + "\n")
    with open(os.path.join(out, "diff.json"), "w", encoding="utf-8", newline="\n") as fh:
        json.dump(js, fh, ensure_ascii=False, indent=1, default=str)
    changed_service = [t for t, e in js["service"].items() if any(e.get(k) for k in ("inserted", "deleted", "updated", "meta_only"))]
    print(f"diff: schema {len(sd['tables_created'])}+/{len(sd['tables_dropped'])}-/{len(sd['tables_altered'])}~ tables, "
          f"service tables changed: {', '.join(changed_service) or 'none'}, data tables changed: {len(dd['changed'])} -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
