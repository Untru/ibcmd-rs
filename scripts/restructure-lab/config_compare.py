"""The `Config` rows of two lab databases compared by bytes and after inflate (the check 4 of docs/apply/restructuring.md 12.6, without the
compression as a difference): a row whose stored bytes differ but whose raw-deflate text is the same is a compression difference, not a
content one.

usage: python config_compare.py <db A> <db B> [--list N]
Prints the counts (equal bytes, equal after inflate, different, only in A, only in B) and the differing rows by kind (the suffix of the name).
"""
import collections
import hashlib
import sys
import zlib

from db import connect


def digests(db):
    """{(FileName, PartNo): (sha of bytes, sha of inflated bytes or of bytes when not deflated)}"""
    out = {}
    with connect(db) as c:
        cur = c.cursor()
        cur.execute("SELECT FileName, PartNo, BinaryData FROM dbo.Config")
        while True:
            batch = cur.fetchmany(200)
            if not batch:
                break
            for name, part, data in batch:
                raw = bytes(data) if data is not None else b""
                try:
                    inflated = zlib.decompress(raw, -15)
                except zlib.error:
                    inflated = raw
                out[(name, part)] = (hashlib.sha1(raw).digest(), hashlib.sha1(inflated).digest())
    return out


def kind(name):
    if "." in name:
        return name.rsplit(".", 1)[1]
    return "uuid" if len(name) == 36 else name


def main():
    a_db, b_db = sys.argv[1], sys.argv[2]
    listed = int(sys.argv[sys.argv.index("--list") + 1]) if "--list" in sys.argv else 0
    a, b = digests(a_db), digests(b_db)
    equal = same_text = 0
    different = []
    for key, (raw, text) in a.items():
        other = b.get(key)
        if other is None:
            continue
        if other[0] == raw:
            equal += 1
        elif other[1] == text:
            same_text += 1
        else:
            different.append(key)
    only_a = sorted(set(a) - set(b))
    only_b = sorted(set(b) - set(a))
    print("rows: %d in A, %d in B" % (len(a), len(b)))
    print("equal bytes: %d; equal after inflate only: %d; different text: %d; only in A: %d; only in B: %d"
          % (equal, same_text, len(different), len(only_a), len(only_b)))
    by_kind = collections.Counter(kind(name) for name, _ in different)
    print("different text by kind:", dict(by_kind.most_common(12)))
    for key in different[:listed]:
        print("  different:", key)
    for key in only_a[:listed]:
        print("  only in A:", key)
    for key in only_b[:listed]:
        print("  only in B:", key)


main()
