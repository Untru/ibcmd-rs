# Which staged rows differ in content from Config?  Stages several sets of the path experiment (our tool, tree1c) one after the other into ONE
# fresh clone and compares ConfigSave with Config after each (stage_vs_config.ps1).  Nothing is applied, no native command runs, no lock.
param([string]$Database = 'ibcmd_rs_04_trace_c5_probe', [switch]$Restore)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$ours = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
if ($Restore) {
    & pwsh -NoProfile -File 'F:\ibcmd\lab\04\tools\restore-clone.ps1' -Corpus bsp8327 -Name $Database -Track trace -Purpose 'trace: probe of staged rows (stage_vs_config), nothing is applied'
    if ($LASTEXITCODE -ne 0) { throw 'restore failed' }
}
$sets = [ordered]@{
    'doc (Y1/Y6, short)'          = @('Documents/_ДемоПоручениеЭкспедитору')
    'catalog (X1, short)'         = @('Catalogs/_ДемоКонтрагенты')
    'cm + doc (Y13, short)'       = @('CommonModules/_ДемоЛокализацияКлиентСервер', 'Documents/_ДемоПоручениеЭкспедитору')
    'cm + catalog (Y2, short)'    = @('CommonModules/_ДемоЛокализацияКлиентСервер', 'Catalogs/_ДемоКонтрагенты')
    'doc + catalog (Y7, LONG)'    = @('Documents/_ДемоПоручениеЭкспедитору', 'Catalogs/_ДемоКонтрагенты')
    'cm + doc + catalog (Y5, LONG)' = @('CommonModules/_ДемоЛокализацияКлиентСервер', 'Documents/_ДемоПоручениеЭкспедитору', 'Catalogs/_ДемоКонтрагенты')
}
$env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
$env:PYTHONIOENCODING = 'utf-8'
foreach ($k in $sets.Keys) {
    $pp = @(); foreach ($x in $sets[$k]) { $pp += '--path-prefix'; $pp += $x }
    & $ours mssql-stage-source-objects --database $Database --source-root "$lab\tree1c" --platform 8.3.27 --replace-config-save --allow-non-lab @pp *>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "stage of '$k' failed" }
    "=== $k"
    & pwsh -NoProfile -File "$lab\scripts\stage_vs_config.ps1" -Database $Database | Where-Object { $_ -notmatch '`tsame$' }
}
$env:IBCMD_RS_NATIVE_FORM_WRITER = $null
