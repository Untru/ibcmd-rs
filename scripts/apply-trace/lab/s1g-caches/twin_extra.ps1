# Checks 7, 8 and 9 of the twin protocol (docs/apply/restructuring.md, 12.6) for a case of S1-F, on trace-track twins
# (the ddl kit's scripts of the same names accept ibcmd_rs_04_ddl_* databases only):
#   noop     7. a native `config apply` on our twin says there is nothing to apply (under the native lock)
#   export   8. native `config export` of both twins and `ibcmd-rs source-diff` of the two trees
#   session  9. a session job (f_created.bsl) in the 8.3.27 cluster on both twins, outputs compared
#   pwsh -NoProfile -File twin_extra.ps1 -Nat <db> -Own <db> -Label n1 [-Steps noop,export,session] [-Exe <ibcmd-rs.exe>]
param(
    [Parameter(Mandatory = $true)][string]$Nat,
    [Parameter(Mandatory = $true)][string]$Own,
    [Parameter(Mandatory = $true)][string]$Label,
    [string[]]$Steps = @('noop', 'export', 'session'),
    [string]$Exe = 'F:\ibcmd\lab\05\s1g\bin\ibcmd-rs-f.exe',
    [string]$Job = "$PSScriptRoot\f_created.bsl"
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$Steps = @($Steps | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
foreach ($db in $Nat, $Own) {
    if ($db -notmatch '^ibcmd_rs_05_trace_[a-z0-9_]+$') { throw "trace-track lab databases only (got $db)" }
}
$env:DDL_LOCK_TRACK = 'trace'
. 'F:\ibcmd\lab\04\restructure\tools\native_lock.ps1'
$k = 'F:\ibcmd\lab\05\s1g\kit'
$out = "F:\ibcmd\lab\05\s1g\out\extra_$Label"
New-Item -ItemType Directory -Force $out | Out-Null
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
function Native-Base($db) {
    $data = "$k\ibdata\$db"
    New-Item -ItemType Directory -Force $data | Out-Null
    return @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data", '--user=Администратор')
}

if ($Steps -contains 'noop') {
    Log "check 7: native config apply on $Own"
    $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments (@('infobase', 'config', 'apply') + (Native-Base $Own) + @('--force', '--dynamic=disable')) -Log "$out\native_noop" -TimeoutSec 3600
    "exit $rc"
    Get-Content "$out\native_noop.out" -Tail 4 -ErrorAction SilentlyContinue
}
if ($Steps -contains 'export') {
    Log 'check 8: native exports and source-diff'
    foreach ($db in $Nat, $Own) {
        $dir = "F:\ibcmd\lab\05\s1g\export\$db"
        if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
        New-Item -ItemType Directory -Force $dir | Out-Null
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments (@('infobase', 'config', 'export') + (Native-Base $db) + @('--threads=4', $dir)) -Log "$out\export_$db" -TimeoutSec 3600 -NoLock
        "export $db exit=$rc in {0:n0}s" -f $sw.Elapsed.TotalSeconds
    }
    & $Exe source-diff "F:\ibcmd\lab\05\s1g\export\$Nat" "F:\ibcmd\lab\05\s1g\export\$Own" > "$out\export_diff.json" 2> "$out\export_diff.err"
    $d = Get-Content "$out\export_diff.json" -Raw -Encoding UTF8 | ConvertFrom-Json
    "export diff: " + ($d.summary | ConvertTo-Json -Compress)
}
if ($Steps -contains 'session') {
    Log 'check 9: cluster session on both twins'
    $reg = 'F:\ibcmd\lab\04\tools\register-ib.ps1'
    foreach ($side in 'nat', 'own') {
        $db = if ($side -eq 'nat') { $Nat } else { $Own }
        pwsh -NoProfile -File $reg register -Database $db -Platform 8.3.27 -Track trace 2>&1 | Select-Object -Last 1
        try {
            pwsh -NoProfile -File "$k\job.ps1" -Job $Job -Database $db -TimeoutSec 900 > "$out\session_$side.txt" 2>&1
        } finally {
            pwsh -NoProfile -File $reg unregister -Database $db -Platform 8.3.27 -Track trace 2>&1 | Select-Object -Last 1
        }
    }
    $a = Get-Content "$out\session_nat.txt" -Raw
    $b = Get-Content "$out\session_own.txt" -Raw
    "session outputs identical: " + ($a -eq $b) + " (" + (Get-Content "$out\session_own.txt").Count + " lines)"
}
Log "done $Label"
