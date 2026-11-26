# Path experiment, fourth round (native-apply-trace.md, section 6.4): NATIVE staging of a sparse set of module edits, so that the
# stage holds no form rows re-rendered by our writer.  One arm per call; each native command holds the native lock alone
# (import files, then apply).
#   Y8  catalog object module + document manager module (two data objects, both container-kind modules, no form row)
#   Y9  the form module of ONE catalog form (one data object, the module text inside the form row)
#   Y10 common module + document manager module (one non-data object + one data object)
#   Y11 object modules of two catalogs (two data objects of one kind)
#   Y12 two common modules (two objects without tables)
#   Y14 common module + catalog object module staged natively (Y2 staged the same set with our tool and went short)
param([Parameter(Mandatory = $true)][ValidateSet('Y8', 'Y9', 'Y10', 'Y11', 'Y12', 'Y14')][string]$Id)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$out = "$lab\paths"
function Log($m) { Write-Host ('[{0}] PATHS6 {1} {2}' -f (Get-Date -Format 'HH:mm:ss'), $Id, $m) }
$plans = @{
    Y8 = @{ Db = 'ibcmd_rs_04_trace_c5_doc'; Files = @('Catalogs/_ДемоКонтрагенты/Ext/ObjectModule.bsl', 'Documents/_ДемоПоручениеЭкспедитору/Ext/ManagerModule.bsl')
            Note = 'native sparse stage: catalog object module + document manager module (two data objects, container modules only)' }
    Y9 = @{ Db = 'ibcmd_rs_04_trace_c5_union'; Files = @('Catalogs/_ДемоГруппыДоступаНоменклатуры/Forms/ФормаЭлемента/Ext/Form/Module.bsl')
            Note = 'native sparse stage: the form module of one catalog form (one data object, module text inside the form row)' }
    Y10 = @{ Db = 'ibcmd_rs_04_trace_c5_pair'; Files = @('CommonModules/_ДемоЛокализацияКлиентСервер/Ext/Module.bsl', 'Documents/_ДемоПоручениеЭкспедитору/Ext/ManagerModule.bsl')
             Note = 'native sparse stage: common module + document manager module (one object without tables, one with)' }
    Y11 = @{ Db = 'ibcmd_rs_04_trace_c4_x4'; Files = @('Catalogs/_ДемоКонтрагенты/Ext/ObjectModule.bsl', 'Catalogs/_ДемоОрганизации/Ext/ObjectModule.bsl')
             Note = 'native sparse stage: object modules of two catalogs (two data objects of one kind)' }
    Y14 = @{ Db = 'ibcmd_rs_04_trace_c5_union'; Files = @('CommonModules/_ДемоЛокализацияКлиентСервер/Ext/Module.bsl', 'Catalogs/_ДемоКонтрагенты/Ext/ObjectModule.bsl')
             Note = 'native sparse stage: common module + catalog object module (Y2 staged the same set with our tool)' }
    Y12 = @{ Db = 'ibcmd_rs_04_trace_c4_paths'; Files = @('CommonModules/_ДемоЛокализацияКлиентСервер/Ext/Module.bsl', 'CommonModules/_ДемоЛокализация/Ext/Module.bsl')
             Note = 'native sparse stage: two common modules (two objects without tables)' }
}
$p = $plans[$Id]
$db = $p.Db
function Sql($q) { (& sqlcmd -S localhost -E -C -d $db -h -1 -W -Q "SET NOCOUNT ON; $q" | Out-String).Trim() }
$base = "$lab\sparse_$Id"
if (Test-Path -LiteralPath $base) { Remove-Item -LiteralPath $base -Recurse -Force -Confirm:$false }
foreach ($f in $p.Files) {
    $dst = Join-Path $base ($f -replace '/', '\')
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dst) | Out-Null
    [IO.File]::WriteAllBytes($dst, [IO.File]::ReadAllBytes((Join-Path $ref ($f -replace '/', '\'))) + [Text.Encoding]::UTF8.GetBytes("`r`n// ibcmd-rs trace, path experiment $Id`r`n"))
}
New-Item -ItemType Directory -Force -Path "$lab\ibdata\$db" | Out-Null
if ((Sql 'SELECT COUNT(*) FROM ConfigSave') -ne '0') { throw "ConfigSave of $db is not empty" }
$idBefore = Sql 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'

Log 'waiting for the native lock (import files)'
& pwsh -NoProfile -File $lock acquire trace -Name native -TimeoutMin 240
if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' }
try {
    & $ibcmd infobase config import files --dbms=MSSQLServer --db-server=localhost "--db-name=$db" "--data=$lab\ibdata\$db" --user=Администратор "--base-dir=$base" --partial @($p.Files) *>&1 | Out-File -Encoding utf8 -LiteralPath "$out\$Id-import.log"
    $importExit = $LASTEXITCODE
} finally {
    & pwsh -NoProfile -File $lock release trace -Name native
}
if ($importExit -ne 0) { throw "native import files failed ($importExit)" }
$rows = Sql 'SELECT COUNT(*) FROM ConfigSave'
$env:PYTHONIOENCODING = 'utf-8'
Log "staged $rows rows"
& python (Join-Path $PSScriptRoot 'check_stage.py') $db
if ($LASTEXITCODE -ne 0) { throw 'the native stage is not complete' }

Log 'waiting for the native lock (apply)'
& pwsh -NoProfile -File $lock acquire trace -Name native -TimeoutMin 240
if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' }
try {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost "--db-name=$db" "--data=$lab\ibdata\$db" --user=Администратор --force --dynamic=disable *>&1 | Out-File -Encoding utf8 -LiteralPath "$out\$Id-apply.log"
    $exit = $LASTEXITCODE
    $sec = [Math]::Round($sw.Elapsed.TotalSeconds, 1)
} finally {
    & pwsh -NoProfile -File $lock release trace -Name native
}
$idAfter = Sql 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
$log = Get-Content -LiteralPath "$out\$Id-apply.log" -Raw
$line = "{0}`t{1}`tstaged={2}`texit={3}`t{4}s`tstructure_phase={5}`texchange_registration={6}`tregister_rebuilt_log={7}`tregister_ids_changed={8}`t{9}" -f `
    $Id, $p.Note, $rows, $exit, $sec, ($log -match 'Обработка структуры базы данных'), ($log -match 'Регистрация изменений в планах обмена'),
    ($log -match 'Реструктуризация\s+Таблица регистрации'), ($idBefore -ne $idAfter), (Get-Date -Format s)
Add-Content -LiteralPath "$out\summary.tsv" -Value $line -Encoding utf8
Log $line
