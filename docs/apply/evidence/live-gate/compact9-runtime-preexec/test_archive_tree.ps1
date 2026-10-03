$ErrorActionPreference='Stop'
. "$PSScriptRoot/archive_tree.ps1"
$root=$script:Compact9TreeRoot
$dirs=@([pscustomobject]@{path=$root})+@(1..4887|ForEach-Object{[pscustomobject]@{path="$root\D$_"}})
$files=@(1..597|ForEach-Object{[pscustomobject]@{file="$root\F$_";bytes=1;sha256=('A'*64)}})+@(1..16|ForEach-Object{[pscustomobject]@{file="F:\ibcmd\lab\05\wave3\load\logs\prior$_";bytes=1;sha256=('A'*64)}})
$script:capture=[pscustomobject]@{directories=$dirs;verified_prior613=$files}
function Read-Compact9CapturedArchive{return $script:capture}
function Assert-Compact9ArchivePath {param($Path);return $Path}
$script:mode='';$script:metadataCalls=0;$script:disposed=0
function Get-Compact9TreeAttributes($Path){$script:metadataCalls++;$a=$(if($Path -eq $root -or $Path -match '\\D\d+$'){[IO.FileAttributes]::Directory}else{[IO.FileAttributes]::Normal});if(($script:mode -eq 'reparse-dir' -and $Path.EndsWith('\D1')) -or ($script:mode -eq 'reparse-file' -and $Path.EndsWith('\F1'))){$a=$a -bor [IO.FileAttributes]::ReparsePoint};return $a}
function Get-Compact9TreeEnumerator($Path){
 $v=@();if($Path -ceq $root){$v=@($dirs|Select-Object -Skip 1|ForEach-Object{$_.path})+@($files|Select-Object -First 597|ForEach-Object{$_.file});if($script:mode -eq 'missing-empty'){$v=@($v|Where-Object{$_ -cne "$root\D1"})};if($script:mode -eq 'missing-file'){$v=@($v|Where-Object{$_ -cne "$root\F1"})};if($script:mode -eq 'extra-dir'){$v+=@("$root\extra")};if($script:mode -eq 'extra-file'){$v+=@("$root\extra-file")}}
 $it=[pscustomobject]@{index=-1;values=$v;repeat=($script:mode -eq 'budget' -and $Path -ceq $root)}
 $it|Add-Member ScriptMethod MoveNext {$this.index++;if($this.repeat){return $this.index -lt 65536};return $this.index -lt $this.values.Count}
 $it|Add-Member ScriptProperty Current {if($this.repeat){return "$root\D1"};return $this.values[$this.index]}
 $it|Add-Member ScriptMethod Dispose {$script:disposed++}
 return ,$it
}
function Get-Item {param($LiteralPath,[switch]$Force);[pscustomobject]@{Length=1}}
function Get-FileHash {param($LiteralPath);[pscustomobject]@{Hash=$(if($script:mode -eq 'raw-change' -and $LiteralPath.EndsWith('\F1')){'B'*64}else{'A'*64})}}
$result=Assert-Compact9CapturedArchiveTree
if($result.total_nodes -ne 5485 -or $result.directories -ne 4888 -or $result.ownership_authority -or $result.signal_authority){throw 'measured positive tree scope changed'}
'PASS actual tree function all5485 nodes/613 byte witnesses; no ownership or signal authority'
foreach($mode in @('missing-empty','missing-file','extra-dir','extra-file','reparse-dir','reparse-file','raw-change','budget')){
 $script:mode=$mode;$script:metadataCalls=0;$script:disposed=0;$caught=$false;$reason=''
 try{Assert-Compact9CapturedArchiveTree|Out-Null}catch{$caught=$true;$reason=$_.Exception.Message}
 if(!$caught -or !$script:disposed){throw "tree refusal/disposal failed$mode"}
 if($mode -eq 'budget' -and ($reason -cne 'tree discovery budget' -or $script:metadataCalls -ne 65536)){throw "budget not before final child metadata: $reason/$script:metadataCalls"}
 "PASS actual streaming tree $mode refuses/disposes; no filesystem/SQL/FIFO actions"
}
'PASS9 actual tree pure cases; real FS/starts/signals/SQL/FIFO/moves/deletes0'
