@echo off
rem Build SiegeCastle if needed, then launch it from this folder
rem (so it finds assets\ and the DirectX shader compiler DLLs).
cd /d "%~dp0"
if not exist target\release\siegecastle.exe (
    cargo build --release -j 4 || exit /b 1
)
start "" target\release\siegecastle.exe
