"""Byte-exact round trip parse -> dumps of DBSchema and DBNames (the platform's brace layout).
usage: python rt_check.py [snapshot-label] [database]"""
import sys
import zlib

import bracefmt as bf
import lab

label = sys.argv[1] if len(sys.argv) > 1 else "a2_after"
db = sys.argv[2] if len(sys.argv) > 2 else lab.DB

raw = lab.dbschema(label, db)
txt = raw.decode("utf-8")
assert txt.startswith("﻿")
out = "﻿" + bf.dumps(bf.parse(raw))
print("DBSchema roundtrip equal:", out == txt, len(out), len(txt))

b = lab.row(label, "Params", "DBNames", db=db)
t = zlib.decompress(b, -15).decode("utf-8")
o2 = ("﻿" if t.startswith("﻿") else "") + bf.dumps(bf.parse(t))
print("DBNames roundtrip equal:", o2 == t, len(o2), len(t))
