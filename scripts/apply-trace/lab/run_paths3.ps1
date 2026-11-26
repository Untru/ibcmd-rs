# X3: the apply of the natively staged sparse import (validate_sparse_import.ps1): an object module (container kind) and a
# managed form module (text inside the form row) of the БСП demo, 7 ConfigSave rows.  One native apply under the native lock,
# the platform's log lines and the register ids tell the path (same summary.tsv as run_paths.ps1 / run_paths2.ps1).
param([string]$Database = 'ibcmd_rs_04_trace_c4_paths')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
function Sql($q) { (& sqlcmd -S localhost -E -C -d $Database -h -1 -W -Q "SET NOCOUNT ON; $q" | Out-String).Trim() }
$rows = Sql 'SELECT COUNT(*) FROM ConfigSave'
$idBefore = Sql 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
& pwsh -NoProfile -File $lock acquire trace -Name native -TimeoutMin 240
if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' }
try {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost "--db-name=$Database" "--data=$lab\ibdata\$Database" --user=Администратор --force --dynamic=disable *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\paths\X3-apply.log"
    $exit = $LASTEXITCODE
    $sec = [Math]::Round($sw.Elapsed.TotalSeconds, 1)
} finally {
    & pwsh -NoProfile -File $lock release trace -Name native
}
$idAfter = Sql 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
$log = Get-Content -LiteralPath "$lab\paths\X3-apply.log" -Raw
$line = "X3`tnative sparse stage: catalog object module (container) + managed form module (form row)`tstaged=$rows`texit=$exit`t${sec}s`tstructure_phase=$($log -match 'Обработка структуры базы данных')`texchange_registration=$($log -match 'Регистрация изменений в планах обмена')`tregister_rebuilt_log=$($log -match 'Реструктуризация\s+Таблица регистрации')`tregister_ids_changed=$($idBefore -ne $idAfter)`t$(Get-Date -Format s)"
Add-Content -LiteralPath "$lab\paths\summary.tsv" -Value $line -Encoding utf8
$line
