@echo off
REM PromptPack Marketplace Dev Script

setlocal enabledelayedexpansion

for /f "tokens=*" %%a in ('echo prompt $E ^| cmd') do set "ESC=%%a"
set "BLUE=%ESC%[0;34m"
set "GREEN=%ESC%[0;32m"
set "NC=%ESC%[0m"

echo %BLUE%[INFO]%NC% Starting PromptPack Marketplace in development mode...
echo %BLUE%[INFO]%NC% This will start Vite dev server and Tauri app
echo.

npm run tauri dev