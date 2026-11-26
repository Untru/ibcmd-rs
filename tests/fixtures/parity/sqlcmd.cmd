@echo off
if "%~1"=="-?" (
  echo fake sqlcmd 1.0
  exit /b 0
)
echo Config file 0 1 deadbeef
echo ConfigSave file 0 1 deadbeef
exit /b 0
