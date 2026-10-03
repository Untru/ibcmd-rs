. 'F:/ibcmd/lab/05/wave3/load/compact9-runtime-v1/runtime_closure.ps1'
$script:Compact9RuntimeRawBounded=${function:Invoke-LiveBounded}
function Invoke-LiveBounded {
 param([string]$Executable,[string[]]$Arguments,[int]$TimeoutSeconds=30)
 Assert-Compact9RuntimeClosure
 return & $script:Compact9RuntimeRawBounded $Executable $Arguments $TimeoutSeconds
}
