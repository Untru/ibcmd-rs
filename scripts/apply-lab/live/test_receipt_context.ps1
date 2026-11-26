# Pure path/OS mocks: no directory creation, process launch or signal.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/process.ps1"
$saved=$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT
$script:starts=0;$script:reparse='';$script:missing=$false
function New-LiveBoundedProcess { $script:starts++;throw 'real spawn forbidden' }
function Test-Path {param($LiteralPath,$PathType);return !$script:missing}
function Get-Item {param($LiteralPath,[switch]$Force);[pscustomobject]@{Attributes=$(if($LiteralPath -ceq $script:reparse){[IO.FileAttributes]::ReparsePoint}else{[IO.FileAttributes]::Directory})}}
try{
foreach($root in @('F:\ibcmd\lab\05\wave3\load\private4-child-receipts','F:\ibcmd\lab\05\wave3\metadata\body2-child-receipts')){
 $env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT=$root
 if((Get-LiveChildReceiptRoot) -cne $root){throw 'exact owned root refused'}
 "PASS exact owned context $root"
 $script:reparse='F:\ibcmd\lab\05\wave3';$caught=$false
 try{Invoke-LiveBounded never-spawn @() 1|Out-Null}catch{$caught=$_.Exception.Message -ceq 'child receipt reparse ancestry'}
 if(!$caught -or $script:starts){throw 'reparse context reached spawn'};$script:reparse=''
 $script:missing=$true;$caught=$false
 try{Invoke-LiveBounded never-spawn @() 1|Out-Null}catch{$caught=$_.Exception.Message -ceq 'child receipt directory missing'}
 if(!$caught -or $script:starts){throw 'missing directory reached spawn'};$script:missing=$false
}
foreach($root in @('D:\foreign','F:\ibcmd\lab\05\wave3\metadata\foreign','F:\ibcmd\lab\05\wave3\metadata\body2-child-receipts-other','F:ibcmd\lab\05\wave3\metadata\body2-child-receipts','\ibcmd\lab\05\wave3\metadata\body2-child-receipts','body2-child-receipts')){
 $env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT=$root;$caught=$false
 try{Invoke-LiveBounded never-spawn @() 1|Out-Null}catch{$caught=$_.Exception.Message -match 'fully qualified child receipt root required|unknown child receipt context'}
 if(!$caught -or $script:starts){throw "foreign/relative context reached spawn: $root"}
}
'PASS foreign/relative/reparse/missing refusal; realStarts=realSignals=SQLcalls=registryWrites=0'
}finally{$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT=$saved}
