# Bisect of the long/short path (native-apply-trace.md, section 6.4): the container-only set of case 1c went the LONG way
# although its parts X1 (catalog object module) and X2 (common module) went the short way.  Two stages on existing clones, then both
# native applies under one hold of the native lock (nothing else happens inside it).
#   Y1  Documents/_ДемоПоручениеЭкспедитору alone (manager module edited)       on c4_x4
#   Y2  CommonModules/_ДемоЛокализацияКлиентСервер + Catalogs/_ДемоКонтрагенты  on c4_paths
param([switch]$ApplyOnly)   # the stages are made already (c4_x4 and c4_paths hold them): only the two applies
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ours = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$out = "$lab\paths"
function Log($m) { Write-Host ('[{0}] PATHS4 {1}' -f (Get-Date -Format 'HH:mm:ss'), $m) }
function Sql($db, $q) { (& sqlcmd -S localhost -E -C -d $db -h -1 -W -Q "SET NOCOUNT ON; $q" | Out-String).Trim() }

$plan = @(
    @{ Id = 'Y1'; Db = 'ibcmd_rs_04_trace_c4_x4'; Tree = 'tree1c'; Prefixes = @('Documents/_ДемоПоручениеЭкспедитору'); Note = 'document manager module edited (the document with its forms and help staged)'; Restore = $false },
    @{ Id = 'Y2'; Db = 'ibcmd_rs_04_trace_c4_paths'; Tree = 'tree1c'; Prefixes = @('CommonModules/_ДемоЛокализацияКлиентСервер', 'Catalogs/_ДемоКонтрагенты'); Note = 'common module + catalog object module edited (X2 + X1 together)'; Restore = $false }
)
foreach ($p in $plan) {
    if ($ApplyOnly) {
        $p.Rows = Sql $p.Db 'SELECT COUNT(*) FROM ConfigSave'
        $p.IdBefore = Sql $p.Db 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
        New-Item -ItemType Directory -Force -Path "$lab\ibdata\$($p.Db)" | Out-Null
        continue
    }
    if ($p.Restore) {
        Log "$($p.Id): restore a fresh clone $($p.Db)"
        & pwsh -NoProfile -File 'F:\ibcmd\lab\04\tools\restore-clone.ps1' -Corpus bsp8327 -Name $p.Db -Track trace `
            -Purpose 'trace: path-selection experiment (long or short native apply), one staged change per clone'
        if ($LASTEXITCODE -ne 0) { throw "restore of $($p.Db) failed" }
    }
    Log "$($p.Id): stage $($p.Prefixes -join ' + ') of $($p.Tree) into $($p.Db)"
    $env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
    $pp = @(); foreach ($x in $p.Prefixes) { $pp += '--path-prefix'; $pp += $x }
    & $ours mssql-stage-source-objects --database $p.Db --source-root "$lab\$($p.Tree)" --platform 8.3.27 --replace-config-save --allow-non-lab @pp *>&1 | Out-File -Encoding utf8 -LiteralPath "$out\$($p.Id)-stage.log"
    $code = $LASTEXITCODE
    $env:IBCMD_RS_NATIVE_FORM_WRITER = $null
    if ($code -ne 0) { throw "stage $($p.Id) failed ($code)" }
    $p.Rows = Sql $p.Db 'SELECT COUNT(*) FROM ConfigSave'
    $p.IdBefore = Sql $p.Db 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
    New-Item -ItemType Directory -Force -Path "$lab\ibdata\$($p.Db)" | Out-Null
}

Log 'all stages made; waiting for the native lock (two applies back to back)'
& pwsh -NoProfile -File $lock acquire trace -Name native -TimeoutMin 240
if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' }
try {
    foreach ($p in $plan) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        & $ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost "--db-name=$($p.Db)" "--data=$lab\ibdata\$($p.Db)" --user=Администратор --force --dynamic=disable *>&1 | Out-File -Encoding utf8 -LiteralPath "$out\$($p.Id)-apply.log"
        $p.Exit = $LASTEXITCODE
        $p.Sec = [Math]::Round($sw.Elapsed.TotalSeconds, 1)
    }
} finally {
    & pwsh -NoProfile -File $lock release trace -Name native
}
foreach ($p in $plan) {
    $idAfter = Sql $p.Db 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
    $log = Get-Content -LiteralPath "$out\$($p.Id)-apply.log" -Raw
    $line = "{0}`t{1}`tstaged={2}`texit={3}`t{4}s`tstructure_phase={5}`texchange_registration={6}`tregister_rebuilt_log={7}`tregister_ids_changed={8}`t{9}" -f `
        $p.Id, $p.Note, $p.Rows, $p.Exit, $p.Sec, ($log -match 'Обработка структуры базы данных'), ($log -match 'Регистрация изменений в планах обмена'),
        ($log -match 'Реструктуризация Таблица регистрации'), ($p.IdBefore -ne $idAfter), (Get-Date -Format s)
    Add-Content -LiteralPath "$out\summary.tsv" -Value $line -Encoding utf8
    Log $line
}
Log 'paths4 done'
