"""What a staging left in ConfigSave, against Config: the record of an import phase.

usage: python record.py <database> <case dir>     prints one JSON object

For every edited object of the case (the uuid in its XML) it says whether the object's descriptor is in ConfigSave
and whether the staged descriptor differs from the stored one (after inflate); for the rest of ConfigSave it counts the
rows that differ from Config, apart from `root`, `version`, `versions` and `deleted`.
"""
import json
import os
import re
import sys
import zlib

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import db as dbm  # noqa: E402


def inflate(raw):
    try:
        return zlib.decompress(raw, -15)
    except Exception:
        return raw


def uuid_of(path):
    with open(path, encoding="utf-8-sig") as f:
        head = f.read(4000)
    match = re.search(r'<(?:Catalog|Document|[A-Za-z]+) uuid="([0-9a-f-]{36})"', head[head.find("<MetaDataObject"):])
    return match.group(1) if match else None


def main():
    database, case_dir = sys.argv[1], sys.argv[2]
    with open(os.path.join(case_dir, "files.txt"), encoding="utf-8") as f:
        files = [line.strip() for line in f if line.strip()]
    config = {r[0]: bytes(r[1]) for r in dbm.rows(database, "SELECT FileName, BinaryData FROM Config WHERE PartNo = 0")}
    save = {r[0]: bytes(r[1]) for r in dbm.rows(database, "SELECT FileName, BinaryData FROM ConfigSave WHERE PartNo = 0")}
    all_save_rows = dbm.rows(database, "SELECT COUNT(*) FROM ConfigSave")[0][0]
    edited = []
    for rel in files:
        uuid = uuid_of(os.path.join(case_dir, "stage", rel))
        staged = uuid in save
        edited.append({
            "file": rel,
            "uuid": uuid,
            "staged": staged,
            "differs_from_stored": (inflate(save[uuid]) != inflate(config[uuid])) if staged and uuid in config else None,
        })
    ignore = {"root", "version", "versions", "deleted"}
    differing = [name for name, data in save.items()
                 if name not in ignore and name in config and inflate(data) != inflate(config[name])]
    new_rows = [name for name in save if name not in config and name not in ignore]
    print(json.dumps({
        "database": database,
        "config_rows_part0": len(config),
        "configsave_rows": all_save_rows,
        "configsave_part0_rows": len(save),
        "has_deleted_row": "deleted" in save,
        "edited": edited,
        "edited_staged": sum(1 for e in edited if e["staged"]),
        "edited_differing": sum(1 for e in edited if e["differs_from_stored"]),
        "other_rows_differing_from_config": len(differing) - sum(1 for e in edited if e["differs_from_stored"]),
        "rows_new_to_config": new_rows[:10],
        "rows_new_to_config_count": len(new_rows),
    }, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
