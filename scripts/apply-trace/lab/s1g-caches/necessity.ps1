# The necessity of the derived caches (docs/apply/restructuring.md 12.5) measured on a lab database of the
# trace track, row by row: one row absent or stale at a time, a stand-alone server on the database and a
# thin-client session that runs the probe of the new catalog. The rows the database holds when the script
# starts are "good" (native's or ours); before every variant all of them are put back.
#   pwsh -NoProfile -File necessity.ps1 -Database ibcmd_rs_05_trace_x -Label ours [-Variants absent:1a621f0f,stale:1a621f0f,...]
# The stale bytes are the stored rows of the state before the case (F:\ibcmd\lab\05\s1g\rows\stale).
# Report: F:\ibcmd\lab\05\s1g\kit\out\necessity_<Label>.txt
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Label,
    [string[]]$Variants = @('absent:1a621f0f', 'stale:1a621f0f', 'absent:a07b62f0', 'stale:a07b62f0', 'absent:2203278d', 'stale:2203278d', 'absent:ea13a2c9', 'stale:ea13a2c9'),
    [string]$Job = 'F:\ibcmd\lab\04\restructure\probe\jobs\newcat.bsl',
    [int]$Port = 5614,
    [int]$JobTimeout = 150
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$Variants = @($Variants | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if ($Database -notmatch '^ibcmd_rs_05_trace_[a-z0-9_]+$') { throw "trace-track lab databases only (got $Database)" }
$k = 'F:\ibcmd\lab\05\s1g\kit'
$staleDir = 'F:\ibcmd\lab\05\s1g\rows\stale'
$goodDir = "F:\ibcmd\lab\05\s1g\rows\good_$Label"
New-Item -ItemType Directory -Force $goodDir, "$k\out", "$k\logs" | Out-Null
$report = "$k\out\necessity_$Label.txt"
Set-Content -LiteralPath $report -Encoding utf8 -Value ("necessity {0} on {1} at {2}" -f $Label, $Database, (Get-Date -Format s))
function Note([string]$text) { Add-Content -LiteralPath $report -Encoding utf8 -Value $text; Write-Host $text }
$all = @('1a621f0f-5568-4183-bd9f-f6ef670e7090.si', '2203278d-ef4f-4f68-98f1-feb257d53ecc.si', '42ed49cc-765d-4314-bc2d-af425af7bf13.si',
         'a07b62f0-1f01-484a-93d9-d42764cedac0.si', 'c4629235-4823-4320-b8b5-1d08f4c6d612.si', 'ea13a2c9-0c2f-40fa-b855-710387e3271d.si',
         'facbfffe-feb2-4d30-8930-a557b185e5c4.si', 'fe8acd6a-22c9-4b5a-aeae-232a1c8324cb.si')
function RowName([string]$prefix) { $all | Where-Object { $_ -like "$prefix*" } | Select-Object -First 1 }
function Stop-Srv { pwsh -NoProfile -File "$k\srv.ps1" stop -Port $Port | Out-Null }

Stop-Srv
foreach ($row in $all) { pwsh -NoProfile -File "$k\dbrow.ps1" dump -Database $Database -Name $row -Out "$goodDir\$row" | Out-Null }
Note "good rows dumped to $goodDir"

foreach ($variant in $Variants) {
    $mode, $prefix = $variant -split ':'
    $row = RowName $prefix
    Note "== $variant ($row)"
    foreach ($good in $all) { pwsh -NoProfile -File "$k\dbrow.ps1" put -Database $Database -Name $good -Blob "$goodDir\$good" | Out-Null }
    if ($mode -eq 'absent') {
        pwsh -NoProfile -File "$k\dbrow.ps1" delete -Database $Database -Name $row | Out-Null
    } elseif ($mode -eq 'stale') {
        pwsh -NoProfile -File "$k\dbrow.ps1" put -Database $Database -Name $row -Blob "$staleDir\$row.deflated" | Out-Null
    } elseif ($mode -ne 'good') { throw "unknown mode $mode" }
    $srvOut = "$k\logs\ibsrv_$Port.out"
    if (Test-Path $srvOut) { [IO.File]::Delete($srvOut) }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    try {
        $starter = Start-Process pwsh -ArgumentList @('-NoProfile', '-File', "$k\srv.ps1", 'start', '-DbName', $Database, '-Plat', '8327', '-Port', "$Port") -PassThru -WindowStyle Hidden -RedirectStandardOutput "$k\logs\srv_start_$Port.log" -RedirectStandardError "$k\logs\srv_start_$Port.err"
        $deadline = (Get-Date).AddSeconds(150)
        $ready = $false
        while ((Get-Date) -lt $deadline) {
            if ((Test-Path $srvOut) -and ((Get-Content $srvOut -Raw -ErrorAction SilentlyContinue) -match 'ready')) { $ready = $true; break }
            if ($starter.HasExited -and $starter.ExitCode -ne 0) { break }
            Start-Sleep -Milliseconds 500
        }
        if (-not $ready) {
            $why = (Get-Content "$k\logs\srv_start_$Port.err" -Raw -ErrorAction SilentlyContinue)
            $err = (Get-Content "$k\logs\ibsrv_$Port.err" -Raw -ErrorAction SilentlyContinue)
            $why = ([string]$why -replace '\s+', ' ').Trim()
            $err = ([string]$err -replace '\s+', ' ').Trim()
            Note ("  the server did not become ready ({0:n1} s): {1} {2}" -f $sw.Elapsed.TotalSeconds, $why, $err)
            continue
        }
        Note ("  server ready in {0:n1} s" -f $sw.Elapsed.TotalSeconds)
        $sw.Restart()
        try {
            $result = & pwsh -NoProfile -File "$k\job.ps1" -Job $Job -Port $Port -TimeoutSec $JobTimeout 2>&1
            Note ("  job took {0:n1} s" -f $sw.Elapsed.TotalSeconds)
            $result | ForEach-Object { Note "  | $_" }
        } catch {
            Note ("  job failed after {0:n1} s: {1}" -f $sw.Elapsed.TotalSeconds, ($_.Exception.Message -replace '\s+', ' '))
        }
    } finally {
        Stop-Srv
    }
}
# leave the good rows in place
foreach ($good in $all) { pwsh -NoProfile -File "$k\dbrow.ps1" put -Database $Database -Name $good -Blob "$goodDir\$good" | Out-Null }
Note 'good rows put back'
Note 'done'
