@echo off
setlocal

cd /d "%~dp0\.."

set "LEFT_DIR=E:\ibcmd_lab\manual_ibcmd\ibcmd"
set "RIGHT_DIR=E:\ibcmd_lab\manual_ours\ibcmd_rs_source_only"
set "DIFF_JSON=E:\ibcmd_lab\manual_diff.json"

if not "%~1"=="" set "LEFT_DIR=%~1"
if not "%~2"=="" set "RIGHT_DIR=%~2"
if not "%~3"=="" set "DIFF_JSON=%~3"

set "EXE=target\release\ibcmd-rs.exe"
if not exist "%EXE%" (
    echo Missing "%EXE%". Build it first:
    echo cargo build --release
    exit /b 1
)

if not exist "%LEFT_DIR%" (
    echo Left tree not found: "%LEFT_DIR%"
    exit /b 1
)

if not exist "%RIGHT_DIR%" (
    echo Right tree not found: "%RIGHT_DIR%"
    exit /b 1
)

for %%I in ("%DIFF_JSON%") do mkdir "%%~dpI" 2>nul

echo Building source diff:
echo   left : "%LEFT_DIR%"
echo   right: "%RIGHT_DIR%"
echo   out  : "%DIFF_JSON%"

"%EXE%" source-diff ^
  -o "%DIFF_JSON%" ^
  "%LEFT_DIR%" ^
  "%RIGHT_DIR%"

if errorlevel 1 exit /b %errorlevel%

echo Done: "%DIFF_JSON%"
