# The twin protocol of docs/apply/restructuring.md 12.6 for a case whose native twin is applied (snapshot <NatLabel> taken) and whose
# own twin is staged: our side through `mssql-config-apply --allow-restructure s1 --i-have-a-backup` (dry run, rehearsal + snapshot
# to prove it changed nothing, the real run), then checks 2-6 (twin_check.ps1), 7 (a native config apply on our twin), 8 (native
# exports of both, ibcmd-rs source-diff) and 9 (a session job on both, outputs compared).
#   pwsh -NoProfile -File twin_run.ps1 -Case d1 -Nat ibcmd_rs_04_ddl_s2_d1_nat -Own ibcmd_rs_04_ddl_s2_d1_own `
#        -Base ibcmd_rs_04_ddl_s2_d1_base -StagedLabel d1_staged -Job jobs\s2_d1.bsl [-Exe <ibcmd-rs.exe>]
# -Base / -StagedLabel: the snapshot of the staged state (for check 10). Skip a step with -Skip export,session,noop.
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [Parameter(Mandatory = $true)][string]$Nat,
    [Parameter(Mandatory = $true)][string]$Own,
    [Parameter(Mandatory = $true)][string]$Base,
    [Parameter(Mandatory = $true)][string]$StagedLabel,
    [string]$Job = '',
    [string]$Exe = 'F:\ibcmd\lab\04\restructure\bin\ibcmd-rs-d.exe',
    [string]$NatLabel = 'nat_after',
    [string[]]$Skip = @()
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$kit = $PSScriptRoot
$o = "$lab\out\run_$Case"
New-Item -ItemType Directory -Force $o | Out-Null
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
$common = @('mssql-config-apply', '--platform-profile', 'platform-8.3.27.2214', '--database', $Own, '--allow-restructure', 's1', '--allow-non-lab')

Log 'dry run'
& $Exe @common --dry-run --report "$o\dry.json" > "$o\dry.txt" 2> "$o\dry.err"
"dry exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\dry.err" -Tail 5; throw 'the dry run refused' }
Log 'rehearsal'
& $Exe @common --rehearse --i-have-a-backup --report "$o\rehearse.json" --script-output "$o\script_rehearse.sql" > "$o\rehearse.txt" 2> "$o\rehearse.err"
"rehearse exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\rehearse.err" -Tail 5; throw 'the rehearsal failed' }
Push-Location $kit
try {
    python snapshot.py $Own "${Case}_rehearsed" | Select-Object -Last 1
    python snapdiff.py $Base $StagedLabel $Own "${Case}_rehearsed" > "$o\rehearse_diff.txt"
    "check 10 (a rehearsal changes nothing):"
    Get-Content "$o\rehearse_diff.txt" -TotalCount 6
} finally { Pop-Location }
Log 'real run'
& $Exe @common --i-have-a-backup --report "$o\real.json" --script-output "$o\script_real.sql" --recovery-dir "$o\recovery" > "$o\real.txt" 2> "$o\real.err"
"real exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\real.err" -Tail 5; throw 'the real run failed' }
$report = Get-Content "$o\real.json" -Raw -Encoding UTF8 | ConvertFrom-Json
"structure: " + (($report.structure.objects) -join ' | ')

Log 'checks 2-6'
$env:TWIN_OUT = "$lab\out"
pwsh -NoProfile -File "$kit\twin_check.ps1" -Case $Case -Nat $Nat -Own $Own -Report "$o\real.json" -NatLabel $NatLabel 2>&1 | Tee-Object "$o\twin_check.txt" | Select-String -Pattern '^(\d\.|rebuilt|changed|16 of|DBNames text|entries that differ|only in|_|== only)' | ForEach-Object { $_.Line }

if ($Skip -notcontains 'noop') {
    Log 'check 7: native config apply on our twin'
    pwsh -NoProfile -File "$kit\apply_only.ps1" -Database $Own *> "$o\native_noop.txt"
    Get-Content "$o\native_noop.txt" | Select-String -Pattern 'не требуется|exit=' | ForEach-Object { $_.Line }
}
if ($Skip -notcontains 'export') {
    Log 'check 8: native exports and source-diff'
    foreach ($db in $Nat, $Own) {
        # a folder of an earlier run would be compared instead of a failed export
        Remove-Item -Recurse -Force "$lab\export\$db" -ErrorAction SilentlyContinue
        pwsh -NoProfile -File "$kit\export_tree.ps1" -Database $db -Out "$lab\export\$db" 2>&1 | Select-Object -Last 1
        if ($LASTEXITCODE -ne 0) { throw "the native export of $db failed" }
    }
    & $Exe source-diff "$lab\export\$Nat" "$lab\export\$Own" > "$o\export_diff.json" 2>&1
    python -c "import json; d=json.load(open(r'$o\export_diff.json',encoding='utf-8')); print('export diff', d['summary'])"
}
if ($Job -and $Skip -notcontains 'session') {
    Log 'check 9: cluster session'
    $reg = 'F:\ibcmd\lab\04\tools\register-ib.ps1'
    foreach ($side in 'nat', 'own') {
        $db = if ($side -eq 'nat') { $Nat } else { $Own }
        pwsh -NoProfile -File $reg register -Database $db -Platform 8.3.27 -Track ddl 2>&1 | Select-Object -Last 1
        try {
            pwsh -NoProfile -File "$kit\session_job.ps1" -Job $Job -Database $db -TimeoutSec 600 > "$o\session_$side.txt" 2>&1
        } finally {
            pwsh -NoProfile -File $reg unregister -Database $db -Platform 8.3.27 -Track ddl 2>&1 | Select-Object -Last 1
        }
    }
    "session outputs identical: " + ((Get-Content "$o\session_nat.txt" -Raw) -eq (Get-Content "$o\session_own.txt" -Raw)) + " (" + (Get-Content "$o\session_own.txt").Count + " lines)"
}
Log "done $Case"
