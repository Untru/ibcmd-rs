# Starts / stops the two background samplers of a run (SQL state from master; sessions/connections through RAS).
#   pwsh -NoProfile -File timelines.ps1 start -Run <run> -Database <db> [-MaxSeconds 3600]
#   pwsh -NoProfile -File timelines.ps1 stop  -Run <run>
# Output: runs\<run>\timeline-sql.tsv, runs\<run>\timeline-rac.tsv ; pids in runs\<run>\timelines.pids ; stop flag runs\<run>\timelines.stop
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('start', 'stop')][string]$Action,
    [Parameter(Mandatory = $true)][string]$Run,
    [string]$Database = '',
    [int]$MaxSeconds = 3600
)
$lab = 'F:\ibcmd\lab\05\online'
$dir = Join-Path $lab "runs\$Run"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$stopFile = Join-Path $dir 'timelines.stop'
$pids = Join-Path $dir 'timelines.pids'
if ($Action -eq 'start') {
    if (-not $Database) { throw '-Database is required' }
    if (Test-Path -LiteralPath $stopFile) { Remove-Item -LiteralPath $stopFile }
    $sql = Start-Process -FilePath 'powershell.exe' -PassThru -WindowStyle Hidden -ArgumentList @(
        '-NoProfile', '-File', "`"$lab\tools\sql-timeline.ps1`"", '-Database', $Database, '-Out', "`"$dir\timeline-sql.tsv`"",
        '-IntervalMs', '100', '-MaxSeconds', $MaxSeconds, '-StopFile', "`"$stopFile`"")
    $rac = Start-Process -FilePath 'pwsh.exe' -PassThru -WindowStyle Hidden -ArgumentList @(
        '-NoProfile', '-File', "`"$lab\tools\rac-timeline.ps1`"", '-Database', $Database, '-Out', "`"$dir\timeline-rac.tsv`"",
        '-IntervalMs', '1000', '-MaxSeconds', $MaxSeconds, '-StopFile', "`"$stopFile`"")
    $load = Start-Process -FilePath 'pwsh.exe' -PassThru -WindowStyle Hidden -ArgumentList @(
        '-NoProfile', '-File', "`"$lab\tools\load-sampler.ps1`"", '-Out', "`"$dir\load.tsv`"", '-MaxSeconds', $MaxSeconds, '-StopFile', "`"$stopFile`"")
    Set-Content -LiteralPath $pids -Value "$($sql.Id)`n$($rac.Id)`n$($load.Id)" -Encoding ascii
    "samplers started: sql pid $($sql.Id), rac pid $($rac.Id), load pid $($load.Id)"
} else {
    Set-Content -LiteralPath $stopFile -Value 'stop' -Encoding ascii
    if (Test-Path -LiteralPath $pids) {
        foreach ($id in (Get-Content -LiteralPath $pids)) {
            $p = Get-Process -Id ([int]$id) -ErrorAction SilentlyContinue
            if ($p) {
                if (-not $p.WaitForExit(15000)) { Stop-Process -Id $p.Id -Force; "killed sampler pid $($p.Id)" } else { "sampler pid $($p.Id) ended" }
            }
        }
    }
}
