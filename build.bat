@echo off
setlocal

:: ============================================================
::  WOL Service - build everything in one command:
::    1. cargo fmt --check        (rustfmt formatting)
::    2. cargo clippy -D warnings (linter, warnings are errors)
::    3. Windows: cargo build --release --bins
::    4. Linux:   cargo zigbuild --release --bins (zig required)
::    5. All artifacts are copied to dist\
::  Requirements: Rust/Cargo; zig in PATH for the Linux build.
:: ============================================================

cd /d "%~dp0"

set TARGET_LINUX=x86_64-unknown-linux-gnu

where zig >nul 2>&1
if errorlevel 1 (
    set HAVE_ZIG=0
    echo WARNING: zig not found in PATH - Linux build will be skipped.
) else (
    set HAVE_ZIG=1
)

echo [1/5] cargo fmt --check ...
cargo fmt -- --check
if errorlevel 1 goto :fmt_fail

echo [2/5] cargo clippy ...
cargo clippy --all-targets -- -D warnings
if errorlevel 1 goto :clippy_fail

echo [3/5] Windows build (all binaries)...
cargo build --release --bins
if errorlevel 1 goto :build_fail

if not "%HAVE_ZIG%"=="1" goto :package

echo [4/5] Linux build via cargo zigbuild (target %TARGET_LINUX%)...
cargo zigbuild --release --target %TARGET_LINUX% --bins
if errorlevel 1 goto :build_fail

:package
echo [5/5] Copying artifacts to dist\...
if not exist "dist" mkdir "dist"

copy /y "target\release\wol_service.exe" "dist\wol_service.exe" >nul || goto :copy_fail

if "%HAVE_ZIG%"=="1" (
    copy /y "target\%TARGET_LINUX%\release\wol_service" "dist\wol_service" >nul || goto :copy_fail
)

echo.
echo === Build OK. Artifacts in dist\: ===
dir /b dist
echo.
pause
exit /b 0

:fmt_fail
echo FAILED: code is not rustfmt-clean. Run "cargo fmt" and repeat.
pause
exit /b 1

:clippy_fail
echo FAILED: clippy reported problems. See output above.
pause
exit /b 1

:build_fail
echo FAILED: build error. See output above.
pause
exit /b 1

:copy_fail
echo FAILED: could not copy artifacts to dist\. See output above.
pause
exit /b 1
