# Wave-1 cases (S1-B/C/D): stage the edited files of a case on a pristine БСП clone with the native partial import,
# snapshot and back up the staged state, restore the two twins from the backup.
#   pwsh -NoProfile -File stage_case.ps1 -Case b1|b2|c1|d0|d1 [-Base bsp8327]
# Needs tree_s2\<case> made by edit_cases_s2.py. Databases: ibcmd_rs_04_ddl_s2_<case>_base / _nat / _own; the backup
# bak\ibcmd_rs_04_ddl_s2_<case>_base_<case>_staged.bak is the twin source. The native import runs under the lab lock.
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z][0-9]$')][string]$Case,
    [string]$Base = 'bsp8327'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$tools = "$lab\tools"
$db = "ibcmd_rs_04_ddl_s2_${Case}_base"
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
Log "restore $db"
# MIX_BASE_BAK: a base that already had a native apply (the cases that create an object: the platform stages a new object on such a base only)
$baseArgs = if ($env:MIX_BASE_BAK) { @('-Corpus', 'bak', '-Bak', $env:MIX_BASE_BAK) } else { @('-Corpus', $Base) }
pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 @baseArgs -Name $db -Track ddl -Purpose "S1 wave 1, case ${Case}: pristine БСП; the case is staged by the native partial import, then a COPY_ONLY backup for the twins" | Select-Object -Last 1
Log "import the files of the case"
$files = (Get-Content "$lab\tree_s2\$Case\files.txt" | Where-Object { $_ }) -join ','
pwsh -NoProfile -File "$tools\import_files.ps1" -Database $db -BaseDir "$lab\tree_s2\$Case\stage" -Files $files | Select-Object -Last 3
# a failed import stages nothing (the platform wants the files an object refers to -- its forms, templates -- in the base dir too, listed or not)
$staged = [int](sqlcmd -S localhost -E -C -h -1 -W -d $db -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave")
if ($staged -eq 0) { throw "the native import staged nothing (see $lab\logs\native-import-files-$db.err)" }
Log "snapshot the staged state"
Push-Location $tools
python snapshot.py $db "${Case}_staged" | Select-Object -Last 1
Pop-Location
$bak = "$lab\bak\${db}_${Case}_staged.bak"
Log "backup $bak"
sqlcmd -S localhost -E -C -b -Q "BACKUP DATABASE [$db] TO DISK = N'$bak' WITH COPY_ONLY, COMPRESSION, INIT" | Select-Object -Last 1
foreach ($side in 'nat', 'own') {
    Log "restore the $side twin"
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak $bak -Name "ibcmd_rs_04_ddl_s2_${Case}_$side" -Track ddl -Purpose "S1 wave 1, case ${Case}: twin ($side) of the staged state" | Select-Object -Last 1
}
Log "done: $db, _nat, _own"
