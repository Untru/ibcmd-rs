# Exact 8.5 thin observer in the private wave3 cluster (6541), owned DB prefix only.
# Poll/lazy only. Identity-bound cleanup and explicit copy/security dialog priming.
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('start', 'stop', 'list')][string]$Action,
    [string]$Database = '',
    [string]$Label = '',
    [ValidateSet('poll', 'lazy')][string]$Mode = 'poll',
    [switch]$All,
    [ValidateSet('F:\ibcmd\lab\05\wave3\platform85')][string]$LabRoot = 'F:\ibcmd\lab\05\wave3\platform85',
    [ValidateSet('localhost:6541')][string]$Srvr = 'localhost:6541',
    [ValidateRange(5,300)][int]$TimeoutSec = 120,
    [ValidateRange(0,300)][int]$PrimeAfterSec = 30
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\lib.ps1"
$obsDir = Join-Path $LabRoot 'obs'
$epf = Join-Path $LabRoot 'observer\IbcmdRsObserver.epf'
$client = 'C:\Program Files\1cv8\8.5.1.1150\bin\1cv8c.exe'
RequireLabPaths85 @($obsDir,$epf)
if ($Label) { RequireLabel85 $Label }
if ($Action -eq 'start') {
    RequireOwnedNames85 @($Database)
    RequireFreshObserver85 $obsDir $Label
    New-Item -ItemType Directory -Force $obsDir | Out-Null
}

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class ObsWin {
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X; public int Y; }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern bool ScreenToClient(IntPtr h, ref Point p);
    public static bool ClickOwned(IntPtr h, uint pid, int x, int y) {
        uint actual; GetWindowThreadProcessId(h, out actual); if (actual != pid) return false;
        var point = new Point { X=x, Y=y }; if (!ScreenToClient(h, ref point)) return false;
        var packed = new IntPtr(point.X | (point.Y << 16));
        return PostMessage(h,0x201,new IntPtr(1),packed) && PostMessage(h,0x202,IntPtr.Zero,packed);
    }
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
    $identityPath = Join-Path $obsDir "$label.identity.json"
    if (-not (Test-Path -LiteralPath $identityPath)) { return $false }
    $identity = Get-Content -LiteralPath $identityPath -Raw | ConvertFrom-Json
    $bound = $p -and $p.ExecutablePath -eq $client -and
        $p.CreationDate.ToUniversalTime().Ticks -eq ([datetime]$identity.born).ToUniversalTime().Ticks -and
        $p.CommandLine -eq $identity.command
    if (-not $bound) { return $false }
    try { RequireObserverCommand85 $p.CommandLine $label; return $true } catch { return $false }
}

switch ($Action) {
    'list' {
        Get-ChildItem $obsDir -Filter '*.pid' -ErrorAction SilentlyContinue | ForEach-Object {
            RequireLabel85 $_.BaseName
            RequireLabPaths85 @($_.FullName)
            $id = [int](Get-Content -LiteralPath $_.FullName -Raw).Trim()
            '{0}: pid {1} {2}' -f $_.BaseName, $id, $(if (Get-Process -Id $id -ErrorAction SilentlyContinue) { 'running' } else { 'gone' })
        }
    }
    'stop' {
        $files = if ($All) { @(Get-ChildItem $obsDir -Filter '*.pid') } elseif ($Label) { @(Get-ChildItem $obsDir -Filter "$Label.pid") } else { throw 'give -Label or -All' }
        foreach ($f in $files) {
            RequireLabel85 $f.BaseName
            RequireLabPaths85 @($f.FullName,(Join-Path $obsDir "$($f.BaseName).identity.json"))
            $id = [int](Get-Content -LiteralPath $f.FullName -Raw).Trim()
            if (-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) { "$($f.BaseName): pid $id already gone"; continue }
            if (-not (Test-LabClient $id $f.BaseName)) { "$($f.BaseName): pid $id is not our client any more; left alone"; continue }
            Stop-Process -Id $id -Force
            "$($f.BaseName): stopped pid $id"
        }
    }
    'start' {
        if ($Database -notmatch '^ibcmd_rs_05_p85_w3_[a-z0-9_]+$') { throw 'the database must belong to platform85 wave3' }
        if ($Label -notmatch '^[a-z0-9-]+$') { throw 'label: [a-z0-9-]+' }
        if (-not (Test-Path -LiteralPath $epf)) { throw "build the observer first: build_observer.ps1 ($epf)" }
        $journal = Join-Path $obsDir "$Label.log"
        if (Test-Path -LiteralPath $journal) { throw "the journal exists already: $journal" }
        $args1 = "ENTERPRISE /S`"$srvr\$Database`" /N`"Администратор (обычное приложение)`" /P`"`" /Execute`"$epf`" /C`"$Label;$Mode;$obsDir\`" /DisableStartupMessages /DisableStartupDialogs /L ru /Out`"$obsDir\$Label.client-out.txt`" -NoTruncate"
        $p = Start-Process -FilePath $client -ArgumentList $args1 -PassThru -WindowStyle Hidden
        Set-Content -LiteralPath (Join-Path $obsDir "$Label.pid") -Value $p.Id -Encoding ascii
        $ownedClient = Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)"
        @{born=$ownedClient.CreationDate.ToUniversalTime().ToString('o');command=$ownedClient.CommandLine} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $obsDir "$Label.identity.json") -Encoding utf8
        $t0 = Get-Date
        $primed = $false
        while (((Get-Date) - $t0).TotalSeconds -lt $TimeoutSec) {
            if (Test-Path -LiteralPath $journal) { break }
            if ($p.HasExited) { "client exited (code $($p.ExitCode)) before its first journal line"; exit 2 }
            if (-not $primed -and ((Get-Date) - $t0).TotalSeconds -gt $PrimeAfterSec) {
                # Only the copy-confirmation modal of this owned client. Sending Enter to
                # every top-level window did not select the 8.5 dialog's explicit copy button.
                $windows = [ObsWin]::ForProcess([uint32]$p.Id)
                $titles = $windows | ForEach-Object { [ObsWin]::Title($_) } | Where-Object { $_ }
                "no journal after $PrimeAfterSec s; windows of pid $($p.Id): $($titles -join ' | ')"
                if (-not (Test-LabClient $p.Id $Label)) { throw 'client identity changed before dialog action' }
                & "$PSScriptRoot\prime_observer.ps1" -Label $Label
                $pidCondition = [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$p.Id)
                $ownedWindows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,$pidCondition)
                $modal = @($ownedWindows | Where-Object { $_.Current.Name -eq 'Информационная база была перемещена или восстановлена из резервной копии' })
                if ($modal.Count -eq 1) {
                    $buttonCondition = [System.Windows.Automation.AndCondition]::new(
                        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty,'Это копия информационной базы'),
                        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty,[System.Windows.Automation.ControlType]::Button))
                    $button = $modal[0].FindFirst([System.Windows.Automation.TreeScope]::Descendants,$buttonCondition)
                    if ($button) {
                        $r = $button.Current.BoundingRectangle
                        if ($r.Width -le 0 -or $r.Height -le 0) { throw 'copy button has no valid bounds' }
                        if (-not [ObsWin]::ClickOwned([IntPtr]$modal[0].Current.NativeWindowHandle,[uint32]$p.Id,[int]($r.X+$r.Width/2),[int]($r.Y+$r.Height/2))) { throw 'owned modal click failed' }
                        'owned copy confirmation dispatched; journal still required'
                    }
                }
                $primed = $true
            }
            Start-Sleep -Milliseconds 300
        }
        if (Test-Path -LiteralPath $journal) { "started: $Label mode=$Mode pid=$($p.Id) primed=$primed after $([int]((Get-Date) - $t0).TotalSeconds) s" } else { "NOT OPENED within $TimeoutSec s (pid $($p.Id)); stopping owned client"; if (-not $p.HasExited) { $p.Kill(); $p.WaitForExit(5000) | Out-Null }; exit 2 }
    }
}
