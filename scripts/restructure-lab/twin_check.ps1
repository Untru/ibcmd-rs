# The offline part of the twin protocol (docs/apply/restructuring.md, 12.6, checks 2-6) for a case whose twins are
# both applied: the native one (snapshot <NatLabel> taken) and ours (run through the apply; this script takes its snapshot).
#   pwsh -NoProfile -File twin_check.ps1 -Case b1 -Nat ibcmd_rs_04_ddl_s2_b1_nat -Own ibcmd_rs_04_ddl_s2_b1_own -Report <report.json of the real run>
# Writes out\s2_<Case>_twin\{diff,except,config,dbschema,si}.txt and prints the verdict lines. Read-only for the
# databases (the snapshot only reads). The rebuilt tables come from the report (through_apply.structure.tables).
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [Parameter(Mandatory = $true)][string]$Nat,
    [Parameter(Mandatory = $true)][string]$Own,
    [Parameter(Mandatory = $true)][string]$Report,
    [string]$NatLabel = 'nat_after',
    [string]$OwnLabel = 'own_after',
    [switch]$SkipSnapshot,
    [switch]$SnapshotNat
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$out = if ($env:TWIN_OUT) { "$env:TWIN_OUT\${Case}_twin" } else { "$lab\out\s2_${Case}_twin" }
New-Item -ItemType Directory -Force $out | Out-Null
$json = Get-Content -LiteralPath $Report -Raw -Encoding UTF8 | ConvertFrom-Json
# the report of mssql-restructure --through-apply nests the apply's report; mssql-config-apply prints it as it is
$structure = if ($json.through_apply) { $json.through_apply.structure } else { $json.structure }
$tables = @($structure.tables) -join ','
"rebuilt tables: $tables"
Push-Location $PSScriptRoot
try {
    if ($SnapshotNat) {
        "snapshot $Nat $NatLabel"
        python snapshot.py $Nat $NatLabel | Select-Object -Last 1
    }
    if (-not $SkipSnapshot) {
        "snapshot $Own $OwnLabel"
        python snapshot.py $Own $OwnLabel | Select-Object -Last 1
    }
    "2. tables, columns, indexes"
    python snapdiff.py $Nat $NatLabel $Own $OwnLabel > "$out\diff.txt"
    Get-Content "$out\diff.txt" -TotalCount 40
    "3. data of the rebuilt tables (EXCEPT both ways)"
    pwsh -NoProfile -File "$PSScriptRoot\compare_tables.ps1" -A $Nat -B $Own -Tables $tables -Out "$out\except.txt"
    Get-Content "$out\except.txt"
    "4. Config rows"
    pwsh -NoProfile -File "$PSScriptRoot\compare_config.ps1" -A $Nat -B $Own -Out "$out\config.txt"
    Get-Content "$out\config.txt" -TotalCount 20
    "5. DBSchema entries and DBNames"
    python dbschema_cmp.py $Nat $NatLabel $Own $OwnLabel > "$out\dbschema.txt"
    Get-Content "$out\dbschema.txt" -TotalCount 20
    "6. the .si rows"
    python si_diff.py $Nat $NatLabel $Own $OwnLabel > "$out\si.txt"
    Get-Content "$out\si.txt" -TotalCount 20
} finally {
    Pop-Location
}
