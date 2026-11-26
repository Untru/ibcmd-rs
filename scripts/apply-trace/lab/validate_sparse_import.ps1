# Does native `config import files --partial` need the whole tree, or only the listed module files?
# (The УХ case 1 plan must stage module edits without a copy of the 9 GB УХ tree.)  Two edited module files of the
# БСП demo in a sparse base directory: an object module (container kind) and a managed form module (in the form row).
# The database is c4_paths (nothing is applied; ConfigSave is only staged).  One native command under the native lock.
param([string]$Database = 'ibcmd_rs_04_trace_c4_paths')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\trace'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ref = 'F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native'
$base = "$lab\sparse_base"
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$files = @('Catalogs/_ДемоКонтрагенты/Ext/ObjectModule.bsl', 'Catalogs/_ДемоГруппыДоступаНоменклатуры/Forms/ФормаЭлемента/Ext/Form/Module.bsl')
if (Test-Path -LiteralPath $base) { Remove-Item -LiteralPath $base -Recurse -Force -Confirm:$false }
foreach ($f in $files) {
    $src = Join-Path $ref ($f -replace '/', '\')
    $dst = Join-Path $base ($f -replace '/', '\')
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dst) | Out-Null
    $orig = [IO.File]::ReadAllBytes($src)
    $add = [Text.Encoding]::UTF8.GetBytes("`r`n// ibcmd-rs trace, sparse native import check`r`n")
    [IO.File]::WriteAllBytes($dst, $orig + $add)
}
function Sql($q) { (& sqlcmd -S localhost -E -C -d $Database -h -1 -W -Q "SET NOCOUNT ON; $q" | Out-String).Trim() }
"ConfigSave rows before: $(Sql 'SELECT COUNT(*) FROM ConfigSave')"
& pwsh -NoProfile -File $lock acquire trace -Name native -TimeoutMin 120
if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' }
try {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $ibcmd infobase config import files --dbms=MSSQLServer --db-server=localhost "--db-name=$Database" "--data=$lab\ibdata\$Database" --user=Администратор "--base-dir=$base" --partial @files *>&1 | Out-File -Encoding utf8 -LiteralPath "$lab\p85\sparse-import.log"
    $exit = $LASTEXITCODE
    "native import exit $exit in $([Math]::Round($sw.Elapsed.TotalSeconds, 1)) s"
} finally {
    & pwsh -NoProfile -File $lock release trace -Name native
}
"ConfigSave rows after: $(Sql 'SELECT COUNT(*) FROM ConfigSave')"
"names: $((Sql 'SELECT TOP 40 FileName FROM ConfigSave ORDER BY FileName') -replace '\s+', ' ')"
Get-Content -LiteralPath "$lab\p85\sparse-import.log" -Tail 8
