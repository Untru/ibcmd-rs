# Empty-database load: native vs ibcmd-rs, on a fresh disposable database.
# Pavel asked for loading into an empty database on 2026-09-24 ("после замеров
# займись загрузкой в пустую базу"); databases are ibcmd_rs_empty_* only, files
# on F: (D:\SQL, the server default, is nearly full).
#
#   native: CREATE DATABASE -> ibcmd infobase create --import <tree> --apply
#   ours:   CREATE DATABASE -> ibcmd infobase create (empty configuration)
#           -> ibcmd-rs mssql-stage-source-objects --base-free
#           -> ibcmd infobase config apply --force
#   both:   native `config export` of the result -> ibcmd-rs source-diff
#           against the reference tree.
#
# usage: pwsh -File run_empty.ps1 -Corpus bsp8327 -Mode ours [-Tag t1] [-Exe path]
param(
    [Parameter(Mandatory = $true)][ValidateSet('bsp8327', 'uha8327', 'bsp85', 'uha85')][string]$Corpus,
    [Parameter(Mandatory = $true)][ValidateSet('native', 'ours')][string]$Mode,
    [string]$Tag = 't1',
    [string]$Exe = 'F:\ibcmd\src\ibcmd-rs-model\target\release\ibcmd-rs.exe',
    [string[]]$ExtraStageArgs = @()
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$lab = 'F:\ibcmd\lab\model\empty'
$sqlcmd = 'C:\Program Files\Microsoft SQL Server\Client SDK\ODBC\180\Tools\Binn\SQLCMD.EXE'
$date = Get-Date -Format 'yyyyMMdd'
$corpora = @{
    bsp8327 = @{ Build = '8.3.27.2214'; Ver = '2.20'; Consts = 'F:\ibcmd\lab\roundtrip\bsp_always_used_constants.txt'
        Ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native' }
    uha8327 = @{ Build = '8.3.27.2214'; Ver = '2.20'
        Ref = 'E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native' }
    bsp85 = @{ Build = '8.5.1.1150'; Ver = '2.21'
        Ref = 'F:\ibcmd\lab\v85\ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2\native' }
    uha85 = @{ Build = '8.5.1.1150'; Ver = '2.21'
        Ref = 'F:\ibcmd\lab\v85\native\uha_20260923\native' }
}
$c = $corpora[$Corpus]
$ibcmd = "C:\Program Files\1cv8\$($c.Build)\bin\ibcmd.exe"
$db = "ibcmd_rs_empty_${Corpus}_${Mode}_${date}_$Tag"
$run = Join-Path $lab $db
$logs = Join-Path $run 'logs'
$data = Join-Path $run 'ibdata'
$sqldata = Join-Path $lab 'sqldata'
if (Test-Path -LiteralPath $run) { throw "$run exists; pick another -Tag" }
New-Item -ItemType Directory -Force -Path $logs, $data, $sqldata | Out-Null

function Step([string]$name, [scriptblock]$block) {
    $log = Join-Path $logs "$name.log"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $ErrorActionPreference = 'Continue'
    & $block *>&1 | Out-File -Encoding utf8 -LiteralPath $log
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    $sw.Stop()
    $line = "{0}`t{1}`t{2}`t{3}`t{4:N1}`t{5}" -f (Get-Date -Format s), $db, $Mode, $name, $sw.Elapsed.TotalSeconds, $code
    Add-Content -LiteralPath (Join-Path $lab 'runs.tsv') -Value $line -Encoding utf8
    $line
    if ($code -ne 0) { throw "$name failed (exit $code), see $log" }
}

$na = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data")

# The database itself, on F:, before the platform formats it.
$create = @"
CREATE DATABASE [$db]
  ON (NAME = N'$db', FILENAME = N'$sqldata\$db.mdf', SIZE = 256MB, FILEGROWTH = 256MB)
  LOG ON (NAME = N'${db}_log', FILENAME = N'$sqldata\${db}_log.ldf', SIZE = 128MB, FILEGROWTH = 256MB);
ALTER DATABASE [$db] SET RECOVERY SIMPLE;
"@
$createFile = Join-Path $logs 'create_database.sql'
[System.IO.File]::WriteAllText($createFile, $create, [System.Text.UTF8Encoding]::new($true))
Step 'sql_create_database' { & $sqlcmd -S localhost -E -C -b -f 65001 -i $createFile }

if ($Mode -eq 'native') {
    Step 'native_create_import_apply' { & $ibcmd infobase create @na --locale=ru_RU --import=$($c.Ref) --apply --force }
} else {
    Step 'native_create_empty' { & $ibcmd infobase create @na --locale=ru_RU }
    $env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
    # An empty database has no always-used constants: no IBCMD_RS_ALWAYS_USED_CONSTANTS.
    Remove-Item Env:IBCMD_RS_ALWAYS_USED_CONSTANTS -ErrorAction SilentlyContinue
    $stage = @('mssql-stage-source-objects', '--database', $db, '--source-root', $c.Ref,
        '--source-version', $c.Ver, '--sqlcmd', $sqlcmd, '--replace-config-save', '--allow-non-lab',
        '--base-free') + $ExtraStageArgs
    Step 'ours_stage_base_free' { & $Exe @stage }
    Step 'native_apply' { & $ibcmd infobase config apply @na --force --dynamic=disable }
}

$out = Join-Path $run 'export'
Step 'native_export' { & $ibcmd infobase config export @na --force $out }
Step 'source_diff' { & $Exe source-diff -o (Join-Path $run 'diff.json') $c.Ref $out }
Write-Host "done: $run"
