@echo off
if "%PARITY_FAKE_BCP_MODE%"=="fail-version" exit /b 31
if "%~1"=="-v" echo fake bcp 1.0
exit /b 0
