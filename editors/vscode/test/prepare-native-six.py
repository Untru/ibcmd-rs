#!/usr/bin/env python3
"""Create an opt-in native acceptance fixture without deleting any artifacts.

Required: absolute IBCMD_TEST_ROOT and IBCMD_TEST_NATIVE_BINARY paths.
All native commands use explicit --db-path, --data and --temp paths.
"""
import gzip
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
test_root = Path(os.environ["IBCMD_TEST_ROOT"])
native_binary = Path(os.environ["IBCMD_TEST_NATIVE_BINARY"])
if not test_root.is_absolute() or not native_binary.is_absolute():
    raise ValueError("Use explicit absolute native executable and external test paths")
test_root.mkdir(parents=True, exist_ok=True)
run = test_root / ("issue380-six-" + uuid.uuid4().hex[:10])
run.mkdir()
(run / "db").mkdir()
(run / "native-temp").mkdir()
fixture = repo / "tests/fixtures/file-infobase"
(run / "db/1Cv8.1CD").write_bytes(gzip.decompress((fixture / "tiny.1CD.gz").read_bytes()))
shutil.copytree(fixture / "tiny-source", run / "initial")
original = "ФайловаяБазаПроверка"
original_id = "6d1c2a0e-1cd0-4f00-9a00-000000000001"


def clone_module(source, name):
    shutil.copytree(source / "CommonModules" / original, source / "CommonModules" / name)
    descriptor = (source / "CommonModules" / (original + ".xml")).read_text(
        encoding="utf-8-sig"
    )
    descriptor = descriptor.replace(original, name).replace(original_id, str(uuid.uuid4()))
    (source / "CommonModules" / (name + ".xml")).write_text(descriptor, encoding="utf-8-sig")


config = run / "initial/Configuration.xml"
text = config.read_text(encoding="utf-8-sig")
for name in ["StatusClean", "StatusFile", "StatusBoth", "StatusOnlyDB"]:
    clone_module(run / "initial", name)
    text = text.replace("</ChildObjects>", f"<CommonModule>{name}</CommonModule>\n</ChildObjects>")
config.write_text(text, encoding="utf-8-sig")
common = [
    f"--db-path={run / 'db'}",
    f"--data={run / 'native-data'}",
    f"--temp={run / 'native-temp'}",
]
commands = []


def native(tag, *args):
    argv = [str(native_binary), "infobase", "config", *args, *common]
    result = subprocess.run(
        argv, cwd=run, stdin=subprocess.DEVNULL, capture_output=True, timeout=120
    )
    (run / (tag + ".stdout")).write_bytes(result.stdout)
    (run / (tag + ".stderr")).write_bytes(result.stderr)
    commands.append({"tag": tag, "argv": argv, "rc": result.returncode})
    (run / "commands.json").write_text(json.dumps(commands, ensure_ascii=False, indent=2))
    print(tag, result.returncode, result.stdout.decode(errors="replace").strip(), flush=True)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))


native("initial-import", "import", str(run / "initial"))
native("baseline-export", "export", str(run / "baseline"))
native("baseline-status", "export", "status", f"--base={run / 'baseline/ConfigDumpInfo.xml'}")
shutil.copytree(run / "baseline", run / "edit")
paths = []
for name in [original, "StatusBoth"]:
    relative = f"CommonModules/{name}/Ext/Module.bsl"
    paths.append(relative)
    with (run / "edit" / relative).open("a") as module:
        module.write("\n// native selected module edit for issue 380\n")
native("selected-import", "import", "files", f"--base-dir={run / 'edit'}", "--partial", *paths)
native("module-status", "export", "status", f"--base={run / 'baseline/ConfigDumpInfo.xml'}")
shutil.copytree(
    run / "baseline",
    run / "source",
    ignore=lambda directory, names: [
        name for name in names if name in ["StatusOnlyDB", "StatusOnlyDB.xml"]
    ],
)
for name in ["StatusFile", "StatusBoth"]:
    with (run / f"source/CommonModules/{name}/Ext/Module.bsl").open("a") as module:
        module.write("\n// independent local edit for issue 380\n")
clone_module(run / "source", "StatusOnlyFile")
(run / "workspace").mkdir()
(run / "workspace/ibcmd-rs.toml").write_text(
    '[[database]]\nname="issue380-native"\nplatform="8.5.1"\ndb-path='
    + json.dumps(str(run / "db"))
    + "\n"
)
expected = {
    f"CommonModule.{original}": "database",
    "CommonModule.StatusClean": "same",
    "CommonModule.StatusFile": "file",
    "CommonModule.StatusBoth": "both",
    "CommonModule.StatusOnlyDB": "databaseOnly",
    "CommonModule.StatusOnlyFile": "fileOnly",
}
(run / "expected-states.json").write_text(json.dumps(expected, ensure_ascii=False, indent=2))
(test_root / "native-six-run-path").write_text(str(run))
print("RUN", run, flush=True)
