# Sequential queue of the remaining case runs (each capture takes the heavy lock itself).
#   A. case 2a: attribute added (ConfigSave already staged in ibcmd_rs_04_trace_c2_attr): native apply, exclusive
#   B. case 2b: new catalog: stage into the same infobase, native apply, exclusive
#   C. case 3: our import of the whole tree into the new infobase, native apply, export, source-diff
#   D. case 1b: second exclusive apply on the cleaned infobase c1_disable (second edit already staged)
#   E. case 1b: the same on the twin c1_force, dynamic (stage of the second edit, then apply --dynamic=force)
param([string[]]$Steps = @('A', 'B', 'C', 'D', 'E'))
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$ours = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
$apply = "$lab\scripts\run_native_apply.ps1"
function Log($m) { Write-Host ('[{0}] QUEUE {1}' -f (Get-Date -Format 'HH:mm:ss'), $m) }
$db2 = 'ibcmd_rs_04_trace_c2_attr'

if ($Steps -contains 'A') {
    Log 'A: case 2a attribute added, native apply --dynamic=disable'
    & pwsh -NoProfile -File $apply -Database $db2 -Tag c2a-attribute-added -Dynamic disable `
        -Note 'case 2a: String(25) attribute ТрассаРеквизит added to catalog _ДемоВидыНоменклатуры; targeted stage (9 ConfigSave rows); native apply exclusive'
}
if ($Steps -contains 'B') {
    Log 'B: case 2b stage of the new catalog (targeted), then native apply'
    $env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
    & $ours mssql-stage-source-objects --database $db2 --source-root "$lab\tree\c2b" --platform 8.3.27 --replace-config-save --allow-non-lab `
        --path-prefix 'Catalogs/ТрассаНовыйСправочник' --path-prefix 'Configuration' *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\captures\c2b_stage.log"
    $code = $LASTEXITCODE
    $env:IBCMD_RS_NATIVE_FORM_WRITER = $null
    if ($code -ne 0) { throw "stage of the new catalog failed ($code), see $lab\captures\c2b_stage.log" }
    & sqlcmd -S localhost -E -C -d $db2 -f 65001 -W -s '|' -Q "SET NOCOUNT ON; SELECT FileName, PartNo, DataSize FROM ConfigSave ORDER BY FileName" *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\captures\c2b_configsave_after_stage.txt"
    & pwsh -NoProfile -File $apply -Database $db2 -Tag c2b-new-catalog -Dynamic disable `
        -Note 'case 2b: new catalog ТрассаНовыйСправочник (small, no attributes) registered in Configuration.xml; targeted stage; native apply exclusive'
}
if ($Steps -contains 'C') {
    Log 'C: case 3 new database'
    & pwsh -NoProfile -File "$lab\scripts\run_case3.ps1" -From import
}
if ($Steps -contains 'D') {
    Log 'D: case 1b second exclusive apply on the cleaned infobase (second edit already staged in c1_disable)'
    & pwsh -NoProfile -File $apply -Database ibcmd_rs_04_trace_c1_disable -Tag c1b-second-exclusive -Dynamic disable `
        -Note 'case 1b: second edit of the same 5 modules, staged in the infobase that already took the first exclusive apply; native apply exclusive (steady state)'
}
if ($Steps -contains 'E') {
    Log 'E: case 1b second dynamic apply on the twin: stage the second edit, then native apply --dynamic=force'
    $env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
    & $ours mssql-stage-source-objects --database ibcmd_rs_04_trace_c1_force --source-root "$lab\tree\c1" --platform 8.3.27 --replace-config-save --allow-non-lab `
        --path-prefix 'CommonModules/_ДемоЛокализацияКлиентСервер' --path-prefix 'Catalogs/_ДемоКонтрагенты' --path-prefix 'Documents/_ДемоПоручениеЭкспедитору' `
        --path-prefix 'Catalogs/_ДемоГруппыДоступаНоменклатуры' --path-prefix 'CommonForms/ВопросОбУстановкеВнешнейКомпоненты' *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\captures\c1b_force_stage.log"
    $code = $LASTEXITCODE
    $env:IBCMD_RS_NATIVE_FORM_WRITER = $null
    if ($code -ne 0) { throw "stage of the second edit into c1_force failed ($code)" }
    & pwsh -NoProfile -File $apply -Database ibcmd_rs_04_trace_c1_force -Tag c1b-second-dynamic -Dynamic force `
        -Note 'case 1b twin: second edit of the same 5 modules, staged after the first dynamic apply; native apply --dynamic=force (steady state)'
}
Log 'queue done'
