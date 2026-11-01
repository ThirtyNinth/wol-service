@echo off
setlocal enabledelayedexpansion

set SERVICE_NAME=WOL-LISTENER
set DISPLAY_NAME=WOL LISTENER

:: Укажите путь к wol_service.exe и команду для выполнения при получении пакета
set EXE_PATH=C:\Users\thirtyninth\wol_service.exe
set COMMAND=shutdown /s /t 0 /f


echo  Installing %SERVICE_NAME%


if not exist "%EXE_PATH%" (
    echo ERROR: File not found: %EXE_PATH%
    echo Please copy binary to the target directory.
    pause
    exit /b 1
)

:: Создаем сервис. Аргументы передаются после пути к exe.
sc create %SERVICE_NAME% binPath= "\"%EXE_PATH%\" %COMMAND%" start= auto DisplayName= "%DISPLAY_NAME%" depend= Tcpip/NSI
if %errorlevel% neq 0 (
    echo FAILED to create service. Run this batch file as Administrator.
    pause
    exit /b 1
)

sc failure %SERVICE_NAME% reset= 86400 actions= restart/60000/restart/60000/restart/60000
if %errorlevel% neq 0 (
    echo WARNING: Failed to configure service recovery.
)

sc description %SERVICE_NAME% "Listens for Wake-on-LAN magic packets and executes configured command."
if %errorlevel% neq 0 (
    echo WARNING: Failed to set service description.
)

echo Registering Event Log source %SERVICE_NAME%...
powershell -NoProfile -ExecutionPolicy Bypass -Command "if (-not [System.Diagnostics.EventLog]::SourceExists('%SERVICE_NAME%')) { New-EventLog -LogName Application -Source '%SERVICE_NAME%' }"
if %errorlevel% neq 0 (
    echo WARNING: Failed to register Event Log source.
)

echo Service %SERVICE_NAME% installed successfully.

pause
