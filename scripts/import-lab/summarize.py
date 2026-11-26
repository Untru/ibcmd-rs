"""Summary of a staging matrix run (run_matrix.ps1) as a table and per-change row lists.

  python summarize.py <out\\matrix\\tag> [--md table.md] [--detail]

Reads <change>.<mode>.meta.json (exit code, message, stage mode, ConfigSave rows) and, for a stage that
succeeded, <change>.<mode>.json (rowdiff against the stage of the unchanged tree in the same mode).
`versions` always differs (fresh generation ids) and is reported apart.
"""
import argparse
import glob
import json
import os
import re
from collections import OrderedDict

MODES = ["patch", "bf", "native"]
ORDER = ["attr", "ts", "newcat", "syn", "prop", "attrprop", "attrdel", "newform", "newtpl", "predef", "enumval",
         "rights", "subsys", "nestsub", "ci", "module", "confver", "catdel"]


def label(name, what):
    """`Catalog.Имя [path].0` -> `Catalog.Имя .0`."""
    m = re.match(r"^(.*?) \[[^\]]*\](.*)$", what)
    if m:
        return (m.group(1) + m.group(2)).strip()
    return what


def load(outdir):
    result = OrderedDict()
    for meta_path in sorted(glob.glob(os.path.join(outdir, "*.meta.json"))):
        with open(meta_path, encoding="utf-8-sig") as f:
            meta = json.load(f)
        change, mode = meta["change"], meta["mode"]
        entry = {"meta": meta, "rows": None, "outdir": outdir}
        rows_path = meta_path.replace(".meta.json", ".json")
        if os.path.exists(rows_path) and meta.get("exit") == 0:
            with open(rows_path, encoding="utf-8") as f:
                entry["rows"] = json.load(f)
        result.setdefault(change, {})[mode] = entry
    return result


def changed_rows(entry):
    out = {"different": [], "new": [], "dropped": [], "versions": False}
    for name, row in entry["rows"]["rows"].items():
        cls = row["class"]
        if name == "versions":
            out["versions"] = cls not in ("identical_bytes", "identical_text")
            continue
        if cls == "different":
            out["different"].append((name, label(name, row["what"])))
        elif cls == "only_left":
            out["new"].append((name, label(name, row["what"])))
        elif cls == "only_right":
            out["dropped"].append((name, label(name, row["what"])))
    return out


def cell(entry):
    if entry is None:
        return "-"
    meta = entry["meta"]
    if meta.get("exit") != 0:
        tail = meta.get("tail") or ""
        m = re.findall(r"\[ERROR\] (.*)", tail.replace(" | ", "\n"))
        msg = next((x for x in m if "завершен с ошибкой" not in x), tail)
        return "REFUSED: " + msg[:110]
    rows = changed_rows(entry)
    n = len(rows["different"]) + len(rows["new"]) + len(rows["dropped"])
    if n == 0:
        return "LOST (no row differs from the unchanged stage)"
    return "%d rows (%d changed, %d new, %d dropped)" % (n, len(rows["different"]), len(rows["new"]), len(rows["dropped"]))


def refusal_text(entry):
    """The first line of the refusal: the failure report of the run when there is one, else the log tail."""
    meta = entry["meta"]
    outdir = os.path.normpath(entry["outdir"])
    tag = os.path.basename(outdir)
    report = os.path.join(os.path.dirname(os.path.dirname(outdir)),
                          "import-%s-%s.%s.json" % (tag, meta["change"], meta["mode"]))
    if os.path.exists(report):
        try:
            with open(report, encoding="utf-8-sig") as f:
                text = json.load(f).get("error") or ""
            first = text.strip().splitlines()[0]
            m = re.match(r"a base-free stage needs every row, and (\d+) could not be produced", first)
            if m:
                return "%s rows cannot be built (dangling references)" % m.group(1)
            return first
        except (OSError, ValueError, IndexError):
            pass
    tail = (meta.get("tail") or "").replace(" | ", "\n")
    errors = [x for x in re.findall(r"\[ERROR\] (.*)", tail) if "завершен с ошибкой" not in x]
    return errors[0].strip() if errors else tail.strip()


def short_cell(entry):
    """One table cell for the doc: LOST, REFUSED, or the rows the change reached."""
    if entry is None:
        return "-"
    meta = entry["meta"]
    if meta.get("exit") != 0:
        msg = refusal_text(entry)
        msg = re.sub(r"[0-9a-f]{8}-[0-9a-f-]{27}", "<uuid>", msg)
        msg = re.sub(r"\\\\\?\\\S+", "<path>", msg)
        return "REFUSED: " + msg[:70].replace("|", "/")
    rows = changed_rows(entry)
    labels = []
    for key in ("different", "new", "dropped"):
        for name, what in rows[key]:
            labels.append((key, what))
    if not labels:
        return "**LOST**"
    shown = []
    for key, what in labels[:2]:
        text = what if len(what) <= 44 else what[:41] + "..."
        shown.append(("%s%s" % ("+" if key == "new" else "-" if key == "dropped" else "", text)).replace("|", "/"))
    more = " (+%d)" % (len(labels) - 2) if len(labels) > 2 else ""
    return "%d row(s): %s%s" % (len(labels), "; ".join(shown), more)


def md_safe(text):
    """`<uuid>`, `<Version>` and the like stay visible in Markdown: put them in code spans."""
    return re.sub(r"(<[^<>\s]+>)", r"`\1`", text)


def doc_table(data):
    import edits
    lines = ["| change | what | patch | base-free | native |", "|---|---|---|---|---|"]
    for change in [c for c in ORDER if c in data] + [c for c in data if c not in ORDER]:
        if change not in edits.CHANGES:
            continue
        summary = md_safe(edits.CHANGES[change]["summary"].replace("|", "/"))
        cells = [md_safe(short_cell(data[change].get(m))) for m in MODES]
        lines.append("| `%s` | %s | %s |" % (change, summary, " | ".join(cells)))
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("outdir")
    ap.add_argument("--md")
    ap.add_argument("--detail", action="store_true")
    ap.add_argument("--doc", action="store_true", help="print the table of docs/import/patch-mode.md")
    ap.add_argument("--merge", nargs="*", default=[], help="more matrix folders whose results add to or replace these")
    args = ap.parse_args()
    data = load(args.outdir)
    for extra in args.merge:
        for change, modes in load(extra).items():
            data.setdefault(change, {}).update(modes)
    if args.doc:
        print(doc_table(data))
        return
    lines = ["| change | patch | base-free | native |", "|---|---|---|---|"]
    for change in [c for c in ORDER if c in data] + [c for c in data if c not in ORDER]:
        cells = [cell(data[change].get(m)) for m in MODES]
        lines.append("| %s | %s |" % (change, " | ".join(cells)))
    print("\n".join(lines))
    if args.detail:
        for change in [c for c in ORDER if c in data] + [c for c in data if c not in ORDER]:
            print("\n### " + change)
            for mode in MODES:
                entry = data[change].get(mode)
                if entry is None:
                    continue
                meta = entry["meta"]
                print("- %s: exit=%s rows=%s %ss" % (mode, meta.get("exit"), meta.get("rows"), meta.get("seconds")))
                if entry["rows"] is None:
                    print("    " + (meta.get("tail") or "")[:300])
                    continue
                rows = changed_rows(entry)
                for key in ("different", "new", "dropped"):
                    for name, what in rows[key][:12]:
                        print("    %-9s %s" % (key, what))
                    if len(rows[key]) > 12:
                        print("    %-9s ... %d more" % (key, len(rows[key]) - 12))
    if args.md:
        with open(args.md, "w", encoding="utf-8") as f:
            f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
