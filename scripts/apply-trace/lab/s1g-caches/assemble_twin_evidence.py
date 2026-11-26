"""Assembles the evidence file of the twin runs of S1-F (docs/apply/evidence/new-object/s1f-twin-compare.txt) from the
out files of twin_run.ps1 / twin_extra.ps1 / twin_inject.ps1 / reg_cmp.py.

usage: python assemble_twin_evidence.py <out-file>
The lab folder is F:\\ibcmd\\lab\\05\\s1g (store\\out, logs, out).
"""
import json
import os
import re
import sys

LAB = r"F:\ibcmd\lab\05\s1g"
STORE_OUT = os.path.join(LAB, "store", "out")
OUT = os.path.join(LAB, "out")

# label of the twin_run out folder, the twin_extra label, the case, what it creates
CASES = [
    ("fn1", "fn1", "N1", "a flat catalog with attributes of every kind"),
    ("fn2", "fn2", "N2", "a hierarchical catalog with a tabular section"),
    ("fn3", "fn3", "N3", "a new catalog listed BEFORE a changed catalog (which gains an attribute)"),
    ("fn4", "fn4", "N4", "a document with a periodic numeric number and a tabular section"),
    ("fn5", "fn5", "N5", "a catalog and a document in one stage"),
    ("fn6", "fn6", "N6", "a new catalog listed AFTER a changed catalog (which gains an attribute)"),
    ("fn7", "fn7", "N7", "a new document beside a changed catalog (which gains an attribute)"),
    ("mn3", "mn3", "N3 again", "on the merge with feat/0.4 999e8fd5 (S1-E sections, S1 on 8.5): the binary of the merge commit, a fresh own twin"),
    ("mn5", "mn5", "N5 again", "on the merge with feat/0.4 999e8fd5, a fresh own twin"),
    ("mn6", "mn6", "N6 again", "on the merge with feat/0.4 999e8fd5, a fresh own twin"),
]


def read(path, limit=None):
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8-sig", errors="replace") as f:
        lines = f.read().splitlines()
    return lines if limit is None else lines[:limit]


def section(out, title, lines):
    out.append("-- " + title)
    if lines is None:
        out.append("   (not run)")
    else:
        out.extend("   " + line for line in lines)


def main():
    target = sys.argv[1]
    out = ["S1-F (#402): the twin protocol of docs/apply/restructuring.md 12.6, ours (mssql-config-apply --allow-restructure s1) against the",
           "native apply, on twins of one staged state each (base: a database that already had a native apply). Lab: F:\\ibcmd\\lab\\05\\s1g.",
           "Register: reg_cmp.py (the random _IDRRef keys left out; the _MessageNo of objects the stage does not touch is the documented drift).",
           ""]
    for run, extra, name, what in CASES:
        out.append("=" * 100)
        out.append("%s (%s): %s" % (name, run, what))
        out.append("=" * 100)
        log = read(os.path.join(LAB, "logs", "twin_run_%s.log" % run))
        if log is None:
            out.append("(no run)")
            continue
        keep = [line for line in log if re.match(r"^(dry exit|rehearse exit|real exit|structure:)", line)]
        section(out, "runs", keep)
        section(out, "10. a rehearsal changes nothing", [line for line in log if line.startswith(("added", "removed", "changed", "(0)"))][:4])
        d = os.path.join(STORE_OUT, "%s_twin" % run)
        diff = read(os.path.join(d, "diff.txt"))
        section(out, "2. tables, columns, indexes (nat -> own)", diff and [l for l in diff if l.startswith(("added", "removed", "changed", "  T ", "     "))])
        section(out, "3. data of the created tables (EXCEPT both ways)", read(os.path.join(d, "except.txt")))
        section(out, "4. Config rows (only in A / only in B)", read(os.path.join(d, "config.txt")))
        section(out, "5. DBSchema entries and DBNames", read(os.path.join(d, "dbschema.txt")))
        si = read(os.path.join(d, "si.txt"))
        section(out, "6. the .si rows", si and [l for l in si if l.startswith(("--", "15 of", "16 of"))])
        section(out, "register (ConfigChngR, ExtProps)", read(os.path.join(STORE_OUT, "%s_reg.txt" % run)))
        e = os.path.join(OUT, "extra_%s" % extra)
        noop = read(os.path.join(e, "native_noop.out")) or read(os.path.join(e, "native_noop"))
        section(out, "7. a native config apply on our twin", noop and noop[-1:])
        nat, own = read(os.path.join(e, "session_nat.txt")), read(os.path.join(e, "session_own.txt"))
        if nat is None or own is None:
            section(out, "9. a cluster session on both twins", None)
        else:
            section(out, "9. a cluster session on both twins", ["outputs identical: %s (%d lines)" % (nat == own, len(own))] + own)
        exp = read(os.path.join(e, "export_diff.json"))
        if exp:
            try:
                summary = json.loads("\n".join(exp)).get("summary")
                section(out, "8. native export of both, source-diff", [json.dumps(summary, ensure_ascii=False)])
            except Exception:
                section(out, "8. native export of both, source-diff", exp[:5])
        inj = read(os.path.join(STORE_OUT, "run_%s" % run, "inject_failure.txt"))
        section(out, "12. a failure before COMMIT takes everything back", inj)
        out.append("")
    out.append("=" * 100)
    out.append("N3 through the drop-in: ibcmd-rs infobase config apply --recovery-backup=<file> (the same staged state as the twins of N3)")
    out.append("=" * 100)
    d = os.path.join(STORE_OUT, "dn3_twin")
    section(out, "the run", read(os.path.join(OUT, "dropin_n3", "out.txt")))
    diff = read(os.path.join(d, "diff.txt"))
    section(out, "2. tables, columns, indexes (nat -> own)", diff and [l for l in diff if l.startswith(("added", "removed", "changed", "  T ", "     "))])
    section(out, "3. data of the created and the rebuilt table", read(os.path.join(d, "except.txt")))
    section(out, "4. Config rows (only in A / only in B)", read(os.path.join(d, "config.txt")))
    section(out, "5. DBSchema entries and DBNames", read(os.path.join(d, "dbschema.txt")))
    si = read(os.path.join(d, "si.txt"))
    section(out, "6. the .si rows", si and [l for l in si if l.startswith(("--", "15 of", "16 of"))])
    section(out, "register (ConfigChngR, ExtProps)", read(os.path.join(STORE_OUT, "dn3_reg.txt")))
    out.append("")
    out.append("=" * 100)
    out.append("11. refusals inside a stage that creates an object (dry runs on the first tries of N3, S1 gate, mssql-config-apply)")
    out.append("=" * 100)
    out.append("   changed catalog _ДемоПартнеры (an extension adopts it):")
    out.append("     S1: catalog _ДемоПартнеры is adopted by the extension _ДемоРасширение (... the extension keeps no table of its own for it: the own restructure does not change an object an extension adopts, the platform's apply does")
    out.append("   changed catalog _ДемоКассы (subordinate to owners):")
    out.append("     S1: catalog _ДемоКассы is subordinate to owners: its owner field is not covered")
    out.append("   unit tests: restructure::tests_create (gate: a file that is no module or help page, a new object the plan does not create,")
    out.append("   two objects of one kind), restructure::tests_s1 (the two decoders disagree), restructure::tests_entry (the entry builder's refusals).")
    out.append("")
    with open(target, "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")
    print("wrote", target, len(out), "lines")


main()
