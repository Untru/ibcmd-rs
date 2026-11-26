# S1-K checkpoint 1: the import phase of every built case, one after another, with the binary given.
#   pwsh -NoProfile -File run_phase1.ps1 -Bin <ibcmd-rs.exe> [-Cases 'a1,b1,...'] [-ImportArgs '...']
# The log of each case is out\<case>\phase1.log under the lab; the one-line summaries are printed at the end.
param(
    [Parameter(Mandatory = $true)][string]$Bin,
    [string]$Cases = 'a1,b1,b2,c1,d0,d1,i1',
    [string]$ImportArgs = ''
)
$lab = if ($env:S1K_LAB) { $env:S1K_LAB } else { 'F:\ibcmd\lab\05\ext\s1k' }
$list = $Cases -split ','
foreach ($case in $list) {
    New-Item -ItemType Directory -Force "$lab\out\$case" | Out-Null
    pwsh -NoProfile -File "$PSScriptRoot\import_phase.ps1" -Case $case -Bin $Bin -ImportArgs $ImportArgs *> "$lab\out\$case\phase1.log"
}
foreach ($case in $list) {
    $log = Get-Content "$lab\out\$case\phase1.log" -Encoding UTF8
    $exit = ($log | Select-String -Pattern '^import exit' | Select-Object -First 1).Line
    $why = ($log | Select-String -Pattern '^\[ERROR\]' | Select-Object -First 1).Line
    $rows = ($log | Select-String -Pattern '"configsave_rows"' | Select-Object -First 1).Line
    "{0}: {1} | {2} | {3}" -f $case, $exit, $rows.Trim(), $(if ($why.Length -gt 260) { $why.Substring(0, 260) } else { $why })
}
