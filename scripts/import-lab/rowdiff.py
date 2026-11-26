"""Row-level comparison of two Config-like tables (issue #388, the import track).

  python rowdiff.py <left-db>[:<table>] <right-db>[:<table>] [--tree <xml tree>] [--json out.json]
                    [--show N] [--only-different]

Reads every row of both tables (default table: ConfigSave for the left side, Config for the right
side is NOT assumed - name both, e.g. `mydb:ConfigSave mydb:Config`), joins the parts of a row
(PartNo order), inflates the raw-deflate payload and classifies each file name:

  identical_bytes   same stored bytes
  identical_text    same inflated text, other compressor output
  layout_only       same text once CR/LF are removed
  different         other text
  only_left / only_right   the name is in one table only

`--tree` names an exported XML tree: uuids of the file names are resolved to `Kind.Name` through
the XML files' root elements, so the report reads like a list of objects.

Only SELECTs are run. Lab tool: the databases are the ones of the 0.4 tracks.
"""
import argparse
import hashlib
import json
import os
import re
import sys
import zlib
from collections import Counter, defaultdict

import pyodbc

CONN = ("DRIVER={ODBC Driver 18 for SQL Server};SERVER=localhost;DATABASE=%s;"
        "Trusted_Connection=yes;TrustServerCertificate=yes")


def inflate(b):
    try:
        return zlib.decompress(b, -15)
    except Exception:
        return b


def load_dir(path):
    """A folder of `<FileName>__part<N>.bin` rows (the `--rows-dir` layout)."""
    rows = {}
    pat = re.compile(r"^(.*)__part(\d+)\.bin$")
    for f in os.listdir(path):
        m = pat.match(f)
        if not m:
            continue
        with open(os.path.join(path, f), "rb") as fh:
            data = fh.read()
        e = rows.setdefault(m.group(1), {"parts": [], "sizes": [], "data": []})
        e["parts"].append(int(m.group(2)))
        e["sizes"].append(len(data))
        e["data"].append(data)
    for e in rows.values():
        order = sorted(range(len(e["parts"])), key=lambda i: e["parts"][i])
        for k in ("parts", "sizes", "data"):
            e[k] = [e[k][i] for i in order]
    return finish(rows)


def dump_dir(rows, path):
    os.makedirs(path, exist_ok=True)
    for name, e in rows.items():
        for number, chunk in enumerate(e["chunks"]):
            with open(os.path.join(path, "%s__part%d.bin" % (name, number)), "wb") as fh:
                fh.write(chunk)


def load(db, table):
    """name -> dict(parts, size, raw (bytes), text (bytes))"""
    if db == "dir":
        return load_dir(table)
    rows = {}
    conn = pyodbc.connect(CONN % db, autocommit=True)
    cur = conn.cursor()
    cur.execute("SELECT FileName, PartNo, DataSize, BinaryData FROM dbo.[%s] ORDER BY FileName, PartNo" % table)
    while True:
        batch = cur.fetchmany(200)
        if not batch:
            break
        for name, part, size, data in batch:
            e = rows.setdefault(name, {"parts": [], "sizes": [], "data": []})
            e["parts"].append(part)
            e["sizes"].append(size)
            e["data"].append(bytes(data) if data is not None else b"")
    conn.close()
    return finish(rows)


def finish(rows):
    out = {}
    for name, e in rows.items():
        raw = b"".join(e["data"])
        out[name] = {
            "parts": len(e["parts"]),
            "part_nos": e["parts"],
            "declared": e["sizes"][0],
            "chunks": e["data"],
            "raw": raw,
            "attributes_unused": 0,
        }
    return out


def tree_names(tree):
    """uuid -> 'Kind.Name (relative path)' from the root element of the descriptor XMLs."""
    names = {}
    if not tree:
        return names
    pat = re.compile(rb'<(\w+) uuid="([0-9a-fA-F-]{36})"')
    npat = re.compile(rb"<Name>([^<]*)</Name>")
    for root, _dirs, files in os.walk(tree):
        rel_root = os.path.relpath(root, tree).replace("\\", "/")
        if "/Ext" in "/" + rel_root or rel_root.endswith("/Ext"):
            continue
        for f in files:
            if not f.endswith(".xml"):
                continue
            path = os.path.join(root, f)
            try:
                with open(path, "rb") as fh:
                    head = fh.read(6000)
            except OSError:
                continue
            body = head.replace(b"\xef\xbb\xbf", b"")
            # skip the <MetaDataObject ...> wrapper: the first element carrying a uuid attribute
            m = pat.search(body[body.find(b"<MetaDataObject"):] if b"<MetaDataObject" in body else body)
            if not m:
                continue
            kind = m.group(1).decode()
            uuid = m.group(2).decode().lower()
            n = npat.search(body)
            nm = n.group(1).decode("utf-8", "replace") if n else "?"
            rel = os.path.relpath(path, tree).replace("\\", "/")
            names.setdefault(uuid, "%s.%s [%s]" % (kind, nm, rel))
    return names


def describe(name, names):
    base = name.split(".")[0].split("_")[0]
    suffix = name[len(base):] if name.startswith(base) else ""
    label = names.get(base.lower())
    return "%s%s" % (label or "?", suffix) if label else name


def first_diff(a, b):
    n = min(len(a), len(b))
    i = 0
    # fast skip in chunks
    step = 4096
    while i + step <= n and a[i:i + step] == b[i:i + step]:
        i += step
    while i < n and a[i] == b[i]:
        i += 1
    return i


def context(text, off, width=70):
    s = max(0, off - width)
    e = min(len(text), off + width)
    return text[s:e].decode("utf-8", "replace").replace("\r", "").replace("\n", "\\n")


def _document(b, addr):
    """The document of a v8 container at `addr`: (bytes, ok)."""
    if addr < 0 or addr + 31 > len(b):
        return None
    head = b[addr:addr + 31]
    if head[:2] != b"\r\n" or head[10:11] != b" " or head[19:20] != b" " or head[28:31] != b" \r\n":
        return None
    try:
        size = int(head[2:10], 16)
        page = int(head[11:19], 16)
        nxt = int(head[20:28], 16)
    except ValueError:
        return None
    out = bytearray()
    pos = addr + 31
    while True:
        take = min(page, size - len(out))
        out += b[pos:pos + take]
        if len(out) >= size or nxt == 0x7FFFFFFF:
            break
        head = b[nxt:nxt + 31]
        if head[:2] != b"\r\n":
            return None
        pos = nxt + 31
        try:
            page = int(head[11:19], 16)
            nxt = int(head[20:28], 16)
        except ValueError:
            return None
    return bytes(out)


def parse_container(b):
    """Elements (name, data) of a v8 container, or None when `b` is not one."""
    if len(b) < 47 or b[16:18] != b"\r\n":
        return None
    toc = _document(b, 16)
    if toc is None or len(toc) % 12 != 0:
        return None
    elements = []
    for i in range(0, len(toc), 12):
        h = int.from_bytes(toc[i:i + 4], "little")
        d = int.from_bytes(toc[i + 4:i + 8], "little")
        if h == 0x7FFFFFFF:
            continue
        header = _document(b, h)
        body = _document(b, d)
        if header is None or body is None or len(header) < 24:
            return None
        name = header[20:-4].decode("utf-16le", "replace")
        elements.append((name, body))
    return elements


def containers_equal(a, b, depth=0):
    if depth > 3:
        return False
    ea, eb = parse_container(a), parse_container(b)
    if ea is None or eb is None or len(ea) != len(eb):
        return False
    for (na, da), (nb, db) in zip(ea, eb):
        if na != nb:
            return False
        if da == db:
            continue
        if not containers_equal(da, db, depth + 1):
            return False
    return True


def classify(l, r):
    if l["raw"] == r["raw"]:
        return "identical_bytes"
    lt, rt = inflate(l["raw"]), inflate(r["raw"])
    if lt == rt:
        return "identical_text"
    strip = lambda t: t.replace(b"\r", b"").replace(b"\n", b"")
    if strip(lt) == strip(rt):
        return "layout_only"
    if containers_equal(lt, rt):
        return "container_headers"
    return "different"


def compare(left, right, names, show=25):
    cls = {}
    for name in sorted(set(left) | set(right)):
        l, r = left.get(name), right.get(name)
        if l is None:
            cls[name] = ("only_right", None)
        elif r is None:
            cls[name] = ("only_left", None)
        else:
            cls[name] = (classify(l, r), None)
    counts = Counter(c for c, _ in cls.values())
    return cls, counts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("left")
    ap.add_argument("right")
    ap.add_argument("--tree")
    ap.add_argument("--json")
    ap.add_argument("--show", type=int, default=30)
    ap.add_argument("--only-different", action="store_true")
    ap.add_argument("--dump-left", help="write the left rows to this folder (<name>__part0.bin) and stop")
    args = ap.parse_args()

    def split(x):
        db, _, table = x.partition(":")
        return db, table or "Config"

    ldb, ltable = split(args.left)
    rdb, rtable = split(args.right)
    left = load(ldb, ltable)
    if args.dump_left:
        dump_dir(left, args.dump_left)
        print("dumped %d rows of %s to %s" % (len(left), args.left, args.dump_left))
        return
    right = load(rdb, rtable)
    names = tree_names(args.tree)
    cls, counts = compare(left, right, names)
    print("left  %s.%s: %d names, %d part rows" % (ldb, ltable, len(left), sum(e["parts"] for e in left.values())))
    print("right %s.%s: %d names, %d part rows" % (rdb, rtable, len(right), sum(e["parts"] for e in right.values())))
    for k in ("identical_bytes", "identical_text", "layout_only", "container_headers", "different", "only_left",
              "only_right"):
        print("  %-18s %6d" % (k, counts.get(k, 0)))
    multi_l = {n: e["parts"] for n, e in left.items() if e["parts"] > 1}
    multi_r = {n: e["parts"] for n, e in right.items() if e["parts"] > 1}
    print("multi-part left:", multi_l or "-", "| right:", multi_r or "-")

    benign = ("identical_bytes", "identical_text", "layout_only", "container_headers")
    diffs = [(n, c) for n, (c, _) in cls.items() if c not in benign]
    by_kind = defaultdict(list)
    for n, c in diffs:
        by_kind[c].append(n)
    report = {"left": args.left, "right": args.right, "counts": dict(counts), "rows": {}}
    for c in ("different", "only_left", "only_right"):
        items = by_kind.get(c, [])
        if not items:
            continue
        print("\n== %s (%d)" % (c, len(items)))
        kinds = Counter(describe(n, names).split(".")[0] + ("" if "." not in n else "." + n.split(".", 1)[1].split("_")[0])
                        for n in items)
        print("  by kind:", dict(kinds.most_common(12)))
        for n in items[: args.show]:
            l, r = left.get(n), right.get(n)
            line = "  %s  %s" % (n, describe(n, names))
            if l and r:
                ls = inflate(l["raw"]).replace(b"\r", b"").replace(b"\n", b"")
                rs = inflate(r["raw"]).replace(b"\r", b"").replace(b"\n", b"")
                off = first_diff(ls, rs)
                line += "\n      @%d (newlines stripped)  L: %s\n             R: %s" % (
                    off, context(ls, off), context(rs, off))
            print(line)
        if len(items) > args.show:
            print("  ... %d more" % (len(items) - args.show))
    for n, (c, _) in cls.items():
        l, r = left.get(n), right.get(n)
        report["rows"][n] = {
            "class": c,
            "what": describe(n, names),
            "left": None if l is None else {"parts": l["parts"], "bytes": len(l["raw"]), "text_bytes": None},
            "right": None if r is None else {"parts": r["parts"], "bytes": len(r["raw"]), "text_bytes": None},
        }
    if args.json:
        with open(args.json, "w", encoding="utf-8") as f:
            json.dump(report, f, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
