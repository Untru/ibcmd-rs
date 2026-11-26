"""Summarises the observer journals of a run against an anchor time.

usage: python analyze_obs.py --t0 2026-09-29T17:08:00.000Z [--gap 2.5] label [label ...]

Journal line: <ms since 0001-01-01 UTC>|label|event|session|client marker|server marker|details
Prints, per journal: the marker segments (same client+server pair), the gaps between lines longer than --gap
seconds, error lines and every non-poll event, all with seconds relative to --t0.
"""
import argparse
import datetime as dt
import pathlib
import re
import sys
import time

OBS = pathlib.Path(r"F:\ibcmd\lab\05\online\obs")
EPOCH = dt.datetime(1, 1, 1)


def to_dt(ms):
    return EPOCH + dt.timedelta(milliseconds=int(ms))


def parse_iso(text):
    text = text.strip().rstrip("Z")
    return dt.datetime.fromisoformat(text)


def read_journal(label):
    rows = []
    path = OBS / (label + ".log")
    text = None
    for attempt in range(20):
        try:
            text = path.read_bytes().decode("utf-8-sig", errors="replace")
            break
        except PermissionError:
            time.sleep(0.2)
    if text is None:
        raise PermissionError(str(path))
    for line in text.splitlines():
        line = line.lstrip("\ufeff")
        parts = line.split("|", 6)
        if len(parts) < 7 or not parts[0].isdigit():
            continue
        rows.append({
            "t": to_dt(parts[0]), "label": parts[1], "event": parts[2], "session": parts[3],
            "client": parts[4], "server": parts[5], "details": parts[6],
        })
    return rows


def fmt(t, t0):
    rel = (t - t0).total_seconds()
    return "%s (%+8.2f s)" % (t.strftime("%H:%M:%S.%f")[:-3], rel)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--t0", required=True)
    ap.add_argument("--gap", type=float, default=2.5)
    ap.add_argument("labels", nargs="+")
    args = ap.parse_args()
    t0 = parse_iso(args.t0)
    for label in args.labels:
        rows = read_journal(label)
        print("=== %s: %d lines, session %s" % (label, len(rows), rows[0]["session"] if rows else "?"))
        if not rows:
            continue
        print("  first %s" % fmt(rows[0]["t"], t0))
        print("  last  %s" % fmt(rows[-1]["t"], t0))
        # non-poll events
        for r in rows:
            if r["event"] not in ("poll", "beat"):
                print("  event %-14s %s | %s | %s" % (r["event"], fmt(r["t"], t0), r["session"], r["details"][:230]))
        # segments of markers
        seg = None
        segments = []
        for r in rows:
            if r["event"] not in ("poll",):
                continue
            m = re.search(r"B=(\S+)", r["details"])
            key = (r["client"], r["server"], m.group(1) if m else "-")
            if seg and seg["key"] == key:
                seg["last"] = r["t"]
                seg["n"] += 1
            else:
                seg = {"key": key, "first": r["t"], "last": r["t"], "n": 1}
                segments.append(seg)
        for s in segments:
            print("  markers client=%-22s server=%-22s B=%-40s %s .. %s  n=%d" % (s["key"][0], s["key"][1], s["key"][2], fmt(s["first"], t0), fmt(s["last"], t0), s["n"]))
        # lazy heartbeat
        beats = [r for r in rows if r["event"] == "beat"]
        if beats:
            print("  beats n=%d %s .. %s" % (len(beats), fmt(beats[0]["t"], t0), fmt(beats[-1]["t"], t0)))
        # gaps
        for a, b in zip(rows, rows[1:]):
            d = (b["t"] - a["t"]).total_seconds()
            if d > args.gap:
                print("  gap %6.2f s between %s and %s (%s -> %s)" % (d, fmt(a["t"], t0), fmt(b["t"], t0), a["event"], b["event"]))
        # errors
        for r in rows:
            if "ОШИБКА" in (r["client"] + r["server"] + r["details"]) or "ERR" in r["details"][:6]:
                print("  ERROR %s | %s | %s" % (fmt(r["t"], t0), r["event"], r["details"][:300]))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
