# Native oracle for the empty-database load when native XML import refuses the
# tree: save the source database's configuration to a .cf (read-only for the
# source), then `ibcmd infobase create --load <cf> --apply` into a fresh
# disposable ibcmd_rs_empty_* database on F:, then export and diff.
# Part of Pavel's "можно" (2026-09-25) for the BSP 8.3.27 empty-database run.
param([string]$Tag = 'cf1')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\model\empty'
$sqlcmd = 'C:\Program Files\Microsoft SQL Server\Client SDK\ODBC\180\Tools\Binn\SQLCMD.EXE'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native'
$exe = 'F:\ibcmd\lab\model\bin\d_37c4ba06.exe'
$admin = -join ([char[]](0x0410, 0x0434, 0x043C, 0x0438, 0x043D, 0x0438, 0x0441, 0x0442, 0x0440, 0x0430, 0x0442, 0x043E, 0x0440))
$src = 'ibcmd_rs_bsp_8327_native_20260919'
$db = "ibcmd_rs_empty_bsp8327_nativecf_$(Get-Date -Format yyyyMMdd)_$Tag"
$run = Join-Path $lab $db
$logs = Join-Path $run 'logs'
$sqldata = Join-Path $lab 'sqldata'
if (Test-Path -LiteralPath $run) { throw "$run exists" }
New-Item -ItemType Directory -Force -Path $logs, (Join-Path $run 'ibdata'), (Join-Path $run 'srcdata') | Out-Null

function Step([string]$name, [scriptblock]$block) {
    $log = Join-Path $logs "$name.log"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $ErrorActionPreference = 'Continue'
    & $block *>&1 | Out-File -Encoding utf8 -LiteralPath $log
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    $line = "{0}`t{1}`tnative-cf`t{2}`t{3:N1}`t{4}" -f (Get-Date -Format s), $db, $name, $sw.Elapsed.TotalSeconds, $code
    Add-Content -LiteralPath (Join-Path $lab 'runs.tsv') -Value $line -Encoding utf8
    Write-Host $line
    return $code
}

$cf = Join-Path $run 'bsp8327.cf'
$srcArgs = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$src", "--data=$(Join-Path $run 'srcdata')", "--user=$admin")
if ((Step 'native_config_save_source' { & $ibcmd infobase config save @srcArgs $cf }) -ne 0) { throw 'config save failed' }

$create = @"
CREATE DATABASE [$db]
  ON (NAME = N'$db', FILENAME = N'$sqldata\$db.mdf', SIZE = 256MB, FILEGROWTH = 256MB)
  LOG ON (NAME = N'${db}_log', FILENAME = N'$sqldata\${db}_log.ldf', SIZE = 128MB, FILEGROWTH = 256MB);
ALTER DATABASE [$db] SET RECOVERY SIMPLE;
"@
$createFile = Join-Path $logs 'create_database.sql'
[System.IO.File]::WriteAllText($createFile, $create, [System.Text.UTF8Encoding]::new($true))
if ((Step 'sql_create_database' { & $sqlcmd -S localhost -E -C -b -f 65001 -i $createFile }) -ne 0) { throw 'create failed' }

$na = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$(Join-Path $run 'ibdata')")
$applyCode = Step 'native_create_load_apply' { & $ibcmd infobase create @na --locale=ru_RU --load=$cf --apply --force }
$out = Join-Path $run 'export'
[void](Step 'native_export' { & $ibcmd infobase config export @na --force $out })
[void](Step 'source_diff' { & $exe source-diff -o (Join-Path $run 'diff.json') $ref $out })
Write-Host "apply exit $applyCode; done: $run"
