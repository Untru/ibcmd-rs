<#
.SYNOPSIS
  Turns Extended Events files (.xel) into events.xml.gz and, with -Report, into the trace report.

.DESCRIPTION
  Use it to recover the trace of a run whose report step failed or was interrupted: the .xel files
  stay in C:\temp\ibcmd_rs_04\<Track>\ until trace.ps1 finishes its export.
  events.xml.gz is what trace_report.py --xml reads.

.EXAMPLE
  .\export-xel.ps1 -Pattern 'C:\temp\ibcmd_rs_04\trace\ibcmd_rs_04_trace_1_*.xel' -OutDir F:\lab\trace-x -Report -Database ibcmd_rs_04_trace_x -Tag e01
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Pattern,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [string]$Server = 'localhost',
    [switch]$Report,
    # for the report header
    [string]$Database = '',
    [string]$Tag = '',
    [string]$MetaFile = '',
    [int]$MaxStatementKB = 1024,
    [string]$FocusDb = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
. (Join-Path $PSScriptRoot 'lib\common.ps1')
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$conn = Open-Sql -Server $Server -Database master
try {
    $xml = Join-Path $OutDir 'events.xml.gz'
    $n = Export-XelEvents -Conn $conn -Pattern $Pattern -OutPath $xml
    Write-Log "$n events -> $xml"
} finally { $conn.Close() }
if ($Report) {
    $meta = $MetaFile
    if (-not $meta) {
        $meta = Join-Path $OutDir 'trace-meta.json'
        $o = [ordered]@{ tag = $Tag; database = $Database; server = $Server; note = 'recovered from .xel files by export-xel.ps1'; events = $n }
        [System.IO.File]::WriteAllText($meta, ($o | ConvertTo-Json), $script:Utf8NoBom)
    }
    $env:PYTHONIOENCODING = 'utf-8'
    $reportArgs = @('--xml', $xml, '--max-statement-kb', $MaxStatementKB, '--meta', $meta, '--out', $OutDir)
    if ($FocusDb) { $reportArgs += @('--focus-db', $FocusDb) }
    & python (Join-Path $PSScriptRoot 'trace_report.py') @reportArgs
    if ($LASTEXITCODE -ne 0) { throw "trace_report.py failed ($LASTEXITCODE)" }
}
