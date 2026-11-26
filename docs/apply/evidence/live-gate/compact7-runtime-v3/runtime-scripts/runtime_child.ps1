. 'F:/ibcmd/lab/05/wave3/load/compact7-runtime-v3/runtime_closure.ps1'
$script:Compact7RuntimeRawBounded=${function:Invoke-LiveBounded}
function Invoke-LiveBounded {
 param([string]$Executable,[string[]]$Arguments,[int]$TimeoutSeconds=30)
 Assert-Compact7RuntimeClosure
 return & $script:Compact7RuntimeRawBounded $Executable $Arguments $TimeoutSeconds
}
