"""Assembles an evidence file from the out files of twin_run.ps1 (out/run_<case>/, out/<case>_twin/) and the failure / refusal
files that were made by hand.

usage: python assemble_evidence_run.py <evidence file> <title> <case>...
       (per case: out/failure_s2_<case>.txt, out/refusals_<case>/*.json and out/refusals_<case>.txt are used when they exist)
The cluster session output of the own twin is copied next to the evidence file as s2-<case>-session.txt.
"""
import glob
import json
import os
import shutil
import sys

LAB = os.environ.get("DDL_LAB", r"F:\ibcmd\lab\04\restructure")
OUT = os.path.join(LAB, "out")


def read(path, limit=None):
    with open(path, encoding="utf-8", errors="replace") as f:
        lines = f.read().splitlines()
    return lines[:limit] if limit else lines


def section(lines, title, body):
    lines.append("")
    lines.append("--- " + title)
    lines.extend(body)


def main():
    dest, title, cases = sys.argv[1], sys.argv[2], sys.argv[3:]
    lines = [title]
    for case in cases:
        run = os.path.join(OUT, "run_" + case)
        twin = os.path.join(OUT, case + "_twin")
        real = json.load(open(os.path.join(run, "real.json"), encoding="utf-8"))
        lines += ["", "=" * 100, "CASE " + case, "=" * 100]
        lines.append("plan: " + " | ".join(real["structure"]["objects"]))
        lines.append("rebuilt tables: " + ", ".join(real["structure"]["tables"]))
        lines.append("caches: " + (", ".join(real["structure"]["caches"]) or "(none written)"))
        section(lines, "2. tables, columns, indexes (snapdiff native vs ours)", read(os.path.join(twin, "diff.txt"), 50))
        section(lines, "3. data of the rebuilt tables, EXCEPT both ways (A = native, B = ours)", read(os.path.join(twin, "except.txt")))
        section(lines, "4. Config rows (rows one twin has and the other has not)", read(os.path.join(twin, "config.txt"), 10))
        section(lines, "5. DBSchema entries and DBNames", read(os.path.join(twin, "dbschema.txt"), 5))
        section(lines, "6. the .si rows", read(os.path.join(twin, "si.txt"), 4))
        section(lines, "7. native config apply on our twin", [l for l in read(os.path.join(run, "native_noop.txt")) if "INFO" in l or "exit" in l])
        export = json.load(open(os.path.join(run, "export_diff.json"), encoding="utf-8"))
        section(lines, "8. native export of both, ibcmd-rs source-diff", ["summary: " + json.dumps(export["summary"])])
        nat = open(os.path.join(run, "session_nat.txt"), encoding="utf-8").read()
        own = open(os.path.join(run, "session_own.txt"), encoding="utf-8").read()
        section(lines, "9. cluster session", ["outputs identical: %s (%d lines)" % (nat == own, len(own.splitlines()))])
        shutil.copyfile(os.path.join(run, "session_own.txt"), os.path.join(os.path.dirname(dest), "s2-%s-session.txt" % case))
        section(lines, "10. a rehearsal changes nothing (snapdiff staged vs after the rehearsal)", read(os.path.join(run, "rehearse_diff.txt"), 12))
        refusals = []
        for path in sorted(glob.glob(os.path.join(OUT, "refusals_%s" % case, "*.json"))):
            data = json.load(open(path, encoding="utf-8"))
            refusals.append(os.path.basename(path)[:-5] + ": " + str(data.get("refused")))
            blockers = sorted(data.get("gate", {}).get("blockers", []), key=lambda b: (not b["reason"].startswith("S1"), b["row"] != "root"))
            for blocker in blockers[:3]:
                refusals.append("  %s  %s" % (blocker["row"][:40].ljust(40), blocker["reason"][:230]))
        notes = os.path.join(OUT, "refusals_%s.txt" % case)
        if os.path.exists(notes):
            # refusals_e.ps1: the edits of the staged rows of a fresh twin, each with the S1 blockers of the dry run
            refusals += [line for line in read(notes)[1:] if line.strip()]
        section(lines, "11. refusals (dry run on a fresh twin whose stage has one change more; nothing written)", refusals or ["(unit tests only)"])
        failure = os.path.join(OUT, "failure_s2_%s.txt" % case)
        if os.path.exists(failure):
            section(lines, "12. a THROW before COMMIT on a fresh twin", read(failure))
    with open(dest, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("written", len(lines), "lines")


main()
