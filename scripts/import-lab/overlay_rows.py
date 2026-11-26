"""ConfigSave over Config as a rows folder (issue #388): the staged state a guard would export.

  python overlay_rows.py <db> <out-dir>

Reads Config and ConfigSave of the database (SELECT only) and writes `<name>__part<N>.bin` files: every Config row,
replaced by the ConfigSave row of the same name (all its parts), and without the names the ConfigSave `deleted`
row lists and without the platform's dynamic-update rows (`<id>_dynupdate_<id>[.N]`, `DynamicallyUpdated`,
`versions_dynupdate_<id>`): the native `config apply --dynamic=disable` removes them (measured: 9 844 Config rows
-> 9 838 after the apply of a patch stage that did not mention them), and the staged `<id>.0` then supplies the text. `mssql-dump-config --rows-dir <out-dir>` then exports the state an apply would produce, which
`source-diff` compares with the tree. Lab tool; the folder is a temporary file of the caller.
"""
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rowdiff  # noqa: E402


DYNAMIC_UPDATE = re.compile(r"_dynupdate_|^DynamicallyUpdated$")


def deleted_names(row):
    """The names of a `deleted` row: `<count>,"<name>",0,...` (raw deflate, BOM)."""
    if row is None:
        return set()
    text = rowdiff.inflate(row["raw"]).decode("utf-8-sig", "replace")
    return set(re.findall(r'"([^"]+)"', text))


def main():
    db, out = sys.argv[1], sys.argv[2]
    config = rowdiff.load(db, "Config")
    save = rowdiff.load(db, "ConfigSave")
    removed = deleted_names(save.get("deleted"))
    merged = {}
    for name, row in config.items():
        if name in save or name in removed or DYNAMIC_UPDATE.search(name):
            continue
        merged[name] = row
    for name, row in save.items():
        if name == "deleted":
            continue
        merged[name] = row
    if os.path.exists(out):
        shutil.rmtree(out)
    rowdiff.dump_dir(merged, out)
    print("overlay %s: %d Config names, %d staged, %d deleted -> %d names in %s" % (
        db, len(config), len(save), len(removed), len(merged), out))


if __name__ == "__main__":
    main()
