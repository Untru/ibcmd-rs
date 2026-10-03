$ErrorActionPreference='Stop'
$root='F:/ibcmd/lab/05/wave3/load/compact7-runtime-v3'
. "$root/runtime_closure.ps1"
$script:ActualRuntimePath=${function:Assert-Compact7RuntimePath}
$script:ActualRuntimeClosure=${function:Assert-Compact7RuntimeClosure}
$sha='A'*64;$script:calls=0;$script:mode='normal';$env:IBCMD_RS_COMPACT7_RUNTIME_MANIFEST_SHA=$sha
$roles=@('pwsh','python','sqlcmd','git','ibcmd','1cv8','1cv8c','rac','ras','ragent','rmngr','rphost','dbda')
$entries=@(0..69|ForEach-Object {[pscustomobject]@{file=$(if($_ -lt $roles.Count){"F:/runtime-pure/$($roles[$_]).exe"}else{"F:/runtime-pure/file-$_.exe"});bytes=100;sha256=$sha}})
$script:manifest=[pscustomobject]@{format=3;runtime_executed=$false;database='ibcmd_rs_05_load_w3_compact7_20261002';binary_sha256='81CC4526E860EDB9412D60ECFBDF2EED71376D40DBE2ADA648D4CCFFD7077AE0';files=$entries;commands=@('pwsh','python','sqlcmd','git'|ForEach-Object {[pscustomobject]@{name=$_;path="F:/runtime-pure/$_.exe"}});path_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($env:PATH)));selection_environment=@();executables=@(0..12|ForEach-Object {[pscustomobject]@{role=$roles[$_];file=$entries[$_].file}});preserved=@([pscustomobject]@{file='F:/runtime-pure/prior.json';bytes=100;sha256=$sha;kind='raw';members=1})}
function Get-Item {param($LiteralPath,[switch]$Force);if($LiteralPath -match '\.(exe|json)$'){$o=[IO.FileInfo]::new($LiteralPath);$o|Add-Member NoteProperty Length $(if($script:mode -ceq 'length' -and $LiteralPath -match 'file-69') {101}else{100}) -Force}else{$o=[IO.DirectoryInfo]::new($LiteralPath)};$o|Add-Member NoteProperty Attributes $(if($script:mode -ceq 'reparse'){[IO.FileAttributes]::ReparsePoint}else{[IO.FileAttributes]::Normal}) -Force;return $o}
function Get-FileHash {param($LiteralPath,$Algorithm);[pscustomobject]@{Hash=$(if(($script:mode -ceq 'digest' -and $LiteralPath -match 'file-69') -or ($script:mode -ceq 'retained' -and $LiteralPath -match 'oldraw')){'B'*64}else{$sha})}}
function Read-Compact7RuntimeJson($Path){if($Path.EndsWith('prior.json')){return [pscustomobject]@{raw=@([pscustomobject]@{file='F:/runtime-pure/oldraw.exe';bytes=100;sha256=$sha})}};return $script:manifest}
function Get-Command {param($Name,$CommandType);[pscustomobject]@{Source=$(if($script:mode -ceq 'resolution' -and $Name -ceq 'python'){'F:/foreign/python.exe'}else{"F:/runtime-pure/$Name.exe"})}}
Assert-Compact7RuntimeClosure -RetainedEvidence
'PASS actual runtime closure normal exact script/executable lengths/hash/PATH/resolution/retained evidence'
foreach($mode in @('digest','length','resolution','retained','reparse','PATH','relative','duplicate')){
 $script:mode=$mode;$oldPath=$env:PATH;$original=$entries[69].file
 if($mode -ceq 'PATH'){$env:PATH+=';F:/foreign'}
 if($mode -ceq 'relative'){$entries[69].file='F:relative.exe'}
 if($mode -ceq 'duplicate'){$entries[69].file=$entries[0].file}
 $caught=$false;try{Assert-Compact7RuntimeClosure -RetainedEvidence}catch{$caught=$true}finally{$env:PATH=$oldPath;$entries[69].file=$original}
 if(!$caught){throw "Actual closure failed to refuse $mode"};"PASS actual runtime closure $mode drift refuses before dispatch"
}
$script:mode='normal'
function Invoke-LiveBounded {param($Executable,$Arguments,$TimeoutSeconds);$script:calls++;return [pscustomobject]@{ExitCode=0;Stdout='pure';Stderr=''}}
# Dot-source actual child adapter, then restore the filesystem mocks overwritten by its module import.
. "$root/runtime_child.ps1"
${function:Assert-Compact7RuntimeClosure}=$script:ActualRuntimeClosure
function Read-Compact7RuntimeJson($Path){if($Path.EndsWith('prior.json')){return [pscustomobject]@{raw=@([pscustomobject]@{file='F:/runtime-pure/oldraw.exe';bytes=100;sha256=$sha})}};return $script:manifest}
$r=Invoke-LiveBounded 'pwsh' @('pure') 2;if($r.ExitCode -ne 0 -or $script:calls -ne 1){throw 'Actual adapter positive changed'}
foreach($mode in @('digest','length','resolution','reparse')){$script:mode=$mode;$before=$script:calls;$caught=$false;try{Invoke-LiveBounded 'pwsh' @('pure') 2|Out-Null}catch{$caught=$true};if(!$caught -or $script:calls -ne $before){throw 'Adapter spawned after refused closure'};"PASS actual final bounded-dispatch $mode refusal rawCalls0"}
$script:mode='normal'
function Assert-Compact7RuntimeClosure {param([switch]$RetainedEvidence);$script:checks++;if($script:refuse){throw 'pure closure drift'}}
function Start-Process {param($FilePath,$ArgumentList,[switch]$WindowStyle,[switch]$PassThru);$script:spawns++;throw 'Mock spawn reached'}
$script:spawns=0;$script:checks=0;$script:refuse=$true
foreach($relative in @('compact7-tools/cluster/private-start.ps1','compact7-tools/cluster/start.ps1','compact7-tools/live/obs.ps1')){
 $a=[Management.Automation.Language.Parser]::ParseFile("$root/$relative",[ref]$null,[ref]$null)
 $starts=$a.FindAll({param($n)$n -is [Management.Automation.Language.AssignmentStatementAst] -and $n.Extent.Text -match '=\s*Start-Process '},$true)
 foreach($start in $starts){$block=$start.Parent;$s=@($block.Statements);$index=[Array]::IndexOf($s,$start);if($index -lt 1 -or $s[$index-1].Extent.Text -cne 'Assert-Compact7RuntimeClosure'){throw 'Direct spawn lacks immediate actual closure guard'};try{. ([scriptblock]::Create($s[$index-1].Extent.Text+"`n"+$start.Extent.Text))}catch{if($_.Exception.Message -cne 'pure closure drift'){throw}}}
}
if($script:spawns -or $script:checks -ne 5){throw 'Direct startup/observer guards failed'}
'PASS actual ragent/RAS/thin-client direct spawn guards refuse before Start-Process; realStarts0'
foreach($relative in @('compact7_native.ps1','compact7-tools/live/process.ps1')){
 $a=[Management.Automation.Language.Parser]::ParseFile("$root/$relative",[ref]$null,[ref]$null)
 $starts=$a.FindAll({param($n)$n -is [Management.Automation.Language.IfStatementAst] -and $n.Extent.Text -match '\Aif\s*\(\s*(?:-not |!)\$p\.Start\(\)'},$true)
 if($starts.Count -ne 1){throw 'Direct native/helper process start inventory'}
 $start=$starts[0];$s=@($start.Parent.Statements);$i=[Array]::IndexOf($s,$start);if($i -lt 1 -or $s[$i-1].Extent.Text -cne 'Assert-Compact7RuntimeClosure'){throw 'Direct native/helper start lacks immediate guard'}
 try{. ([scriptblock]::Create($s[$i-1].Extent.Text+"`n"+$start.Extent.Text))}catch{if($_.Exception.Message -cne 'pure closure drift'){throw}}
}
'PASS actual native/helper Process.Start direct guards refuse before start'

$controller=[Management.Automation.Language.Parser]::ParseFile("$root/compact7_lifetime_v2.ps1",[ref]$null,[ref]$null)
$outer=@($controller.EndBlock.Statements|Where-Object {$_ -is [Management.Automation.Language.TryStatementAst]})[0]
foreach($kind in @('worker','heavy')){$s=@($outer.Body.Statements);$i=@(0..($s.Count-1)|Where-Object {$s[$_].Extent.Text -match ('Run "\$prefix-'+$kind+'-acquire"')})[0];if($s[$i+1].Extent.Text -cne 'Assert-Compact7RuntimeClosure -RetainedEvidence'){throw 'Post-grant deep closure check missing'};try{. ([scriptblock]::Create($s[$i+1].Extent.Text))}catch{};"PASS actual $kind post-grant retained closure check before next mutation"}
if($controller.EndBlock.Statements[-1].Extent.Text -cne 'Assert-Compact7RuntimeClosure -RetainedEvidence'){throw 'Final retained closure check missing'}
$lines=Get-Content "$root/compact7_lifetime_v2.ps1"
$entry=[Array]::IndexOf($lines,'Assert-Compact7RuntimeClosure -RetainedEvidence');$temp=@(0..($lines.Count-1)|Where-Object {$lines[$_] -match 'New-Item.*taskTemp'})[0];if($entry -lt 0 -or $entry -ge $temp){throw 'Entry check follows mutation'}
'PASS entry before first runtime directory mutation; deep final retained closure check'
'ALL PASS actual runtime-only closure additions; realStarts=Signals=SQL=FIFOacquires=Releases=Moves=Deletes=0'
