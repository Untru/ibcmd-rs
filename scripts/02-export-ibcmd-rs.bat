@echo off
setlocal

cd /d "%~dp0\.."

if not defined DB_NAME set "DB_NAME=ut_ibcmd"
if not defined DB_SERVER set "DB_SERVER=localhost"
if not defined DB_USER set "DB_USER=sa"
if not defined SOURCE_VERSION set "SOURCE_VERSION=2.20"
if not defined RUN_ROOT set "RUN_ROOT=E:\ibcmd_lab\ut"
if not "%~1"=="" set "RUN_ROOT=%~1"

if not defined IBCMD_DB_PSW (
    echo Set IBCMD_DB_PSW before running this script.
    echo Example: set "IBCMD_DB_PSW=your_sql_password"
    exit /b 1
)

set "EXE=target\release\ibcmd-rs.exe"
if not exist "%EXE%" (
    echo Missing "%EXE%". Build it first:
    echo cargo build --release
    exit /b 1
)

set "OUT_DIR=%RUN_ROOT%\ut_ibcmd"

mkdir "%RUN_ROOT%" 2>nul

echo Exporting ibcmd-rs source tree to "%OUT_DIR%"
"%EXE%" infobase config export ^
  "%OUT_DIR%" ^
  --format xml ^
  "--source-version=%SOURCE_VERSION%" ^
  --dbms MSSQLServer ^
  "--db-server=%DB_SERVER%" ^
  "--db-name=%DB_NAME%" ^
  "--db-user=%DB_USER%" ^
  --db-pwd-env IBCMD_DB_PSW ^
  --overwrite

if errorlevel 1 exit /b %errorlevel%

echo Done: "%OUT_DIR%"
