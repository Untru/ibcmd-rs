"""Exercise #363 on a disposable MSSQL twin initialized with two_variable/input.cf."""

import argparse
from pathlib import Path
import re
import subprocess
import sys

from compare_import_files_twins import DESCRIPTOR, MODULE, replace_once


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--server", default="localhost")
    parser.add_argument("--database", required=True)
    parser.add_argument("work", type=Path, help="new directory for inputs and native exports")
    args = parser.parse_args()
    if not re.fullmatch(r"ibcmd_rs_363_[a-zA-Z0-9_]+", args.database):
        parser.error("--database must name a disposable ibcmd_rs_363_* test database")

    work = args.work.resolve()
    work.mkdir(parents=True)
    conn = ["--dbms=MSSQLServer", f"--db-server={args.server}", f"--db-name={args.database}"]

    def native(command, *paths):
        subprocess.run([args.native, "infobase", "config", command, *conn,
                        f"--data={work / 'data'}", *paths], check=True)

    def staged_rows():
        query = "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
        output = subprocess.check_output(["sqlcmd", "-E", "-C", "-b", "-S", args.server,
                                         "-d", args.database, "-h", "-1", "-W", "-Q", query])
        return int(output.strip())

    if staged_rows():
        raise SystemExit("Expected a twin with empty ConfigSave")
    baseline = work / "baseline"
    native("export", baseline)
    sparse = work / "sparse"
    owner = "CommonForms/ФормаСлева.xml"
    for path in [MODULE, owner, DESCRIPTOR]:
        target = sparse / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((baseline / path).read_bytes())
    module = sparse / MODULE
    module.write_bytes(replace_once(module.read_bytes(), "Номер = 1;", "Номер = 2;"))
    right = sparse / DESCRIPTOR
    edited = replace_once(right.read_bytes(), "<Comment/>", "<Comment>Изменено частичной загрузкой</Comment>")
    right.write_bytes(replace_once(edited, "<UseStandardCommands>false", "<UseStandardCommands>true"))
    cmd = [args.product, "infobase", "config", "import", "files", *conn,
           f"--base-dir={sparse}", "--partial"]
    refusal = subprocess.run([*cmd, "--no-check", DESCRIPTOR], capture_output=True)
    if (refusal.returncode == 0
            or b"UseStandardCommands" not in refusal.stdout + refusal.stderr
            or staged_rows() != 0):
        raise SystemExit(f"Expected refusal before writing ConfigSave: {refusal!r}")

    right.write_bytes(edited)
    subprocess.run([*cmd, MODULE, DESCRIPTOR], check=True)
    if staged_rows() == 0:
        raise SystemExit("No rows staged for the two edits")
    native("apply", "--force")
    result = work / "export"
    native("export", result)
    checker = Path(__file__).with_name("compare_import_files_twins.py")
    subprocess.run([sys.executable, checker, baseline, result], check=True)


if __name__ == "__main__":
    main()
