#!/usr/bin/env bash
# Virtual empty-database cycle, offline: every row a load into an EMPTY
# infobase would write (audit-empty-stage --rows-out), exported from those
# rows alone (mssql-dump-config --rows-dir), diffed against the reference.
# usage: ve.sh <bsp8327|uha8327|bsp85|uha85> <run id> [binary]
set -uo pipefail
corpus=$1; run=$2
B=${3:-/f/ibcmd/src/ibcmd-rs-model/target/iter/ibcmd-rs.exe}
case "$corpus" in
  bsp8327) REF=F:/ibcmd/lab/parity/ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2/native
           ROWS=F:/ibcmd/lab/rawrows/bsp/Config; VER=2.20; CONSTS=F:/ibcmd/lab/roundtrip/bsp_always_used_constants.txt ;;
  uha8327) REF=E:/ibcmd_lab/parity/ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head/native
           ROWS=F:/ibcmd/lab/rawrows/uha/Config; VER=2.20; CONSTS=F:/ibcmd/lab/roundtrip/uha_always_used_constants.txt ;;
  bsp85)   REF=F:/ibcmd/lab/v85/ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2/native
           ROWS=F:/ibcmd/lab/v85/rawrows/bsp/Config; VER=2.21; CONSTS=F:/ibcmd/lab/v85/rawrows/bsp_always_used_constants.txt ;;
  uha85)   REF=F:/ibcmd/lab/v85/native/uha_20260923/native
           ROWS=F:/ibcmd/lab/v85/rawrows/uha/Config; VER=2.21; CONSTS=F:/ibcmd/lab/v85/rawrows/uha_always_used_constants.txt ;;
  *) echo "corpus?"; exit 2 ;;
esac
R=/f/ibcmd/lab/model/integration/ve_${corpus}_${run}
[ -e "$R" ] && { echo "$R exists; pick another run id"; exit 2; }
mkdir -p "$R"
W=$(cygpath -m "$R")
t0=$(date +%s)
# An empty database has no always-used constants: no IBCMD_RS_ALWAYS_USED_CONSTANTS.
IBCMD_RS_NATIVE_FORM_WRITER=always \
  "$B" audit-empty-stage "$REF" "$ROWS" --source-version "$VER" --rows-out "$W/rows" -o "$W/empty.json" > "$R/stage.log" 2>&1
echo "stage exit $? $(( $(date +%s) - t0 )) s, rows $(find "$R/rows" -type f | wc -l)"
sed -n 2p "$R/stage.log"
t0=$(date +%s)
"$B" mssql-dump-config --rows-dir "$W/rows" -o "$W/dump" --overwrite --extract-module-text \
  --extract-metadata-xml --no-binary-rows --source-version "$VER" > "$R/export.log" 2>&1
echo "export exit $? $(( $(date +%s) - t0 )) s"
MSYS2_ARG_CONV_EXCL='*' robocopy "$(cygpath -w "$R/dump")" "$(cygpath -w "$R/tree")" /E /MOVE \
  /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw Config_module_text ConfigSave_module_text \
  /XF manifest.json '*.json' > /dev/null
"$B" source-diff -o "$W/diff.json" "$REF" "$W/tree" > "$R/diff.log" 2>&1
python -X utf8 - "$W/diff.json" <<'PY'
import json, sys, collections
d = json.load(open(sys.argv[1], encoding='utf-8'))
print('diff', d['summary'])
by = collections.Counter()
for x in d['differences']:
    if x['status'] != 'unchanged':
        parts = x['path'].split('/')
        tail = parts[-1] if len(parts) < 2 or parts[-2] == 'Ext' else ('<object>.xml' if len(parts) == 2 else parts[-2] + '/*')
        by[(x['status'], parts[0], tail)] += 1
for (status, top, tail), n in sorted(by.items(), key=lambda kv: -kv[1])[:40]:
    print(f'{n:6} {status:10} {top}/{tail}')
PY
