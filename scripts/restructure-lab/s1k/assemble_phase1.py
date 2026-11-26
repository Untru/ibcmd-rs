"""Assembles the record of the import phase of every case (out/<case>/...) into one text.

usage: python assemble_phase1.py <lab dir> <title> > evidence.txt
"""
import json
import os
import re
import subprocess
import sys

lab, title = sys.argv[1], sys.argv[2]
here = os.path.dirname(os.path.abspath(__file__))
cases = subprocess.run([sys.executable, "-X", "utf8", os.path.join(here, "cases.py"), "list"], capture_output=True, text=True,
                       encoding="utf-8", env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}).stdout.splitlines()
plain = re.compile(r"\x1b\[[0-9;]*m")


def cut(line, width=400):
    return line if len(line) <= width else line[:width] + " ..."


def read(path):
    try:
        with open(path, encoding="utf-8-sig", errors="replace") as f:
            return plain.sub("", f.read())
    except OSError:
        return ""


print(title)
for entry in cases:
    case, operation, state, name = entry.split("\t")
    out = os.path.join(lab, "out", case)
    print("\n" + "=" * 78)
    print("== case %s (operation %s, %s): %s" % (case, operation, state, name))
    if not os.path.exists(os.path.join(out, "record.json")):
        print("   not run (%s)" % ("the operation is not built in the gate yet" if state != "built" else "no record"))
        continue
    print("== edited files:")
    for line in read(os.path.join(out, "files.txt")).split():
        print("     " + line)
    log = read(os.path.join(out, "phase1.log"))
    print("== " + next((l for l in log.splitlines() if l.startswith("import exit")), "import exit ?"))
    print("== import output (the model export's progress lines left out):")
    lines = [l for l in (read(os.path.join(out, "import.out")) + read(os.path.join(out, "import.err"))).splitlines()
             if l.strip() and not l.startswith("model export")]
    for l in lines[:24]:
        print("   " + cut(l))
    if len(lines) > 24:
        print("   ... (%d more lines)" % (len(lines) - 24))
    record = json.loads(read(os.path.join(out, "record.json")))
    print("== what it left in ConfigSave: %d rows; edited objects staged %d of %d; other rows differing from Config %d"
          % (record["configsave_rows"], record["edited_staged"], len(record["edited"]), record["other_rows_differing_from_config"]))
    print("== our apply, dry run over that stage:")
    for l in log.splitlines():
        if re.search(r"apply dry-run exit|nothing_to_apply|\"structure\"", l):
            print("   " + l.strip())
