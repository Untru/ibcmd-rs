# Which staged changes make the native exclusive apply take the LONG path (structure phase, register rebuild,
# CAS GC ...) and which the SHORT one (case 2n of native-apply-trace.md)?  One fresh clone, a series of small
# stages made by our mssql-stage-source-objects, one native `config apply --dynamic=disable` after each.
# No XE trace, no snapshots: the platform's own log lines and the change register ids tell the path.
#   X0  the stage of case 2n again (catalog _ДемоВидыНоменклатуры of tree bsp8327: nothing new reaches the apply)
#   X1  catalog object module edited                (Catalogs/_ДемоКонтрагенты, tree c1)
#   X2  common module edited                        (CommonModules/_ДемоЛокализацияКлиентСервер, tree c1)
#   X3  managed form module edited                  (Catalogs/_ДемоГруппыДоступаНоменклатуры, tree c1)
#   X4  synonym of a catalog changed, no module     (Catalogs/_ДемоКонтрагенты, tree c4 = bsp8327 + the synonym)
#   X5  X4 again on the result (a stage equal to the database: nothing changed at all)
param([string]$Database = 'ibcmd_rs_04_trace_c4_paths', [string[]]$Steps = @('X0', 'X1', 'X2', 'X3', 'X4', 'X5'), [switch]$SkipRestore)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ours = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$data = "$lab\ibdata\$Database"
$out = "$lab\paths"
New-Item -ItemType Directory -Force -Path $data, $out | Out-Null
function Log($m) { Write-Host ('[{0}] PATHS {1}' -f (Get-Date -Format 'HH:mm:ss'), $m) }
function Sql($q) { (& sqlcmd -S localhost -E -C -d $Database -h -1 -W -Q "SET NOCOUNT ON; $q" | Out-String).Trim() }

if (-not $SkipRestore) {
    Log 'restore a fresh clone of the БСП 8.3.27 corpus'
    & pwsh -NoProfile -File 'F:\ibcmd\lab\04\tools\restore-clone.ps1' -Corpus bsp8327 -Name $Database -Track trace `
        -Purpose 'trace: which staged changes make the native apply take the long path (structure phase) and which the short one'
    if ($LASTEXITCODE -ne 0) { throw 'restore failed' }
}
if (($Steps -contains 'X4' -or $Steps -contains 'X5') -and -not (Test-Path "$lab\tree\c4\Catalogs")) {
    Log 'tree c4 = tree bsp8327 + the synonym of _ДемоКонтрагенты changed'
    & robocopy "$lab\tree\bsp8327" "$lab\tree\c4" /MIR /NFL /NDL /NJH /NJS /NP /R:1 /W:1 | Out-Null
    $f = "$lab\tree\c4\Catalogs\_ДемоКонтрагенты.xml"
    $b = [IO.File]::ReadAllBytes($f)
    $old = [Text.Encoding]::UTF8.GetBytes('<v8:content>Демо: Контрагенты</v8:content>')
    $new = [Text.Encoding]::UTF8.GetBytes('<v8:content>Демо: Контрагенты (трасса)</v8:content>')
    $s = [Text.Encoding]::UTF8.GetString($b)
    $t = [Text.Encoding]::UTF8.GetString($old)
    $i = $s.IndexOf($t)
    if ($i -lt 0) { throw 'synonym not found' }
    $s2 = $s.Substring(0, $i) + [Text.Encoding]::UTF8.GetString($new) + $s.Substring($i + $t.Length)
    [IO.File]::WriteAllBytes($f, [byte[]]([byte[]](0xEF, 0xBB, 0xBF) + [Text.Encoding]::UTF8.GetBytes($s2.TrimStart([char]0xFEFF))))
}

function Run-Step($id, $tree, $prefix, $note) {
    Log "$id : $note"
    $env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
    $stageLog = "$out\$id-stage.log"
    & $ours mssql-stage-source-objects --database $Database --source-root "$lab\tree\$tree" --platform 8.3.27 --replace-config-save --allow-non-lab --path-prefix $prefix *>&1 | Out-File -Encoding utf8 -LiteralPath $stageLog
    $code = $LASTEXITCODE
    $env:IBCMD_RS_NATIVE_FORM_WRITER = $null
    if ($code -ne 0) { Log "$id : stage failed ($code), see $stageLog"; return }
    $rows = Sql 'SELECT COUNT(*) FROM ConfigSave'
    $idBefore = Sql 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
    & pwsh -NoProfile -File $lock acquire trace -Name native -TimeoutMin 120
    if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' }
    try {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        & $ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost "--db-name=$Database" "--data=$data" --user=Администратор --force --dynamic=disable *>&1 | Out-File -Encoding utf8 -LiteralPath "$out\$id-apply.log"
        $exit = $LASTEXITCODE
        $sec = [Math]::Round($sw.Elapsed.TotalSeconds, 1)
    } finally {
        & pwsh -NoProfile -File $lock release trace -Name native
    }
    $idAfter = Sql 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
    $log = Get-Content -LiteralPath "$out\$id-apply.log" -Raw
    $structure = $log -match 'Обработка структуры базы данных'
    $exchange = $log -match 'Регистрация изменений в планах обмена'
    $register = $log -match 'Реструктуризация Таблица регистрации'
    $line = "{0}`t{1}`tstaged={2}`texit={3}`t{4}s`tstructure_phase={5}`texchange_registration={6}`tregister_rebuilt_log={7}`tregister_ids_changed={8}`t{9}" -f $id, $note, $rows, $exit, $sec, $structure, $exchange, $register, ($idBefore -ne $idAfter), (Get-Date -Format s)
    Add-Content -LiteralPath "$out\summary.tsv" -Value $line -Encoding utf8
    Log $line
}

if ($Steps -contains 'X0') { Run-Step 'X0' 'bsp8327' 'Catalogs/_ДемоВидыНоменклатуры' 'repeat of the 2n stage (patch mode drops the attribute: no semantic change)' }
if ($Steps -contains 'X1') { Run-Step 'X1' 'c1' 'Catalogs/_ДемоКонтрагенты' 'catalog object module edited' }
if ($Steps -contains 'X2') { Run-Step 'X2' 'c1' 'CommonModules/_ДемоЛокализацияКлиентСервер' 'common module edited' }
if ($Steps -contains 'X3') { Run-Step 'X3' 'c1' 'Catalogs/_ДемоГруппыДоступаНоменклатуры' 'managed form module edited' }
if ($Steps -contains 'X4') { Run-Step 'X4' 'c4' 'Catalogs/_ДемоКонтрагенты' 'catalog synonym changed, no module change' }
if ($Steps -contains 'X5') { Run-Step 'X5' 'c4' 'Catalogs/_ДемоКонтрагенты' 'the same stage again: equal to the database' }
Log 'paths done'
