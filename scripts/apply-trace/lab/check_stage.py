"""Is the stage in ConfigSave complete?  (read-only; the versions completeness test of native-apply-trace.md section 4.1)

    python check_stage.py <database>

Tests: no `commit` row and no `*.new` row in ConfigSave; `root`, `version` and `versions` present; every entry of the staged
`versions` whose guid differs from the one in `Config` (or is new) has a ConfigSave row.  A stage holds only the rows whose
version guid changed, so the row count says nothing.  Needs sqlcmd and dump_rows.ps1 (same folder).
"""
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


def sql(db, q):
    out = subprocess.run(["sqlcmd", "-S", "localhost", "-E", "-C", "-d", db, "-h", "-1", "-W", "-Q", "SET NOCOUNT ON; " + q],
                         capture_output=True, text=True, encoding="utf-8", check=True).stdout
    return [l.strip() for l in out.splitlines() if l.strip()]


def dump(db, table, name, out):
    subprocess.run(["pwsh", "-NoProfile", "-File", os.path.join(HERE, "dump_rows.ps1"), "-Database", db, "-Names", name, "-Out", out, "-Table", table],
                   check=True, capture_output=True)
    return open(os.path.join(out, name), "rb").read().decode("utf-8-sig")


def entries(text):
    # {1,<count>,"",<generation>,"name",<guid>,"name",<guid>,...}
    return dict(re.findall(r'"([^"]+)",([0-9a-fA-F-]{36})', text))


def main(db):
    names = sql(db, "SELECT DISTINCT FileName FROM ConfigSave ORDER BY FileName")
    bad = []
    if "commit" in names:
        bad.append("ConfigSave holds `commit`")
    bad += ["ConfigSave holds %s" % n for n in names if n.endswith(".new")]
    for n in ("root", "version", "versions"):
        if n not in names:
            bad.append("ConfigSave lacks `%s`" % n)
    if "versions" in names:
        with tempfile.TemporaryDirectory() as tmp:
            staged = entries(dump(db, "ConfigSave", "versions", os.path.join(tmp, "s")))
            current = entries(dump(db, "Config", "versions", os.path.join(tmp, "c")))
        changed = [n for n, g in staged.items() if current.get(n) != g]
        missing = [n for n in changed if n not in names]
        print("%s: ConfigSave %d rows; versions: %d entries staged, %d in Config; %d differ, %d of them without a ConfigSave row"
              % (db, len(names), len(staged), len(current), len(changed), len(missing)))
        if missing:
            bad.append("no ConfigSave row for: " + ", ".join(missing[:8]) + (" ..." if len(missing) > 8 else ""))
        extra = [n for n in names if n not in ("root", "version", "versions") and n not in changed]
        if extra:
            print("  rows staged although their guid is unchanged: %d (e.g. %s)" % (len(extra), ", ".join(extra[:4])))
    print("COMPLETE" if not bad else "INCOMPLETE: " + "; ".join(bad))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
