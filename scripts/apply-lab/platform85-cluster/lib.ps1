# Separate exact-build wave3 cluster. Never identifies ownership by a port alone.
$script:Root = 'F:\ibcmd\lab\05\wave3\platform85\cluster'
$script:Bin = 'C:\Program Files\1cv8\8.5.1.1150\bin'
$script:StatePath = Join-Path $script:Root 'state.json'
$script:Data = Join-Path $script:Root 'srvinfo'
$script:Ports = @(6540,6541,6545) + @(6560..6591)
function RequireLabPaths85([string[]]$Paths) {
    $base = 'F:\ibcmd\lab\05\wave3\platform85\'
    foreach ($inputPath in $Paths) {
        $path = [IO.Path]::GetFullPath($inputPath)
        if (-not $path.StartsWith($base,[StringComparison]::OrdinalIgnoreCase)) { throw 'path is outside the owned platform85 lab' }
        for ($probe = $path; $probe; $probe = [IO.Path]::GetDirectoryName($probe)) {
            if ((Test-Path -LiteralPath $probe) -and ((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw 'owned platform85 lab path has reparse ancestry'
            }
        }
    }
}
function RequireLabel85([string]$Label) {
    if ($Label -notmatch '^[a-z0-9-]+$') { throw 'label: [a-z0-9-]+' }
}
function RequireFreshObserver85([string]$ObsDir,[string]$Label) {
    RequireLabel85 $Label
    $paths = @('log','pid','identity.json','client-out.txt') | ForEach-Object { Join-Path $ObsDir "$Label.$_" }
    RequireLabPaths85 $paths
    if (@($paths | Where-Object { Test-Path -LiteralPath $_ }).Count) { throw 'observer label has existing artifacts; preserve failed-launch ownership and choose a fresh label' }
}
function RequireObserverCommand85([string]$Command,[string]$Label) {
    RequireLabel85 $Label
    if ($Command -notmatch 'localhost:6541\\(?<database>ibcmd_rs_05_p85_w3_[a-z0-9_]+)') { throw 'observer command does not bind the private database' }
    $database = $Matches.database
    if ($Command -notmatch [regex]::Escape('F:\ibcmd\lab\05\wave3\platform85\observer\IbcmdRsObserver.epf') -or $Command -notmatch [regex]::Escape("/C`"$Label;")) {
        throw 'observer command does not bind the owned EPF/label'
    }
    RequireOwnedNames85 @($database)
}
function RequireRoot85 {
    $path = [IO.Path]::GetFullPath($script:Root)
    if ($path -ne 'F:\ibcmd\lab\05\wave3\platform85\cluster') { throw 'unexpected private cluster root' }
    for ($probe = $path; $probe; $probe = [IO.Path]::GetDirectoryName($probe)) {
        if (Test-Path -LiteralPath $probe) {
            if ((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'private cluster root has reparse ancestry'
            }
        }
    }
    foreach ($file in @($script:StatePath, (Join-Path $script:Root 'srvinfo'))) {
        if ((Test-Path -LiteralPath $file) -and ((Get-Item -LiteralPath $file -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'private cluster state/data is a reparse point'
        }
    }
}
RequireRoot85
. "$PSScriptRoot\..\live\process.ps1"
function Rac85([string[]]$Arguments) {
    Invoke-LiveRac -Executable "$script:Bin\rac.exe" -Arguments (@('localhost:6545') + $Arguments)
}
function State85 {
    if (Test-Path -LiteralPath $script:StatePath) {
        $s = Get-Content -LiteralPath $script:StatePath -Raw | ConvertFrom-Json
        if ($s.platform -ne '8.5.1.1150' -or $s.root -ne $script:Root -or $s.track -ne 'p85') { throw 'invalid private cluster state' }
        foreach ($a in @($s.anchors)) {
            $agent = $a.executable -eq "$script:Bin\ragent.exe" -and $a.command -eq "`"$script:Bin\ragent.exe`" -agent -port 6540 -regport 6541 -range 6560:6591 -d $script:Data"
            $ras = $a.executable -eq "$script:Bin\ras.exe" -and $a.command -eq "`"$script:Bin\ras.exe`" cluster --port=6545 localhost:6540"
            if (-not $agent -and -not $ras) { throw 'private anchor command does not bind the owned data/ports' }
        }
        $s
    }
}
function Identity85($p) {
    [pscustomobject]@{ pid = [int]$p.ProcessId; born = $p.CreationDate.ToUniversalTime().ToString('o'); executable = $p.ExecutablePath; command = $p.CommandLine }
}
function Same85($p, $i) {
    $p -and ([int]$p.ProcessId -eq $i.pid) -and
        ($p.CreationDate.ToUniversalTime().Ticks -eq ([datetime]$i.born).ToUniversalTime().Ticks) -and
        ($p.ExecutablePath -eq $i.executable) -and ($p.CommandLine -eq $i.command)
}
function Owned85($state) {
    if (-not $state) { return @() }
    $all = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('ragent.exe','rmngr.exe','rphost.exe','ras.exe','dbda.exe') })
    $owned = @{}
    foreach ($anchor in (@($state.anchors) + @($state.known))) {
        if (-not $anchor) { continue }
        $exeName = [IO.Path]::GetFileName($anchor.executable)
        if ($exeName -notin @('ragent.exe','rmngr.exe','rphost.exe','ras.exe','dbda.exe') -or $anchor.executable -ne (Join-Path $script:Bin $exeName)) { throw 'private identity has an unexpected executable' }
        $p = $all | Where-Object { $_.ProcessId -eq $anchor.pid } | Select-Object -First 1
        if (Same85 $p $anchor) { $owned[[int]$p.ProcessId] = $p }
        elseif ($p) { throw "private cluster PID reused: $($anchor.pid)" }
    }
    do {
        $added = $false
        foreach ($p in $all) {
            if (-not $owned.ContainsKey([int]$p.ProcessId) -and $owned.ContainsKey([int]$p.ParentProcessId)) {
                if ($p.ExecutablePath -ne (Join-Path $script:Bin $p.Name)) { throw 'unexpected private descendant executable' }
                if ($p.CreationDate -lt $owned[[int]$p.ParentProcessId].CreationDate) { throw 'ambiguous descendant creation time' }
                $owned[[int]$p.ProcessId] = $p; $added = $true
            }
        }
    } while ($added)
    @($owned.Values)
}
function Remember85($state) {
    $map = @{}
    foreach ($i in (@($state.anchors) + @($state.known))) { if ($i) { $map[[int]$i.pid] = $i } }
    foreach ($p in @(Owned85 $state)) { $map[[int]$p.ProcessId] = Identity85 $p }
    if (-not $state.PSObject.Properties['known']) { $state | Add-Member -NotePropertyName known -NotePropertyValue @() }
    $state.known = @($map.Values)
    Save85 $state
}
function Save85($state) {
    $state | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $script:StatePath -Encoding utf8
}
function RequireOwnedNames85([string[]]$Names) {
    $owned = @(Import-Csv -LiteralPath 'F:\ibcmd\lab\04\databases.tsv' -Delimiter "`t" |
        Where-Object { $_.track -eq 'p85' } | ForEach-Object { $_.database })
    foreach ($name in $Names) {
        if ($name -notmatch '^ibcmd_rs_05_p85_w3_[a-z0-9_]+$' -or $name -notin $owned) {
            throw 'private cluster contains a foreign/unmanifested registration; no process may be signalled'
        }
    }
}
function Snapshot85($state, [string]$Name) {
    $log = Join-Path $script:Root ("$Name-" + (Get-Date -Format yyyyMMddHHmmssfff) + '.log')
    $raw = @(Rac85 @('infobase','summary','list',"--cluster=$($state.cluster)"))
    $raw | Set-Content -LiteralPath $log -Encoding utf8
    $names = @($raw | Select-String '^name\s*:' | ForEach-Object { ($_ -replace '^name\s*:\s*','').Trim().Trim('"') })
    RequireOwnedNames85 $names
    Rac85 @('connection','list',"--cluster=$($state.cluster)") | Add-Content -LiteralPath $log -Encoding utf8
    Rac85 @('process','list',"--cluster=$($state.cluster)") | Add-Content -LiteralPath $log -Encoding utf8
    [pscustomobject]@{ path=$log; names=$names }
}
