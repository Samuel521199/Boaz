@echo off
REM Boaz 一键构建：自动下载 WebView2、编译、打包。Samuel 2026-02-23
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-and-pack.ps1"
pause
