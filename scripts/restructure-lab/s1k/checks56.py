"""Checks 5 and 6 of docs/apply/restructuring.md 12.6 between the native twin and ours, from the snapshots.

usage: python checks56.py <native db> <native label> <own db> <own label>

5: the `DBNames` texts equal after inflate; the `DBSchema` entries equal but the tables the platform upgrades on its own
   (`DbCopies`, `DbCopiesUpdates`);
6: the 16 `.si` rows of `Params`: how many have the same text after inflate (si_diff.py prints the differing ones).
"""
import os
import subprocess
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))
import bracefmt as bf  # noqa: E402
import lab  # noqa: E402

nat_db, nat_label, own_db, own_label = sys.argv[1:5]

a = zlib.decompress(lab.row(nat_label, "Params", "DBNames", 0, nat_db), -15)
b = zlib.decompress(lab.row(own_label, "Params", "DBNames", 0, own_db), -15)
print("5. DBNames text equal after inflate: %s (%d / %d bytes)" % (a == b, len(a), len(b)))

# The platform's own system tables that a native apply of a stage with a created object adds on this build (the trace track's drift,
# docs/apply/new-object.md 8: `_DbCopiesInfoBaseUse` and its companions; ours does not), and the two tables it upgrades on its own.
PLATFORM_OWN = {"DbCopies", "DbCopiesUpdates", "DbCopiesInfoBaseUse", "DbCopiesUpdateStat", "DbCopiesUpdateTableStat", "WebSocketClients"}
if a != b:
    def without_platform_names(text):
        lines = text.decode("utf-8-sig").split("\r\n")
        added = ("DbCopiesInfoBaseUse", "DbCopiesUpdateStat", "DbCopiesUpdateTableStat")
        # the entries are appended at the end: the last entry that stays loses its comma when they are taken out
        kept = [line.rstrip(",") for line in lines[2:] if not any(('"%s",' % name) in line for name in added)]
        return kept, len(lines) - 2 - len(kept), lines[:2]
    ka, dropped_a, head_a = without_platform_names(a)
    kb, dropped_b, head_b = without_platform_names(b)
    print("5. DBNames equal after taking out the entries of the platform's own system tables (native %d, own %d taken out): %s; headers native %s own %s"
          % (dropped_a, dropped_b, ka == kb, head_a, head_b))


def tables(db, label):
    d = bf.parse(lab.dbschema(label, db))
    return {str(t[0]): bf.dumps(t) for t in d[1][1:]}


ta, tb = tables(nat_db, nat_label), tables(own_db, own_label)
differ = sorted([k for k in ta if ta[k] != tb.get(k)] + [k for k in tb if k not in ta])
print("5. DBSchema entries: native %d, own %d; differing: %s" % (len(ta), len(tb), differ))
tolerated = PLATFORM_OWN
print("5. DBSchema equal but the platform's own upgrades: %s (%s)" % (set(differ) <= tolerated, ", ".join(differ) or "none"))

result = subprocess.run(
    [sys.executable, "-X", "utf8", os.path.join(os.path.dirname(HERE), "si_diff.py"), nat_db, nat_label, own_db, own_label],
    capture_output=True, text=True, encoding="utf-8", env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})
last = [line for line in result.stdout.splitlines() if line.strip()][-1:]
print("6. " + (last[0] if last else result.stderr[-200:]))
