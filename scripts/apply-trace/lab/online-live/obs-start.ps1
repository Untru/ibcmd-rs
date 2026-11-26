# Starts one observer client (1cv8c.exe thin client, /Execute IbcmdRsObserver.epf) on a registered lab infobase and
# waits until it has written its first `open` line.
#   pwsh -NoProfile -File obs-start.ps1 -Database <db> -Label <label> [-Mode poll|txn] [-Thick] [-TimeoutSec 180]
# Writes obs\<label>.log (the observer's own journal), obs\<label>.pid, obs\<label>.start.txt (UTC bookends).
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Label,
    [ValidateSet('poll', 'txn', 'lazy')][string]$Mode = 'poll',
    [switch]$Thick,
    [int]$TimeoutSec = 180
)
. F:\ibcmd\lab\05\online\tools\lab-env.ps1
if ($Database -notmatch '^ibcmd_rs_0[45]_[a-z0-9_]+$') { throw 'lab databases only' }
if ($Label -notmatch '^[a-z0-9-]+$') { throw 'label: [a-z0-9-]+' }
$log = Join-Path $Lab "obs\$Label.log"
if (Test-Path -LiteralPath $log) { throw "journal exists already: $log" }
$exe = Join-Path $Bin ($(if ($Thick) { '1cv8.exe' } else { '1cv8c.exe' }))
$outLog = Join-Path $Lab "obs\$Label.client-out.txt"
$cmdLine = "ENTERPRISE /S`"$Srvr\$Database`" /N`"Администратор`" /P`"`" /Execute`"$Epf`" /C`"$Label;$Mode`" /DisableStartupMessages /DisableStartupDialogs /L ru /Out`"$outLog`" -NoTruncate"
$t0 = Now-Iso
$p = Start-Process -FilePath $exe -ArgumentList $cmdLine -PassThru
Set-Content -LiteralPath (Join-Path $Lab "obs\$Label.pid") -Value $p.Id -Encoding ascii
$deadline = (Get-Date).AddSeconds($TimeoutSec)
$opened = $false
while ((Get-Date) -lt $deadline) {
    if (Test-Path -LiteralPath $log) { $opened = $true; break }
    if ($p.HasExited) { break }
    Start-Sleep -Milliseconds 250
}
$t1 = Now-Iso
Set-Content -LiteralPath (Join-Path $Lab "obs\$Label.start.txt") -Encoding utf8 -Value "label=$Label`nmode=$Mode`nclient=$(Split-Path $exe -Leaf)`npid=$($p.Id)`nlaunched_utc=$t0`nfirst_journal_line_utc=$t1`nopened=$opened`nexited=$($p.HasExited)"
if (-not $opened) {
    $titles = Get-Process -Id $p.Id -ErrorAction SilentlyContinue | ForEach-Object { $_.MainWindowTitle }
    "NOT OPENED within $TimeoutSec s; exited=$($p.HasExited); window title='$titles'"
    exit 2
}
"opened: $Label pid=$($p.Id) launched=$t0 journal=$t1"
