$ErrorActionPreference='Stop'
. "$PSScriptRoot/lib.ps1"
$when=[datetime]'2026-10-01T00:00:00Z'
function Fake85([int]$Id,[int]$Parent,[string]$Name) {
 [pscustomobject]@{ProcessId=$Id;ParentProcessId=$Parent;Name=$Name;ExecutablePath="$script:Bin\$Name";CreationDate=$when;CommandLine="test private $Name"}
}
$agent=Fake85 700001 1 'ragent.exe'
$child=Fake85 700002 700001 'rphost.exe'
$child.CreationDate=$when.AddSeconds(1)
$foreign=Fake85 700003 2 'rphost.exe'
$script:FakeProcesses=@($agent,$child,$foreign)
function Get-CimInstance { param($ClassName,$Filter); $script:FakeProcesses }
$a=Identity85 $agent
$state=[pscustomobject]@{anchors=@($a);known=@()}
if(@(Owned85 $state).Count -ne 2){throw 'descendant inventory includes foreign or misses own'}
$childIdentity=Identity85 $child
$state.known=@($childIdentity)
$script:FakeProcesses=@($child,$foreign)
if(@(Owned85 $state).Count -ne 1){throw 'orphan disappeared after anchor exit'}
$json=$state|ConvertTo-Json -Depth 6
$state=$json|ConvertFrom-Json
if(@(Owned85 $state).Count -ne 1){throw 'serialized UTC birth changed identity'}
$child.CreationDate=$when.AddSeconds(20)
$refused=$false
try{Owned85 $state|Out-Null}catch{$refused=$_.Exception.Message -like '*PID reused*'}
if(-not $refused){throw 'PID reuse accepted'}
$child.CreationDate=$when.AddSeconds(1)
$child.CommandLine='foreign replacement command'
$refused=$false
try{Owned85 $state|Out-Null}catch{$refused=$_.Exception.Message -like '*PID reused*'}
if(-not $refused){throw 'command drift accepted'}
$child.CommandLine=$childIdentity.command
$state=[pscustomobject]@{anchors=@($a);known=@()}
$script:FakeProcesses=@($agent,$child)
$child.ExecutablePath='C:\foreign\rphost.exe'
$refused=$false
try{Owned85 $state|Out-Null}catch{$refused=$_.Exception.Message -like '*unexpected private descendant*'}
if(-not $refused){throw 'foreign executable descendant accepted'}
'PASS descendant inventory, foreign exclusion, orphan union, UTC roundtrip, PID/command/executable drift refusal (no signals)'
function Import-Csv { param($LiteralPath,$Delimiter); [pscustomobject]@{track='p85';database='ibcmd_rs_05_p85_w3_mock_20261001'} }
RequireOwnedNames85 @('ibcmd_rs_05_p85_w3_mock_20261001')
foreach($invalid in @('bsp','ibcmd_rs_05_p85_w3_unmanifested_20261001')) {
 $refused=$false
 try { RequireOwnedNames85 @($invalid) } catch { $refused=$_.Exception.Message -like '*foreign/unmanifested*' }
 if(-not $refused){throw "unmanifested registration accepted: $invalid"}
}
'PASS exact manifest registry superset refusal including disconnected names (no signals)'
RequireLabPaths85 @('F:\ibcmd\lab\05\wave3\platform85\obs')
foreach($invalid in @('D:\foreign','F:\ibcmd\lab\05\wave3\platform85\..\foreign')) {
 $refused=$false
 try { RequireLabPaths85 @($invalid) } catch { $refused=$_.Exception.Message -like '*outside*' }
 if(-not $refused){throw "foreign observer path accepted: $invalid"}
}
foreach($invalid in @('../native-old','native*','')) {
 $refused=$false
 try { RequireLabel85 $invalid } catch { $refused=$true }
 if(-not $refused){throw "unsafe observer label accepted: $invalid"}
}
$command='1cv8c.exe ENTERPRISE /S"localhost:6541\ibcmd_rs_05_p85_w3_mock_20261001" /Execute"F:\ibcmd\lab\05\wave3\platform85\observer\IbcmdRsObserver.epf" /C"mock;poll;owned"'
RequireObserverCommand85 $command 'mock'
foreach($invalid in @($command.Replace('6541','3541'),$command.Replace('mock_20261001','unmanifested_20261001'),$command.Replace('IbcmdRsObserver.epf','foreign.epf'),$command.Replace('/C"mock;','/C"other;'))) {
 $refused=$false
 try { RequireObserverCommand85 $invalid 'mock' } catch { $refused=$true }
 if(-not $refused){throw 'foreign observer command accepted'}
}
function Test-Path { param($LiteralPath); $LiteralPath.EndsWith('.pid') }
function Get-Item { param($LiteralPath,[switch]$Force); [pscustomobject]@{Attributes=[IO.FileAttributes]::Normal} }
$refused=$false
try { RequireFreshObserver85 'F:\ibcmd\lab\05\wave3\platform85\obs' 'failed-launch' } catch { $refused=$_.Exception.Message -like '*existing artifacts*' }
if(-not $refused){throw 'PID-only failed launch can be overwritten'}
'PASS observer path/label/command/manifest and PID-only artifact reuse refusal (mocked, no writes/launches)'
$oldRoot=$script:Root
$script:Root='D:\foreign-cluster'
$refused=$false
try { RequireRoot85 } catch { $refused=$_.Exception.Message -like '*unexpected private cluster root*' }
$script:Root=$oldRoot
if(-not $refused){throw 'foreign root accepted'}
function Test-Path { param($LiteralPath); $true }
function Get-Item {
 param($LiteralPath,[switch]$Force)
 [pscustomobject]@{Attributes=$(if($LiteralPath -eq 'F:\ibcmd\lab\05\wave3'){[IO.FileAttributes]::ReparsePoint}else{[IO.FileAttributes]::Directory})}
}
$refused=$false
try { RequireRoot85 } catch { $refused=$_.Exception.Message -like '*reparse ancestry*' }
if(-not $refused){throw 'reparse root ancestry accepted'}
'PASS fixed F root and reparse ancestry refusal (mocked, no mutations/signals)'
