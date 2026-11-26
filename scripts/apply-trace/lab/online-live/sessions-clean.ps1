# Terminates every session of ONE lab infobase (used after the lab's own clients were closed; sessions of killed
# clients stay listed as hibernated until the cluster expires them). Lab databases only.
#   pwsh -NoProfile -File sessions-clean.ps1 -Database <db> [-List]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [switch]$List
)
. F:\ibcmd\lab\05\online\tools\lab-env.ps1
if ($Database -notmatch '^ibcmd_rs_0[45]_[a-z0-9_]+$') { throw 'lab databases only' }
$c = Get-ClusterId
$id = Get-InfobaseId $Database
foreach ($s in Get-LabSessions $id) {
    "session $($s['session']) n=$($s['session-id']) app=$($s['app-id']) started=$($s['started-at']) last=$($s['last-active-at']) hibernate=$($s['hibernate'])"
    if (-not $List) {
        & $Rac $Ras session terminate "--cluster=$c" "--session=$($s['session'])" 2>&1 | Select-Object -First 2
        "  terminate rc=$LASTEXITCODE"
    }
}
"sessions left: " + @(Get-LabSessions $id).Count
