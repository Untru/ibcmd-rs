# Machine load (total CPU %, free RAM GB) every 5 s: the lab shares the workstation with other tracks, so timings need this context.
param([Parameter(Mandatory = $true)][string]$Out, [int]$MaxSeconds = 3600, [string]$StopFile = '')
"utc`tcpu_pct`tfree_ram_gb" | Set-Content -LiteralPath $Out -Encoding ascii
$deadline = (Get-Date).AddSeconds($MaxSeconds)
while ((Get-Date) -lt $deadline) {
    if ($StopFile -and (Test-Path -LiteralPath $StopFile)) { break }
    $cpu = (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average
    $ram = [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB, 1)
    Add-Content -LiteralPath $Out -Encoding ascii -Value ("{0}`t{1}`t{2}" -f [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fffZ'), $cpu, $ram)
    Start-Sleep -Seconds 5
}
