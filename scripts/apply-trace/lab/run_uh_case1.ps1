# ERP УХ 8.3.27, case 1 (modules only): native exclusive apply under the capture kit.
# PREPARED, NOT STARTED.  Needs the coordinator's go and OK for the УХ clone (about 5 GB of data files) and refuses to
# run without -Go.  The plan and the numbers behind the settings are in docs/apply/uh-case1-plan.md.
#
#   pwsh -NoProfile -File run_uh_case1.ps1 -Go [-Variant forms|containers] [-Pad 0..12] [-Database ibcmd_rs_04_trace_uh_mod]
#
# Variants: forms      five modules (common module, catalog object module, document manager module, managed form module,
#                      common form module): like case 1 of the БСП (1x).
#           containers the first three only.
# -Pad N   N extra small common modules (default 5): the stage then has 3 + 2 x (modules + N) rows, more than 20, which the
#          experiment of native-apply-trace.md section 6.4 shows to take the LONG path (the case whose statement count is
#          asked for) whatever the descriptors do.  The runner prints the predicted path before the apply: rule 1 (a staged
#          descriptor differs from Config; the native import re-saved the catalog and document descriptors on the БСП)
#          or rule 2 (more than 20 staged rows).
#
# Steps (each native command holds its lock alone; the snapshots, the trace report and the diff run without a lock):
#   0 guards: -Go, free space on F: at least 40 GB, the database must not exist
#   1 restore-clone.ps1 -Corpus uha8327 (shrinks the log)
#   2 sparse base directory with the edited module files only (a copy of the 9 GB reference tree is not allowed,
#     E:\ is read-only and a hard-link tree cannot cross volumes)
#   3 native `config import files --base-dir <sparse> --partial <files>` (native lock), completeness test check_stage.py
#   4 capture.ps1: snapshot (row counts only), XE trace, native apply --dynamic=disable (heavy, then native lock,
#     released right after the command), snapshot, diff, report
param(
    [switch]$Go,
    [ValidateSet('forms', 'containers')][string]$Variant = 'forms',
    # extra small common modules (one comment line appended each): the long path starts above 20 staged rows (rule 2 of
    # native-apply-trace.md section 6.4); 3 rows + 2 per edited module.  0 = the bare variant.
    [ValidateRange(0, 12)][int]$Pad = 5,
    [string]$Database = 'ibcmd_rs_04_trace_uh_mod',
    # the clone exists already (restored by hand with restore-clone.ps1 -Corpus uha8327) and its ConfigSave is empty
    [switch]$SkipRestore,
    [string]$Platform = '8.3.27.2214',
    [int]$MinFreeGB = 40
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
if (-not $Go) { throw 'prepared, not started: run with -Go after the coordinator OK' }
$kit = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\apply-trace'
$lab = 'F:\ibcmd\lab\04\trace'
$lockScript = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ref = 'E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native'
$ibcmd = "C:\Program Files\1cv8\$Platform\bin\ibcmd.exe"
$data = "$lab\ibdata\$Database"
$base = "$lab\uhbase"
function Log($m) { Write-Host ('[{0}] UH {1}' -f (Get-Date -Format 'HH:mm:ss'), $m) }
function Sql($q) { (& sqlcmd -S localhost -E -C -d $Database -h -1 -W -Q "SET NOCOUNT ON; $q" | Out-String).Trim() }
function Lock($name, $timeout) { & pwsh -NoProfile -File $lockScript acquire trace -Name $name -TimeoutMin $timeout; if ($LASTEXITCODE -ne 0) { throw "$name lock not acquired" } }
function Unlock($name) { & pwsh -NoProfile -File $lockScript release trace -Name $name }

# 0 guards
$free = (Get-PSDrive F).Free / 1GB
if ($free -lt $MinFreeGB) { throw ("F: has {0:N1} GB free, the plan needs {1}: tell the coordinator" -f $free, $MinFreeGB) }
$exists = (& sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'$Database'" | Out-String).Trim()
if ($SkipRestore) {
    if ($exists -ne '1') { throw "-SkipRestore: $Database does not exist" }
    if ((Sql 'SELECT COUNT(*) FROM ConfigSave') -ne '0') { throw "-SkipRestore: ConfigSave of $Database is not empty" }
} elseif ($exists -ne '0') { throw "$Database exists already" }

# the edits: the same kind as the БСП case 1 (a comment line appended, BOM and CRLF kept)
$edits = @(
    @{ Kind = 'common module'; Path = 'CommonModules\СтроковыеФункцииКлиентСервер\Ext\Module.bsl'; Add = "`r`n// ibcmd-rs trace, case 1 (УХ): common module edited`r`n" },
    @{ Kind = 'catalog object module'; Path = 'Catalogs\ВидыКонтактнойИнформации\Ext\ObjectModule.bsl'; Add = "`r`n// ibcmd-rs trace, case 1 (УХ): catalog object module edited`r`n" },
    @{ Kind = 'document manager module'; Path = 'Documents\ЗаказПоставщику\Ext\ManagerModule.bsl'; Add = "`r`n// ibcmd-rs trace, case 1 (УХ): document manager module edited`r`n" }
)
if ($Variant -eq 'forms') {
    $edits += @(
        @{ Kind = 'managed form module'; Path = 'Catalogs\Валюты\Forms\ФормаЭлемента\Ext\Form\Module.bsl'; Add = "`r`n// ibcmd-rs trace, case 1 (УХ): managed form module edited`r`n" },
        @{ Kind = 'common form module'; Path = 'CommonForms\ВопросОбУстановкеВнешнейКомпоненты\Ext\Form\Module.bsl'; Add = "`r`n// ibcmd-rs trace, case 1 (УХ): common form module edited`r`n" }
    )
}

$padNames = @('ОплатаСервисаКлиентПереопределяемый', 'СообщенияВМоделиСервисаПовтИсп', 'ЭлектроннаяПодписьВМоделиСервисаПереопределяемый', 'ОповещениеПользователейБТСПереопределяемый',
    'ТарификацияВызовСервера', 'ТрансляцияXDTOПереопределяемый', 'ФайлыБТСВызовСервера', 'РаботаСКурсамиВалютВызовСервера', 'РаботаСБанкамиВызовСервера',
    'ДополнительныеОтчетыИОбработкиГлобальный', 'ОбращенияВТехническуюПоддержкуГлобальный', 'БТС')
foreach ($n in ($padNames | Select-Object -First $Pad)) {
    $edits += @{ Kind = 'padding common module'; Path = "CommonModules\$n\Ext\Module.bsl"; Add = "`r`n// ibcmd-rs trace, case 1 (УХ): padding module edited`r`n" }
}

# 1 clone
if ($SkipRestore) { Log "clone $Database exists (free $([Math]::Round($free, 1)) GB)" } else {
    Log "restore $Database from the uha8327 corpus (free $([Math]::Round($free, 1)) GB)"
    & pwsh -NoProfile -File 'F:\ibcmd\lab\04\tools\restore-clone.ps1' -Corpus uha8327 -Name $Database -Track trace -Purpose "trace phase 3: ERP УХ 8.3.27 case 1 ($Variant), native exclusive apply traced"
    if ($LASTEXITCODE -ne 0) { throw 'restore failed' }
}
New-Item -ItemType Directory -Force -Path $data | Out-Null

# 2 sparse base directory (only the edited files)
if (Test-Path -LiteralPath $base) { Remove-Item -LiteralPath $base -Recurse -Force -Confirm:$false }
$partial = @()
foreach ($e in $edits) {
    $dst = Join-Path $base $e.Path
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dst) | Out-Null
    [IO.File]::WriteAllBytes($dst, [IO.File]::ReadAllBytes((Join-Path $ref $e.Path)) + [Text.Encoding]::UTF8.GetBytes($e.Add))
    $partial += ($e.Path -replace '\\', '/')
}
Log ("edited files: " + ($partial -join '; '))

# 3 native staging, one command under the native lock
Lock 'native' 120
try {
    & $ibcmd infobase config import files --dbms=MSSQLServer --db-server=localhost "--db-name=$Database" "--data=$data" "--base-dir=$base" --partial @partial *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\uh-import-files.log"
    $importExit = $LASTEXITCODE
} finally { Unlock 'native' }
if ($importExit -ne 0) { throw "native import files failed ($importExit), see $lab\uh-import-files.log" }
$rows = Sql 'SELECT COUNT(*) FROM ConfigSave'
Log "ConfigSave rows: $rows; the completeness test (versions entries against ConfigSave rows, no commit / *.new) follows"
$env:PYTHONIOENCODING = 'utf-8'
& python (Join-Path $PSScriptRoot 'check_stage.py') $Database
if ($LASTEXITCODE -ne 0) { throw 'the native stage is not complete (check_stage.py)' }
# which path will the apply take?  (native-apply-trace.md section 6.4: a staged descriptor that differs from Config, or more than 20 staged rows)
$diff = @(& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'stage_vs_config.ps1') -Database $Database | Where-Object { $_ -notmatch '`tsame$' })
$descDiff = @($diff | Where-Object { $_ -match '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}`t' }).Count
$predicted = if ($descDiff -gt 0) { "LONG (rule 1: $descDiff staged descriptor row(s) differ from Config)" } elseif ([int]$rows -gt 20) { "LONG (rule 2: $rows staged rows)" } else { "SHORT ($rows staged rows, descriptors unchanged)" }
Log "predicted path: $predicted"
$diff | Out-File -Encoding utf8 -LiteralPath "$lab\uh-stage-vs-config.txt"

# 4 traced apply: heavy first, then native, both around the native command only
$cap = @{ Database = $Database; Tag = "uh-case1-$Variant-exclusive"; OutRoot = "$lab\captures"; BlobStore = "$lab\blobs"; Track = 'trace'
    TimeoutMinutes = 90; MaxStatementKB = 32; DataChecksum = 'counts'; ContentMaxKB = 256; ContentBudgetMB = 512
    Note = "ERP УХ 8.3.27 case 1 ($Variant): $($edits.Count) module texts staged by native import files, native apply --dynamic=disable"
    Exe = $ibcmd; ArgumentList = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$data", '--force', '--dynamic=disable')
    BeforeCommand = { & pwsh -NoProfile -File $lockScript acquire trace -Name heavy -TimeoutMin 240; if ($LASTEXITCODE -ne 0) { throw 'heavy lock not acquired' }
                      & pwsh -NoProfile -File $lockScript acquire trace -Name native -TimeoutMin 240; if ($LASTEXITCODE -ne 0) { & pwsh -NoProfile -File $lockScript release trace -Name heavy; throw 'native lock not acquired' } }
    AfterCommand  = { & pwsh -NoProfile -File $lockScript release trace -Name native; & pwsh -NoProfile -File $lockScript release trace -Name heavy } }
& (Join-Path $kit 'capture.ps1') @cap
Log 'done: delete the sparse base directory, the ibdata folder and events.xml.gz of the capture when parsed; list the database in STATUS.md'
