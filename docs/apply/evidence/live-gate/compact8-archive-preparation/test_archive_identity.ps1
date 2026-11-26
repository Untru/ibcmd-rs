$ErrorActionPreference='Stop'
. "$PSScriptRoot/archive_identity.ps1"
$script:count=0
function Refuse([scriptblock]$Body,[string]$Reason){$caught=$false;try{& $Body}catch{$caught=$true;if($_.Exception.Message -notlike "*$Reason*"){throw}};if(!$caught){throw "expected refusal: $Reason"};$script:count++}
function Check($Value,$Label){if(!$Value){throw $Label};$script:count++}
function Current($Old){[pscustomobject]@{pid=$Old.pid;born=$Old.born;exe=$Old.exe;command=$Old.command;parent=500}}
$root='F:\ibcmd\lab\05\wave3\load\cluster'
$old=[pscustomobject]@{pid=99324;born='2026-10-01T21:15:13.035463Z';exe='C:\Program Files\1cv8\8.3.27.2214\bin\ras.exe';command='"C:\Program Files\1cv8\8.3.27.2214\bin\ras.exe" cluster --port=5545 localhost:5540';parent_key=''}
$c=Current $old
Refuse {Assert-Compact8CensusExcludesOriginal @($old) @($c) $root} 'overlapping birth'
foreach($tick in 1..9){$c=Current $old;$c.born=([DateTimeOffset]::Parse($old.born).AddTicks($tick)).UtcDateTime.ToString('o');$c.exe='C:\other\pwsh.exe';$c.command='different';Refuse {Assert-Compact8CensusExcludesOriginal @($old) @($c) $root} 'overlapping birth'}
$c=Current $old;$c.born=([DateTimeOffset]::Parse($old.born).AddTicks(10)).UtcDateTime.ToString('o');$c.exe='C:\other\pwsh.exe';$c.command='different'
$r=Assert-Compact8CensusExcludesOriginal @($old) @($c) $root
Check ($r.foreign_reused.Count -eq 1 -and $r.foreign_reused[0].original_parent_missing -and !$r.stop_authority -and !$r.signal_authority) 'disjoint complete foreign exclusion only'
$c=Current $old;$c.born='2026-10-02T06:38:27.2300620Z';Refuse {Assert-Compact8CensusExcludesOriginal @($old) @($c) $root} 'original command'
foreach($field in @('born','exe','command','parent')){$c=Current $old;$c.born='2026-10-02T06:38:27.2300620Z';$c.exe='C:\other\pwsh.exe';$c.command='different';$c.$field=$null;Refuse {Assert-Compact8CensusExcludesOriginal @($old) @($c) $root} 'missing'}
$c=Current $old;$c.born='2026-10-02T06:38:27.2300620Z';$c.exe='C:\other\pwsh.exe';$c.command="pwsh F:/ibcmd/lab/05/wave3/load/cluster/foo";Refuse {Assert-Compact8CensusExcludesOriginal @($old) @($c) $root} 'private-root'
foreach($field in @('born','exe','command')){$bad=$old.PSObject.Copy();$bad.$field='';Refuse {Assert-Compact8CensusExcludesOriginal @($bad) @() $root} 'missing'}
$bad=$old.PSObject.Copy();$bad.parent_key='guess';Refuse {Assert-Compact8CensusExcludesOriginal @($bad) @() $root} 'ancestry'
$c=Current $old;$c.born='2026-10-02T06:38:27.2300620Z';$c.exe='C:\other\pwsh.exe';$c.command='different';Refuse {Assert-Compact8CensusExcludesOriginal @($old) @($c,$c) $root} 'duplicate'
Check ((Assert-Compact8CensusExcludesOriginal @($old) @() $root).foreign_reused.Count -eq 0) 'absent original'
$short=$old.PSObject.Copy();$short.born='2026-10-01T21:15:08.44952Z'
$c=Current $short;$c.exe='C:\other\pwsh.exe';$c.command='different';$c.born=([DateTimeOffset]::Parse($short.born).AddTicks(99)).UtcDateTime.ToString('o')
Refuse {Assert-Compact8CensusExcludesOriginal @($short) @($c) $root} 'overlapping birth'
$c.born=([DateTimeOffset]::Parse($short.born).AddTicks(100)).UtcDateTime.ToString('o')
Check ((Assert-Compact8CensusExcludesOriginal @($short) @($c) $root).foreign_reused.Count -eq 1) 'actual five-digit precision boundary'
$bad=$old.PSObject.Copy();$bad.command='different original';Refuse {Assert-Compact8CensusExcludesOriginal @($old,$bad) @() $root} 'conflicting saved'
$bad=$old.PSObject.Copy();$bad.parent_key='12345:639264860816124670';Refuse {Assert-Compact8CensusExcludesOriginal @($bad) @() $root} 'parent not bound'
'PASS '+$script:count+' pure identity cases; starts/signals/moves/deletes/acquires=0'
