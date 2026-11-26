# Lists the top-level windows of ONE process (title, size) and saves a PNG of each (PrintWindow renders the window itself,
# not the screen: nothing else on the desktop is captured). For diagnosing what a lab 1C client shows.
#   powershell -NoProfile -File winshot.ps1 -ProcessId <pid> -OutDir <dir>
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$OutDir
)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class WinEnum {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public static List<IntPtr> ForProcess(uint pid) {
        var list = new List<IntPtr>();
        EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid && IsWindowVisible(h)) list.Add(h); return true; }, IntPtr.Zero);
        return list;
    }
}
'@
[void][WinEnum]::SetProcessDPIAware()
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$i = 0
foreach ($h in [WinEnum]::ForProcess([uint32]$ProcessId)) {
    $sb = New-Object System.Text.StringBuilder 512
    [void][WinEnum]::GetWindowText($h, $sb, 512)
    $r = New-Object WinEnum+RECT
    [void][WinEnum]::GetWindowRect($h, [ref]$r)
    $w = $r.Right - $r.Left; $hgt = $r.Bottom - $r.Top
    "window 0x{0:X} title='{1}' size={2}x{3}" -f $h.ToInt64(), $sb.ToString(), $w, $hgt
    if ($w -gt 0 -and $hgt -gt 0 -and $w -lt 6000 -and $hgt -lt 4000) {
        $bmp = New-Object System.Drawing.Bitmap $w, $hgt
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $hdc = $g.GetHdc()
        [void][WinEnum]::PrintWindow($h, $hdc, 2)
        $g.ReleaseHdc($hdc)
        $file = Join-Path $OutDir ("pid{0}-w{1}.png" -f $ProcessId, $i)
        $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bmp.Dispose()
        "  saved $file"
    }
    $i++
}
if ($i -eq 0) { "no visible top-level windows for pid $ProcessId" }
