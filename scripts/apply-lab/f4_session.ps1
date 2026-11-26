# A NEW external-connection session of a lab infobase (8.3.27 cluster, registered with register-ib.ps1) reads what the F-4 repro
# changed: the value under "Telegram" of ТипыВнешнихСистем() (the online generation writes "G2MARK") and the function
# ПроверкаIbcmdRsF4() that the exclusive promotion adds. Windows PowerShell 5.1 (powershell.exe): the COM connector is 64-bit.
#   powershell -NoProfile -File f4_session.ps1 -Database <db>
# Prints one line: telegram=<value or error:...> probe=<value or error:...>
param([Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_04_apply_')][string]$Database)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$flags = [System.Reflection.BindingFlags]
function Fail($e) { $i = $e.Exception; while ($i.InnerException) { $i = $i.InnerException }; return 'error:' + $i.Message.Trim() }
$connector = New-Object -ComObject V83.COMConnector
$connection = $connector.Connect("Srvr=`"localhost:2541`";Ref=`"$Database`";Usr=`"Администратор`";Pwd=`"`"")
try {
    try {
        $module = [System.__ComObject].InvokeMember('ОбсужденияСлужебныйКлиентСервер', $flags::GetProperty, $null, $connection, $null)
        $types = [System.__ComObject].InvokeMember('ТипыВнешнихСистем', $flags::InvokeMethod, $null, $module, $null)
        $telegram = [string][System.__ComObject].InvokeMember('Telegram', $flags::GetProperty, $null, $types, $null)
    } catch { $telegram = Fail $_ }
    try {
        $module = [System.__ComObject].InvokeMember('РаботаСКлассификаторамиКлиентСервер', $flags::GetProperty, $null, $connection, $null)
        $probe = [string][System.__ComObject].InvokeMember('ПроверкаIbcmdRsF4', $flags::InvokeMethod, $null, $module, $null)
    } catch { $probe = Fail $_ }
    "telegram=$telegram probe=$probe"
} finally {
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($connection)
}
