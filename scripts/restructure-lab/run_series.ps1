# Cumulative cases on a trace clone: edit tree -> native import (repeated until the staging is complete)
# -> traced native apply.   pwsh -NoProfile -File run_series.ps1 -Cases h,c -Prev g_after -Database <db>
# Findings behind the retry: the FIRST native import after an apply stages only a subset of the rows (~9600 of
# ~9840) or fails with "Ссылка на неизвестный предопределенный элемент"; the apply of such a partial stage fails
# with "Нарушена целостность структуры конфигурации". The second import stages everything.
param(
    [Parameter(Mandatory = $true)][string]$Cases,
    [Parameter(Mandatory = $true)][string]$Prev,
    [string]$Database = 'ibcmd_rs_04_ddl_bsp8327_a',
    [int]$MinRows = 9800
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_ddl_* (got $Database)" }
. "$PSScriptRoot\native_lock.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
foreach ($c in ($Cases -split ",")) {
    "=== case $c  ($(Get-Date -Format s)) ==="
    python "$PSScriptRoot\edit_tree.py" $c
    if ($LASTEXITCODE -ne 0) { throw "edit_tree $c failed" }
    $data = "$lab\ibdata\$Database"
    $rows = 0
    foreach ($attempt in 1..4) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        # the native import runs under the lab "native" lock (held for the command only); the row count check
        # below is outside it
        $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Log "$lab\logs\native-import-$c-$attempt" -Arguments @(
            'infobase', 'config', 'import', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database",
            "--data=$data", "--user=$(Get-LabUser '8327')", "$lab\tree\bsp8327")
        $o = Get-Content "$lab\logs\native-import-$c-$attempt.out" -ErrorAction SilentlyContinue
        $rows = [int](sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave")
        "import #$attempt exit=$rc in {0:n1}s -> ConfigSave rows {1}: {2}" -f $sw.Elapsed.TotalSeconds, $rows, (($o | Select-Object -Last 1) -join ' ')
        if ($rc -eq 0 -and $rows -ge $MinRows) { break }
    }
    if ($rows -lt $MinRows) { throw "native import of case $c never staged a complete set ($rows rows)" }
    pwsh -NoProfile -File "$PSScriptRoot\run_case.ps1" -Database $Database -Label $c -PrevSnap $Prev
    if ($LASTEXITCODE -ne 0) { throw "run_case $c failed" }
    $Prev = "${c}_after"
}
"=== series done ($(Get-Date -Format s)) ==="
