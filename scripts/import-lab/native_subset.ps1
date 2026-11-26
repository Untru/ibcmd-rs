# Does the platform's own `config import` accept a tree with these edits? (issue #388)
#
#   pwsh -NoProfile -File native_subset.ps1 -Changes predef,enumval -Tag r1 [-Database ibcmd_rs_04_import_bsp_nat]
#
# Puts the working tree back to the base, applies the named edits (edits.py), runs the native `config import`
# into a lab clone (one hold of the native lock, nothing else) and reports the exit code, the rows the run left
# in ConfigSave and the platform's message. Used to find which edit of a combined tree the platform refuses.
# The working tree is a scratch copy: it is reset first. Lab databases only (ibcmd_rs_04_import_*).
param(
    [Parameter(Mandatory = $true)][string[]]$Changes,
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$Database = 'ibcmd_rs_04_import_bsp_nat',
    [string]$Work = ''
)
$Changes = @($Changes | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'; $env:PYTHONDONTWRITEBYTECODE = '1'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. "$here\native.ps1"
$lab = $script:Lab
if (-not $Work) { $Work = "$lab\tree\combo2" }
$base = "$lab\tree\base"
$free = (Get-PSDrive F).Free / 1GB
if ($free -lt 25) { throw "F: has $([math]::Round($free,1)) GB free; not starting" }

python "$here\edits.py" reset --base $base --work $Work | Out-Null
python "$here\edits.py" apply ($Changes -join ',') --base $base --work $Work | Out-Null
$r = Invoke-NativeImport -Db $Database -Tree $Work -Tag "sub-$Tag"
$msg = (Get-Content "$lab\logs\native-import-sub-$Tag.err.txt", "$lab\logs\native-import-sub-$Tag.out.txt" -ErrorAction SilentlyContinue |
        Where-Object { $_ -match '\[ERROR\]' } | Select-Object -Unique -First 3) -join ' | '
"{0}: [{1}] exit={2} rows={3} {4}s {5}" -f $Tag, ($Changes -join ','), $r.Exit, $r.Rows, $r.Seconds, $msg
