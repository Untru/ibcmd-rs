"""Inflate two raw-deflate row dumps and print the differing regions.  usage: inflate_diff.py a.bin b.bin"""
import difflib
import sys
import zlib

sys.stdout.reconfigure(encoding="utf-8")
texts = []
for path in sys.argv[1:3]:
    data = open(path, "rb").read()
    plain = zlib.decompress(data, -15)
    texts.append(plain)
    print(path.split(chr(92))[-1][:80], len(data), "->", len(plain), plain[:50])
a, b = texts
sm = difflib.SequenceMatcher(None, a, b, autojunk=False)
for tag, i1, i2, j1, j2 in sm.get_opcodes():
    if tag != "equal":
        print(tag, repr(a[max(0, i1 - 60):i2 + 60]), "=>", repr(b[max(0, j1 - 60):j2 + 60]))
