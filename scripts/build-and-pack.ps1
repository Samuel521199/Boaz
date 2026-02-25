#Requires -Version 5.1
# Build and pack Boaz: outputs release\Boaz folder. Copy to USB and run boaz-ui.exe.
# Run from repo root: .\scripts\build-and-pack.ps1
# 注意：构建前请关闭正在运行的 boaz-ui.exe，否则复制可能失败

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir

Set-Location $projectRoot

# Try with Yara first (vendored builds from source); fallback to no Yara if it fails
Write-Host "[*] Building boaz-core (with Yara first)..." -ForegroundColor Cyan
Push-Location boaz-core
$errPrev = $ErrorActionPreference
$ErrorActionPreference = "Continue"
cargo build --release
$coreExit = $LASTEXITCODE
$ErrorActionPreference = $errPrev
if ($coreExit -ne 0) {
    Write-Host "[!] Build with Yara failed. Trying without Yara..." -ForegroundColor Yellow
    cargo build --release --no-default-features
    $coreExit = $LASTEXITCODE
    if ($coreExit -ne 0) {
        Pop-Location
        Write-Error "boaz-core build failed (exit $coreExit). Check output above."
    }
    Write-Host "[+] boaz-core built (no Yara)." -ForegroundColor Green
} else {
    Write-Host "[+] boaz-core built with Yara (rule scan enabled)." -ForegroundColor Green
}
Pop-Location

Write-Host "[*] Building boaz-daemon (The Eye of Boaz)..." -ForegroundColor Cyan
Push-Location boaz-daemon
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    Write-Error "boaz-daemon build failed (exit $LASTEXITCODE). Check output above."
}
Write-Host "[+] boaz-daemon built." -ForegroundColor Green
Pop-Location

# WebView2 Fixed Runtime required for PE/offline; 缺失时自动调用 setup-webview2.ps1 下载
$webview2Dir = Join-Path $projectRoot "boaz-ui\src-tauri\webview2"
$webview2Ok = $false
if (Test-Path $webview2Dir) {
    $wv2Exe = Get-ChildItem $webview2Dir -Recurse -Filter "msedgewebview2.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($wv2Exe) { $webview2Ok = $true }
}
if (-not $webview2Ok) {
    Write-Host "[*] WebView2 not found. Running setup-webview2.ps1 (auto-download)..." -ForegroundColor Cyan
    & (Join-Path $scriptDir "setup-webview2.ps1")
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[!] WebView2 setup failed. Check network or run: .\scripts\setup-webview2.ps1 -CabPath `"path\to\file.cab`"" -ForegroundColor Red
        exit 1
    }
}

Write-Host "[*] Building boaz-ui..." -ForegroundColor Cyan
$ErrorActionPreference = "Continue"
$null = cargo tauri --version 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "[*] Tauri CLI not found. Installing (cargo install tauri-cli)..." -ForegroundColor Yellow
    cargo install tauri-cli
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Tauri CLI install failed. Check network or run manually: cargo install tauri-cli" -ForegroundColor Red
        exit 1
    }
    $null = cargo tauri --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Tauri CLI still not found after install. Try: cargo install tauri-cli" -ForegroundColor Red
        exit 1
    }
    Write-Host "[+] Tauri CLI installed." -ForegroundColor Green
}
Push-Location boaz-ui
cargo tauri build
$uiExit = $LASTEXITCODE
$ErrorActionPreference = $errPrev
if ($uiExit -ne 0) { Pop-Location; Write-Error "boaz-ui build failed (exit $uiExit). Check output above." }
Pop-Location

$outDir = Join-Path $projectRoot "release\Boaz"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

# Workspace 下 target 在根目录
$targetRelease = if (Test-Path (Join-Path $projectRoot "target\release\boaz-core.exe")) { Join-Path $projectRoot "target\release" } else { $null }
$coreExe = if ($targetRelease) { Join-Path $targetRelease "boaz-core.exe" } else { Join-Path $projectRoot "boaz-core\target\release\boaz-core.exe" }
Copy-Item -Path $coreExe -Destination $outDir -Force
Write-Host "[+] Copied boaz-core.exe" -ForegroundColor Green

$daemonExe = if ($targetRelease) { Join-Path $targetRelease "boaz-daemon.exe" } else { Join-Path $projectRoot "boaz-daemon\target\release\boaz-daemon.exe" }
if (Test-Path $daemonExe) {
    Copy-Item -Path $daemonExe -Destination $outDir -Force
    Write-Host "[+] Copied boaz-daemon.exe (The Eye of Boaz)" -ForegroundColor Green
}

# Workspace 构建输出在 projectRoot\target\release；package 单独构建在 boaz-ui\src-tauri\target\release
$uiDir = Join-Path $projectRoot "target\release"
if (-not (Test-Path (Join-Path $uiDir "boaz-ui.exe"))) {
    $uiDir = Join-Path $projectRoot "boaz-ui\src-tauri\target\release"
}
Get-ChildItem -Path $uiDir -File -ErrorAction SilentlyContinue | Where-Object { $_.Extension -match "\.(exe|dll)$" -and $_.Name -notmatch "boaz-core" } | ForEach-Object {
    try {
        Copy-Item -Path $_.FullName -Destination $outDir -Force -ErrorAction Stop
        Write-Host "[+] Copied $($_.Name)" -ForegroundColor Green
    } catch {
        Write-Host "[!] 无法复制 $($_.Name)，请先关闭正在运行的 Boaz 程序" -ForegroundColor Red
    }
}
$bundleDir = Join-Path $uiDir "bundle\msi"
if (Test-Path $bundleDir) {
    Get-ChildItem (Join-Path $bundleDir "*.msi") -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item -Path $_.FullName -Destination $outDir -Force
        Write-Host "[+] Copied $($_.Name)" -ForegroundColor Green
    }
}

# Copy WebView2 Fixed Runtime for PE / offline use
$webview2Dest = Join-Path $outDir "webview2"
if (Test-Path $webview2Dir) {
    if (Test-Path $webview2Dest) {
        try {
            Remove-Item $webview2Dest -Recurse -Force -ErrorAction Stop
        } catch {
            Write-Host "[!] 无法删除旧 webview2（可能被 Boaz 占用），请先关闭 Boaz 后重试；跳过 webview2 更新" -ForegroundColor Yellow
        }
    }
    if (-not (Test-Path $webview2Dest)) {
        Copy-Item -Path $webview2Dir -Destination $webview2Dest -Recurse -Force
        Write-Host "[+] Copied webview2 (PE/offline runtime)" -ForegroundColor Green
    }
}

# Copy frontend to app/ for PE file:// fallback (asset protocol may fail in PE)
$appSrc = Join-Path $projectRoot "boaz-ui\src"
$appDest = Join-Path $outDir "app"
if (Test-Path $appSrc) {
    if (Test-Path $appDest) { Remove-Item $appDest -Recurse -Force }
    Copy-Item -Path $appSrc -Destination $appDest -Recurse -Force
    Write-Host "[+] Copied app/ (PE file fallback)" -ForegroundColor Green
    # 额外复制 index.html 到根目录，作为 app/ 缺失时的第二回退
    $indexSrc = Join-Path $appSrc "index.html"
    if (Test-Path $indexSrc) {
        Copy-Item -Path $indexSrc -Destination $outDir -Force
        Write-Host "[+] Copied index.html to root (fallback)" -ForegroundColor Green
    }
}

$readmePath = Join-Path $outDir "README.txt"
$readme = @"
Boaz - Quick Start (Samuel, 2026-02-23)
======================================
1. Copy this whole Boaz folder to your USB stick.
2. On the machine to scan: open the folder, double-click boaz-ui.exe.
3. Type the drive to scan (e.g. C:\ or D:\), click Start Scan, check the circle: green=OK, yellow=review, red=issues.

The Eye of Boaz (实时监控): In the UI, step 3 lets you start/stop process+network monitoring. Leave drive blank for full scan, or enter C,D for specific drives. Interval default 10 sec.

No extra install on the target PC. boaz-core.exe runs alone. boaz-ui.exe uses bundled WebView2 (webview2 folder) - works in PE and offline. If GUI shows white screen in PE, try Run-Boaz-UI.bat instead of double-clicking exe. If GUI fails, run Scan-Core-Only.bat (console-style report) or: boaz-core.exe --mount-point D:\ --human

Log: a console pops up with the app; in the app use "Run log" to copy or save when something goes wrong. Yara: set rule path in Advanced; if this build has no Yara, build on WSL/Linux for rule scan.
"@
[System.IO.File]::WriteAllText($readmePath, $readme, [System.Text.Encoding]::UTF8)
Write-Host "[+] Created README.txt" -ForegroundColor Green

$utf8NoBom = New-Object System.Text.UTF8Encoding $false
$batPath = Join-Path $outDir "Scan-Core-Only.bat"
$batContent = @"
@echo off
REM No GUI - run core only (e.g. when WebView2 is missing). Samuel 2026-02-23
set /p DRIVE=Enter drive to scan (e.g. D:\): 
"%~dp0boaz-core.exe" --mount-point %DRIVE% --human
pause
"@
[System.IO.File]::WriteAllText($batPath, $batContent, $utf8NoBom)
Write-Host "[+] Created Scan-Core-Only.bat (no-GUI fallback)" -ForegroundColor Green

$uiBatPath = Join-Path $outDir "Run-Boaz-UI.bat"
$uiBatContent = @"
@echo off
REM PE white screen fix: set WebView2 user data to app dir. Samuel 2026-02-23
REM To hide console: set BOAZ_HIDE_CONSOLE=1
set "WEBVIEW2_USER_DATA_FOLDER=%~dp0webview2_data"
if not exist "%WEBVIEW2_USER_DATA_FOLDER%" mkdir "%WEBVIEW2_USER_DATA_FOLDER%"
"%~dp0boaz-ui.exe" %*
"@
[System.IO.File]::WriteAllText($uiBatPath, $uiBatContent, $utf8NoBom)
Write-Host "[+] Created Run-Boaz-UI.bat (PE white screen workaround)" -ForegroundColor Green

Write-Host ""
Write-Host "Done. Output folder: $outDir" -ForegroundColor Green
Write-Host "Copy that folder to USB, then double-click boaz-ui.exe to run." -ForegroundColor Yellow
