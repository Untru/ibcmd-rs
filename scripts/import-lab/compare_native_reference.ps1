# Verify stored own-cycle reports against an independent native twin, including dump-info inventory.
param([Parameter(Mandatory)][string]$OwnTag, [Parameter(Mandatory)][string]$NativeTag, [Parameter(Mandatory)][string]$Exe)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\native.ps1"
$own = Get-Content "$script:Lab\out\override-acceptance\$OwnTag.json" -Raw | ConvertFrom-Json
$native = Get-Content "$script:Lab\out\native-reference-$NativeTag.json" -Raw | ConvertFrom-Json
foreach ($cycle in @($own, $native)) {
    foreach ($step in @('import', 'apply', 'export')) {
        if ($cycle.$step.Exit -ne 0) { throw "$step failed" }
    }
}
if (-not $own.import.verification -or $own.import.verification.checked_files -le 0 -or $own.import.verification.checked_files -ne $own.import.verification.identical_files) { throw 'own guard failed' }
$left = "$script:Lab\out\export\$OwnTag"
$right = "$script:Lab\out\export\$NativeTag"
$diff = "$script:Lab\out\export\$OwnTag-vs-$NativeTag.diff.json"
if (Test-Path -LiteralPath $diff) { throw "comparison artifact already exists: $diff" }
if (Test-Path -LiteralPath "$script:Lab\out\override-acceptance\$OwnTag-vs-native.json") { throw 'comparison report already exists' }
& $Exe source-diff -o $diff $left $right 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'source-diff failed' }
$d = Get-Content $diff -Raw | ConvertFrom-Json
$other = @($d.differences | Where-Object { $_.status -ne 'unchanged' -and $_.path -ne 'ConfigDumpInfo.xml' })
$info = python "$PSScriptRoot\compare_dump_info.py" "$left\ConfigDumpInfo.xml" "$right\ConfigDumpInfo.xml" | ConvertFrom-Json
if ($LASTEXITCODE -notin @(0, 1) -or -not $info) { throw 'dump-info comparison failed' }
$report = [ordered]@{
    own_tag = $OwnTag; native_tag = $NativeTag; own_cycle = $own; native_cycle = $native
    twin_diff_summary = $d.summary; differing_files = @($other | ForEach-Object { $_.path })
    twin_dump_info = $info
    equal_except_config_version = ($other.Count -eq 0 -and $info.equal_except_config_version)
}
$report | ConvertTo-Json -Depth 12 | Set-Content "$script:Lab\out\override-acceptance\$OwnTag-vs-native.json" -Encoding UTF8
"independent native reference: equal_except_config_version=$($report.equal_except_config_version), files=$($d.summary.unchanged)"
if (-not $report.equal_except_config_version) { throw 'own export differs from the independent native reference' }
