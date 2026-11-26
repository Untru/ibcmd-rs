# The drop-in route once (S1-E acceptance): `ibcmd-rs infobase config apply --recovery-backup=<file>` on our twin of a staged case, under the
# lab's native lock, then twin_check.ps1 against the native twin (the checks 2-6 of 12.6), the platform's apply afterwards (7) and the export
# comparison (8). The report of the run that made the plan (twin_run.ps1's real.json) names the rebuilt tables.
#   pwsh -NoProfile -File dropin_run.ps1 -Case e4 -Nat ibcmd_rs_04_ddl_s2_e4_nat -Own ibcmd_rs_04_ddl_s2_e4_dropin -Exe <ibcmd-rs.exe> `
#        -Report F:\ibcmd\lab\04\restructure\out\run_e4\real.json
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [Parameter(Mandatory = $true)][string]$Nat,
    [Parameter(Mandatory = $true)][string]$Own,
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Report,
    [string]$NatLabel = 'nat_after'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
if ($Own -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only (got $Own)" }
$kit = $PSScriptRoot
. "$kit\native_lock.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$o = "$lab\out\dropin_$Case"
New-Item -ItemType Directory -Force $o | Out-Null
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
$data = "$lab\ibdata\$Own"
New-Item -ItemType Directory -Force $data | Out-Null
$backup = "$lab\bak\dropin_${Case}_$([Guid]::NewGuid().ToString('N').Substring(0, 8)).bak"
$dbArgs = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Own", "--data=$data",
            '--user=Администратор', '--force', '--dynamic=disable')

Log 'without a backup option: refused, nothing written'
$before = sqlcmd -S localhost -E -C -h -1 -W -d $Own -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
$rc = Invoke-NativeCommand -Ibcmd $Exe -Arguments $dbArgs -Log "$o\apply-nobackup" -TimeoutSec 1800
$after = sqlcmd -S localhost -E -C -h -1 -W -d $Own -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
"exit $rc; ConfigSave rows $before -> $after"
Get-Content "$o\apply-nobackup.err" -Tail 3 -ErrorAction SilentlyContinue

Log "with --recovery-backup=$backup"
$sw = [Diagnostics.Stopwatch]::StartNew()
$rc = Invoke-NativeCommand -Ibcmd $Exe -Arguments ($dbArgs + "--recovery-backup=$backup") -Log "$o\apply" -TimeoutSec 3600
"exit $rc in {0:n1}s; backup {1} ({2:n0} MB)" -f $sw.Elapsed.TotalSeconds, (Test-Path $backup), ((Get-Item $backup -ErrorAction SilentlyContinue).Length / 1MB)
Get-Content "$o\apply.out" -Tail 8 -ErrorAction SilentlyContinue
Get-Content "$o\apply.err" -Tail 4 -ErrorAction SilentlyContinue
if ($rc -ne 0) { throw 'the drop-in apply failed' }

Log 'checks 2-6'
$env:TWIN_OUT = "$lab\out"
pwsh -NoProfile -File "$kit\twin_check.ps1" -Case "${Case}_dropin" -Nat $Nat -Own $Own -Report $Report -NatLabel $NatLabel 2>&1 | Tee-Object "$o\twin_check.txt" | Select-String -Pattern '^(\d\.|rebuilt|changed|16 of|DBNames text|entries that differ|only in|_|== only)' | ForEach-Object { $_.Line }
Log 'check 7: the platform apply on our twin'
pwsh -NoProfile -File "$kit\apply_only.ps1" -Database $Own *> "$o\native_noop.txt"
Get-Content "$o\native_noop.txt" | Select-String -Pattern 'не требуется|exit=' | ForEach-Object { $_.Line }
Log 'check 8: exports'
$db = $Own
Remove-Item -Recurse -Force "$lab\export\$db" -ErrorAction SilentlyContinue
pwsh -NoProfile -File "$kit\export_tree.ps1" -Database $db -Out "$lab\export\$db" 2>&1 | Select-Object -Last 1
if (-not (Test-Path "$lab\export\$Nat")) { pwsh -NoProfile -File "$kit\export_tree.ps1" -Database $Nat -Out "$lab\export\$Nat" 2>&1 | Select-Object -Last 1 }
& $Exe source-diff "$lab\export\$Nat" "$lab\export\$Own" > "$o\export_diff.json" 2>&1
python -c "import json; d=json.load(open(r'$o\export_diff.json',encoding='utf-8')); print('export diff', d['summary'])"
Log "done $Case"
