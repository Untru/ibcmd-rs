"""List small common modules of a native export with their environment flags.

Read-only. Usage: python find_marker_modules.py <native_export_dir> [max_bytes]
"""
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
max_bytes = int(sys.argv[2]) if len(sys.argv) > 2 else 3000
flags = ["Global", "ClientManagedApplication", "Server", "ExternalConnection",
         "ClientOrdinaryApplication", "ServerCall", "Privileged", "ReturnValuesReuse"]
sys.stdout.reconfigure(encoding="utf-8")
rows = []
for module_dir in sorted((root / "CommonModules").iterdir()):
    if not module_dir.is_dir():
        continue
    body = module_dir / "Ext" / "Module.bsl"
    meta = root / "CommonModules" / (module_dir.name + ".xml")
    if not body.exists() or not meta.exists():
        continue
    size = body.stat().st_size
    if size > max_bytes:
        continue
    xml = meta.read_text(encoding="utf-8-sig")
    values = {}
    for flag in flags:
        m = re.search(r"<%s>([^<]*)</%s>" % (flag, flag), xml)
        values[flag] = m.group(1) if m else "?"
    rows.append((size, module_dir.name, values))
for size, name, values in sorted(rows):
    ab = {"Global":"G","ClientManagedApplication":"CM","Server":"S","ExternalConnection":"EC","ClientOrdinaryApplication":"CO","ServerCall":"SC","Privileged":"P","ReturnValuesReuse":"R"}
    short = " ".join("%s=%s" % (ab[k], v[:1]) for k, v in values.items())
    print("%6d %-60s %s" % (size, name, short))
