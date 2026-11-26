# Port acceptance of one case through `mssql-config-apply --allow-restructure s1 --i-have-a-backup`:
# dry run, rehearsal, the real run, checks 2-6 against the native twin (twin_check.ps1), 7 (a native config apply on our twin says
# "not required") and 8 (native exports of both, ibcmd-rs source-diff).
#   pwsh -NoProfile -File port_case.ps1 -Case t1|b1|c1|b2 [-Exe <ibcmd-rs.exe>]
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9]+$')][string]$Case,
    [string]$Exe = 'F:\ibcmd\lab\04\restructure\bin\ibcmd-rs-port.exe'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$lab = 'F:\ibcmd\lab\04\restructure'
$kit = 'F:\ibcmd\src\ibcmd-rs-04-restructure\scripts\restructure-lab'
$own = "ibcmd_rs_04_ddl_p_${Case}_own"
$nat = "ibcmd_rs_04_ddl_p_${Case}_nat"
$o = "$lab\out\port_$Case"
New-Item -ItemType Directory -Force $o | Out-Null
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
$common = @('mssql-config-apply', '--platform-profile', 'platform-8.3.27.2214', '--database', $own, '--allow-restructure', 's1', '--allow-non-lab')

Log "dry run"
& $Exe @common --dry-run --report "$o\dry.json" > "$o\dry.txt" 2> "$o\dry.err"
"dry exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\dry.err" -Tail 5; throw 'the dry run refused' }
Log "rehearsal"
& $Exe @common --rehearse --i-have-a-backup --report "$o\rehearse.json" --script-output "$o\script_rehearse.sql" > "$o\rehearse.txt" 2> "$o\rehearse.err"
"rehearse exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\rehearse.err" -Tail 5; throw 'the rehearsal failed' }
Log "real run"
& $Exe @common --i-have-a-backup --report "$o\real.json" --script-output "$o\script_real.sql" --recovery-dir "$o\recovery" > "$o\real.txt" 2> "$o\real.err"
"real exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\real.err" -Tail 5; throw 'the real run failed' }
$report = Get-Content "$o\real.json" -Raw -Encoding UTF8 | ConvertFrom-Json
"structure: " + (($report.structure.objects) -join ' | ')

Log "checks 2-6"
$env:TWIN_OUT = "$lab\out\port"
pwsh -NoProfile -File "$kit\twin_check.ps1" -Case $Case -Nat $nat -Own $own -Report "$o\real.json" -SnapshotNat 2>&1 | Tee-Object "$o\twin_check.txt" | Select-String -Pattern '^(\d\.|rebuilt|changed|16 of|DBNames text|entries that differ|only in|_|== only)' | ForEach-Object { $_.Line }

Log "check 7: native config apply on our twin"
pwsh -NoProfile -File "$kit\apply_only.ps1" -Database $own *> "$o\native_noop.txt"
Get-Content "$o\native_noop.txt" | Select-String -Pattern 'не требуется|exit=|INFO' | ForEach-Object { $_.Line }

Log "check 8: native exports and source-diff"
foreach ($side in 'nat', 'own') {
    pwsh -NoProfile -File "$kit\export_tree.ps1" -Database "ibcmd_rs_04_ddl_p_${Case}_$side" -Out "$lab\export\p_${Case}_$side" 2>&1 | Select-Object -Last 1
}
& $Exe source-diff "$lab\export\p_${Case}_nat" "$lab\export\p_${Case}_own" > "$o\export_diff.json" 2>&1
python -c "import json; d=json.load(open(r'$o\export_diff.json',encoding='utf-8')); print('export diff', d['summary'])"
Log "done $Case"
