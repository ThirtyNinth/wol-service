@echo off
setlocal

set SERVICE_NAME=WOL-LISTENER

echo Stopping service %SERVICE_NAME%...

sc stop %SERVICE_NAME% >nul 2>&1

echo Deleting service %SERVICE_NAME%...

sc delete %SERVICE_NAME%

if %errorlevel% neq 0 (
    echo FAILED. Run this batch file as Administrator.
    pause
    exit /b 1
)

echo Unregistering Event Log source %SERVICE_NAME%...
powershell -NoProfile -ExecutionPolicy Bypass -Command "if ([System.Diagnostics.EventLog]::SourceExists('%SERVICE_NAME%')) { Remove-EventLog -Source '%SERVICE_NAME%' }"
if %errorlevel% neq 0 (
    echo WARNING: Failed to unregister Event Log source.
)

echo Service %SERVICE_NAME% removed.

pause
