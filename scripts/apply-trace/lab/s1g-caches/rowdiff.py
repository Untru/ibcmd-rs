"""Which cache rows differ between two ddl-lab snapshots (inflated)."""
import json
import os
import sys
import zlib

ROOTS = {
    "ddl": r"F:\ibcmd\lab\04\restructure",
    "trace": r"F:\ibcmd\lab\05\s1g\store",
}
ROWS = [
    "0b698dcd", "1a621f0f", "215d232c", "2203278d", "42ed49cc", "59274b8d", "a07b62f0", "c40aafd6",
    "c4629235", "c77bc206", "cf8b5e0f", "e05c0074", "ea13a2c9", "facbfffe", "fd1b2a86", "fe8acd6a",
]


def load(root, db, label):
    base = os.path.join(ROOTS[root], "snap", db, label)
    svc = json.load(open(os.path.join(base, "svc.json"), encoding="utf-8"))
    out = {}
    for table in ("Params", "Config"):
        parts = {}
        for row in svc[table]:
            parts.setdefault(row["name"], {})[row["part"]] = row["sha"]
        out[table] = parts
    return ROOTS[root], out


def get(snap, table, name):
    root, data = snap
    parts = data[table].get(name)
    if parts is None:
        return None
    b = b"".join(open(os.path.join(root, "blobs", parts[k]), "rb").read() for k in sorted(parts))
    try:
        return zlib.decompress(b, -15)
    except Exception:
        return b


def row_full(short, snap):
    root, data = snap
    for name in data["Params"]:
        if name.startswith(short):
            return name
    return None


if __name__ == "__main__":
    a = load(*sys.argv[1].split(":"))
    b = load(*sys.argv[2].split(":"))
    for short in ROWS:
        na = row_full(short, a)
        ta = get(a, "Params", na)
        tb = get(b, "Params", row_full(short, b))
        if ta == tb:
            print(short, "same", len(ta))
        else:
            print(short, "DIFF", len(ta) if ta else None, len(tb) if tb else None)
