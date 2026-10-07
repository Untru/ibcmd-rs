$ErrorActionPreference='Stop'
# Read-only exclusion proof. This never grants signal/stop/archive-move authority.
function Get-Compact8BirthInterval([string]$Born) {
    if($Born -cnotmatch '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.(\d{1,7})Z$'){throw 'archive identity birth precision/UTC missing'}
    $digits=$Matches[1].Length
    $date=[DateTimeOffset]::ParseExact($Born,"yyyy-MM-dd'T'HH:mm:ss.FFFFFFF'Z'",[Globalization.CultureInfo]::InvariantCulture,[Globalization.DateTimeStyles]::AssumeUniversal)
    $width=[long][Math]::Pow(10,7-$digits)
    [pscustomobject]@{start=$date.UtcTicks;end=$date.UtcTicks+$width;digits=$digits}
}
function Assert-Compact8IdentityFields($Identity,[switch]$Current) {
    if(!$Identity -or [long]$Identity.pid -le 0 -or [long]$Identity.pid -gt [uint32]::MaxValue){throw 'archive identity PID missing'}
    if([string]::IsNullOrWhiteSpace([string]$Identity.exe) -or ![IO.Path]::IsPathFullyQualified([string]$Identity.exe)){throw 'archive identity executable missing'}
    if([string]::IsNullOrWhiteSpace([string]$Identity.command)){throw 'archive identity command missing'}
    [void](Get-Compact8BirthInterval ([string]$Identity.born))
    if($Current -and (!$Identity.PSObject.Properties['parent'] -or [long]$Identity.parent -le 0 -or [long]$Identity.parent -gt [uint32]::MaxValue)){throw 'archive identity current parent missing'}
}
function Assert-Compact8SavedIdentity($Saved) {
    Assert-Compact8IdentityFields $Saved
    if(![string]::IsNullOrEmpty([string]$Saved.parent_key)){
        if([string]$Saved.parent_key -cnotmatch '^([1-9][0-9]*):([1-9][0-9]*)$' -or [long]$Matches[1] -gt [uint32]::MaxValue){throw 'archive saved ancestry malformed'}
    }
}
function Assert-Compact8CensusExcludesOriginal($SavedIdentities,$Census,[string]$ClusterRoot) {
    if(![IO.Path]::IsPathFullyQualified($ClusterRoot)){throw 'archive cluster root fully qualified required'}
    $root=$ClusterRoot.Replace('/','\').TrimEnd('\')
    $saved=@($SavedIdentities);if(!$saved.Count -or $saved.Count -gt 32 -or @($Census).Count -gt 16384){throw 'archive identity census budget'}
    foreach($old in $saved){
        Assert-Compact8SavedIdentity $old
        foreach($duplicate in @($saved|Where-Object{[long]$_.pid -eq [long]$old.pid})){
            foreach($field in @('born','exe','command','parent_key')){if([string]$duplicate.$field -cne [string]$old.$field){throw 'archive conflicting saved PID identity'}}
        }
        if(![string]::IsNullOrEmpty([string]$old.parent_key)){
            $parts=([string]$old.parent_key).Split(':');$parentId=[long]$parts[0];$parentBirth=[long]$parts[1]
            if(!@($saved|Where-Object{[long]$_.pid -eq $parentId -and (Get-Compact8BirthInterval ([string]$_.born)).start -eq $parentBirth}).Count){throw 'archive saved ancestry parent not bound'}
        }
    }
    $seen=[Collections.Generic.HashSet[long]]::new();$foreign=[Collections.Generic.List[object]]::new()
    foreach($p in $Census){
        if(!$seen.Add([long]$p.pid)){throw 'archive duplicate current PID'}
        if(([string]$p.command).Replace('/','\').IndexOf($root,[StringComparison]::OrdinalIgnoreCase) -ge 0){throw 'archive private-root process present'}
        $matches=@($saved|Where-Object{[long]$_.pid -eq [long]$p.pid})
        if(!$matches.Count){continue}
        Assert-Compact8IdentityFields $p -Current
        $current=Get-Compact8BirthInterval ([string]$p.born)
        foreach($old in $matches){
            $original=Get-Compact8BirthInterval ([string]$old.born)
            if($current.start -lt $original.end -and $original.start -lt $current.end){throw 'archive original/overlapping birth process present'}
            # A new instance running the identical old executable/command is also refused.
            if([string]::Equals([string]$p.exe,[string]$old.exe,[StringComparison]::OrdinalIgnoreCase) -and [string]::Equals([string]$p.command,[string]$old.command,[StringComparison]::Ordinal)){throw 'archive new instance of original command present'}
        }
        $foreign.Add([ordered]@{pid=[long]$p.pid;parent=[long]$p.parent;born=[string]$p.born;exe=[string]$p.exe;command_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes([string]$p.command)));original_parent_missing=@($matches|Where-Object{[string]::IsNullOrEmpty([string]$_.parent_key)}).Count -gt 0;read_only_exclusion_only=$true})
    }
    [pscustomobject]@{foreign_reused=@($foreign);stop_authority=$false;signal_authority=$false}
}
