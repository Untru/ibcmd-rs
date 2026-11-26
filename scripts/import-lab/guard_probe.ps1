# Would a guard have caught it? (issue #388)  Stage a change in a mode into the staging clone, build the state an
# apply would produce (ConfigSave over Config, overlay_rows.py), export it offline with the model
# (`mssql-dump-config --rows-dir`) and diff the export with the edited tree. A file that differs is a change the
# stage did not carry: the guard's verdict. Temporary folders are deleted at the end.
#
#   pwsh -NoProfile -File guard_probe.ps1 -Changes attr,ts -Mode patch [-Database ibcmd_rs_04_import_bsp_s1]
param(
    [Parameter(Mandatory = $true)][string[]]$Changes,
    [ValidateSet('patch', 'bf')][string]$Mode = 'patch',
    [string]$Database = 'ibcmd_rs_04_import_bsp_s1',
    [string]$Exe = ''
)
$Changes = @($Changes | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'; $env:PYTHONDONTWRITEBYTECODE = '1'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. "$here\native.ps1"
$lab = $script:Lab
if (-not $Exe) { $Exe = "$lab\bin\ibcmd-rs-v0.exe" }
$base = "$lab\tree\base"; $work = "$lab\tree\work"
$outDir = "$lab\out\guard"
New-Item -ItemType Directory -Force $outDir | Out-Null
$free = (Get-PSDrive F).Free / 1GB
if ($free -lt 25) { throw "F: has $([math]::Round($free,1)) GB free; not starting" }

foreach ($change in $Changes) {
    python "$here\edits.py" reset --base $base --work $work | Out-Null
    python "$here\edits.py" apply $change --base $base --work $work | Out-Null
    $res = if ($Mode -eq 'bf') { Invoke-OursImport -Db $Database -Tree $work -Tag "guard-$change-$Mode" -BaseFree -Exe $Exe }
           else { Invoke-OursImport -Db $Database -Tree $work -Tag "guard-$change-$Mode" -Exe $Exe }
    if ($res.Exit -ne 0) {
        "{0,-9} {1,-5} stage refused (exit {2}): {3}" -f $change, $Mode, $res.Exit, (($res.Tail -split ' \| ')[1])
        continue
    }
    $ov = "$lab\out\ov_$change"
    python "$here\overlay_rows.py" $Database $ov | Out-Null
    $dump = "$lab\out\ov_${change}_dump"
    & $Exe mssql-dump-config --rows-dir $ov -o $dump --overwrite --extract-module-text --extract-metadata-xml --no-binary-rows --source-version 2.20 *> "$outDir\$change.$Mode.export.log"
    $tree = "$lab\out\ov_${change}_tree"
    robocopy $dump $tree /E /MOVE /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw Config_module_text ConfigSave_module_text /XF manifest.json '*.json' /NFL /NDL /NJH /NJS /NP | Out-Null
    $diff = "$outDir\$change.$Mode.diff.json"
    & $Exe source-diff -o $diff $work $tree *> $null
    $d = Get-Content $diff -Raw | ConvertFrom-Json
    $files = @($d.differences | Where-Object { $_.status -ne 'unchanged' -and $_.path -ne 'ConfigDumpInfo.xml' })
    "{0,-9} {1,-5} staged {2} rows; export vs tree: {3} file(s) differ{4}" -f $change, $Mode, $res.Rows, $files.Count, $(if ($files.Count) { ': ' + (($files | Select-Object -First 4 | ForEach-Object { "$($_.status) $($_.path)" }) -join '; ') } else { '' })
    Remove-Item $ov, $dump, $tree -Recurse -Force -ErrorAction SilentlyContinue
}
python "$here\edits.py" reset --base $base --work $work | Out-Null
