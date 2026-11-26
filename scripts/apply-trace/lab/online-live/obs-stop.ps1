# Stops observer clients that THIS lab started (by pid file, and only if the command line still names the lab database).
#   pwsh -NoProfile -File obs-stop.ps1 [-Label <label>] [-All]
param(
    [string]$Label = '',
    [switch]$All
)
$obs = 'F:\ibcmd\lab\05\online\obs'
$files = if ($All) { Get-ChildItem -LiteralPath $obs -Filter '*.pid' } elseif ($Label) { Get-ChildItem -LiteralPath $obs -Filter "$Label.pid" } else { throw 'give -Label or -All' }
foreach ($f in $files) {
    $id = [int](Get-Content -LiteralPath $f.FullName -Raw).Trim()
    $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$id" -ErrorAction SilentlyContinue
    if (-not $proc) { "$($f.BaseName): pid $id already gone"; continue }
    if ($proc.CommandLine -notmatch 'ibcmd_rs_0[45]_' -or $proc.CommandLine -notmatch [regex]::Escape($f.BaseName)) {
        "$($f.BaseName): pid $id is not our observer any more; left alone"; continue
    }
    Stop-Process -Id $id -Force
    "$($f.BaseName): stopped pid $id"
}
