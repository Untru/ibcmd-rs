#!/usr/bin/env python3
"""Maintenance of a blob store written by snapshot.ps1.

  python blobstore.py rebuild <store>   rewrite index.tsv from the pack files (after a lost or damaged index; nothing may write meanwhile)
  python blobstore.py merge   <store>   add the pack members missing from index.tsv (safe next to a running snapshot, Windows)
  python blobstore.py verify  <store>   check that every index entry decompresses to its sha256
  python blobstore.py stats   <store>   number of blobs and bytes

A store is a folder with append-only pack-*.bin files (concatenated gzip members) and index.tsv
(`sha256 <TAB> pack <TAB> offset <TAB> gzip length <TAB> raw length`).
"""

import hashlib
import os
import sys
import zlib


def iter_members(path):
    """(offset, gzip length, raw bytes) of every gzip member of a pack file; linear time."""
    size = os.path.getsize(path)
    with open(path, "rb") as fh:
        pos = 0
        while pos < size:
            fh.seek(pos)
            d = zlib.decompressobj(31)
            out = bytearray()
            consumed = 0
            while not d.eof:
                chunk = fh.read(65536)
                if not chunk:
                    raise ValueError(f"{path}: truncated gzip member at {pos}")
                out += d.decompress(chunk)
                consumed += len(chunk)
            consumed -= len(d.unused_data)
            yield pos, consumed, bytes(out)
            pos += consumed


def scan_packs(store):
    packs = sorted(f for f in os.listdir(store) if f.startswith("pack-") and f.endswith(".bin"))
    lines = {}
    for pack in packs:
        try:
            for off, length, raw in iter_members(os.path.join(store, pack)):
                sha = hashlib.sha256(raw).hexdigest()
                lines[sha] = f"{sha}\t{pack}\t{off}\t{length}\t{len(raw)}\n"
        except ValueError as e:
            print("warning:", e, file=sys.stderr)
    return packs, lines


def _index_mutex_name(store):
    """The name snapshot.ps1 (BlobPack) uses to serialize index appends: FNV-1a over the lower-case path."""
    h = 2166136261
    for ch in store.lower():
        h = ((h ^ ord(ch)) * 16777619) & 0xFFFFFFFF
    return "Global\\ibcmd_rs_blobstore_%08x" % h


def merge(store):
    """Append the pack members that are missing from index.tsv, safely next to a running snapshot."""
    import ctypes
    store = os.path.abspath(store)
    packs, found = scan_packs(store)
    index = os.path.join(store, "index.tsv")
    known = set(read_index(store)) if os.path.exists(index) else set()
    missing = [ln for sha, ln in found.items() if sha not in known]
    kernel32 = ctypes.windll.kernel32
    kernel32.CreateMutexW.restype = ctypes.c_void_p
    kernel32.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    kernel32.ReleaseMutex.argtypes = [ctypes.c_void_p]
    handle = kernel32.CreateMutexW(None, False, _index_mutex_name(store))
    kernel32.WaitForSingleObject(handle, 60000)
    try:
        with open(index, "ab") as fh:
            fh.write("".join(missing).encode("utf-8"))
    finally:
        kernel32.ReleaseMutex(handle)
    print(f"{len(found)} blobs in {len(packs)} packs, {len(known)} were indexed, {len(missing)} added")


def rebuild(store):
    packs, lines = scan_packs(store)
    index = os.path.join(store, "index.tsv")
    if os.path.exists(index):
        os.replace(index, index + ".bak")
    with open(index, "w", encoding="utf-8", newline="\n") as fh:
        fh.writelines(lines.values())
    print(f"rebuilt {len(lines)} entries from {len(packs)} packs")


def read_index(store):
    entries = {}
    with open(os.path.join(store, "index.tsv"), encoding="utf-8") as fh:
        for line in fh:
            p = line.rstrip("\n").split("\t")
            if len(p) >= 5 and len(p[0]) == 64:
                entries[p[0]] = (p[1], int(p[2]), int(p[3]), int(p[4]))
    return entries


def verify(store):
    bad = 0
    entries = read_index(store)
    for sha, (pack, off, length, raw_len) in entries.items():
        with open(os.path.join(store, pack), "rb") as fh:
            fh.seek(off)
            raw = zlib.decompress(fh.read(length), 31)
        if hashlib.sha256(raw).hexdigest() != sha or len(raw) != raw_len:
            bad += 1
            print("bad entry", sha)
    print(f"{len(entries)} entries, {bad} bad")
    return 1 if bad else 0


def stats(store):
    entries = read_index(store)
    print(f"{len(entries)} blobs, {sum(e[3] for e in entries.values())} raw bytes, {sum(e[2] for e in entries.values())} stored bytes")


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in ("rebuild", "merge", "verify", "stats"):
        raise SystemExit(__doc__)
    cmd, store = sys.argv[1], sys.argv[2]
    if cmd == "rebuild":
        rebuild(store)
    elif cmd == "merge":
        merge(store)
    elif cmd == "verify":
        sys.exit(verify(store))
    else:
        stats(store)
