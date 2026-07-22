@echo off
setlocal

cd /d "%~dp0\.."

if not defined DB_NAME set "DB_NAME=ut_ibcmd"
if not defined DB_SERVER set "DB_SERVER=localhost"
if not defined DB_USER set "DB_USER=sa"
if not defined RUN_ROOT set "RUN_ROOT=E:\ibcmd_lab"
if not "%~1"=="" set "RUN_ROOT=%~1"
@REM if not defined USER_NAME set "USER_NAME=Администратор"

if not defined IBCMD_DB_PSW (
    echo Set IBCMD_DB_PSW before running this script.
    echo Example: set "IBCMD_DB_PSW=your_sql_password"
    exit /b 1
)

if not defined IBCMD_EXE (
    for /f "delims=" %%I in ('where ibcmd.exe 2^>nul') do (
        if not defined IBCMD_EXE set "IBCMD_EXE=%%I"
    )
)

if not defined IBCMD_EXE set "IBCMD_EXE=C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe"

if not exist "%IBCMD_EXE%" (
    echo ibcmd.exe not found: "%IBCMD_EXE%"
    echo Set IBCMD_EXE to the full path of ibcmd.exe.
    exit /b 1
)

set "OUT_DIR=%RUN_ROOT%\ut_ibcmd"
set "DATA_DIR=%RUN_ROOT%\ut_ibcmd_data"

mkdir "%RUN_ROOT%" 2>nul
mkdir "%DATA_DIR%" 2>nul

echo Exporting ibcmd baseline to "%OUT_DIR%"
"%IBCMD_EXE%" infobase config export ^
  "--dbms=MSSQLServer" ^
  "--db-server=%DB_SERVER%" ^
  "--db-name=%DB_NAME%" ^
  "--db-user=%DB_USER%" ^
  "--db-pwd=%IBCMD_DB_PSW%" ^
  "--data=%DATA_DIR%" ^
  --force ^
  "%OUT_DIR%"

if errorlevel 1 exit /b %errorlevel%

echo Done: "%OUT_DIR%"
