# Observer / writer clients (1cv8c.exe thin clients running IbcmdRsObserver.epf) on a lab clone in the WORKER LAB CLUSTER (Srvr localhost:5541).
#   pwsh -NoProfile -File obs.ps1 start -Database <db> -Label <label> [-Mode poll|writer|txn|lazy] [-TimeoutSec 120]
#   pwsh -NoProfile -File obs.ps1 stop  -Label <label> | -All
#   pwsh -NoProfile -File obs.ps1 list
# Every client writes a journal F:\ibcmd\lab\05\live\obs\<label>.log (the epf, see observer\src ... Module.bsl) and a pid file.
# A client that shows no journal line within -PrimeAfterSec is looking at the "database moved or restored" dialog of a freshly restored
# clone (БСП asks once per clone whether it was copied or moved): the script answers it -- Enter on the modal window of THIS client,
# which the dialog takes as its default answer ("copied", the title then says [КОПИЯ]) -- and waits again. The answer is stored in the
# clone, so it is asked once (this is the priming). Nothing else on the desktop is touched: windows are looked up by process id.
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('start', 'stop', 'list')][string]$Action,
    [string]$Database = '',
    [string]$Label = '',
    [ValidateSet('poll', 'writer', 'txn', 'lazy')][string]$Mode = 'poll',
    [switch]$All,
    [string]$LabRoot = 'F:\ibcmd\lab\05\wave1\live',
    [ValidatePattern('^localhost:(2541|5541)$')][string]$Srvr = 'localhost:5541',
    [int]$TimeoutSec = 120,
    [int]$PrimeAfterSec = 30
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$obsDir = Join-Path $LabRoot 'obs'
$epf = Join-Path $LabRoot 'observer\IbcmdRsObserver.epf'
$client = 'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8c.exe'
New-Item -ItemType Directory -Force $obsDir | Out-Null

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class ObsWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    public static List<IntPtr> ForProcess(uint pid) {
        var list = new List<IntPtr>();
        EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid && IsWindowVisible(h)) list.Add(h); return true; }, IntPtr.Zero);
        return list;
    }
    public static string Title(IntPtr h) { var sb = new StringBuilder(512); GetWindowText(h, sb, 512); return sb.ToString(); }
}
'@

function Test-LabClient([int]$processId, [string]$label) {
    $p = Get-CimInstance Win32_Process -Filter "ProcessId=$processId" -ErrorAction SilentlyContinue
    $p -and $p.CommandLine -match 'ibcmd_rs_0[45]_' -and $p.CommandLine -match [regex]::Escape($label)
}

switch ($Action) {
    'list' {
        Get-ChildItem $obsDir -Filter '*.pid' -ErrorAction SilentlyContinue | ForEach-Object {
            $id = [int](Get-Content -LiteralPath $_.FullName -Raw).Trim()
            '{0}: pid {1} {2}' -f $_.BaseName, $id, $(if (Get-Process -Id $id -ErrorAction SilentlyContinue) { 'running' } else { 'gone' })
        }
    }
    'stop' {
        $files = if ($All) { @(Get-ChildItem $obsDir -Filter '*.pid') } elseif ($Label) { @(Get-ChildItem $obsDir -Filter "$Label.pid") } else { throw 'give -Label or -All' }
        foreach ($f in $files) {
            $id = [int](Get-Content -LiteralPath $f.FullName -Raw).Trim()
            if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) { "$($f.BaseName): pid $id already gone"; continue }
            if (-not (Test-LabClient $id $f.BaseName)) { "$($f.BaseName): pid $id is not our client any more; left alone"; continue }
            Stop-Process -Id $id -Force
            "$($f.BaseName): stopped pid $id"
        }
    }
    'start' {
        if ($Database -notmatch '^ibcmd_rs_05_[a-z0-9_]+$') { throw 'the database of a worker lab run is ibcmd_rs_05_*' }
        if ($Label -notmatch '^[a-z0-9-]+$') { throw 'label: [a-z0-9-]+' }
        if (-not (Test-Path -LiteralPath $epf)) { throw "build the observer first: build_observer.ps1 ($epf)" }
        $journal = Join-Path $obsDir "$Label.log"
        if (Test-Path -LiteralPath $journal) { throw "the journal exists already: $journal" }
        $args1 = "ENTERPRISE /S`"$srvr\$Database`" /N`"Администратор`" /P`"`" /Execute`"$epf`" /C`"$Label;$Mode;$obsDir\`" /DisableStartupMessages /DisableStartupDialogs /L ru /Out`"$obsDir\$Label.client-out.txt`" -NoTruncate"
        $p = Start-Process -FilePath $client -ArgumentList $args1 -PassThru -WindowStyle Hidden
        Set-Content -LiteralPath (Join-Path $obsDir "$Label.pid") -Value $p.Id -Encoding ascii
        $t0 = Get-Date
        $primed = $false
        while (((Get-Date) - $t0).TotalSeconds -lt $TimeoutSec) {
            if (Test-Path -LiteralPath $journal) { break }
            if ($p.HasExited) { "client exited (code $($p.ExitCode)) before its first journal line"; exit 2 }
            if (-not $primed -and ((Get-Date) - $t0).TotalSeconds -gt $PrimeAfterSec) {
                # the modal window(s) of this client: answer with Enter (the default button)
                $windows = [ObsWin]::ForProcess([uint32]$p.Id)
                $titles = $windows | ForEach-Object { [ObsWin]::Title($_) } | Where-Object { $_ }
                "no journal after $PrimeAfterSec s; windows of pid $($p.Id): $($titles -join ' | ')"
                foreach ($h in $windows) {
                    [void][ObsWin]::PostMessage($h, 0x0100, [IntPtr]0x0D, [IntPtr]0x001C0001)
                    Start-Sleep -Milliseconds 60
                    [void][ObsWin]::PostMessage($h, 0x0101, [IntPtr]0x0D, [IntPtr]([Int64]0xC01C0001 -band 0xFFFFFFFF))
                }
                $primed = $true
            }
            Start-Sleep -Milliseconds 300
        }
        if (Test-Path -LiteralPath $journal) { "started: $Label mode=$Mode pid=$($p.Id) primed=$primed after $([int]((Get-Date) - $t0).TotalSeconds) s" } else { "NOT OPENED within $TimeoutSec s (pid $($p.Id)); stopping owned client"; if (-not $p.HasExited) { $p.Kill(); $p.WaitForExit(5000) | Out-Null }; exit 2 }
    }
}
