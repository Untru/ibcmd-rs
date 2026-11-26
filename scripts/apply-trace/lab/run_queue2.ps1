# Queue 2 (after the first queue stopped at the failed stage of the new catalog by our import):
#   A1/A2: case 2a: attribute added, staged NATIVELY (import files --partial), then native apply exclusive
#   B1/B2: case 2b: new catalog, staged natively, then native apply exclusive
#   C: case 3 new database (our import, native apply, export, source-diff)
#   D/E: case 1b steady-state applies
param([string[]]$Steps = @('A1', 'A2', 'B1', 'B2', 'C', 'D', 'E'))
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$ours = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
$apply = "$lab\scripts\run_native_apply.ps1"
$imp = "$lab\scripts\run_native_import_files.ps1"
function Log($m) { Write-Host ('[{0}] QUEUE2 {1}' -f (Get-Date -Format 'HH:mm:ss'), $m) }
$db2 = 'ibcmd_rs_04_trace_c2_attr'

if ($Steps -contains 'A1') {
    Log 'A1: native import files --partial: the attribute change of catalog _ДемоВидыНоменклатуры'
    & pwsh -NoProfile -File $imp -Database $db2 -Tag c2a1-native-stage-attribute -BaseDir "$lab\tree\bsp8327" -Files 'Catalogs/_ДемоВидыНоменклатуры.xml' `
        -Note 'case 2a stage by the NATIVE partial import: String(25) attribute ТрассаРеквизит added to catalog _ДемоВидыНоменклатуры'
}
if ($Steps -contains 'A2') {
    Log 'A2: native apply --dynamic=disable of the natively staged attribute'
    & pwsh -NoProfile -File $apply -Database $db2 -Tag c2a2-native-apply-attribute -Dynamic disable `
        -Note 'case 2a: native apply exclusive of the natively staged String(25) attribute (DDL expected)'
}
if ($Steps -contains 'B1') {
    Log 'B1: native import files --partial: the new catalog + Configuration.xml'
    & pwsh -NoProfile -File $imp -Database $db2 -Tag c2b1-native-stage-new-catalog -BaseDir "$lab\tree\c2b" -Files 'Catalogs/ТрассаНовыйСправочник.xml|Configuration.xml' `
        -Note 'case 2b stage by the NATIVE partial import: new catalog ТрассаНовыйСправочник (no attributes) + Configuration.xml'
}
if ($Steps -contains 'B2') {
    Log 'B2: native apply --dynamic=disable of the natively staged new catalog'
    & pwsh -NoProfile -File $apply -Database $db2 -Tag c2b2-native-apply-new-catalog -Dynamic disable `
        -Note 'case 2b: native apply exclusive of the natively staged new catalog (DDL expected: CREATE TABLE)'
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
Log 'queue2 done'
