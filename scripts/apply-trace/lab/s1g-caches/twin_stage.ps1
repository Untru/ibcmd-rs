# The twins of one native case of S1-F (n1..n5), from ONE staged state so that native and ours start from
# byte-identical ConfigSave rows (docs/apply/restructuring.md, 12.6):
#   1. a clone of the base that already had a native apply (c2 after native) -> ibcmd_rs_05_trace_<t>_st
#   2. the case staged by the native partial import (under the native lock), a full snapshot <case>_staged
#   3. a COPY_ONLY backup of it, and the two twins <t>_nat and <t>_own restored from the backup
#   4. -Native: the native apply on <t>_nat (under the native lock) and its full snapshot nat_after
#   pwsh -NoProfile -File twin_stage.ps1 -Case n1 -Tag tw1 [-Native]
# The stage rows come from F:\ibcmd\lab\05\s1g\n\<case> (see stage_case.ps1). Lab databases only.
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [Parameter(Mandatory = $true)][string]$Tag,
    [switch]$Native
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$env:DDL_LAB = 'F:\ibcmd\lab\05\s1g\store'
$env:DDL_LOCK_TRACK = 'trace'
$k = 'F:\ibcmd\lab\05\s1g\kit'
$ddlKit = Join-Path (Split-Path (Split-Path (Split-Path $PSScriptRoot))) 'restructure-lab'
$baseBak = 'F:\ibcmd\lab\04\restructure\bak\ibcmd_rs_04_ddl_bsp8327_c2_native_after.bak'
$st = "ibcmd_rs_05_trace_${Tag}_st"
$nat = "ibcmd_rs_05_trace_${Tag}_nat"
$own = "ibcmd_rs_05_trace_${Tag}_own"
$bak = "F:\ibcmd\lab\05\s1g\bak\${Case}_staged.bak"
New-Item -ItemType Directory -Force (Split-Path $bak) | Out-Null
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
$restore = 'F:\ibcmd\lab\04\tools\restore-clone.ps1'

if (-not (Test-Path -LiteralPath $bak)) {
    Log "staging clone $st"
    pwsh -NoProfile -File $restore -Corpus bak -Bak $baseBak -Name $st -Track trace -Purpose "S1-F twins of ${Case}: the staged state"
    pwsh -NoProfile -File "$k\stage_case.ps1" -Database $st -Case $Case -Steps import
    Log 'full snapshot of the staged state'
    Push-Location $ddlKit
    python snapshot.py $st "${Case}_staged" | Select-Object -Last 1
    Pop-Location
    sqlcmd -S localhost -E -C -b -Q "BACKUP DATABASE [$st] TO DISK = N'$bak' WITH COPY_ONLY, COMPRESSION, INIT, STATS = 100"
    if ($LASTEXITCODE -ne 0) { throw 'backup failed' }
}
foreach ($twin in $nat, $own) {
    Log "twin $twin"
    pwsh -NoProfile -File $restore -Corpus bak -Bak $bak -Name $twin -Track trace -Purpose "S1-F twin of $Case ($($twin -replace '.*_',''))"
}
if ($Native) {
    Log "native apply on $nat"
    pwsh -NoProfile -File "$k\stage_case.ps1" -Database $nat -Case $Case -Steps apply
    Push-Location $ddlKit
    python snapshot.py $nat 'nat_after' | Select-Object -Last 1
    Pop-Location
}
Log 'done'
