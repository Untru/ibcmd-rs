# The platform's `config import` of a tree into one of MY lab clones, with a data directory of its own for the database (the import lab does the same:
# ibdata\<database>), under the native lock, stdin closed, stdout/stderr in logs\native_<Tag>.*.
#   pwsh -NoProfile -File native_import.ps1 -Database <db> -Tree <dir> -Tag <tag> [-Partial]
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_04_apply_')][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tree,
    [Parameter(Mandatory = $true)][string]$Tag,
    [int]$TimeoutSec = 3000,
    [int]$LockTimeoutMin = 120
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\apply'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$data = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force $data | Out-Null
$stdin = "$lab\work\empty_stdin.txt"
$out = "$lab\logs\native_$Tag.out"; $err = "$lab\logs\native_$Tag.err"
$argsList = @('infobase', 'config', 'import', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$data", '--user="Администратор"', $Tree)
$acq = & pwsh -NoProfile -File $lock acquire apply -Name native -TimeoutMin $LockTimeoutMin
if ($LASTEXITCODE -ne 0) { throw "native lock: $acq" }
try {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe' -ArgumentList $argsList -NoNewWindow -PassThru -RedirectStandardInput $stdin -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $p.Handle
    if (-not $p.WaitForExit($TimeoutSec * 1000)) { Stop-Process -Id $p.Id -Force; 'TIMEOUT'; exit 124 }
    "exit=$($p.ExitCode) elapsed=$([math]::Round($sw.Elapsed.TotalSeconds,1)) s"
    Get-Content -Tail 6 -Encoding UTF8 $out
    Get-Content -Tail 6 -Encoding UTF8 $err
    exit $p.ExitCode
}
finally { & pwsh -NoProfile -File $lock release apply -Name native | Out-Null }
