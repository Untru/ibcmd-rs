"""Gap statistics of the observer journals (baseline stall of the thin client) and the gaps around given commit times.

usage: python poll_gaps.py [--commit label=ISO ...] label [label ...]
"""
import argparse
import statistics
import sys

sys.path.insert(0, r"F:\ibcmd\lab\05\online\tools")
import analyze_obs as a  # noqa: E402

sys.stdout.reconfigure(encoding="utf-8")
ap = argparse.ArgumentParser()
ap.add_argument("--commit", action="append", default=[])
ap.add_argument("labels", nargs="+")
args = ap.parse_args()
commits = []
for spec in args.commit:
    name, iso = spec.split("=", 1)
    commits.append((name, a.parse_iso(iso)))
print("gap = time between two consecutive journal lines of one session (poll or beat)")
for label in args.labels:
    rows = [r for r in a.read_journal(label) if r["event"] in ("poll", "beat")]
    if len(rows) < 3:
        continue
    gaps = [((b["t"] - x["t"]).total_seconds(), x["t"]) for x, b in zip(rows, rows[1:])]
    vals = sorted(g for g, _ in gaps)
    print("%-10s lines %5d  median %.2f  p95 %.2f  p99 %.2f  max %.2f s" % (
        label, len(rows), statistics.median(vals), vals[int(len(vals) * .95)], vals[int(len(vals) * .99)], vals[-1]))
    for name, c in commits:
        near = [(g, t) for g, t in gaps if abs((t - c).total_seconds()) <= 30]
        if near:
            mx = max(near)
            print("    within 30 s of %s: %d intervals, max %.2f s at %+.1f s" % (name, len(near), mx[0], (mx[1] - c).total_seconds()))
