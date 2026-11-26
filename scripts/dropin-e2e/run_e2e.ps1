# End-to-end check of the user-facing 0.4 flow on БСП 8.3.27 clones (issue #343).
#
# OURS: a fresh clone of the corpus; our exe (copied as ibcmd.exe, run with no sqlcmd/bcp folder on PATH, as in the 0.3
# verification scripts/empty-load/run_dropin_import.ps1 -NoSqlTools) does `ibcmd infobase config import <edited tree>`
# and `ibcmd infobase config apply`; the platform's `config export` writes the result out; `source-diff` compares it with the
# edited tree; ConfigDumpInfo.xml is compared with `configVersion` blanked (the platform stamps new generation ids).
# NATIVE: a second clone of the same corpus; the platform's own `config import` of the same tree and `config apply`
# (each native write under the lab's `native` lock, one command per hold), optionally its export and the same diff.
#
# Every step is timed (the wait for the lock is not counted); streams are kept capped at 1 MB; runs.tsv has one line per step.
#
# usage: pwsh -NoProfile -File run_e2e.ps1 -Exe <ibcmd-rs.exe> -Tree <edited tree> -Tag e1 [-Side ours|native|both] [-NativeExport]
#        [-Lab <folder>] [-KeepExports]
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Tree,
    [string]$Tag = 'e1',
    [ValidateSet('ours', 'native', 'both')][string]$Side = 'both',
    [string]$Lab = 'F:\ibcmd\lab\04\dropin-apply\e2e',
    [switch]$NativeExport,
    [switch]$KeepExports,
    [string]$Ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe',
    [string]$LockScript = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1',
    [string]$RestoreScript = 'F:\ibcmd\lab\04\tools\restore-clone.ps1',
    [string]$LockTrack = 'rcheck',
    [string]$DbPrefix = 'ibcmd_rs_04_rcheck_e2e',
    [int]$StepTimeoutMin = 90
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$run = Join-Path $Lab $Tag
New-Item -ItemType Directory -Force -Path $run, (Join-Path $Lab 'bin') | Out-Null
$runsTsv = Join-Path $run 'runs.tsv'
$user = 'Администратор'
$results = [ordered]@{}

# the platform's tools loop on a prompt when stdin is closed and the user is missing: keep 1 MB of each stream and count the rest
if (-not ('CapCopy' -as [type])) {
    Add-Type -TypeDefinition @'
public static class CapCopy {
    public static System.Threading.Tasks.Task<long> Run(System.IO.Stream src, System.IO.Stream dst, long cap) {
        return System.Threading.Tasks.Task.Run(() => {
            var buf = new byte[65536]; long total = 0; int n;
            while ((n = src.Read(buf, 0, buf.Length)) > 0) {
                long room = cap - dst.Length;
                if (room > 0) dst.Write(buf, 0, (int)System.Math.Min(room, (long)n));
                total += n;
            }
            return total;
        });
    }
}
'@
}

# our exe copied as ibcmd.exe (the drop-in is the same program under the platform's name)
$ours = Join-Path $Lab 'bin\ibcmd.exe'
Copy-Item -LiteralPath $Exe -Destination $ours -Force

function Get-PathWithoutSqlTools {
    ($env:PATH -split ';' | Where-Object {
            $_ -and -not (Test-Path -LiteralPath (Join-Path $_ 'sqlcmd.exe')) -and -not (Test-Path -LiteralPath (Join-Path $_ 'bcp.exe'))
        }) -join ';'
}

# One command: its streams, exit code and seconds go to <run>\<name>\ and runs.tsv. -Lock holds the native lock for this one command.
function Invoke-Step {
    param([string]$Name, [string]$Program, [string[]]$Arguments, [switch]$Lock, [switch]$NoSqlTools, [switch]$AllowFailure, [string]$DataDir = '')
    $dir = Join-Path $run $Name
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    if ($DataDir) { New-Item -ItemType Directory -Force -Path $DataDir | Out-Null }
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $Program
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    foreach ($a in $Arguments) { $psi.ArgumentList.Add($a) }
    if ($NoSqlTools) { $psi.Environment['PATH'] = Get-PathWithoutSqlTools }
    if ($Lock) {
        & pwsh -NoProfile -File $LockScript acquire $LockTrack -Name native -TimeoutMin 180 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "the native lock was not granted for $Name" }
    }
    $load = try { (Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average).Average } catch { -1 }
    $code = $null; $seconds = 0.0; $outBytes = [byte[]]@(); $errBytes = [byte[]]@(); $outTotal = 0; $errTotal = 0
    try {
        $started = Get-Date
        $p = [System.Diagnostics.Process]::Start($psi)
        $p.StandardInput.Close()
        $so = [System.IO.MemoryStream]::new(); $se = [System.IO.MemoryStream]::new()
        $t1 = [CapCopy]::Run($p.StandardOutput.BaseStream, $so, 1048576)
        $t2 = [CapCopy]::Run($p.StandardError.BaseStream, $se, 1048576)
        if ($p.WaitForExit($StepTimeoutMin * 60 * 1000)) { $code = $p.ExitCode } else { try { $p.Kill($true) } catch {}; $code = 'TIMEOUT' }
        [void][System.Threading.Tasks.Task]::WaitAll(@($t1, $t2), 30000)
        $seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
        $outBytes = $so.ToArray(); $errBytes = $se.ToArray()
        if ($t1.IsCompleted) { $outTotal = $t1.Result }
        if ($t2.IsCompleted) { $errTotal = $t2.Result }
    } finally {
        if ($Lock) { & pwsh -NoProfile -File $LockScript release $LockTrack -Name native | Out-Null }
        if ($DataDir) { Remove-Item -LiteralPath $DataDir -Recurse -Force -ErrorAction SilentlyContinue }
    }
    [IO.File]::WriteAllText((Join-Path $dir 'stdout.txt'), [Text.Encoding]::UTF8.GetString($outBytes), [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $dir 'stderr.txt'), [Text.Encoding]::UTF8.GetString($errBytes), [Text.UTF8Encoding]::new($false))
    Set-Content -LiteralPath (Join-Path $dir 'cmd.txt') -Encoding utf8 -Value ((@($Program) + $Arguments) -join ' ')
    $line = "{0}`t{1}`t{2}`t{3}`t{4}`t{5}`tcpu {6}%" -f (Get-Date -Format s), $Tag, $Name, $seconds, $code, "stdout $outTotal B, stderr $errTotal B", $load
    Add-Content -LiteralPath $runsTsv -Value $line -Encoding utf8
    Write-Host $line
    $results[$Name] = [ordered]@{ seconds = $seconds; exit = "$code"; cpu_percent = $load }
    if ($code -ne 0 -and -not $AllowFailure) { throw "$Name failed (exit $code), see $dir" }
    return $code
}

function New-Clone([string]$Database, [string]$Purpose) {
    $t = [Diagnostics.Stopwatch]::StartNew()
    & pwsh -NoProfile -File $RestoreScript -Corpus bsp8327 -Name $Database -Track $LockTrack -Purpose $Purpose | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "restore of $Database failed" }
    Add-Content -LiteralPath $runsTsv -Encoding utf8 -Value ("{0}`t{1}`trestore {2}`t{3}`t0" -f (Get-Date -Format s), $Tag, $Database, [math]::Round($t.Elapsed.TotalSeconds, 1))
}

# ConfigDumpInfo.xml of two trees, `configVersion` blanked: equal?
function Compare-DumpInfo([string]$Left, [string]$Right) {
    $a = [IO.File]::ReadAllText((Join-Path $Left 'ConfigDumpInfo.xml'))
    $b = [IO.File]::ReadAllText((Join-Path $Right 'ConfigDumpInfo.xml'))
    $blank = { param($t) [regex]::Replace($t, 'configVersion="[^"]*"', 'configVersion=""') }
    $x = & $blank $a; $y = & $blank $b
    if ($x -eq $y) { return 'identical with configVersion blanked' }
    $xl = $x -split "`n"; $yl = $y -split "`n"
    for ($i = 0; $i -lt [math]::Min($xl.Count, $yl.Count); $i++) {
        if ($xl[$i] -ne $yl[$i]) { return "differs at line $($i + 1): [$($xl[$i].Trim())] against [$($yl[$i].Trim())]" }
    }
    return "differs in length ($($xl.Count) against $($yl.Count) lines)"
}

function Test-Export([string]$Name, [string]$Database, [string]$Out) {
    $data = Join-Path $run "ibdata_$Name"
    $na = @('infobase', 'config', 'export', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$data", "--user=$user", '--threads=8', $Out)
    [void](Invoke-Step -Name "${Name}_native_export" -Program $Ibcmd -Arguments $na -DataDir $data)
    $diff = Join-Path $run "${Name}_source_diff.json"
    [void](Invoke-Step -Name "${Name}_source_diff" -Program $ours -Arguments @('source-diff', '-o', $diff, $Tree, $Out) -AllowFailure)
    $info = Compare-DumpInfo $Tree $Out
    Add-Content -LiteralPath $runsTsv -Encoding utf8 -Value ("{0}`t{1}`t{2} ConfigDumpInfo.xml`t{3}" -f (Get-Date -Format s), $Tag, $Name, $info)
    Write-Host "$Name ConfigDumpInfo.xml: $info"
    $results["${Name}_dump_info"] = $info
    if (Test-Path -LiteralPath $diff) {
        $report = Get-Content -LiteralPath $diff -Raw -Encoding UTF8 | ConvertFrom-Json
        $others = @($report.differences | Where-Object { $_.status -ne 'unchanged' -and $_.path -ne 'ConfigDumpInfo.xml' } | ForEach-Object { "$($_.status) $($_.path)" })
        $results["${Name}_source_diff_summary"] = [ordered]@{
            left_only = $report.summary.left_only; right_only = $report.summary.right_only
            different = $report.summary.different; unchanged = $report.summary.unchanged
            different_other_than_config_dump_info = $others
        }
        Write-Host ("{0} source-diff: {1} unchanged, {2} different, {3} left only, {4} right only; other than ConfigDumpInfo.xml: {5}" -f $Name,
            $report.summary.unchanged, $report.summary.different, $report.summary.left_only, $report.summary.right_only, $others.Count)
    }
    if (-not $KeepExports) { Remove-Item -LiteralPath $Out -Recurse -Force -ErrorAction SilentlyContinue }
}

$stamp = Get-Date -Format 'yyyyMMdd'
if ($Side -in 'ours', 'both') {
    $db = "${DbPrefix}_${Tag}_ours"
    New-Clone $db "0.4 end-to-end, ours: import + apply + native export ($Tag)"
    $data = Join-Path $run 'ibdata_ours'
    $na = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data", "--user=$user")
    [void](Invoke-Step -Name 'ours_import' -Program $ours -NoSqlTools -DataDir $data -Arguments (@('infobase', 'config', 'import') + $na + @("--report=$(Join-Path $run 'ours_import.json')", $Tree)))
    [void](Invoke-Step -Name 'ours_apply' -Program $ours -NoSqlTools -DataDir $data -AllowFailure -Arguments (@('infobase', 'config', 'apply') + $na + @('--force', '--dynamic=disable', "--report=$(Join-Path $run 'ours_apply.json')")))
    if ($results['ours_apply'].exit -eq '0') { Test-Export 'ours' $db (Join-Path $run 'export_ours') }
}
if ($Side -in 'native', 'both') {
    $db = "${DbPrefix}_${Tag}_nat"
    New-Clone $db "0.4 end-to-end, native: import + apply ($Tag)"
    $data = Join-Path $run 'ibdata_native'
    $na = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", "--data=$data", "--user=$user")
    [void](Invoke-Step -Name 'native_import' -Program $Ibcmd -Lock -DataDir $data -AllowFailure -Arguments (@('infobase', 'config', 'import') + $na + @($Tree)))
    if ($results['native_import'].exit -eq '0') {
        [void](Invoke-Step -Name 'native_apply' -Program $Ibcmd -Lock -DataDir $data -AllowFailure -Arguments (@('infobase', 'config', 'apply') + $na + @('--force', '--dynamic=disable')))
        if ($NativeExport -and $results['native_apply'].exit -eq '0') { Test-Export 'native' $db (Join-Path $run 'export_native') }
    }
}
$results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $run 'summary.json') -Encoding utf8
Write-Host "done: $run"
