@echo off
echo This legacy script is disabled to prevent mutable exports. Use:
echo powershell -ExecutionPolicy Bypass -File scripts\export-ibcmd-vs-ours.ps1 -DbName ^<database^> -RunId ^<immutable-id^>
exit /b 1
