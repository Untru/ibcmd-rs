"""The evidence file of the S1 combination matrix (docs/apply/restructuring.md 12.15): the summaries mix_case.ps1 and mix_ours.ps1 wrote, in the
order of the matrix, long lines cut.

usage: python assemble_mix_evidence.py <out dir of the lab> <evidence file>
       (the out dir is F:\\ibcmd\\lab\\04\\restructure\\out; a case that ran twice keeps its first pass in pass1_<case>)
"""
import os
import sys

MUST_WORK = ["m1", "m2", "m3", "m4", "m5", "m6", "m7", "f1", "f3"]
REFUSED = ["r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8"]
CUT = 700


def read(path):
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8-sig") as f:
        return [line.rstrip("\n") for line in f]


def cut(line):
    return line if len(line) <= CUT else line[:CUT] + " ..."


def block(title, lines):
    out = ["-" * 100, title, "-" * 100]
    out += [cut(line) for line in lines]
    out.append("")
    return out


def main():
    out_dir, target = sys.argv[1], sys.argv[2]
    text = [
        "S1 combination matrix (issue #391, docs/apply/restructuring.md 12.15): the stages of edit_cases_s4.py through the drop-in route",
        "`ibcmd-rs infobase config apply --recovery-backup=<file>`, staged with the platform's partial import (mix_case.ps1) and, for the cases",
        "that must work, with OUR import of the full tree as well (mix_case.ps1 -Ours, mix_ours.ps1). Summaries as the scripts wrote them.",
        "",
    ]
    for case in MUST_WORK + REFUSED:
        first = read(os.path.join(out_dir, "pass1_%s" % case, "summary.txt"))
        main_summary = read(os.path.join(out_dir, "mix_%s" % case, "summary.txt"))
        ours = read(os.path.join(out_dir, "ours_%s" % case, "summary.txt"))
        text.append("=" * 100)
        text.append("CASE %s" % case)
        text.append("=" * 100)
        if first:
            text += block("%s: the platform's route, first pass (the binary before the merge of feat/0.4 029b4a2b)" % case, first)
        if main_summary:
            text += block("%s: mix_case.ps1 (%s)" % (case, "second pass" if first else "the platform's route"), main_summary)
        if ours:
            text += block("%s: mix_ours.ps1, the route through our import" % case, ours)
        if not (first or main_summary or ours):
            text.append("(no summary)")
    with open(target, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(text) + "\n")
    print("%d lines -> %s" % (len(text), target))


main()
