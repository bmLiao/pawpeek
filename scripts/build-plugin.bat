@echo off
REM Build and package the pawpeek plugin.
REM
REM Usage:
REM   build-plugin.bat                              # build current platform + package
REM   build-plugin.bat windows                      # package windows (uses target/release/)
REM   build-plugin.bat macos   path\to\mac-binary   # package macos with pre-built binary
REM   build-plugin.bat linux   path\to\linux-binary # package linux with pre-built binary

setlocal enabledelayedexpansion

echo === Building PawPeek Plugin ===

set PLATFORM=%~1
set BINARY_PATH=%~2

REM If no args, detect platform and build
if "%PLATFORM%"=="" (
    set PLATFORM=windows
    set BINARY_PATH=target\release\pawpeek.exe
    echo Building for current platform...
    cargo build --release
    if errorlevel 1 (
        echo Build failed!
        exit /b 1
    )
    goto :package
)

REM If binary path provided, use it directly
if not "%BINARY_PATH%"=="" (
    if not exist "%BINARY_PATH%" (
        echo Error: Binary not found: %BINARY_PATH%
        exit /b 1
    )
    echo [PRE-BUILT] %BINARY_PATH%
    goto :package
)

REM Platform specified but no binary path — look for current build
if "%PLATFORM%"=="windows" (
    set BINARY_PATH=target\release\pawpeek.exe
) else (
    set BINARY_PATH=target\release\pawpeek
)
if not exist "!BINARY_PATH!" (
    echo Error: Binary not found at !BINARY_PATH!. Build it first or provide a path.
    exit /b 1
)

:package
set PACKAGE=dist\pawpeek-plugin-%PLATFORM%.zip
echo Platform: %PLATFORM%
echo Packaging to %PACKAGE%...

REM Create dist directory
if not exist dist mkdir dist

REM 打包資料夾保留在 dist\ 下，可直接當本機 marketplace：
REM   claude plugin marketplace add <repo>\dist\pawpeek-plugin-windows
set TMPDIR=%CD%\dist\pawpeek-plugin-%PLATFORM%
if exist "%TMPDIR%" rmdir /S /Q "%TMPDIR%"
mkdir "%TMPDIR%"

REM Copy plugin files
xcopy /E /I /Q ".claude-plugin" "%TMPDIR%\.claude-plugin\" >nul
copy /Y ".claude-plugin\marketplace.json" "%TMPDIR%\marketplace.json" >nul
xcopy /E /I /Q "hooks" "%TMPDIR%\hooks\" >nul
xcopy /E /I /Q "commands" "%TMPDIR%\commands\" >nul
xcopy /E /I /Q "assets" "%TMPDIR%\assets\" >nul
copy /Y "README.md" "%TMPDIR%\" >nul

REM Copy binary (at root of zip, matching hooks.json path)
mkdir "%TMPDIR%\bin" >nul
copy /Y "%BINARY_PATH%" "%TMPDIR%\bin\" >nul
if errorlevel 1 (
    echo Error: Failed to copy binary!
    exit /b 1
)

REM GNU toolchain 編出的 exe 需要旁邊有 WebView2Loader.dll（MSVC 版不需要）
for %%B in ("%BINARY_PATH%") do set BIN_DIR=%%~dpB
if exist "!BIN_DIR!WebView2Loader.dll" (
    copy /Y "!BIN_DIR!WebView2Loader.dll" "%TMPDIR%\bin\" >nul
) else (
    for /f "delims=" %%D in ('dir /S /B "target\release\build\WebView2Loader.dll" 2^>nul ^| findstr /I "\\x64\\"') do (
        copy /Y "%%D" "%TMPDIR%\bin\" >nul
    )
)

REM Create zip (use abs path since pushd changes %CD%)
set ABS_PACKAGE=%CD%\%PACKAGE%
pushd "%TMPDIR%"
powershell -Command "Compress-Archive -Path '*' -DestinationPath '%ABS_PACKAGE%' -Force"
popd

echo.
echo === Package created ===
echo Folder: %TMPDIR%
echo File: %PACKAGE%
for %%F in ("%PACKAGE%") do set SIZE=%%~zF
echo Size: %SIZE% bytes
