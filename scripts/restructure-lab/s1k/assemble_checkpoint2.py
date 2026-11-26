"""Assembles the record of checkpoint 2 (out/<case>/...) into one text: a table per case, then the raw lines of each check.

usage: python assemble_checkpoint2.py <lab dir> "<title>" [case,case,...] > docs/apply/evidence/restructuring/s1k-checkpoint2.txt
"""
import json
import os
import re
import sys

lab, title = sys.argv[1], sys.argv[2]
order = (sys.argv[3] if len(sys.argv) > 3 else "a1,b1,b2,c1,d0,d1,e5,e6,i1").split(",")
plain = re.compile(r"\x1b\[[0-9;]*m")


def read(path):
    try:
        with open(path, encoding="utf-8-sig", errors="replace") as f:
            return plain.sub("", f.read())
    except OSError:
        return ""


def cut(line, width=300):
    return line if len(line) <= width else line[:width] + " ..."


def first(text, pattern):
    match = re.search(pattern, text, re.M)
    return match.group(0).strip() if match else ""


def grab(text, pattern, group=1, default="?"):
    match = re.search(pattern, text, re.M)
    return match.group(group) if match else default


def load_json(path):
    try:
        return json.loads(read(path))
    except ValueError:
        return {}


print(title)
rows = []
for case in order:
    out = os.path.join(lab, "out", case)
    log = read(os.path.join(out, "checkpoint2.log")) + read(os.path.join(out, "phase1.log"))
    phase = load_json(os.path.join(out, "phase1.json"))
    record = load_json(os.path.join(out, "record.json"))
    real = load_json(os.path.join(out, "real.json"))
    apply = real.get("apply", real)
    structure = apply.get("structure") or {}
    check3 = read(os.path.join(out, "check3.txt"))
    check4 = read(os.path.join(out, "check4.txt"))
    check56 = read(os.path.join(out, "check56.txt"))
    check8 = read(os.path.join(out, "check8.txt"))
    noop = read(os.path.join(out, "native_noop.txt"))
    c11 = read(os.path.join(out, "check11.txt"))
    c12 = read(os.path.join(out, "check12.txt"))
    rehearse = read(os.path.join(out, "rehearse_diff.txt"))
    session_nat, session_own = read(os.path.join(out, "session_nat.txt")), read(os.path.join(out, "session_own.txt"))
    row = {
        "case": case,
        "import": "exit %s, %ss" % (phase.get("import_exit", "?"), phase.get("import_seconds", "?")),
        "staged": "%s rows, %s edited staged, %s others differ" % (
            record.get("configsave_rows", "?"), record.get("edited_staged", "?"), record.get("other_rows_differing_from_config", "?")),
        "apply": ("ok, " + "; ".join(structure.get("objects", []))[:80]) if structure else ("refused" if c11 else "-"),
        "3": first(check3, r"checked \d+ tables.*") or "-",
        "4": first(check4, r"4\. common rows.*") or "-",
        "5-6": " | ".join(line.strip() for line in check56.splitlines() if re.match(r"[56]\. ", line) and "DBSchema entries" not in line) or "-",
        "7": "not required" if "не требуется" in noop else ("? " + first(noop, r"exit.*")) if noop else "-",
        "8": first(check8, r"8\. native export of both.*") or "-",
        "9": ("identical (%d lines)" % len(session_own.splitlines())) if session_nat and session_nat == session_own else ("different" if session_nat else "-"),
        "10": "no change" if re.search(r"^changed \(0\)|^\(0\)", rehearse, re.M) else ("?" if rehearse else "-"),
        "11": first(c11, r"check 11:.*") or "-",
        "12": first(c12, r"digest unchanged: \w+") or "-",
    }
    rows.append(row)

print("\n== summary")
for row in rows:
    print("\n-- %s" % row["case"])
    for key in ("import", "staged", "apply", "3", "4", "5-6", "7", "8", "9", "10", "11", "12"):
        print("   %-7s %s" % (key, cut(row[key], 400)))

for case in order:
    out = os.path.join(lab, "out", case)
    print("\n" + "=" * 78)
    print("== case %s: the raw lines" % case)
    for name, pattern in (
        ("files.txt", None), ("import.out", r"^\[|import|Загрузка|error|Error"), ("check3.txt", None), ("check4.txt", None),
        ("check_register.txt", None), ("check56.txt", None), ("check8.txt", None), ("check11.txt", None), ("check12.txt", None),
    ):
        text = read(os.path.join(out, name))
        if not text.strip():
            continue
        print("-- " + name)
        for line in text.splitlines()[:40]:
            if pattern is None or re.search(pattern, line):
                if not line.startswith("model export"):
                    print("   " + cut(line))
