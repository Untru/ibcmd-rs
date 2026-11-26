#!/usr/bin/env bash
# Offline export check of a build: export a stored Config row
# set (old path or model path) into its own folder, diff against the
# reference tree. Nothing is shared with the tracks' own check folders.
# usage: xcheck.sh <bsp|uha|bsp85|uha85> <old|model> <run id> <binary>
# XCHECK_VERSION_FLAGS, when set, replaces `--source-version <VER>`: e.g.
# "--platform 8.5.1", or empty to let the settings (IBCMD_RS_PLATFORM,
# ibcmd-rs.toml) and auto-detection decide.
set -uo pipefail
corpus=$1; mode=$2; run=$3; B=$4
case "$corpus" in
  bsp)   ROWS=F:/ibcmd/lab/rawrows/bsp/Config; VER=2.20
         REF=F:/ibcmd/lab/parity/ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2/native ;;
  uha)   ROWS=F:/ibcmd/lab/rawrows/uha/Config; VER=2.20
         REF=E:/ibcmd_lab/parity/ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head/native ;;
  bsp85) ROWS=F:/ibcmd/lab/v85/rawrows/bsp/Config; VER=2.21
         REF=F:/ibcmd/lab/v85/ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2/native ;;
  uha85) ROWS=F:/ibcmd/lab/v85/rawrows/uha/Config; VER=2.21
         REF=F:/ibcmd/lab/v85/native/uha_20260923/native ;;
  *) echo "corpus?"; exit 2 ;;
esac
case "$mode" in
  old) FLAG="--legacy-export" ;;
  model) FLAG="" ;;   # the default since the model export (012b8c2c)
  *) echo "mode?"; exit 2 ;;
esac
if [ "${XCHECK_VERSION_FLAGS+set}" = set ]; then VFLAGS=$XCHECK_VERSION_FLAGS; else VFLAGS="--source-version $VER"; fi
R=/f/ibcmd/lab/model/integration/xcheck_${corpus}_${mode}_${run}
[ -e "$R" ] && { echo "$R exists; pick another run id"; exit 2; }
mkdir -p "$R"
W=$(cygpath -m "$R")
t0=$(date +%s)
"$B" mssql-dump-config --rows-dir "$ROWS" -o "$W/dump" --overwrite --extract-module-text \
  --extract-metadata-xml --no-binary-rows $VFLAGS $FLAG > "$R/export.log" 2>&1
echo "$corpus $mode export exit $? $(( $(date +%s) - t0 )) s"
MSYS2_ARG_CONV_EXCL='*' robocopy "$(cygpath -w "$R/dump")" "$(cygpath -w "$R/tree")" /E /MOVE \
  /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw Config_module_text ConfigSave_module_text \
  /XF manifest.json '*.json' > /dev/null
"$B" source-diff -o "$W/diff.json" "$REF" "$W/tree" > "$R/diff.log" 2>&1
python -X utf8 - "$W/diff.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1], encoding='utf-8'))
print('diff', d['summary'])
for x in [x for x in d['differences'] if x['status'] != 'unchanged'][:10]:
    print('  ', x['status'], x['path'])
PY
