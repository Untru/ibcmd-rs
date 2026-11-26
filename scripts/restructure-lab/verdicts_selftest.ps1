# Bounded, DB-free negative controls for the actual acceptance helpers used by
# mix_case.ps1 and mix_ours.ps1. Fixtures stay under the caller's lab output.
param([Parameter(Mandatory = $true)][string]$Out)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\verdicts.ps1"
New-Item -ItemType Directory -Force $Out | Out-Null
$script:checks = 0
function Accept([string]$Name, [scriptblock]$Body) {
    & $Body
    $script:checks++
    "PASS $Name"
}
function Reject([string]$Name, [scriptblock]$Body) {
    $failed = $false
    try { & $Body } catch { $failed = $true }
    if (-not $failed) { throw "negative control was accepted: $Name" }
    $script:checks++
    "PASS rejects $Name"
}
function Text([string]$Name, [string]$Value) {
    $path = Join-Path $Out $Name
    Set-Content -LiteralPath $path -Value $Value -Encoding UTF8
    $path
}
$noop = Text noop.txt '[INFO] Обновление конфигурации базы данных не требуется'
$applied = Text applied.txt '[INFO] Обновление конфигурации базы данных успешно завершено'
Accept 'native no-op' { Assert-LabNoop 0 $noop }
Reject 'native failure despite no-op text' { Assert-LabNoop 1 $noop }
Reject 'native apply was not a no-op' { Assert-LabNoop 0 $applied }
Reject 'native export failure' { Assert-LabCommand 1 'native export' }
$except = Text except.txt "_Reference1`trows 2 / 2`tonly in A 0`tonly in B 0"
Accept 'all rebuilt tables equal' { Assert-LabExcept $except @('_Reference1') }
$different = Text different.txt "_Reference1`trows 2 / 2`tonly in A 1`tonly in B 0"
Reject 'rebuilt data difference' { Assert-LabExcept $different @('_Reference1') }
$missing = Text missing.txt '_Reference1 missing in A'
Reject 'missing rebuilt table' { Assert-LabExcept $missing @('_Reference1') }
$empty = Text empty.txt ''
Reject 'empty EXCEPT output' { Assert-LabExcept $empty @('_Reference1') }
Reject 'incomplete rebuilt-table coverage' { Assert-LabExcept $except @('_Reference1','_Document2') }
Reject 'no expected tables' { Assert-LabExcept $empty @() }
$duplicate = Text duplicate.txt ((Get-Content $except -Raw) * 2)
Reject 'duplicate table output' { Assert-LabExcept $duplicate @('_Reference1') }
$config = Text config.txt "== only in A (0 rows)`n== only in B (0 rows)"
Accept 'Config rows equal' { Assert-LabConfig $config }
Reject 'truncated Config result' { Assert-LabConfig $empty }
$badConfig = Text bad_config.txt "== only in A (1 rows)`n== only in B (0 rows)"
Reject 'Config data mismatch' { Assert-LabConfig $badConfig }
function Export([string]$Name, [int]$Equal, [int]$Different, [string[]]$Statuses) {
    $path = Join-Path $Out $Name
    $entries = @($Statuses | ForEach-Object { @{status=$_; path='fixture.xml'} })
    @{summary=@{left_only=0;right_only=0;different=$Different;unchanged=$Equal};differences=$entries} |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $path -Encoding UTF8
    $path
}
$export = Export export.json 1 0 @('unchanged')
Accept 'native exports identical' { Assert-LabExport 0 $export }
Reject 'source-diff process failure despite valid JSON' { Assert-LabExport 1 $export }
$badExport = Export bad_export.json 1 1 @('unchanged','different')
Reject 'unexpected native export difference' { Assert-LabExport 0 $badExport }
Accept 'OUR route allows declared differences for path validator' { Assert-LabExport 0 $badExport -AllowDifferences }
$emptyExport = Export empty_export.json 0 0 @()
Reject 'empty native exports' { Assert-LabExport 0 $emptyExport }
$truncated = Export truncated.json 2 0 @('unchanged')
Reject 'truncated source-diff entries' { Assert-LabExport 0 $truncated }
$unknown = Export unknown.json 1 0 @('not-a-status')
Reject 'unknown source-diff status' { Assert-LabExport 0 $unknown }
"PASS $script:checks acceptance controls; no database or native command was used"
