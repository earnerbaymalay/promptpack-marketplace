@echo off
REM PromptPack Marketplace Build Script
REM PowerShell-compatible (no && chains, no sed/grep, no heredoc)

setlocal enabledelayedexpansion

REM Colors
for /f "tokens=*" %%a in ('echo prompt $E ^| cmd') do set "ESC=%%a"
set "RED=%ESC%[0;31m"
set "GREEN=%ESC%[0;32m"
set "YELLOW=%ESC%[1;33m"
set "BLUE=%ESC%[0;34m"
set "NC=%ESC%[0m"

echo %BLUE%[INFO]%NC% PromptPack Marketplace Build Script
echo.

REM Check prerequisites
echo %BLUE%[INFO]%NC% Checking prerequisites...
where node >nul 2>nul
if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% Node.js not found. Install from https://nodejs.org/
    exit /b 1
)
where npm >nul 2>nul
if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% npm not found.
    exit /b 1
)
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% Rust/Cargo not found. Install from https://rustup.rs/
    exit /b 1
)
echo %GREEN%[OK]%NC% Prerequisites satisfied
echo.

REM Install frontend dependencies
echo %BLUE%[INFO]%NC% Installing frontend dependencies...
npm install
if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% npm install failed
    exit /b 1
)
echo %GREEN%[OK]%NC% Frontend dependencies installed
echo.

REM Build Rust backend
echo %BLUE%[INFO]%NC% Building Rust backend (debug)...
cargo build --manifest-path src-tauri/Cargo.toml
if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% cargo build failed
    exit /b 1
)
echo %GREEN%[OK]%NC% Rust backend built
echo.

REM Build frontend
echo %BLUE%[INFO]%NC% Building frontend...
npm run build
if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% Frontend build failed
    exit /b 1
)
echo %GREEN%[OK]%NC% Frontend built
echo.

REM Build Tauri app
echo %BLUE%[INFO]%NC% Building Tauri application...
npm run tauri build
if %errorlevel% neq 0 (
    echo %RED%[ERROR]%NC% Tauri build failed
    exit /b 1
)
echo %GREEN%[OK]%NC% Tauri application built successfully
echo.

echo %GREEN%[SUCCESS]%NC% All builds completed!
echo %BLUE%[INFO]%NC% Run 'npm run tauri dev' for development mode