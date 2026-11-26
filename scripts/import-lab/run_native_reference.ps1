# Independent native partial-import/apply/export reference for edited 8.5 fixtures whose dump-info is stale.
param([Parameter(Mandatory)][string]$Database, [Parameter(Mandatory)][string]$Tree, [Parameter(Mandatory)][string]$Tag,
      [switch]$ReuseEmptyStage)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\native.ps1"
Set-NativePlatform '8.5'
Assert-LabDb $Database
if ((Get-PSDrive F).Free -lt 25GB) { throw 'F: needs at least 25 GB free' }
foreach ($artifact in @("$script:Lab\out\native-reference-$Tag.json", "$script:Lab\out\export\$Tag")) {
    if (Test-Path -LiteralPath $artifact) { throw "reference artifact already exists: $artifact" }
}
if ($ReuseEmptyStage) {
    if ((Get-ConfigSaveRows $Database) -ne 0) { throw 'cannot reuse a pending or failed stage' }
} else {
    & pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bsp85 -Name $Database -Track import -Purpose "independent native reference $Tag"
    if ($LASTEXITCODE -ne 0) { throw 'restore failed' }
}
$report = [ordered]@{ database = $Database; tree = $Tree; platform = '8.5' }
$touched = Get-Content "$Tree.touched.json" -Raw | ConvertFrom-Json
$files = @($touched | Where-Object { Test-Path -LiteralPath (Join-Path $Tree $_) -PathType Leaf })
if ($files.Count -eq 0) { throw 'the reference needs the edited fixture file list' }
if ($files -notcontains 'Configuration.xml') { $files += 'Configuration.xml' }
$report.import_files = $files
$report.import = Invoke-NativeImportFiles -Db $Database -BaseDir $Tree -Files $files -Tag $Tag
$report | ConvertTo-Json -Depth 6 | Set-Content "$script:Lab\out\native-reference-$Tag.json" -Encoding UTF8
if ($report.import.Exit -ne 0) { throw "native import failed: $($report.import.Tail)" }
$report.apply = Invoke-NativeApply -Db $Database -Tag $Tag
if ($report.apply.Exit -ne 0) { throw "native apply failed: $($report.apply.Tail)" }
$report.export = Invoke-NativeExport -Db $Database -OutDir "$script:Lab\out\export\$Tag" -Tag $Tag
if ($report.export.Exit -ne 0) { throw 'native export failed' }
$report | ConvertTo-Json -Depth 6 | Set-Content "$script:Lab\out\native-reference-$Tag.json" -Encoding UTF8
$report | ConvertTo-Json -Depth 6
