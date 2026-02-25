#Requires -Version 5.1
# Prepare WebView2 Fixed Runtime for boaz-ui (PE / offline use).
# Run from repo root: .\scripts\setup-webview2.ps1
# 无参数时自动从 NuGet 下载；也可 -CabPath "path\to\file.cab" 或 -DownloadUrl "url"

param(
    [string]$CabPath,
    [string]$DownloadUrl,
    [switch]$SkipAutoDownload
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$webview2Dir = Join-Path $projectRoot "boaz-ui\src-tauri\webview2"

# WebView2 版本（NuGet 包版本，与 Microsoft 官方同步）
$WebView2Version = "145.0.3800.70"
$NuGetPackageId = "webview2.runtime.x64"
$NuGetDownloadUrl = "https://api.nuget.org/v3-flatcontainer/$NuGetPackageId/$WebView2Version/$NuGetPackageId.$WebView2Version.nupkg"

if (Test-Path $webview2Dir) {
    $existing = Get-ChildItem $webview2Dir -Recurse -Filter "msedgewebview2.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($existing) {
        Write-Host "[+] WebView2 runtime already present at $webview2Dir" -ForegroundColor Green
        exit 0
    }
}

$cabFile = $null
$nupkgFile = $null
if ($CabPath) {
    if (-not (Test-Path $CabPath)) {
        Write-Error "Cab file not found: $CabPath"
    }
    $cabFile = $CabPath
} elseif ($DownloadUrl) {
    Write-Host "[*] Downloading WebView2 from URL..." -ForegroundColor Cyan
    $cabFile = Join-Path $env:TEMP "Microsoft.WebView2.FixedVersionRuntime.cab"
    try {
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $cabFile -UseBasicParsing
    } catch {
        Write-Error "Download failed: $_"
    }
} elseif (-not $SkipAutoDownload) {
    Write-Host "[*] Auto-downloading WebView2 from NuGet (v$WebView2Version)..." -ForegroundColor Cyan
    $nupkgFile = Join-Path $env:TEMP "webview2.runtime.x64.$WebView2Version.nupkg"
    try {
        Invoke-WebRequest -Uri $NuGetDownloadUrl -OutFile $nupkgFile -UseBasicParsing
    } catch {
        Write-Host "[!] Auto-download failed: $_" -ForegroundColor Red
        Write-Host "    Fallback: download from https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section" -ForegroundColor Yellow
        Write-Host "    Then run: .\scripts\setup-webview2.ps1 -CabPath `"path\to\file.cab`"" -ForegroundColor Yellow
        exit 1
    }
} else {
    Write-Host "WebView2 Fixed Runtime required. Run without -SkipAutoDownload to auto-download, or use -CabPath." -ForegroundColor Yellow
    exit 1
}

$expandOut = Join-Path $env:TEMP "webview2_expand"
if (Test-Path $expandOut) { Remove-Item $expandOut -Recurse -Force }
New-Item -ItemType Directory -Force -Path $expandOut | Out-Null
$expandFull = (Resolve-Path -LiteralPath $expandOut).Path

# 若从 NuGet 下载了 nupkg：先解压 nupkg（zip），从中找 .cab 或 runtime 目录
if ($nupkgFile -and (Test-Path $nupkgFile)) {
    Write-Host "[*] Extracting NuGet package..." -ForegroundColor Cyan
    try {
        Expand-Archive -Path $nupkgFile -DestinationPath $expandOut -Force
    } catch {
        Write-Host "[!] Expand-Archive failed. Trying 7-Zip..." -ForegroundColor Yellow
        $7zCmd = Get-Command 7z -ErrorAction SilentlyContinue
        if ($7zCmd) {
            & $7zCmd.Source x $nupkgFile -o"$expandFull" -y | Out-Null
        } else {
            Write-Error "Cannot extract nupkg. Install 7-Zip or use -CabPath."
        }
    }
    # NuGet 包内可能是 content/WebView2/ 或 build/ 等，递归找 msedgewebview2.exe 或 .cab
    $exeInNupkg = Get-ChildItem $expandOut -Recurse -Filter "msedgewebview2.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    $cabInNupkg = Get-ChildItem $expandOut -Recurse -Filter "*.cab" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($exeInNupkg) {
        $runtimeDir = $exeInNupkg.DirectoryName
        if (Test-Path $webview2Dir) { Remove-Item $webview2Dir -Recurse -Force }
        New-Item -ItemType Directory -Force -Path $webview2Dir | Out-Null
        Get-ChildItem $runtimeDir | Copy-Item -Destination $webview2Dir -Recurse -Force
        Remove-Item $expandOut -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item $nupkgFile -Force -ErrorAction SilentlyContinue
        $check = Get-ChildItem $webview2Dir -Recurse -Filter "msedgewebview2.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($check) {
            Write-Host "[+] WebView2 Fixed Runtime ready at $webview2Dir (from NuGet)" -ForegroundColor Green
        }
        exit 0
    }
    if ($cabInNupkg) {
        $cabFile = $cabInNupkg.FullName
        $expandOut = Join-Path $env:TEMP "webview2_cab_expand"
        if (Test-Path $expandOut) { Remove-Item $expandOut -Recurse -Force }
        New-Item -ItemType Directory -Force -Path $expandOut | Out-Null
        $expandFull = (Resolve-Path -LiteralPath $expandOut).Path
    }
}

# .cab 解压（来自 -CabPath、-DownloadUrl 或 NuGet 包内的 cab）
if ($cabFile -and (Test-Path $cabFile)) {
    Write-Host "[*] Extracting CAB..." -ForegroundColor Cyan
    $cabFull = (Resolve-Path -LiteralPath $cabFile).Path
    $expanded = $false
    try {
        & cmd.exe /c "expand `"$cabFull`" -F:* `"$expandFull`""
        $expanded = ($LASTEXITCODE -eq 0)
    } catch { }
    if (-not $expanded) {
        $7zCmd = Get-Command 7z -ErrorAction SilentlyContinue
        if ($7zCmd) {
            & $7zCmd.Source x $cabFull -o"$expandFull" -y | Out-Null
            $expanded = ($LASTEXITCODE -eq 0)
        }
    }
    if (-not $expanded) {
        Write-Host "CAB extract failed." -ForegroundColor Red
        exit 1
    }
}

# Cab creates nested folders; find folder with msedgewebview2.exe and flatten to webview2
$exe = Get-ChildItem $expandOut -Recurse -Filter "msedgewebview2.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if (Test-Path $webview2Dir) { Remove-Item $webview2Dir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $webview2Dir | Out-Null
if ($exe) {
    $runtimeDir = $exe.DirectoryName
    Get-ChildItem $runtimeDir | Copy-Item -Destination $webview2Dir -Recurse -Force
} else {
    Get-ChildItem $expandOut | Move-Item -Destination $webview2Dir -Force
}
Remove-Item $expandOut -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $env:TEMP "webview2_expand") -Recurse -Force -ErrorAction SilentlyContinue
if ($DownloadUrl) { Remove-Item $cabFile -Force -ErrorAction SilentlyContinue }
if ($nupkgFile -and (Test-Path $nupkgFile)) { Remove-Item $nupkgFile -Force -ErrorAction SilentlyContinue }

$check = Get-ChildItem $webview2Dir -Recurse -Filter "msedgewebview2.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $check) {
    Write-Host "[!] msedgewebview2.exe not found. Structure may differ." -ForegroundColor Yellow
    Write-Host "    Ensure webview2 folder contains the runtime. Check: $webview2Dir" -ForegroundColor Yellow
} else {
    Write-Host "[+] WebView2 Fixed Runtime ready at $webview2Dir" -ForegroundColor Green
}
