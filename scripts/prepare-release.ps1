# 构建 release 并复制到 release/Boaz/
# 确保 boaz-ui、boaz-daemon、boaz-test-threat 都在同一目录，便于监控测试

$ErrorActionPreference = "SilentlyContinue"
$BoazRoot = if ($PSScriptRoot) { Split-Path -Parent $PSScriptRoot } else { "E:\Boaz" }
$ReleaseDir = Join-Path $BoazRoot "release\Boaz"
$TargetDir = Join-Path $BoazRoot "target\release"

Write-Host "[*] Building release..." -ForegroundColor Cyan
Set-Location $BoazRoot

cargo build -p boaz-daemon --release -q
cargo build -p boaz-test-threat --release -q
cargo tauri build

if (-not (Test-Path $ReleaseDir)) {
    New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null
}

Copy-Item "$TargetDir\boaz-daemon.exe" $ReleaseDir -Force
Copy-Item "$TargetDir\boaz-test-threat.exe" $ReleaseDir -Force

# boaz-ui.exe 在 target/release/ 或 bundle 子目录
$UiExe = Join-Path $TargetDir "boaz-ui.exe"
if (Test-Path $UiExe) {
    Copy-Item $UiExe $ReleaseDir -Force
} else {
    $Found = Get-ChildItem -Path $TargetDir -Filter "boaz-ui.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($Found) { Copy-Item $Found.FullName $ReleaseDir -Force }
}

Write-Host "[OK] Release prepared: $ReleaseDir" -ForegroundColor Green
Write-Host "    boaz-ui.exe, boaz-daemon.exe, boaz-test-threat.exe"
