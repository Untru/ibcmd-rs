# One experiment on the derived `*.si` caches of a ddl-track lab database (research kit).
#   pwsh -NoProfile -File cache_variant.ps1 -Database <db> -Rows ea13a2c9,1a621f0f -Mode stale|native|delete -Job <file.bsl> -Label <name>
#     [-StaleSnap <db/label>] [-NativeSnap <db/label>] [-Port 5514] [-NoRun]
# Puts the named `*.si` rows (by name prefix; `siVersions` when named) into the wanted state --
#   stale:  the bytes of the snapshot -StaleSnap (the configuration before the change),
#   native: the bytes of the snapshot -NativeSnap (what the platform wrote),
#   delete: no row --
# starts a stand-alone server on the database, runs the job in a thin-client session, prints the result and the times, stops the server.
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [string[]]$Rows = @(),
    [ValidateSet('stale', 'native', 'delete', 'none')][string]$Mode = 'none',
    [string]$Job = '',
    [string]$Label = 'x',
    [switch]$NoRun,
    [string]$StaleSnap = 'ibcmd_rs_04_ddl_bsp8327_a/a2_staged',
    [string]$NativeSnap = 'ibcmd_rs_04_ddl_bsp8327_c2/c2_now',
    [int]$Port = 5514
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$tools = "$lab\tools"
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only (got $Database)" }
# `pwsh -File` passes one string: -Rows a,b,c arrives as 'a,b,c'
$Rows = @($Rows | ForEach-Object { $_ -split ',' } | Where-Object { $_ })

function Read-Svc([string]$snap) {
    $db, $label = $snap -split '/', 2
    Get-Content "$lab\snap\$db\$label\svc.json" -Raw -Encoding UTF8 | ConvertFrom-Json
}
function Stop-Server { pwsh -NoProfile -File "$tools\srv.ps1" stop -Port $Port | Out-Null }

Stop-Server
if ($Mode -ne 'none') {
    $svc = Read-Svc $(if ($Mode -eq 'stale') { $StaleSnap } else { $NativeSnap })
    $names = @()
    foreach ($prefix in $Rows) {
        $found = @($svc.Params | Where-Object { $_.part -eq 0 -and ($_.name -like "$prefix*") } | ForEach-Object { $_.name })
        if ($found.Count -eq 0) { throw "no Params row starts with '$prefix' in the snapshot" }
        $names += $found
    }
    foreach ($name in $names) {
        if ($Mode -eq 'delete') {
            pwsh -NoProfile -File "$tools\params_row.ps1" delete -Database $Database -Name $name | Out-Null
            "delete  $name"
        } else {
            $entry = $svc.Params | Where-Object { $_.name -eq $name -and $_.part -eq 0 } | Select-Object -First 1
            $blob = "$lab\blobs\$($entry.sha)"
            pwsh -NoProfile -File "$tools\params_row.ps1" put -Database $Database -Name $name -Blob $blob | Out-Null
            "$Mode  $name ($($entry.len) bytes)"
        }
    }
}
if ($NoRun) { return }
if (-not $Job) { throw '-Job is required unless -NoRun' }
$out = "$lab\logs\ibsrv_$Port.out"
if (Test-Path $out) { [IO.File]::Delete($out) }
$sw = [Diagnostics.Stopwatch]::StartNew()
$starter = Start-Process pwsh -ArgumentList @('-NoProfile', '-File', "$tools\srv.ps1", 'start', '-DbName', $Database, '-Plat', '8327', '-Port', "$Port") -PassThru -WindowStyle Hidden -RedirectStandardOutput "$lab\logs\srv_start_$Port.log" -RedirectStandardError "$lab\logs\srv_start_$Port.err"
$deadline = (Get-Date).AddSeconds(240)
while ((Get-Date) -lt $deadline) {
    if ((Test-Path $out) -and ((Get-Content $out -Raw -ErrorAction SilentlyContinue) -match 'ready')) { break }
    if ($starter.HasExited -and $starter.ExitCode -ne 0) { throw "server start failed: $(Get-Content "$lab\logs\srv_start_$Port.err" -Raw)" }
    Start-Sleep -Milliseconds 500
}
if (-not ((Test-Path $out) -and ((Get-Content $out -Raw) -match 'ready'))) { Stop-Server; throw 'the server did not become ready in 240 s' }
"server ready in {0:n1} s" -f $sw.Elapsed.TotalSeconds
$sw.Restart()
$result = pwsh -NoProfile -File "$tools\session_job.ps1" -Job $Job -Port $Port -TimeoutSec 400 2>&1
"job took {0:n1} s" -f $sw.Elapsed.TotalSeconds
$result | Tee-Object -FilePath "$lab\out\cache_$Label.txt"
Stop-Server
