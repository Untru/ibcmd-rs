# Which staged changes make the native exclusive apply take the LONG path?  Second round: the native lock is
# contested by six tracks, so the stages are made first, on separate databases, and the native applies run back to
# back under ONE hold of the `native` lock (nothing else happens inside it).
#   X1  catalog object module edited          (Catalogs/_ДемоКонтрагенты, tree c1)         on c4_paths (X0 was applied there)
#   X2  common module edited                  (CommonModules/_ДемоЛокализацияКлиентСервер, tree c1)   on a fresh clone
#   X4  catalog synonym changed, no module    (Catalogs/_ДемоКонтрагенты, tree c4)          on a fresh clone
param([switch]$ApplyOnly)   # the stages are made already (the databases c4_paths, c4_x2, c4_x4 hold them): only the three applies
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ours = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$out = "$lab\paths"
function Log($m) { Write-Host ('[{0}] PATHS2 {1}' -f (Get-Date -Format 'HH:mm:ss'), $m) }
function Sql($db, $q) { (& sqlcmd -S localhost -E -C -d $db -h -1 -W -Q "SET NOCOUNT ON; $q" | Out-String).Trim() }

$plan = @(
    @{ Id = 'X1'; Db = 'ibcmd_rs_04_trace_c4_paths'; Tree = 'c1'; Prefix = 'Catalogs/_ДемоКонтрагенты'; Note = 'catalog object module edited'; Restore = $false },
    @{ Id = 'X2'; Db = 'ibcmd_rs_04_trace_c4_x2'; Tree = 'c1'; Prefix = 'CommonModules/_ДемоЛокализацияКлиентСервер'; Note = 'common module edited'; Restore = $true },
    @{ Id = 'X4'; Db = 'ibcmd_rs_04_trace_c4_x4'; Tree = 'c4'; Prefix = 'Catalogs/_ДемоКонтрагенты'; Note = 'catalog synonym changed, no module change'; Restore = $true }
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
    Log "$($p.Id): stage $($p.Prefix) of tree $($p.Tree) into $($p.Db)"
    $env:IBCMD_RS_NATIVE_FORM_WRITER = 'always'
    & $ours mssql-stage-source-objects --database $p.Db --source-root "$lab\tree\$($p.Tree)" --platform 8.3.27 --replace-config-save --allow-non-lab --path-prefix $p.Prefix *>&1 | Out-File -Encoding utf8 -LiteralPath "$out\$($p.Id)-stage.log"
    $code = $LASTEXITCODE
    $env:IBCMD_RS_NATIVE_FORM_WRITER = $null
    if ($code -ne 0) { throw "stage $($p.Id) failed ($code)" }
    $p.Rows = Sql $p.Db 'SELECT COUNT(*) FROM ConfigSave'
    $p.IdBefore = Sql $p.Db 'SELECT TOP 1 CONVERT(varchar(34), _IDRRef, 1) FROM _ConfigChngR ORDER BY _IDRRef'
    New-Item -ItemType Directory -Force -Path "$lab\ibdata\$($p.Db)" | Out-Null
}

Log 'all stages made; waiting for the native lock (three applies back to back)'
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
Log 'paths2 done'
