<#
.SYNOPSIS
  One run under observation: snapshot before, trace of the command, snapshot
  after, diff, and one report folder.

.DESCRIPTION
  <OutRoot>\<yyyyMMdd-HHmmss>-<Tag>\
      before\   snapshot.ps1 output before the command
      trace\    trace.ps1 output (summary.md, groups.md, timeline.md, service-writes.tsv, ddl.sql, ...)
      after\    snapshot.ps1 output after the command (-Reference before: changed rows keep their content)
      diff\     diff.py output (diff.md, diff.json)
      report.md the headline of all of it, with links
  The blob store (row content shared by all snapshots) defaults to <OutRoot>\blobs.
  The command runs even when it fails; the after snapshot and the diff are made either way.

.EXAMPLE
  $ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
  $db = 'ibcmd_rs_04_trace_x'
  .\capture.ps1 -Database $db -Tag apply-exclusive -OutRoot F:\lab\04\trace\c1 `
    -Command { & $ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=$db --user=Администратор --force --dynamic=disable }
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tag,
    [scriptblock]$Command,
    [string]$Exe,
    [string[]]$ArgumentList = @(),
    [string]$OutRoot = '',
    [string]$Server = 'localhost',
    [string]$Track = 'trace',
    [string]$BlobStore = '',
    [switch]$IncludeStatements,
    [switch]$AllDatabases,
    [string]$Predicate = '',
    [switch]$KeepXel,
    # preserve raw XML as well as parsed TSV for independent completeness review
    [switch]$KeepRaw,
    [int]$TimeoutMinutes = 0,
    # script blocks run right before / after the command under trace (e.g. take and release a lab lock)
    [scriptblock]$BeforeCommand,
    [scriptblock]$AfterCommand,
    [int]$MaxStatementKB = 1024,
    [int]$ContentMaxKB = 512,
    [int]$ContentBudgetMB = 1024,
    [ValidateSet('full', 'counts', 'none')][string]$DataChecksum = 'full',
    # use an earlier snapshot as "before" instead of taking one now
    [string]$BeforeSnapshot = '',
    [string]$Note = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
. (Join-Path $PSScriptRoot 'lib\common.ps1')

if (-not $Command -and -not $Exe) { throw 'give -Command <scriptblock> or -Exe <path> [-ArgumentList ...]' }
if (-not $OutRoot) { $OutRoot = (Get-Location).Path }
New-Item -ItemType Directory -Force -Path $OutRoot | Out-Null
$OutRoot = (Resolve-Path -LiteralPath $OutRoot).Path
if (-not $BlobStore) { $BlobStore = Join-Path $OutRoot 'blobs' }
$dir = Join-Path $OutRoot ('{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), $Tag)
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$sw = [Diagnostics.Stopwatch]::StartNew()
$snapArgs = @{ Database = $Database; Server = $Server; BlobStore = $BlobStore; ContentMaxKB = $ContentMaxKB; ContentBudgetMB = $ContentBudgetMB; DataChecksum = $DataChecksum }

# 1. before
if ($BeforeSnapshot) {
    $before = $BeforeSnapshot
    Write-Log "before = $before (existing snapshot)"
} else {
    $before = Join-Path $dir 'before'
    & (Join-Path $PSScriptRoot 'snapshot.ps1') @snapArgs -Out $before -Label "before $Tag"
}

# 2. the command under trace
$traceDir = Join-Path $dir 'trace'
$traceArgs = @{ Database = $Database; Tag = $Tag; OutDir = $traceDir; Server = $Server; Track = $Track; MaxStatementKB = $MaxStatementKB; Note = $Note; TimeoutMinutes = $TimeoutMinutes }
if ($IncludeStatements) { $traceArgs['IncludeStatements'] = $true }
if ($AllDatabases) { $traceArgs['AllDatabases'] = $true }
if ($Predicate) { $traceArgs['Predicate'] = $Predicate }
if ($KeepXel) { $traceArgs['KeepXel'] = $true }
if ($KeepRaw) { $traceArgs['KeepRaw'] = $true }
if ($BeforeCommand) { $traceArgs['BeforeCommand'] = $BeforeCommand }
if ($AfterCommand) { $traceArgs['AfterCommand'] = $AfterCommand }
$traceError = $null
try {
    if ($Command) { & (Join-Path $PSScriptRoot 'trace.ps1') @traceArgs -Command $Command }
    else { & (Join-Path $PSScriptRoot 'trace.ps1') @traceArgs -Exe $Exe -ArgumentList $ArgumentList }
} catch {
    $traceError = $_
    Write-Log "trace failed: $_"
}

# 3. after (also when the command or the trace failed: the state is evidence too)
$after = Join-Path $dir 'after'
& (Join-Path $PSScriptRoot 'snapshot.ps1') @snapArgs -Out $after -Reference $before -Label "after $Tag"

# 4. diff
$py = Get-Command python -ErrorAction SilentlyContinue
$diffDir = Join-Path $dir 'diff'
if ($py) {
    $env:PYTHONIOENCODING = 'utf-8'
    & $py.Source (Join-Path $PSScriptRoot 'diff.py') --before $before --after $after --out $diffDir --blobs $BlobStore
} else {
    Write-Log 'python not found: snapshots are written, the diff is skipped'
}

# 5. report.md
$meta = $null
$metaPath = Join-Path $traceDir 'trace-meta.json'
if (Test-Path -LiteralPath $metaPath) { $meta = Get-Content -LiteralPath $metaPath -Raw -Encoding UTF8 | ConvertFrom-Json }
$lines = @(
    "# Capture: $Tag",
    '',
    "- database: ``$Database`` on ``$Server``",
    "- command: ``$(if ($meta) { $meta.command } elseif ($Command) { $Command.ToString().Trim() } else { $Exe + ' ' + ($ArgumentList -join ' ') })``",
    "- exit code: $(if ($meta) { $meta.exit_code } else { '?' }), command seconds: $(if ($meta) { $meta.command_seconds } else { '?' }), whole capture: $([Math]::Round($sw.Elapsed.TotalSeconds, 1)) s",
    "- note: $Note",
    '',
    'Contents:',
    '- `trace\summary.md` - what the command did to the database (statements, writes, DDL, transactions)',
    '- `trace\timeline.md`, `trace\groups.md`, `trace\service-writes.tsv`, `trace\ddl.sql`, `trace\transactions.tsv`',
    '- `diff\diff.md` - what changed in the tables (rows and schema) between `before\` and `after\`',
    '- `before\`, `after\` - the snapshots (row content is in the blob store `' + $BlobStore + '`)',
    '- `trace\command.log` - the output of the command'
)
if ($traceError) { $lines += ''; $lines += "**the trace step failed: $traceError**" }
Set-Content -LiteralPath (Join-Path $dir 'report.md') -Value ($lines -join "`n") -Encoding UTF8
Write-Log "capture done: $dir"
Write-Output $dir
