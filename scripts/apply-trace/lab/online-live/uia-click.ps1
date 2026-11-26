# UI Automation probe / invoke for ONE window of a lab client (no cursor movement, no focus change if the pattern is supported).
#   powershell -NoProfile -File uia-click.ps1 -ProcessId <pid> -Hwnd 0x3842B6C [-Name '<button text>'] [-List]
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$Hwnd,
    [string]$Name = '',
    [string]$NameFile = '',
    [switch]$List
)
$proc = Get-CimInstance Win32_Process -Filter "ProcessId=$ProcessId" -ErrorAction SilentlyContinue
if (-not $proc -or $proc.CommandLine -notmatch 'ibcmd_rs_0[45]_') { throw "pid $ProcessId is not a lab client" }
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
if ($NameFile) { $Name = (Get-Content -LiteralPath $NameFile -Raw -Encoding UTF8).Trim() }
$h = [IntPtr][Convert]::ToInt64($Hwnd, 16)
$root = [System.Windows.Automation.AutomationElement]::FromHandle($h)
if ($root.Current.ProcessId -ne $ProcessId) { throw "window belongs to pid $($root.Current.ProcessId)" }
"root: name='$($root.Current.Name)' class=$($root.Current.ClassName) type=$($root.Current.ControlType.ProgrammaticName)"
$all = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
"descendants: $($all.Count)"
$i = 0
foreach ($e in $all) {
    $i++
    if ($i -gt 60) { break }
    "  [{0}] type={1} name='{2}' class={3}" -f $i, $e.Current.ControlType.ProgrammaticName, $e.Current.Name, $e.Current.ClassName
}
if ($List -or -not $Name) { return }
$cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $Name)
$el = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
if (-not $el) { throw "no element named '$Name'" }
$pattern = $null
$done = $false
if ($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
    try { $pattern.Invoke(); "invoked '$Name' (InvokePattern)"; $done = $true } catch { "InvokePattern is not implemented: $($_.Exception.GetType().Name)" }
}
if (-not $done) {
    # Focus the element inside the lab client, then post Enter to that window.
    $el.SetFocus()
    Start-Sleep -Milliseconds 200
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class KeyPost { [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l); }
'@
    [void][KeyPost]::PostMessage($h, 0x0100, [IntPtr]0x0D, [IntPtr]0x001C0001)   # WM_KEYDOWN VK_RETURN
    Start-Sleep -Milliseconds 60
    [void][KeyPost]::PostMessage($h, 0x0101, [IntPtr]0x0D, [IntPtr]([Int64]0xC01C0001 -band 0xFFFFFFFF))   # WM_KEYUP
    "focused '$Name' and posted Enter"
}
