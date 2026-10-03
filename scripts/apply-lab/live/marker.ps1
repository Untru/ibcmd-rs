# A NEW external-connection (COM) session of a lab infobase in the worker lab cluster reads the marker of the live-lab runs:
# the value under "Telegram" of ОбсужденияСлужебныйКлиентСервер.ТипыВнешнихСистем() (live_trees.py writes "LIVE-<tag>"), as the server side of
# that session sees it. Windows PowerShell 5.1 (powershell.exe): the COM connector is 64-bit.
#   powershell -NoProfile -File marker.ps1 -Database <db>
# Prints one line: "marker=<value>" or "marker=error:<text>" (exit 3 for a licence text).
param([Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_05_')][string]$Database, [string]$Srvr = 'localhost:5541')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$flags = [System.Reflection.BindingFlags]
function Fail($e) { $i = $e.Exception; while ($i.InnerException) { $i = $i.InnerException }; return 'error:' + $i.Message.Trim() }
try {
    $connector = New-Object -ComObject V83.COMConnector
    $connection = $connector.Connect("Srvr=`"$Srvr`";Ref=`"$Database`";Usr=`"Администратор`";Pwd=`"`"")
} catch {
    $text = Fail $_
    "marker=$text"
    if ($text -match '(?i)лиценз|licen[cs]e|ключ|HASP') { exit 3 }
    exit 2
}
try {
    try {
        $module = [System.__ComObject].InvokeMember('ОбсужденияСлужебныйКлиентСервер', $flags::GetProperty, $null, $connection, $null)
        $types = [System.__ComObject].InvokeMember('ТипыВнешнихСистем', $flags::InvokeMethod, $null, $module, $null)
        $value = [string][System.__ComObject].InvokeMember('Telegram', $flags::GetProperty, $null, $types, $null)
    } catch { $value = Fail $_ }
    "marker=$value"
} finally {
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($connection)
}
