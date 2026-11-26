# One external-connection (COM) session of a lab infobase, held open until a release file appears (or a time limit), so that
# `rac session list` can see it. Windows PowerShell 5.1 (powershell.exe): the COM connector is 64-bit.
#   powershell -NoProfile -File session.ps1 -Srvr localhost:5541 -Database <db> -ReleaseFile <path> [-HoldSec 120]
# Prints: "READY config=<configuration name> version=<version> user=<user>" once connected, "RELEASED" when it lets go, or
# "ERROR <text>" (exit 3 when the text is about a licence: the caller must stop and report, never look for a way round it).
param(
    [Parameter(Mandatory = $true)][string]$Srvr,
    [Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_0[45]_')][string]$Database,
    [Parameter(Mandatory = $true)][string]$ReleaseFile,
    [int]$HoldSec = 120,
    [string]$User = 'Администратор'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$flags = [System.Reflection.BindingFlags]
function Root-Message($e) { $i = $e.Exception; while ($i.InnerException) { $i = $i.InnerException }; $i.Message.Trim() }
try {
    $connector = New-Object -ComObject V83.COMConnector
    $connection = $connector.Connect("Srvr=`"$Srvr`";Ref=`"$Database`";Usr=`"$User`";Pwd=`"`"")
} catch {
    $m = Root-Message $_
    "ERROR $m"
    if ($m -match '(?i)лиценз|licen[cs]e|ключ|HASP') { exit 3 }
    exit 2
}
try {
    $metadata = [System.__ComObject].InvokeMember('Метаданные', $flags::GetProperty, $null, $connection, $null)
    $name = [string][System.__ComObject].InvokeMember('Имя', $flags::GetProperty, $null, $metadata, $null)
    $version = [string][System.__ComObject].InvokeMember('Версия', $flags::GetProperty, $null, $metadata, $null)
    $who = [string][System.__ComObject].InvokeMember('ИмяПользователя', $flags::InvokeMethod, $null, $connection, $null)
    "READY config=$name version=$version user=$who"
    $end = (Get-Date).AddSeconds($HoldSec)
    while ((Get-Date) -lt $end -and -not (Test-Path -LiteralPath $ReleaseFile)) { Start-Sleep -Milliseconds 300 }
} catch {
    "ERROR $(Root-Message $_)"
} finally {
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($connection)
    'RELEASED'
}
