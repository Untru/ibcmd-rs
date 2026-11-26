# S1-K checkpoint 2: every built case through the whole chain with the binary given -- OUR import into a twin of the base, OUR apply
# against the native import + native apply (twin_case.ps1: checks 2-10 and the session of check 9), then check 12 (the injected failure
# on the script our real apply made); the refusal case i1 is check 11 instead. One log per case: out\<case>\checkpoint2.log.
#
#   pwsh -NoProfile -File run_checkpoint2.ps1 -Bin <ibcmd-rs.exe> [-Cases 'a1,b1,b2,c1,d0,d1,i1'] [-ImportArgs '...'] [-Drop]
#
# What each case needs is in the table below; a case without an editor (e1, f1: operations not built yet) is skipped with a note.
# The native side and the lab locks are those of twin_case.ps1 (one native command per hold). -Drop removes the databases of a
# case that passed (drop-lab-dbs.ps1, -MinIdleMinutes 0: they are this run's own); the logs and the evidence stay.
param(
    [Parameter(Mandatory = $true)][string]$Bin,
    [string]$Cases = 'a1,b1,b2,c1,d0,d1,i1',
    [string]$ImportArgs = '',
    # check 9 (a cluster session on both twins) takes minutes and two registrations: once, on these cases
    [string]$SessionCases = 'a1,f1',
    # the import phase was run already (run_phase1.ps1): the twins are staged, go on from there
    [switch]$SkipImport,
    # with -SkipImport: wait for each case's import phase (phase1.json newer than this) before its own side
    [datetime]$Since = [datetime]::MinValue,
    # the native sides run apart (native_side.ps1); twin_case.ps1 waits for each before it compares
    [switch]$NativeApart,
    [switch]$Drop
)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = if ($env:S1K_LAB) { $env:S1K_LAB } else { 'F:\ibcmd\lab\05\ext\s1k' }
$env:S1K_LAB = $lab
$kit = $PSScriptRoot
$jobs = Join-Path (Split-Path $kit -Parent) 'jobs'
# the session job of each case (check 9); i1 is refused, so it has none
$job = @{
    a1 = 's1k_a1.bsl'; b1 = 's2_b1.bsl'; b2 = 's2_b2.bsl'; c1 = 's2_c1.bsl'; d0 = 's2_d0.bsl'; d1 = 's2_d1.bsl'
    e5 = 's2_e.bsl'; e6 = 's2_e.bsl'; f1 = 's1k_f.bsl'; f2 = 's1k_f.bsl'
}
$sessions = @($SessionCases -split ',' | Where-Object { $_ })
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
$summary = @()
foreach ($case in ($Cases -split ',' | Where-Object { $_ })) {
    $o = "$lab\out\$case"
    New-Item -ItemType Directory -Force $o | Out-Null
    $log = "$o\checkpoint2.log"
    "" | Set-Content $log
    $verdict = 'ok'
    Log "== $case" | Tee-Object -FilePath $log -Append
    if (-not $SkipImport) {
        pwsh -NoProfile -File "$kit\import_phase.ps1" -Case $case -Bin $Bin -ImportArgs $ImportArgs *>&1 | Tee-Object -FilePath $log -Append | Out-Null
    } else {
        $marker = "$o\phase1.json"
        $deadline = (Get-Date).AddMinutes(180)
        while (-not ((Test-Path $marker) -and ((Get-Item $marker).LastWriteTime -gt $Since)) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 15 }
    }
    $phase = Get-Content "$o\phase1.json" -Raw -Encoding UTF8 -ErrorAction SilentlyContinue | ConvertFrom-Json
    if (-not $phase) { $summary += "$case : the import phase did not finish"; continue }
    if ($case -eq 'i1') {
        pwsh -NoProfile -File "$kit\check11.ps1" -Case $case -Bin $Bin -Native *>&1 | Tee-Object -FilePath $log -Append | Out-Null
        if ($LASTEXITCODE -ne 0) { $verdict = 'check 11 FAILED' }
    } elseif ($phase.import_exit -ne 0) {
        $verdict = "our import refused or failed (exit $($phase.import_exit)): nothing to apply"
    } else {
        $twinArgs = @('-Case', $case, '-Bin', $Bin, '-Job', (Join-Path $jobs $job[$case]))
        if ($sessions -notcontains $case) { $twinArgs += @('-Skip', 'session') }
        if ($NativeApart) { $twinArgs += '-SkipNative' }
        pwsh -NoProfile -File "$kit\twin_case.ps1" @twinArgs *>&1 | Tee-Object -FilePath $log -Append | Out-Null
        if ($LASTEXITCODE -ne 0) { $verdict = 'twin protocol stopped' }
        pwsh -NoProfile -File "$kit\check12.ps1" -Case $case *>&1 | Tee-Object -FilePath $log -Append | Out-Null
        if ($LASTEXITCODE -ne 0) { $verdict += '; check 12 FAILED' }
    }
    $summary += "{0} : {1} (log {2})" -f $case, $verdict, $log
    if ($Drop -and $verdict -eq 'ok') {
        pwsh -NoProfile -File F:\ibcmd\lab\04\tools\drop-lab-dbs.ps1 -Track ext -Names "ibcmd_rs_05_ext_s1k_${case}_own,ibcmd_rs_05_ext_s1k_${case}_nat" -MinIdleMinutes 0 -Execute | Select-Object -Last 2
        Remove-Item -Force "$o\staged.bak" -ErrorAction SilentlyContinue
    }
}
$summary
