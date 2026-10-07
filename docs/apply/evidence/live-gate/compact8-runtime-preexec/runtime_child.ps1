. 'F:/ibcmd/lab/05/wave3/load/compact8-runtime-v1/runtime_closure.ps1'
$script:Compact8RuntimeRawBounded=${function:Invoke-LiveBounded}
function Invoke-LiveBounded {
 param([string]$Executable,[string[]]$Arguments,[int]$TimeoutSeconds=30)
 Assert-Compact8RuntimeClosure
 return & $script:Compact8RuntimeRawBounded $Executable $Arguments $TimeoutSeconds
}
