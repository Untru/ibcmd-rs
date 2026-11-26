# Read-only origin classification, without native commands or output mutation.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\workload_lab.ps1"
$lab = 'F:\ibcmd\lab\05\wave3\load'
foreach ($notes in @('from F:\ibcmd\lab\dbbak\bsp.bak', 'from F:/ibcmd/lab/05/wave3/load/f5-owned-full.bak')) {
    if (-not (Test-LoadRestoreOrigin $notes $lab)) { throw "valid restored origin refused: $notes" }
}
foreach ($notes in @('from F:\ibcmd\lab\05\wave3\metadata\own.bak', 'from F:\ibcmd\lab\05\wave3\load-other\own.bak', 'from F:\ibcmd\lab\05\wave3\load\..\foreign.bak', 'from F:\ibcmd\lab\dbbak\..\foreign.bak', 'from D:\foreign.bak', 'from own.bak', 'empty', 'from \\server\share\own.bak', 'from F:..\..\lab\05\wave3\load\own.bak', 'from \ibcmd\lab\05\wave3\load\own.bak')) {
    if (Test-LoadRestoreOrigin $notes $lab) { throw "foreign/escaped/relative origin admitted: $notes" }
}
'PASS corpus and exact owned-lab origins; ten foreign/escaped/relative/unrestored origins refused; no native writes'
