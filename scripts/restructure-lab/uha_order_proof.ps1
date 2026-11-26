# The order of the tables in DBSchema on the ERP УХ 8.3.27 (docs/apply/restructuring.md 12.9 point 9, 12.15): a fresh УХ clone, rcheck's stage of the case b1
# (suha\b1\stage, 6 objects rebuilt) staged by the platform's partial import, our drop-in apply with the binary of this branch, and the list of the
# tables before and after. The platform's own list of the same stage was measured by rcheck (suha\logs\b1_schema_order.txt): the six tables at the end,
# after ExtensionsInfoNGS, in the order Reference226, Reference237, Reference308, Reference491, Document1923, Document1927; without them the order is the
# stored one. One native write (the import) under the lab lock, the heavy lock around the import and the apply (the УХ is big).
#   pwsh -NoProfile -File uha_order_proof.ps1 -Exe <ibcmd-rs.exe> [-Cleanup]
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Case = 'b1',
    [switch]$Cleanup
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$kit = $PSScriptRoot
. "$kit\native_lock.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$suha = 'F:\ibcmd\lab\04\restructure-check\suha'
$heavy = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$db = "ibcmd_rs_04_ddl_uha_$Case"
$o = "$lab\out\uha_$Case"
New-Item -ItemType Directory -Force $o | Out-Null
function Log($m) { $line = "[{0}] {1}" -f (Get-Date -Format s), $m; $line; $line | Add-Content "$o\log.txt" -Encoding UTF8 }
function Invoke-Heavy([scriptblock]$Body) {
    & pwsh -NoProfile -File $heavy acquire ddl -Name heavy -TimeoutMin 240 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'the heavy lock was not granted' }
    try { & $Body } finally { & pwsh -NoProfile -File $heavy release ddl -Name heavy | Out-Null }
}
function Dump-Order($label) {
    python "$kit\schema_order_dump.py" $db "$o\order_$label.txt"
    if ($LASTEXITCODE -ne 0) { throw 'schema order dump failed' }
}

Log "restore $db (corpus uha8327)"
Invoke-Heavy {
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus uha8327 -Name $db -Track ddl -Purpose "S1 mix: the order of DBSchema on the ERP UH, rcheck's case $Case" | Select-Object -Last 1
    if ($LASTEXITCODE -ne 0) { throw 'clone restore failed' }
}
$files = @(Get-Content "$suha\$Case\files.txt" -Encoding UTF8 | Where-Object { $_ })
$data = "$lab\ibdata\$db"
New-Item -ItemType Directory -Force $data | Out-Null
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$importArgs = @('infobase', 'config', 'import', 'files', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data",
                "--base-dir=$suha\$Case\stage", '--partial') + $files
Log "native import files --partial ($($files.Count) files)"
$rc = 0
Invoke-Heavy { $script:rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments $importArgs -Log "$o\native_import" -TimeoutSec 3600 }
Log "import exit $rc"
$rows = sqlcmd -S localhost -E -C -h -1 -W -d $db -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
Log "ConfigSave rows: $($rows.Trim())"
if ($rc -ne 0 -or [int]$rows -eq 0) { throw 'the native import staged nothing' }
Dump-Order 'before'

$backup = "$lab\bak\uha_${Case}_$([Guid]::NewGuid().ToString('N').Substring(0, 8)).bak"
$applyArgs = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data", '--platform=8.3.27',
               '--force', '--dynamic=disable', "--recovery-backup=$backup")
Log 'the drop-in apply'
$sw = [Diagnostics.Stopwatch]::StartNew()
$rc2 = 0
Invoke-Heavy { $script:rc2 = Invoke-NativeCommand -Ibcmd $Exe -Arguments $applyArgs -Log "$o\dropin" -TimeoutSec 3600 -NoLock }
Log "drop-in exit $rc2 in $([math]::Round($sw.Elapsed.TotalSeconds, 1)) s"
Get-Content "$o\dropin.err" -Tail 4 -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_.Trim() } | ForEach-Object { Log "  dropin.err: $_" }
if (Test-Path $backup) { Remove-Item -Force $backup }
if ($rc2 -ne 0) { throw 'the drop-in apply failed' }
Dump-Order 'after'
python "$kit\schema_order_dump.py" --compare "$o\order_before.txt" "$o\order_after.txt" "$suha\logs\${Case}_schema_order.txt" | Tee-Object -FilePath "$o\verdict.txt"
if ($LASTEXITCODE -ne 0) { throw 'schema order does not match the native twin' }

if ($Cleanup) {
    Log 'cleanup'
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\drop-lab-dbs.ps1 -Track ddl -Names $db -MinIdleMinutes 1 -Execute 2>&1 | Select-Object -Last 3
    if (Test-Path $data) { Remove-Item -Recurse -Force $data }
}
