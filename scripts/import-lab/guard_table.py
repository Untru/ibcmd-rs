"""The table of docs/import/patch-mode.md section 6.5 from the guard probe (guard_probe.ps1).

  python guard_table.py <out\\guard> <out\\matrix\\v0> [<out\\matrix\\v1> ...]

For every `<change>.patch.diff.json` of the guard folder: what the matrix says patch mode did with the change
(LOST / carried / refused) and which files the export of ConfigSave over Config differs from the tree in.
A lost change must be flagged, a carried one must not. `formdel` counts as lost: patch mode stages the help
pages that linked to the removed form but not the removal (the owner's descriptor and the form's rows stay).
"""
import glob
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import summarize  # noqa: E402


REMOVAL_LOST = {"formdel"}


def main():
    guard = sys.argv[1]
    data = summarize.load(sys.argv[2])
    for extra in sys.argv[3:]:
        for change, modes in summarize.load(extra).items():
            data.setdefault(change, {}).update(modes)
    lines = ["| change | patch mode (matrix) | files where the export of the staged state differs from the tree | guard |",
             "|---|---|---|---|"]
    names = sorted(
        (os.path.basename(p).split(".")[0] for p in glob.glob(os.path.join(guard, "*.patch.diff.json"))),
        key=lambda c: summarize.ORDER.index(c) if c in summarize.ORDER else 99)
    for change in names:
        with open(os.path.join(guard, "%s.patch.diff.json" % change), encoding="utf-8") as f:
            diff = json.load(f)
        files = [x for x in diff["differences"] if x["status"] != "unchanged" and x["path"] != "ConfigDumpInfo.xml"]
        entry = data.get(change, {}).get("patch")
        verdict = summarize.short_cell(entry)
        kind = "LOST" if verdict.startswith("**LOST") or change in REMOVAL_LOST else "REFUSED" if verdict.startswith("REFUSED") else "carried"
        shown = "; ".join("%s `%s`" % (x["status"], x["path"]) for x in files[:3]) + (" (+%d)" % (len(files) - 3) if len(files) > 3 else "")
        guard_says = "refuses" if files else "passes"
        ok = (kind == "LOST" and files) or (kind == "carried" and not files)
        lines.append("| `%s` | %s | %s | %s%s |" % (change, kind, shown or "none", guard_says, "" if ok else " (unexpected)"))
    print("\n".join(lines))


if __name__ == "__main__":
    main()
