# A native ibcmd command on one of MY lab clones with a data directory of its own for the database (a shared --data directory made
# the platform's `config import` of a whole tree fail or stage a wrong set: see docs/apply/own-apply.md, "Removals"), under the
# native lock when the command writes; stdin closed; stdout/stderr in logs\native_<Tag>.*.
#   pwsh -NoProfile -File native_cmd.ps1 -Database <db> -Tag <tag> -Command "infobase config apply --force --dynamic=disable"
#   pwsh -NoProfile -File native_cmd.ps1 -Database <db> -Tag <tag> -Command "infobase config export F:/dir"     (no lock)
# Slashes, not backslashes, in paths inside -Command when it is called from Git Bash.
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_04_apply_')][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tag,
    [Parameter(Mandatory = $true)][string]$Command,
    [int]$TimeoutSec = 3000,
    [int]$LockTimeoutMin = 120,
    [string]$Ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = 'F:\ibcmd\lab\04\apply'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$data = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force $data | Out-Null
$stdin = "$lab\work\empty_stdin.txt"
$out = "$lab\logs\native_$Tag.out"; $err = "$lab\logs\native_$Tag.err"
$argsList = @($Command -split ' ') + @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$data", '--user="Администратор"')
$writes = $Command -match 'config\s+(apply|import|load)|infobase\s+create'
if ($writes) {
    $acq = & pwsh -NoProfile -File $lock acquire apply -Name native -TimeoutMin $LockTimeoutMin
    if ($LASTEXITCODE -ne 0) { throw "native lock: $acq" }
}
try {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Ibcmd -ArgumentList $argsList -NoNewWindow -PassThru -RedirectStandardInput $stdin -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $p.Handle
    if (-not $p.WaitForExit($TimeoutSec * 1000)) { Stop-Process -Id $p.Id -Force; 'TIMEOUT'; exit 124 }
    "exit=$($p.ExitCode) elapsed=$([math]::Round($sw.Elapsed.TotalSeconds,1)) s"
    Get-Content -Tail 8 -Encoding UTF8 $out
    Get-Content -Tail 6 -Encoding UTF8 $err
    exit $p.ExitCode
}
finally { if ($writes) { & pwsh -NoProfile -File $lock release apply -Name native | Out-Null } }
