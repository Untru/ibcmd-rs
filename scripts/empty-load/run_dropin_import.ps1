# Drop-in import check (0.3, track "Drop-in ibcmd"): a fresh disposable
# infobase ibcmd_rs_03_* (files on F:\ibcmd\lab\03\sqldata), created empty by
# native ibcmd, loaded with OUR `ibcmd infobase config import` (the drop-in
# syntax; an empty target switches to the base-free stage by itself), applied
# and exported by native ibcmd, then diffed against the reference tree.
# Pavel allowed writes to new ibcmd_rs_03_* databases on 2026-09-28
# ("Да, одноразовые ibcmd_rs_03_* (Recommended)"). Nothing is dropped.
#
# usage: pwsh -File run_dropin_import.ps1 -Corpus bsp8327 -Tag t1 -Exe <our ibcmd.exe> [-Step all|reimport] [-Date yyyyMMdd] [-NoSqlTools]
param(
    [Parameter(Mandatory = $true)][ValidateSet('bsp8327', 'bsp85', 'uha8327', 'uha85')][string]$Corpus,
    [string]$Tag = 't1',
    [Parameter(Mandatory = $true)][string]$Exe,
    [ValidateSet('all', 'reimport')][string]$Step = 'all',
    # the date in the database name (a reimport on a later day names the first run's)
    [string]$Date = (Get-Date -Format 'yyyyMMdd'),
    # run ours with no sqlcmd.exe or bcp.exe on PATH: the built-in SQL client
    # alone (0.3, #329); the harness's own sqlcmd calls keep the full PATH
    [switch]$NoSqlTools
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$lab = 'F:\ibcmd\lab\03\dropin\import'
$sqldata = 'F:\ibcmd\lab\03\sqldata'
$sqlcmd = 'C:\Program Files\Microsoft SQL Server\Client SDK\ODBC\180\Tools\Binn\SQLCMD.EXE'
$date = $Date
$corpora = @{
    bsp8327 = @{ Build = '8.3.27.2214'
        Ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native' }
    bsp85 = @{ Build = '8.5.1.1150'
        Ref = 'F:\ibcmd\lab\v85\ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2\native' }
    uha8327 = @{ Build = '8.3.27.2214'
        Ref = 'E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native' }
    uha85 = @{ Build = '8.5.1.1150'
        Ref = 'F:\ibcmd\lab\v85\native\uha_20260923\native' }
}
$c = $corpora[$Corpus]
$ibcmd = "C:\Program Files\1cv8\$($c.Build)\bin\ibcmd.exe"
$db = "ibcmd_rs_03_${Corpus}_import_${date}_$Tag"
$run = Join-Path $lab $db
$logs = Join-Path $run 'logs'
$data = Join-Path $run 'ibdata'
if ($Step -eq 'all' -and (Test-Path -LiteralPath $run)) { throw "$run exists; pick another -Tag" }
New-Item -ItemType Directory -Force -Path $logs, $data, $sqldata | Out-Null

function Step([string]$name, [scriptblock]$block, [switch]$AllowFailure) {
    $log = Join-Path $logs "$name.log"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $ErrorActionPreference = 'Continue'
    & $block *>&1 | Out-File -Encoding utf8 -LiteralPath $log
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    $sw.Stop()
    $line = "{0}`t{1}`t{2}`t{3:N1}`t{4}" -f (Get-Date -Format s), $db, $name, $sw.Elapsed.TotalSeconds, $code
    Add-Content -LiteralPath (Join-Path $lab 'runs.tsv') -Value $line -Encoding utf8
    $line
    if ($code -ne 0 -and -not $AllowFailure) { throw "$name failed (exit $code), see $log" }
    return $code
}

$na = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data")

# Our exe, with -NoSqlTools on a PATH without the folders of sqlcmd.exe and bcp.exe.
function Invoke-Ours([string[]]$arguments) {
    if (-not $NoSqlTools) { & $Exe @arguments; return }
    $saved = $env:PATH
    $env:PATH = ($saved -split ';' | Where-Object {
            $_ -and -not (Test-Path -LiteralPath (Join-Path $_ 'sqlcmd.exe')) -and
            -not (Test-Path -LiteralPath (Join-Path $_ 'bcp.exe'))
        }) -join ';'
    try { & $Exe @arguments } finally { $env:PATH = $saved }
}

if ($Step -eq 'all') {
    $create = @"
CREATE DATABASE [$db]
  ON (NAME = N'$db', FILENAME = N'$sqldata\$db.mdf', SIZE = 256MB, FILEGROWTH = 256MB)
  LOG ON (NAME = N'${db}_log', FILENAME = N'$sqldata\${db}_log.ldf', SIZE = 128MB, FILEGROWTH = 256MB);
ALTER DATABASE [$db] SET RECOVERY SIMPLE;
"@
    $createFile = Join-Path $logs 'create_database.sql'
    [System.IO.File]::WriteAllText($createFile, $create, [System.Text.UTF8Encoding]::new($true))
    [void](Step 'sql_create_database' { & $sqlcmd -S localhost -E -C -b -f 65001 -i $createFile })
    [void](Step 'native_create_empty' { & $ibcmd infobase create @na --locale=ru_RU })
    [void](Step 'config_rows_empty' { & $sqlcmd -S localhost -E -C -b -f 65001 -d $db -Q "SET NOCOUNT ON; SELECT COUNT_BIG(*) AS config_rows FROM dbo.Config; SELECT TOP 20 FileName, DataSize FROM dbo.Config ORDER BY FileName" })
    $report = Join-Path $run 'ours_import.json'
    [void](Step 'ours_import' { Invoke-Ours (@('infobase', 'config', 'import') + $na + @("--report=$report", $c.Ref)) })
    # Step prints its log line and returns the exit code: keep the code only.
    $apply = @(Step 'native_apply' { & $ibcmd infobase config apply @na --force --dynamic=disable } -AllowFailure)[-1]
    if ($apply -ne 0) {
        # BSP 8.3.27 on SQL Server 2025: the first apply of a fresh database ends
        # at "Принятие изменений" with SDBL "быстрой вставки"; natively too.
        [void](Step 'native_apply_retry' { & $ibcmd infobase config apply @na --force --dynamic=disable })
    }
    $out = Join-Path $run 'export'
    [void](Step 'native_export' { & $ibcmd infobase config export @na $out })
    [void](Step 'source_diff' { & $Exe source-diff -o (Join-Path $run 'diff.json') $c.Ref $out })
} else {
    # A second import into the now loaded infobase: the patch path.
    $report = Join-Path $run 'ours_reimport.json'
    [void](Step 'ours_reimport' { Invoke-Ours (@('infobase', 'config', 'import') + $na + @("--report=$report", $c.Ref)) })
    [void](Step 'native_reapply' { & $ibcmd infobase config apply @na --force --dynamic=disable } -AllowFailure)
    $out = Join-Path $run 'export_reimport'
    [void](Step 'native_reexport' { & $ibcmd infobase config export @na $out })
    [void](Step 'source_diff_reimport' { & $Exe source-diff -o (Join-Path $run 'diff_reimport.json') $c.Ref $out })
}
Write-Host "done: $run"
