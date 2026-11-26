param([Parameter(Mandatory=$true)][ValidatePattern('^[a-z0-9-]+$')][string]$Label)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\lib.ps1"
$root='F:\ibcmd\lab\05\wave3\platform85'
RequireLabel85 $Label
RequireLabPaths85 @("$root\obs\$Label.pid","$root\obs\$Label.identity.json","$root\obs\$Label.log","$root\observer\IbcmdRsObserver.epf")
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class OwnedPrime85 {
 [StructLayout(LayoutKind.Sequential)] public struct Point { public int X; public int Y; }
 [DllImport("user32.dll")] static extern bool ScreenToClient(IntPtr h, ref Point p);
 [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 public static bool Click(IntPtr h, uint pid, int x, int y) {
  uint actual; GetWindowThreadProcessId(h,out actual); if(actual!=pid)return false;
  var p=new Point { X=x,Y=y }; if(!ScreenToClient(h,ref p))return false;
  var pos=new IntPtr(p.X|(p.Y<<16)); return PostMessage(h,0x201,new IntPtr(1),pos)&&PostMessage(h,0x202,IntPtr.Zero,pos);
 }
}
'@
$id=[int](Get-Content "$root\obs\$Label.pid")
$saved=Get-Content "$root\obs\$Label.identity.json" -Raw | ConvertFrom-Json
for($attempt=0;$attempt -lt 5;$attempt++) {
 $p=Get-CimInstance Win32_Process -Filter "ProcessId=$id"
 if(-not $p -or $p.ExecutablePath -ne 'C:\Program Files\1cv8\8.5.1.1150\bin\1cv8c.exe' -or $p.CreationDate.ToUniversalTime().Ticks -ne ([datetime]$saved.born).ToUniversalTime().Ticks -or $p.CommandLine -ne $saved.command){throw 'owned observer identity mismatch'}
 RequireObserverCommand85 $p.CommandLine $Label
 $cond=[System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty,$id)
 $windows=[System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,$cond)
 $clicked=$false
 foreach($w in $windows) {
  $all=$w.FindAll([System.Windows.Automation.TreeScope]::Descendants,[System.Windows.Automation.Condition]::TrueCondition)
  $security=@($all | Where-Object {$_.Current.Name -like '*Предупреждение безопасности*' -and $_.Current.Name.Contains("$root\observer\IbcmdRsObserver.epf")}).Count -eq 1
  $copied=$w.Current.Name -eq 'Информационная база была перемещена или восстановлена из резервной копии'
  if(-not $security -and -not $copied){continue}
  $buttons=@($all | Where-Object {$_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and (($security -and $_.Current.Name.Trim() -eq 'Да') -or ($copied -and $_.Current.Name -eq 'Это копия информационной базы'))})
  if($buttons.Count -ne 1){continue}
  $r=$buttons[0].Current.BoundingRectangle
  if($r.Width -le 0 -or $r.Height -le 0){throw 'invalid button rectangle'}
  if(-not [OwnedPrime85]::Click([IntPtr]$w.Current.NativeWindowHandle,[uint32]$id,[int]($r.X+$r.Width/2),[int]($r.Y+$r.Height/2))){throw 'owned dialog dispatch failed'}
  "owned dialog dispatched: security=$security copied=$copied"
  $clicked=$true; break
 }
 if(Test-Path "$root\obs\$Label.log") { 'journal present'; exit 0 }
 Start-Sleep -Seconds 2
}
'journal still absent after bounded priming'
exit 1
