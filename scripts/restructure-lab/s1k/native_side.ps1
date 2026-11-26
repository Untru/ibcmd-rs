# S1-K: the native side of the twin protocol of several cases, started as soon as the edit of each case is on disk (its import phase has
# run, `out\<case>\phase1.json` is newer than -Since), so that the native import and apply wait in the queue of the native lock while
# the own side is still being staged. Then twin_case.ps1 -SkipNative does the own side.
#
#   pwsh -NoProfile -File native_side.ps1 -Bin <ibcmd-rs.exe> -Cases "c1,d0,..." -Since "2026-09-30T08:40:00"
param(
    [Parameter(Mandatory = $true)][string]$Bin,
    [Parameter(Mandatory = $true)][string]$Cases,
    [Parameter(Mandatory = $true)][datetime]$Since,
    [int]$WaitMinutes = 120
)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = if ($env:S1K_LAB) { $env:S1K_LAB } else { 'F:\ibcmd\lab\05\ext\s1k' }
$env:S1K_LAB = $lab
$kit = $PSScriptRoot
foreach ($case in ($Cases -split ',' | Where-Object { $_ })) {
    $marker = "$lab\out\$case\phase1.json"
    $deadline = (Get-Date).AddMinutes($WaitMinutes)
    while (-not ((Test-Path $marker) -and ((Get-Item $marker).LastWriteTime -gt $Since))) {
        if ((Get-Date) -gt $deadline) { "[$case] the edit never appeared"; break }
        Start-Sleep -Seconds 15
    }
    if ($case -eq 'i1') { "[$case] refused case: no native side"; continue }
    $phase = Get-Content $marker -Raw -Encoding UTF8 -ErrorAction SilentlyContinue | ConvertFrom-Json
    if ($phase -and $phase.import_exit -ne 0) { "[$case] our import did not stage: no native side needed yet"; continue }
    "[{0}] native side of {1}" -f (Get-Date -Format s), $case
    pwsh -NoProfile -File "$kit\twin_case.ps1" -Case $case -Bin $Bin -NativeOnly *>&1 | Tee-Object "$lab\out\$case\native_side.log"
}
