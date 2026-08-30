@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem XManager launcher — run from repo root (double-click or: run.bat [options])
rem Examples:
rem   run.bat              release build + run
rem   run.bat debug        debug build + run
rem   run.bat bin          run existing release binary only (no rebuild)
rem   run.bat debug bin    run existing debug binary only
rem Frontend hot reload during development:  cd crates\xmanager-tauri\ui && npm install && npm run tauri dev

cd /d "%~dp0" || exit /b 1

set "MODE=release"
set "BIN_ONLY=0"
set "PASS_ARGS="

:parse
if "%~1"=="" goto after_parse
if /I "%~1"=="help" goto usage
if /I "%~1"=="-h" goto usage
if /I "%~1"=="--help" goto usage
if /I "%~1"=="release" (
  set "MODE=release"
  shift
  goto parse
)
if /I "%~1"=="debug" (
  set "MODE=debug"
  shift
  goto parse
)
if /I "%~1"=="cli" (
  shift
  goto run_cli
)
if /I "%~1"=="bin" (
  set "BIN_ONLY=1"
  shift
  goto parse
)
if /I "%~1"=="--" (
  shift
  goto collect_pass
)
set "PASS_ARGS=%PASS_ARGS% %~1"
shift
goto parse

:collect_pass
if "%~1"=="" goto after_parse
set "PASS_ARGS=%PASS_ARGS% %~1"
shift
goto collect_pass

:after_parse

if not exist ".env" (
  echo [warn] .env not found. Copy .env.example to .env and fill OAuth credentials.
  echo.
)

where cargo >nul 2>nul
if errorlevel 1 (
  if "%BIN_ONLY%"=="1" goto try_bin
  echo [error] cargo not found. Install Rust from https://rustup.rs
  exit /b 1
)

if "%BIN_ONLY%"=="1" goto try_bin

echo [info] Building xmanager (Tauri) ^(%MODE%^)...
if not exist "crates\xmanager-tauri\dist\index.html" (
  echo [error] Frontend not built. Run: cd crates\xmanager-tauri ^&^& npm install ^&^& npm run build
  exit /b 1
)
if /I "%MODE%"=="release" (
  cargo run -p xmanager-tauri --release --features custom-protocol -- %PASS_ARGS%
) else (
  cargo run -p xmanager-tauri --features custom-protocol -- %PASS_ARGS%
)
set "EC=%ERRORLEVEL%"
if not "%EC%"=="0" (
  echo [error] XManager exited with code %EC%
  exit /b %EC%
)
exit /b 0

:run_cli
where cargo >nul 2>nul
if errorlevel 1 (
  echo [error] cargo not found. Install Rust from https://rustup.rs
  exit /b 1
)
set "CLI_ARGS="
:cli_collect
if "%~1"=="" goto cli_go
set "CLI_ARGS=!CLI_ARGS! %1"
shift
goto cli_collect
:cli_go
echo [info] Running xmanager-cli...
cargo run -p xmanager-cli -- !CLI_ARGS!
set "EC=%ERRORLEVEL%"
if not "%EC%"=="0" (
  echo [error] xmanager-cli exited with code %EC%
  exit /b %EC%
)
exit /b 0

:try_bin
if /I "%MODE%"=="release" (
  set "EXE=target\release\xmanager.exe"
) else (
  set "EXE=target\debug\xmanager.exe"
)
if not exist "%EXE%" (
  echo [error] Binary not found: %EXE%
  echo        Build first with: run.bat %MODE%
  exit /b 1
)
echo [info] Launching %EXE% ...
"%EXE%" %PASS_ARGS%
set "EC=%ERRORLEVEL%"
if not "%EC%"=="0" (
  echo [error] XManager exited with code %EC%
  exit /b %EC%
)
exit /b 0

:usage
echo Usage: run.bat [release^|debug] [bin] [-- app-args...]
echo        run.bat cli [-- cli-args...]
echo.
echo   release   Build and run release binary ^(default^)
echo   debug     Build and run debug binary
echo   bin       Skip cargo; run existing binary only
echo   cli       Run xmanager-cli ^(JSON automation^)
echo   help      Show this help
echo.
echo Frontend dev with hot reload:
echo   cd crates\xmanager-tauri ^&^& npm install ^&^& npm run tauri dev
echo.
echo Examples:
echo   run.bat
echo   run.bat debug
echo   run.bat release bin
echo   run.bat cli -- --help
echo   run.bat cli -- filter --input tweets.json --max-views 20
exit /b 0
