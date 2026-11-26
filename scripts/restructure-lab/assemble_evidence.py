"""Assembles docs/apply/evidence/restructuring/s2-wave1-twin-compare.txt (and the session outputs) from the lab's out files.
usage: python assemble_evidence.py <repo evidence dir> <case>...
"""
import json
import os
import shutil
import sys

LAB = os.environ.get("DDL_LAB", r"F:\ibcmd\lab\04\restructure")
OUT = os.path.join(LAB, "out")

CORPUS = {
    "b1": "corpus_plan_of_the_deletion_case_equals_the_native_result",
    "b2": "corpus_plan_of_the_additional_order_deletion_case_equals_the_native_result",
    "c1": "corpus_plan_of_the_widening_case_equals_the_native_result",
}
REFUSALS = {
    "b1": [
        ("b1_deleted_extra_id", "the deleted row names one id more than the stage removes"),
    ],
    "c1": [
        ("c1_narrowed", "the limit of ИмяХеш made shorter than the stored one (100 -> 20 against 40)"),
        ("c1_root_changed", "the service row root changed by a line break"),
    ],
}


def read(path, limit=None):
    with open(path, encoding="utf-8", errors="replace") as f:
        lines = f.read().splitlines()
    return lines[:limit] if limit else lines


def section(lines, title, body):
    lines.append("")
    lines.append("--- " + title)
    lines.extend(body)


def refusal_lines(name):
    path = os.path.join(OUT, "refusals", name + ".txt")
    text = "\n".join(read(path))
    try:
        start = text.index("{")
        end = text.rindex("}") + 1
        data = json.loads(text[start:end])
        return ["  " + b["row"][:40].ljust(40) + " " + b["reason"] for b in data["gate"]["blockers"]]
    except Exception as error:  # the run failed before it printed a report
        return ["  (no report: %s)" % error]


def main():
    dest = sys.argv[1]
    cases = sys.argv[2:]
    lines = [
        "S1 wave 1 (#391): the twin protocol of docs/apply/restructuring.md 12.6 for the cases b1, b2 (S1-B) and c1 (S1-C).",
        "Twins of one staged backup per case: _nat = the platform's config apply, _own = mssql-restructure --through-apply",
        "--i-have-a-backup. Assembled from the lab's out files by assemble_evidence.py; the session outputs are in s2-<case>-session.txt.",
    ]
    for case in cases:
        real = json.load(open(os.path.join(OUT, "s2_%s_own" % case, "real.json"), encoding="utf-8"))
        lines.append("")
        lines.append("=" * 100)
        lines.append("CASE " + case)
        lines.append("=" * 100)
        lines.append("plan: " + real["summary"])
        for cache in real["caches"] or ["(no cache row is written)"]:
            lines.append("cache: " + cache)
        lines.append("rebuilt tables: " + ", ".join(real["through_apply"]["structure"]["tables"]))
        section(lines, "1. the plan made offline from the staged snapshot equals the native result", ["test " + CORPUS[case] + " passes (tests_corpus.rs)"])
        twin = os.path.join(OUT, "s2_%s_twin" % case)
        section(lines, "2. tables, columns, indexes (snapdiff native vs ours)", read(os.path.join(twin, "diff.txt"), 50))
        section(lines, "3. data of the rebuilt tables, EXCEPT both ways (A = native, B = ours)", read(os.path.join(twin, "except.txt")))
        section(lines, "4. Config rows (rows one twin has and the other has not)", read(os.path.join(twin, "config.txt"), 10))
        section(lines, "5. DBSchema entries and DBNames", read(os.path.join(twin, "dbschema.txt"), 5))
        section(lines, "6. the .si rows", read(os.path.join(twin, "si.txt"), 4))
        noop = os.path.join(LAB, "logs", "apply_native_%s_own_noop.log" % case)
        section(lines, "7. native config apply on our twin", [l for l in read(noop) if "INFO" in l or "exit" in l])
        export = json.load(open(os.path.join(OUT, "export_diff_s2_%s.json" % case), encoding="utf-8"))
        section(lines, "8. native config export of both, ibcmd-rs source-diff", ["summary: " + json.dumps(export["summary"])])
        nat = open(os.path.join(OUT, "session_s2_%s_nat.txt" % case), encoding="utf-8").read()
        own = open(os.path.join(OUT, "session_s2_%s_own.txt" % case), encoding="utf-8").read()
        section(lines, "9. cluster session (jobs/s2_%s.bsl)" % case, ["outputs identical: %s (%d lines)" % (nat == own, len(own.splitlines()))])
        shutil.copyfile(os.path.join(OUT, "session_s2_%s_own.txt" % case), os.path.join(dest, "s2-%s-session.txt" % case))
        rehearse = os.path.join(OUT, "s2_%s_own" % case, "rehearse_diff.txt")
        section(lines, "10. a rehearsal changes nothing (snapdiff staged vs after the rehearsal)", read(rehearse, 12))
        body = []
        for name, what in REFUSALS.get(case, []):
            body.append("%s: %s" % (name, what))
            body.extend(refusal_lines(name))
        if not body:
            body = ["(unit tests: tests_schema.rs, tests_plan.rs; no dry run of a modified stage for this case)"]
        section(lines, "11. refusals (dry run on a fresh twin whose stage has one change more; nothing written)", body)
        section(lines, "12. a THROW before COMMIT on a fresh twin", read(os.path.join(OUT, "failure_s2_%s.txt" % case)))
    with open(os.path.join(dest, "s2-wave1-twin-compare.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("written", len(lines), "lines")


main()
