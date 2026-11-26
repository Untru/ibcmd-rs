# A native twin of an edit (#393): a fresh clone of the БСП 8.3.27 corpus, the platform's own `config import` of a
# tree (repeated until it stages the whole tree: the first import of a fresh clone stages a subset or fails, see
# docs/apply/own-apply.md "Removals"), a ddl-kit snapshot of the staged state, a COPY_ONLY backup of it, the
# platform's `config apply`, a snapshot after.
#   pwsh -NoProfile -File native_twin.ps1 -Suffix <name> -Tree <dir> [-MinRows 9800]
# The clone is ibcmd_rs_04_apply_<Suffix>_20260930. Logs: logs\twin_<Suffix>.log
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9]{2,12}$')][string]$Suffix,
    [Parameter(Mandatory = $true)][string]$Tree,
    [int]$MinRows = 9800,
    [int]$Tries = 5
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\apply'
$db = "ibcmd_rs_04_apply_${Suffix}_20260930"
$log = "$lab\logs\twin_$Suffix.log"
function Say($text) { $line = "$(Get-Date -Format s) $text"; $line | Tee-Object -FilePath $log -Append }
$env:DDL_LAB = "$lab\ddlkit"; $env:PYTHONIOENCODING = 'utf-8'
$kit = (Resolve-Path (Join-Path $PSScriptRoot '..\restructure-lab')).Path
function Rows($name) { [int](sqlcmd -S localhost -E -C -h -1 -W -d $db -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave") }

Say "restore $db"
& pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bsp8327 -Name $db -Track apply -Purpose "#393 native twin $Suffix" 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
$ok = $false
for ($i = 1; $i -le $Tries; $i++) {
    $r = & pwsh -NoProfile -File "$PSScriptRoot\native_import.ps1" -Database $db -Tree $Tree -Tag "twin_${Suffix}_import$i" 2>&1
    $rows = Rows
    Say "import $i : $($r | Select-Object -First 1) ; ConfigSave rows $rows"
    if ($rows -ge $MinRows) { $ok = $true; break }
}
if (-not $ok) { Say "no full stage after $Tries imports"; exit 2 }
Push-Location $kit
python snapshot.py $db staged 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
Pop-Location
$bak = "$lab\bak\twin_${Suffix}_staged.bak"
sqlcmd -S localhost -E -C -b -Q "BACKUP DATABASE [$db] TO DISK = N'$bak' WITH COPY_ONLY, COMPRESSION, INIT" | Select-Object -Last 1 | ForEach-Object { Say $_ }
$r = & pwsh -NoProfile -File "$PSScriptRoot\native_cmd.ps1" -Database $db -Tag "twin_${Suffix}_apply" -Command "infobase config apply --force --dynamic=disable" -TimeoutSec 3000 2>&1
Say "apply : $($r | Select-Object -First 1)"
Push-Location $kit
python snapshot.py $db nat_after 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
Pop-Location
Say "done $db"
