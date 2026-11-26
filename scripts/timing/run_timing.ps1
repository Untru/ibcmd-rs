# Timing: ibcmd-rs against native ibcmd, export and load, on BSP and ERP УХ,
# platforms 8.3.27 and 8.5. Pavel asked for it on 2026-09-24 ("Выгрузку и
# загрузку"): exports read the existing databases only; loads go into fresh
# disposable copies (ibcmd_rs_tm_*) restored from the backups already taken.
# Every step runs alone and appends one line to timing.tsv.
#
#   export: native `ibcmd infobase config export` against
#           `ibcmd-rs mssql-dump-config --extract-metadata-xml --extract-module-text
#            --no-binary-rows` (the XML tree only, no side outputs)
#   load:   native `ibcmd infobase config import <tree>` + `config apply --force`
#           against ibcmd-rs: a bulk read of the copy's rows (the base rows the
#           stage patches) + `mssql-stage-source-objects` + the publish SQL
#
# usage: pwsh -File run_timing.ps1 -Phase export|load [-Corpus bsp8327,...] [-Tag t1]
param(
    [Parameter(Mandatory = $true)][ValidateSet('export', 'load')][string]$Phase,
    [string[]]$Corpus = @('bsp8327', 'bsp85', 'uha8327', 'uha85'),
    [string]$Tag = 't1',
    [switch]$NativeOnly,
    [switch]$OursOnly,
    [string]$Exe = 'F:\ibcmd\lab\timing\ibcmd-rs.exe',
    [string[]]$ExtraStageArgs = @(),
    [switch]$UseRowsCache
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
# `pwsh -File ... -Corpus a,b` passes "a,b" as one string.
$Corpus = @($Corpus | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

$lab = 'F:\ibcmd\lab\timing'
$exe = $Exe
$ExtraStageArgs = @($ExtraStageArgs | ForEach-Object { $_ -split ' ' } | Where-Object { $_ })
$sqlcmd = 'C:\Program Files\Microsoft SQL Server\Client SDK\ODBC\180\Tools\Binn\SQLCMD.EXE'
$bcp = 'C:\Program Files\Microsoft SQL Server\Client SDK\ODBC\180\Tools\Binn\bcp.exe'
$tsv = Join-Path $lab 'timing.tsv'
$date = '20260924'

# Infobase user names built from code points so the file stays ASCII.
$admin = -join ([char[]](0x0410, 0x0434, 0x043C, 0x0438, 0x043D, 0x0438, 0x0441, 0x0442, 0x0440, 0x0430, 0x0442, 0x043E, 0x0440))
$usual = -join ([char[]](0x043E, 0x0431, 0x044B, 0x0447, 0x043D, 0x043E, 0x0435))
$app = -join ([char[]](0x043F, 0x0440, 0x0438, 0x043B, 0x043E, 0x0436, 0x0435, 0x043D, 0x0438, 0x0435))

$corpora = @{
    bsp8327 = @{ Build = '8.3.27.2214'; Db = 'ibcmd_rs_bsp_8327_native_20260919'
        Bak = 'F:\ibcmd\lab\dbbak\bsp_native_20260923.bak'; Data = 'BSP_Service'; Log = 'BSP_Service_log'
        Ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native'
        Ver = '2.20'; User = $admin; Consts = 'F:\ibcmd\lab\roundtrip\bsp_always_used_constants.txt'
        Rows = 'F:/ibcmd/lab/rawrows/bsp/Config' }
    uha8327 = @{ Build = '8.3.27.2214'; Db = 'ibcmd_rs_uha_8327_parity2_20260920'
        Bak = 'F:\ibcmd\lab\dbbak\uha_parity2_20260924.bak'; Data = 'uha'; Log = 'uha_log'
        Ref = 'E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native'
        Ver = '2.20'; User = ''; Consts = 'F:\ibcmd\lab\roundtrip\uha_always_used_constants.txt'
        Rows = 'F:/ibcmd/lab/rawrows/uha/Config' }
    bsp85 = @{ Build = '8.5.1.1150'; Db = 'ibcmd_rs_bsp_85_src_20260922'
        Bak = 'F:\ibcmd\lab\v85\realcycle\dbbak\ibcmd_rs_bsp_85_src_20260922_20260924.bak'; Data = 'bsp'; Log = 'bsp_log'
        Ref = 'F:\ibcmd\lab\v85\ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2\native'
        Ver = '2.21'; User = "$admin ($usual $app)"; Consts = 'F:\ibcmd\lab\v85\rawrows\bsp_always_used_constants.txt'
        Rows = 'F:/ibcmd/lab/v85/rawrows/bsp/Config' }
    uha85 = @{ Build = '8.5.1.1150'; Db = 'ibcmd_rs_uha_85_src_20260922'
        Bak = 'F:\ibcmd\lab\v85\realcycle\dbbak\ibcmd_rs_uha_85_src_20260922_20260924.bak'; Data = 'uha'; Log = 'uha_log'
        Ref = 'F:\ibcmd\lab\v85\native\uha_20260923\native'
        Ver = '2.21'; User = ''; Consts = 'F:\ibcmd\lab\v85\rawrows\uha_always_used_constants.txt'
        Rows = 'F:/ibcmd/lab/v85/rawrows/uha/Config' }
}

if (-not (Test-Path -LiteralPath $tsv)) {
    "when`tphase`tcorpus`ttool`tstep`tseconds`trc`tnote" | Out-File -Encoding utf8 -LiteralPath $tsv
}

function Record([string]$corpus, [string]$tool, [string]$step, [double]$seconds, [int]$rc, [string]$note) {
    $line = "{0}`t{1}`t{2}`t{3}`t{4}`t{5:N1}`t{6}`t{7}" -f (Get-Date -Format s), $Phase, $corpus, $tool, $step, $seconds, $rc, $note
    Add-Content -LiteralPath $tsv -Value $line -Encoding utf8
    $line
}

# Runs a native or ibcmd-rs command, output to a log file; returns seconds and exit code.
function Timed([string]$log, [scriptblock]$block) {
    $ErrorActionPreference = 'Continue'
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $block *>&1 | Out-File -Encoding utf8 -LiteralPath $log
    $code = $LASTEXITCODE
    $sw.Stop()
    return @($sw.Elapsed.TotalSeconds, $code)
}

# Runs a SQL template with $(NAME) placeholders substituted into a copy beside
# the log (sqlcmd -v splits a value such as F:\x.bak at the drive colon).
function Sql([string]$file, [hashtable]$vars, [string]$log) {
    $text = Get-Content -LiteralPath $file -Raw -Encoding utf8
    foreach ($k in $vars.Keys) { $text = $text.Replace('$(' + $k + ')', [string]$vars[$k]) }
    $copy = "$log.sql"
    [System.IO.File]::WriteAllText($copy, $text, [System.Text.UTF8Encoding]::new($true))
    $a = @('-S', 'localhost', '-E', '-C', '-b', '-W', '-f', '65001', '-i', $copy)
    return Timed $log { & $sqlcmd @a }
}

function NativeArgs($c, [string]$db, [string]$data) {
    $list = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data")
    if ($c.User) { $list += "--user=$($c.User)" }
    return , $list
}

New-Item -ItemType Directory -Force -Path (Join-Path $lab 'sqldata'), (Join-Path $lab 'logs'), (Join-Path $lab 'out') | Out-Null
# Loads: every native load first, then every ibcmd-rs load, so the native
# numbers are taken while nothing of ours is being built or run.
$passes = if ($Phase -eq 'load') { @('native', 'ours') } else { @('all') }
foreach ($pass in $passes) {
foreach ($name in $Corpus) {
    $c = $corpora[$name]
    $ibcmd = "C:\Program Files\1cv8\$($c.Build)\bin\ibcmd.exe"
    $logs = Join-Path $lab "logs\$Phase`_$name`_$Tag"
    New-Item -ItemType Directory -Force -Path $logs | Out-Null

    # Every export writes into its own new folder: native 8.5 refuses a
    # non-empty one, and ours with --overwrite would spend its measured time
    # deleting the previous tree. One timing database per family (bsp | uha).
    $family = $name.Substring(0, 3)
    if ($Phase -eq 'export') {
        if (-not $OursOnly) {
            $out = Join-Path $lab "out\native_export_$name`_$Tag"
            $data = Join-Path $lab "out\native_data_$Phase`_$name`_$Tag"
            if (Test-Path -LiteralPath $out) { throw "exists: $out" }
            New-Item -ItemType Directory -Force -Path $out, $data | Out-Null
            $na = NativeArgs $c $c.Db $data
            $r = Timed (Join-Path $logs 'native_export.log') { & $ibcmd infobase config export @na --force $out }
            $files = (Get-ChildItem -LiteralPath $out -Recurse -File | Measure-Object).Count
            Record $name "native $($c.Build)" 'export' $r[0] $r[1] "files=$files"
        }
        if (-not $NativeOnly) {
            $out = Join-Path $lab "out\ours_export_$name`_$Tag"
            if (Test-Path -LiteralPath $out) { throw "exists: $out" }
            $r = Timed (Join-Path $logs 'ours_export.log') {
                & $exe mssql-dump-config --database $c.Db --server localhost --sqlcmd $sqlcmd --bcp-executable $bcp `
                    -o $out --overwrite --extract-module-text --extract-metadata-xml --source-version $c.Ver --no-binary-rows }
            $files = (Get-ChildItem -LiteralPath $out -Recurse -File | Measure-Object).Count
            Record $name 'ibcmd-rs' 'export' $r[0] $r[1] "files=$files (incl. manifest)"
        }
        continue
    }

    # load
    $db = "ibcmd_rs_tm_$family`_$date"
    if (-not $NativeOnly -and $pass -ne 'native') {
        $r = Sql (Join-Path $lab 'restore.sql') @{ DB = $db; BAK = $c.Bak; DATA = $c.Data; LOG = $c.Log } (Join-Path $logs 'ours_restore.log')
        if ($r[1] -ne 0) { throw "restore of $db failed" }
        # The base rows come from the lab cache of the same database (as in the
        # real cycles): the product has no bulk read of them yet, and without
        # one the stage fetches them object by object.
        $env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
        $env:IBCMD_RS_ALWAYS_USED_CONSTANTS = $c.Consts
        if ($UseRowsCache) { $env:IBCMD_RS_BASE_ROWS_DIR = $c.Rows } else { Remove-Item Env:IBCMD_RS_BASE_ROWS_DIR -ErrorAction SilentlyContinue }
        $scripts = Join-Path $lab "out\ours_scripts_$family\stage_source_objects.sql"
        New-Item -ItemType Directory -Force -Path (Split-Path $scripts) | Out-Null
        $stageArgs = @('mssql-stage-source-objects', '--database', $db, '--source-root', $c.Ref, '--source-version', $c.Ver,
            '--sqlcmd', $sqlcmd, '--replace-config-save', '--allow-non-lab', '--script-output', $scripts) + $ExtraStageArgs
        $r = Timed (Join-Path $logs 'ours_stage.log') { & $exe @stageArgs }
        Remove-Item Env:IBCMD_RS_NATIVE_FORM_WRITER, Env:IBCMD_RS_ALWAYS_USED_CONSTANTS -ErrorAction SilentlyContinue
        Remove-Item Env:IBCMD_RS_BASE_ROWS_DIR -ErrorAction SilentlyContinue
        Record $name 'ibcmd-rs' 'load: stage' $r[0] $r[1] ("exe=" + (Split-Path $exe -Leaf) + " " + ($ExtraStageArgs -join ' ') + $(if ($UseRowsCache) { ' rows-cache' } else { '' }))
        if ($r[1] -ne 0) { throw "stage failed" }
        $r = Sql (Join-Path $lab 'publish.sql') @{ DB = $db } (Join-Path $logs 'ours_publish.log')
        Record $name 'ibcmd-rs' 'load: publish' $r[0] $r[1] ''
    }
    if (-not $OursOnly -and $pass -ne 'ours') {
        $r = Sql (Join-Path $lab 'restore.sql') @{ DB = $db; BAK = $c.Bak; DATA = $c.Data; LOG = $c.Log } (Join-Path $logs 'native_restore.log')
        if ($r[1] -ne 0) { throw "restore of $db failed" }
        $data = Join-Path $lab "out\native_data_$Phase`_$name`_$Tag"
        New-Item -ItemType Directory -Force -Path $data | Out-Null
        $na = NativeArgs $c $db $data
        $r = Timed (Join-Path $logs 'native_import.log') { & $ibcmd infobase config import @na $c.Ref }
        Record $name "native $($c.Build)" 'load: config import' $r[0] $r[1] "$db"
        if ($r[1] -ne 0) { continue }
        $r = Timed (Join-Path $logs 'native_apply.log') { & $ibcmd infobase config apply @na --force --dynamic=disable }
        Record $name "native $($c.Build)" 'load: config apply' $r[0] $r[1] ''
    }
}
}
