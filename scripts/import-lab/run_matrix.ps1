# The staging matrix of issue #388: every change of edits.py, staged three ways (patch = the drop-in default on
# a database that holds the configuration, --base-free, native `config import`) into a staging clone that is
# never applied, each stage compared row by row with the stage of the UNCHANGED tree in the same mode.
#
#   pwsh -NoProfile -File run_matrix.ps1 -Changes attr,ts -Modes patch,bf,native [-Exe <ibcmd-rs.exe>] [-Tag v0]
#
# Needs in $Lab: tree\base (exported tree), tree\work (a copy of it), out\rows_<mode>_pristine (the unchanged
# tree's stage dumped with `rowdiff.py --dump-left`). Results: out\matrix\<tag>\<change>.<mode>.json + .log
param(
    [Parameter(Mandatory = $true)][string[]]$Changes,
    [string[]]$Modes = @('patch', 'bf', 'native'),
    [string]$Database = 'ibcmd_rs_04_import_bsp_s1',
    [string]$Exe = '',
    [string]$Tag = 'v0',
    [string]$WorkName = 'work'
)
$Changes = @($Changes | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Modes = @($Modes | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. "$here\native.ps1"
$lab = $script:Lab
$base = "$lab\tree\base"; $work = "$lab\tree\$WorkName"
$outDir = "$lab\out\matrix\$Tag"
New-Item -ItemType Directory -Force $outDir | Out-Null

function Reset-Work { python "$here\edits.py" reset --base $base --work $work | Out-Null }

foreach ($change in $Changes) {
    Reset-Work
    $info = python "$here\edits.py" apply $change --base $base --work $work
    "=== $change  $info"
    foreach ($mode in $Modes) {
        $tagName = "$change.$mode"
        $res = $null
        switch ($mode) {
            'patch' { $res = Invoke-OursImport -Db $Database -Tree $work -Tag "$Tag-$tagName" -Exe $Exe }
            'bf' { $res = Invoke-OursImport -Db $Database -Tree $work -Tag "$Tag-$tagName" -BaseFree -Exe $Exe }
            'native' { $res = Invoke-NativeImport -Db $Database -Tree $work -Tag "$Tag-$tagName" }
        }
        $meta = @{ change = $change; mode = $mode; exit = $res.Exit; seconds = $res.Seconds; rows = $res.Rows; tail = $res.Tail; stage_mode = $res.Mode }
        $ok = ($res.Exit -eq 0)
        if ($ok -and $mode -ne 'native' -and $res.Rows -le 0) { $ok = $false }
        if ($ok) {
            $baseline = "$lab\out\rows_$($mode)_pristine"
            python "$here\rowdiff.py" "${Database}:ConfigSave" "dir:$baseline" --tree $work --show 40 --json "$outDir\$tagName.json" *> "$outDir\$tagName.log"
        } else {
            "  ($mode) exit=$($res.Exit) rows=$($res.Rows): $($res.Tail)"
        }
        $meta | ConvertTo-Json -Compress | Set-Content -Encoding UTF8 "$outDir\$tagName.meta.json"
        "  {0,-6} exit={1} rows={2} {3}s" -f $mode, $res.Exit, $res.Rows, $res.Seconds
    }
    Reset-Work
}
