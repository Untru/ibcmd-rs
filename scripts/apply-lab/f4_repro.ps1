# The F-4 repro of #408 (finding F-4 of #344) and its acceptance, on lab clones of the БСП 8.3.27 corpus that carries a native
# online generation:
#   1. an online generation of our own on clone A (the module ОбсужденияСлужебныйКлиентСервер, "Telegram" = "G2MARK");
#   2. a byte-equal copy of that state is restored as clone C (a COPY_ONLY backup of A);
#   3. ROUTE 1 on C: `mssql-stage-source-objects` of the module РаботаСКлассификаторамиКлиентСервер (it gets ПроверкаIbcmdRsF4()),
#      a COPY_ONLY backup of the STAGED state, restored as the native twin D, then `mssql-activate-staged-main --mode exclusive`;
#   4. ROUTE 2 on A: `mssql-apply-source-change --mode exclusive` of the same module;
#   5. THE NATIVE TWIN: the platform's own `config apply` of the same stage on D;
#   6. what has to be true afterwards: the promotions succeed, no marker and no `_dynupdate_` row is left, the ordinary rows hold what
#      the aliases held (the native generation and ours), a NEW session sees both online changes and the new function, the native
#      exports of A, C and D are equal, and a native apply after ours says "не требуется".
# Before #408 step 2 the old exclusive command refused (step 1) or, earlier, lost the aliases; RED = exit 1.
#   pwsh -NoProfile -File f4_repro.ps1 -Exe <ibcmd-rs.exe> -Suffix <name> [-Steps base,route1stage,route2,route1,twin,exports,reapply,snap]
# Databases ibcmd_rs_04_apply_<Suffix>{a,c,d}_20260930 (a and c registered in the 8.3.27 cluster with register-ib.ps1; the step list lets
# a stopped run continue: a database that exists is not restored again). Log: logs\f4_<Suffix>.log
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9]{2,8}$')][string]$Suffix,
    [string[]]$Steps = @('base', 'route1stage', 'route2', 'route1', 'twin', 'exports', 'reapply', 'snap')
)
$ErrorActionPreference = 'Stop'
$Steps = @($Steps | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONIOENCODING = 'utf-8'
$lab = 'F:\ibcmd\lab\04\apply'
$tools = 'F:\ibcmd\lab\04\tools'
$dbA = "ibcmd_rs_04_apply_${Suffix}a_20260930"
$dbC = "ibcmd_rs_04_apply_${Suffix}c_20260930"
$dbD = "ibcmd_rs_04_apply_${Suffix}d_20260930"
$rac = 'C:\Program Files\1cv8\8.3.27.2214\bin\rac.exe'
$ras = 'localhost:2545'
$log = "$lab\logs\f4_$Suffix.log"
$G2 = 'ОбсужденияСлужебныйКлиентСервер'
$X1 = 'РаботаСКлассификаторамиКлиентСервер'
$failed = New-Object System.Collections.Generic.List[string]
function Say($text) { $line = "$(Get-Date -Format s) $text"; $line | Tee-Object -FilePath $log -Append }
function Check($name, $ok, $detail = '') {
    if ($ok) { Say "PASS  $name $detail" } else { Say "FAIL  $name $detail"; $failed.Add($name) }
}
function Scalar($db, $query) { ([string](sqlcmd -S localhost -E -C -h -1 -W -d $db -Q "SET NOCOUNT ON; $query" | Select-Object -First 1)).Trim() }
function DbExists($db) { [int](sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'$db'" | Select-Object -First 1) -eq 1 }
# a NEW session of the infobase: "telegram=<value> probe=<value>" (f4_session.ps1)
function Session($db) {
    $o = & powershell.exe -NoProfile -File "$PSScriptRoot\f4_session.ps1" -Database $db 2>&1
    (($o | ForEach-Object { "$_" }) -join ' ').Trim()
}
function Step($name) { $Steps -contains $name }
function Restore($db, $corpus, $bak, $why) {
    if (DbExists $db) { Say "$db exists already"; return }
    Say "restore $db"
    if ($corpus -eq 'bak') {
        & pwsh -NoProfile -File "$tools\restore-clone.ps1" -Corpus bak -Bak $bak -Name $db -Track apply -Purpose $why 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
    } else {
        & pwsh -NoProfile -File "$tools\restore-clone.ps1" -Corpus $corpus -Name $db -Track apply -Purpose $why 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
    }
}
function Backup($db, $file) {
    if (Test-Path $file) { Say "backup $file exists already"; return }
    sqlcmd -S localhost -E -C -b -Q "BACKUP DATABASE [$db] TO DISK = N'$file' WITH COPY_ONLY, COMPRESSION, INIT" | Select-Object -Last 1 | ForEach-Object { Say $_ }
}
function Register($db) {
    & pwsh -NoProfile -File "$tools\register-ib.ps1" register -Database $db -Platform 8.3.27 -Track apply 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
}
function InfobaseId($db) {
    $current = $null
    foreach ($line in & $rac $ras infobase summary list "--cluster=$cluster") {
        if ($line -match '^infobase\s*:\s*(\S+)') { $current = $Matches[1] } elseif ($line -match '^name\s*:\s*(\S+)' -and $Matches[1] -eq $db) { return $current }
    }
    throw "$db is not registered"
}
# The exclusive mode is refused while the cluster lists a client connection (#409 F-3). The RAS connections are the tool's own; a
# session of ours (the COM check) leaves a "JobScheduler" connection of the working process for a moment, so wait until it is gone.
function WaitQuiet($db, [int]$seconds = 120) {
    $ib = InfobaseId $db
    $deadline = (Get-Date).AddSeconds($seconds)
    while ($true) {
        $apps = @(& $rac $ras connection list "--cluster=$cluster" "--infobase=$ib" '--infobase-user=Администратор' | Select-String '^application\s*:' | ForEach-Object { ($_ -replace '^application\s*:\s*', '').Trim().Trim('"') } | Where-Object { $_ -ne 'RAS' })
        if (-not $apps.Count) { Say "the cluster lists no client connection of $db"; return }
        if ((Get-Date) -gt $deadline) { Say "still connected on ${db}: $($apps -join ', ')"; return }
        Start-Sleep -Seconds 3
    }
}
function Native($db, $tag, $command) {
    $r = & pwsh -NoProfile -File "$PSScriptRoot\native_cmd.ps1" -Database $db -Tag $tag -Command $command -TimeoutSec 3000 -LockTimeoutMin 240 2>&1
    $r | ForEach-Object { "$_" }
}
function Run($db, $tag, [string[]]$argv) {
    New-Item -ItemType Directory -Force "$lab\out" | Out-Null
    $out = & $Exe @argv 2>"$lab\out\f4_$Suffix.$tag.err"
    $code = $LASTEXITCODE
    $out | Set-Content "$lab\out\f4_$Suffix.$tag.json" -Encoding UTF8
    $code
}
function Refusal($tag) {
    $stderr = ((Get-Content "$lab\out\f4_$Suffix.$tag.err" -Raw -ErrorAction SilentlyContinue) -replace '\s+', ' ')
    if ($stderr.Length -gt 300) { $stderr = $stderr.Substring(0, 300) }
    $stderr
}
function Common($db) {
    @('--platform-profile', 'platform-8.3.27.2214', '--server', 'localhost', '--database', $db, '--sqlcmd-trust-cert')
}
function Cluster($db) {
    @('--rac', $rac, '--ras-endpoint', $ras, '--cluster-id', $cluster, '--infobase-id', (InfobaseId $db), '--infobase-user', 'Администратор', '--allow-non-lab')
}
function AliasHashes($db) {
    $h = @{}
    sqlcmd -S localhost -E -C -h -1 -W -s"|" -d $db -Q "SET NOCOUNT ON; SELECT FileName, CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2) FROM dbo.Config WHERE FileName LIKE N'%[_]dynupdate[_]%' AND FileName NOT LIKE N'versions[_]dynupdate%'" |
        ForEach-Object { $p = $_ -split '\|'; if ($p.Count -eq 2) { $h[$p[0].Trim()] = $p[1].Trim() } }
    $h
}
# what has to be true on a database after an exclusive promotion that folded the online generations
function After($db, $label, $before, $staged) {
    Check "$label no marker is left" (([int](Scalar $db "SELECT (SELECT COUNT(*) FROM dbo.Config WHERE FileName = N'DynamicallyUpdated') + (SELECT COUNT(*) FROM dbo.Params WHERE FileName = N'DynamicallyUpdated')")) -eq 0)
    Check "$label no _dynupdate_ row is left" (([int](Scalar $db "SELECT COUNT(*) FROM dbo.Config WHERE FileName LIKE N'%[_]dynupdate[_]%'")) -eq 0)
    Check "$label ConfigSave is empty" (([int](Scalar $db "SELECT COUNT(*) FROM dbo.ConfigSave")) -eq 0)
    $lost = 0; $overridden = 0
    foreach ($name in $before.Keys) {
        $target = $name -replace '_dynupdate_[0-9a-fA-F-]{36}', ''
        if ($staged -contains $target) { $overridden++; continue }   # the promotion's own stage replaced the row
        $now = Scalar $db "SELECT CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2) FROM dbo.Config WHERE FileName = N'$target' AND PartNo = 0"
        if ($now -ne $before[$name]) { $lost++; Say "  $target differs from the alias it should have become" }
    }
    Check "$label the ordinary rows hold what the aliases held" ($lost -eq 0) "($lost of $($before.Count) differ; $overridden replaced by the stage itself)"
    $seen = Session $db
    Check "$label a new session still sees the online change" ($seen -match 'telegram=G2MARK') "($seen)"
    Check "$label a new session sees the promoted change" ($seen -match 'probe=F4') "($seen)"
}

New-Item -ItemType Directory -Force "$lab\out\export", "$lab\bak" | Out-Null
$cluster = ((& $rac $ras cluster list) | Select-String '^cluster\s*:' | Select-Object -First 1) -replace '^cluster\s*:\s*', ''
$baseBak = "$lab\bak\f4_${Suffix}_base.bak"
$stagedBak = "$lab\bak\f4_${Suffix}_staged.bak"
$before = @{}
$stagedNames = @()

if (Step 'base') {
    Restore $dbA 'bsp8327' '' "#408 F-4 repro A"
    Register $dbA
    & python "$PSScriptRoot\f4_trees.py" "$lab\src" | ForEach-Object { Say $_ }
    $markers = [int](Scalar $dbA "SELECT COUNT(*) FROM dbo.Config WHERE FileName = N'DynamicallyUpdated'")
    $aliases0 = [int](Scalar $dbA "SELECT COUNT(*) FROM dbo.Config WHERE FileName LIKE N'%[_]dynupdate[_]%'")
    Check 'the base carries a native online generation' ($markers -eq 1 -and $aliases0 -gt 0) "(marker rows $markers, alias rows $aliases0)"
    # 1. our online generation
    $code = Run $dbA 'online' (@('mssql-apply-source-change') + (Common $dbA) + @('--source-root', "$lab\src\g2", '--path', "CommonModules/$G2/Ext/Module.bsl", '--mode', 'online',
            '--script-output', "$lab\out\f4_$Suffix.online.sql", '--recovery-output', "$lab\out\f4_$Suffix.online.recovery.json") + (Cluster $dbA))
    Check 'the online generation is applied' ($code -eq 0) "(exit $code)"
    $seen = Session $dbA
    Check 'a new session sees the online change' ($seen -match 'telegram=G2MARK') "($seen)"
    # 2. a byte-equal copy of this state
    Backup $dbA $baseBak
    Restore $dbC 'bak' $baseBak "#408 F-4 repro C (route 1)"
    Register $dbC
}

if (Step 'route1stage') {
    # ROUTE 1, the first half: the stage
    $code = Run $dbC 'stage' @('mssql-stage-source-objects', '--server', 'localhost', '--database', $dbC, '--source-root', "$lab\src\x1", '--replace-config-save', '--allow-non-lab',
        '--batch-size', '1', '--path-prefix', "CommonModules/$X1", '--per-row')
    Check 'route 1: the module is staged' ($code -eq 0) "(exit $code; $(Refusal 'stage'))"
    $stagedNames = @(sqlcmd -S localhost -E -C -h -1 -W -d $dbC -Q "SET NOCOUNT ON; SELECT FileName FROM dbo.ConfigSave" | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
    Say "staged rows on C: $($stagedNames.Count)"
    Backup $dbC $stagedBak
    Restore $dbD 'bak' $stagedBak "#408 F-4 repro D (native twin of the stage)"
}
if (-not $stagedNames.Count -and (DbExists $dbD)) {
    # a continued run: the names of the rows the stage replaces, from the twin (it is still staged until the native apply)
    $stagedNames = @(sqlcmd -S localhost -E -C -h -1 -W -d $dbD -Q "SET NOCOUNT ON; SELECT FileName FROM dbo.ConfigSave" | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
}

if (Step 'route2') {
    # ROUTE 2 on A
    $before = AliasHashes $dbA
    Say "alias rows before the promotion on A: $($before.Count)"
    WaitQuiet $dbA
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $code = Run $dbA 'exclusive' (@('mssql-apply-source-change') + (Common $dbA) + @('--source-root', "$lab\src\x1", '--path', "CommonModules/$X1/Ext/Module.bsl", '--mode', 'exclusive',
            '--script-output', "$lab\out\f4_$Suffix.exclusive.sql", '--recovery-output', "$lab\out\f4_$Suffix.exclusive.recovery.json") + (Cluster $dbA))
    Check 'route 2: the exclusive promotion (apply-source-change) succeeds' ($code -eq 0) "(exit $code, $([math]::Round($sw.Elapsed.TotalSeconds,1)) s; $(Refusal 'exclusive'))"
    if ($code -ne 0) { Say "RED: $($failed.Count) check(s) failed: $($failed -join '; ')"; exit 1 }
    After $dbA 'route 2:' $before $stagedNames
}

if (Step 'route1') {
    # ROUTE 1, the second half: the activation of the stage
    $before1 = AliasHashes $dbC
    Say "alias rows before the promotion on C: $($before1.Count)"
    WaitQuiet $dbC
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $code = Run $dbC 'activate' (@('mssql-activate-staged-main') + (Common $dbC) + @('--mode', 'exclusive',
            '--script-output', "$lab\out\f4_$Suffix.activate.sql", '--recovery-output', "$lab\out\f4_$Suffix.activate.recovery.json") + (Cluster $dbC))
    Check 'route 1: the exclusive promotion (activate-staged-main) succeeds' ($code -eq 0) "(exit $code, $([math]::Round($sw.Elapsed.TotalSeconds,1)) s; $(Refusal 'activate'))"
    if ($code -ne 0) { Say "RED: $($failed.Count) check(s) failed: $($failed -join '; ')"; exit 1 }
    After $dbC 'route 1:' $before1 $stagedNames
}

if (Step 'twin') {
    # THE NATIVE TWIN: the platform's apply of the same stage
    $r = Native $dbD "f4_${Suffix}_twin_apply" 'infobase config apply --force --dynamic=disable'
    Check 'the native apply of the same stage succeeds' (($r | Select-Object -First 1) -match '^exit=0') "($($r | Select-Object -First 1))"
    Check 'no marker and no _dynupdate_ row is left on the twin' ((([int](Scalar $dbD "SELECT COUNT(*) FROM dbo.Config WHERE FileName = N'DynamicallyUpdated' OR FileName LIKE N'%[_]dynupdate[_]%'")) -eq 0))
}

if (Step 'exports') {
    foreach ($x in @(@('a', $dbA), @('c', $dbC), @('d', $dbD))) {
        $dir = "$lab\out\export\f4_${Suffix}_$($x[0])"
        if (-not (Test-Path $dir)) {
            $r = Native $x[1] "f4_${Suffix}_export_$($x[0])" "infobase config export $($dir -replace '\\','/')"
            Say "native export of $($x[1]): $($r | Select-Object -First 1)"
        }
        $g = Get-Content "$dir\CommonModules\$G2\Ext\Module.bsl" -Raw -Encoding UTF8 -ErrorAction SilentlyContinue
        $p = Get-Content "$dir\CommonModules\$X1\Ext\Module.bsl" -Raw -Encoding UTF8 -ErrorAction SilentlyContinue
        Check "the native export of $($x[0]) holds the online change" ($g -match 'G2MARK')
        Check "the native export of $($x[0]) holds the promoted change" ($p -match 'ПроверкаIbcmdRsF4')
    }
    foreach ($pair in @(@('a', 'd'), @('c', 'd'), @('a', 'c'))) {
        $left = "$lab\out\export\f4_${Suffix}_$($pair[0])"; $right = "$lab\out\export\f4_${Suffix}_$($pair[1])"
        $out = "$lab\out\export\f4_${Suffix}_$($pair[0])$($pair[1]).diff.json"
        & $Exe source-diff -o $out $left $right 2>&1 | Out-Null
        $d = Get-Content $out -Raw -Encoding UTF8 | ConvertFrom-Json
        $diff = @($d.differences | Where-Object { $_.status -ne 'unchanged' })
        $real = @($diff | Where-Object { $_.path -ne 'ConfigDumpInfo.xml' })
        Say ("export $($pair[0]) against $($pair[1]): left_only {0}, right_only {1}, different {2} (ConfigDumpInfo.xml aside: {3} {4}), unchanged {5}" -f $d.summary.left_only, $d.summary.right_only, $d.summary.different, $real.Count, (($real | Select-Object -First 5 | ForEach-Object { $_.path }) -join ', '), $d.summary.unchanged)
        Check "the native exports of $($pair[0]) and $($pair[1]) are equal" ($real.Count -eq 0)
        if ($diff.Count -gt $real.Count) {
            # ConfigDumpInfo.xml carries the ids of the generation of each object; two independent stages of the same change draw
            # their own (the stager writes a new random id per changed object), so route 2 (its own stage) may differ from the twin
            # (route 1's stage) in exactly the lines of the changed module and nowhere else.
            $la = [IO.File]::ReadAllLines("$left\ConfigDumpInfo.xml"); $lb = [IO.File]::ReadAllLines("$right\ConfigDumpInfo.xml")
            $xml = Get-Content "$right\CommonModules\$X1.xml" -Raw -Encoding UTF8
            $ids = @([regex]::Matches($xml, '<CommonModule uuid="([0-9a-f-]{36})"') | ForEach-Object { $_.Groups[1].Value })
            $other = 0; $module = 0
            if ($la.Count -ne $lb.Count) { $other = [Math]::Abs($la.Count - $lb.Count) }
            for ($i = 0; $i -lt [Math]::Min($la.Count, $lb.Count); $i++) {
                if ($la[$i] -ne $lb[$i]) {
                    if (@($ids | Where-Object { $la[$i].Contains($_) }).Count) { $module++ } else { $other++ }
                }
            }
            Check "ConfigDumpInfo.xml of $($pair[0]) and $($pair[1]) differs only in the generation ids of the changed module" ($other -eq 0) "($module line(s) of the module, $other elsewhere)"
        }
    }
}

if (Step 'reapply') {
    # a native apply after ours: nothing to do
    foreach ($x in @(@('a', $dbA), @('c', $dbC))) {
        $r = Native $x[1] "f4_${Suffix}_reapply_$($x[0])" 'infobase config apply --force --dynamic=disable'
        Check "a native apply after ours on $($x[0]) says it is not needed" (($r -join ' ') -match 'не требуется') "($(($r | Select-Object -Skip 1 -First 3) -join ' | '))"
    }
}

if (Step 'snap') {
    # the rows of the database, ours against the platform's (information; the exports above are the criterion)
    $env:DDL_LAB = "$lab\ddlkit"
    $kit = (Resolve-Path (Join-Path $PSScriptRoot '..\restructure-lab')).Path
    Push-Location $kit
    python snapshot.py $dbC own_after 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
    python snapshot.py $dbA own_after 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
    python snapshot.py $dbD nat_after 2>&1 | Select-Object -Last 1 | ForEach-Object { Say $_ }
    foreach ($ours in @($dbC, $dbA)) {
        Say "snapdiff native ($dbD) -> ours ($ours):"
        python snapdiff.py $dbD nat_after $ours own_after --max 8 2>&1 | Where-Object { $_ -match '^(==|added|removed|changed|  |Config|Params|ConfigSave|\(|DBSchema)' -and $_ -notmatch '^    [mM+-] ' } | ForEach-Object { Say $_ }
    }
    Pop-Location
}

if ($failed.Count) { Say "RED: $($failed.Count) check(s) failed: $($failed -join '; ')"; exit 1 }
Say 'GREEN'
