#!/usr/bin/env bash
# Offline round trip of a tree, no SQL (issue #388): every row `--base-free` would stage
# (audit-empty-stage --rows-out) exported again from those rows alone (mssql-dump-config
# --rows-dir), diffed against the tree itself. A difference is a change the compile + export pair
# does not carry through: exactly what a guard after the stage must catch.
#
# usage: ve_tree.sh <tree> <run id> [ibcmd-rs binary] [stored rows dir]
set -uo pipefail
TREE=$1; RUN=$2
B=${3:-/f/ibcmd/lab/04/import/bin/ibcmd-rs-v0.exe}
ROWS=${4:-/f/ibcmd/lab/04/import/out/rows_native_pristine}
VER=2.20
R=/f/ibcmd/lab/04/import/out/ve_${RUN}
[ -e "$R" ] && { echo "$R exists; pick another run id"; exit 2; }
mkdir -p "$R"
W=$(cygpath -m "$R")
T=$(cygpath -m "$TREE")
t0=$(date +%s)
# An empty database has no always-used constants: no IBCMD_RS_ALWAYS_USED_CONSTANTS.
IBCMD_RS_NATIVE_FORM_WRITER=always \
  "$B" audit-empty-stage "$T" "$(cygpath -m "$ROWS")" --source-version "$VER" --rows-out "$W/rows" -o "$W/empty.json" > "$R/stage.log" 2>&1
echo "stage exit $? $(( $(date +%s) - t0 )) s, rows $(find "$R/rows" -type f | wc -l)"
sed -n 2p "$R/stage.log"
t0=$(date +%s)
"$B" mssql-dump-config --rows-dir "$W/rows" -o "$W/dump" --overwrite --extract-module-text \
  --extract-metadata-xml --no-binary-rows --source-version "$VER" > "$R/export.log" 2>&1
echo "export exit $? $(( $(date +%s) - t0 )) s"
MSYS2_ARG_CONV_EXCL='*' robocopy "$(cygpath -w "$R/dump")" "$(cygpath -w "$R/tree")" /E /MOVE \
  /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw Config_module_text ConfigSave_module_text \
  /XF manifest.json '*.json' > /dev/null
"$B" source-diff -o "$W/diff.json" "$T" "$W/tree" > "$R/diff.log" 2>&1
python -X utf8 - "$W/diff.json" <<'PY'
import json, sys, collections
d = json.load(open(sys.argv[1], encoding='utf-8'))
print('diff', d['summary'])
by = collections.Counter()
for x in d['differences']:
    if x['status'] != 'unchanged':
        by[(x['status'], x['path'])] += 1
for (status, path), n in sorted(by.items())[:60]:
    print(f'{n:4} {status:10} {path}')
PY
