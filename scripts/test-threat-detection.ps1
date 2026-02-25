# Boaz threat detection diagnostic
# Run threat in background, then daemon --once to verify detection

$BoazRoot = if ($PSScriptRoot) { Split-Path -Parent $PSScriptRoot } else { "E:\Boaz" }

Write-Host "[*] Boaz threat detection diagnostic" -ForegroundColor Cyan
Write-Host "[*] Root: $BoazRoot"

Set-Location $BoazRoot

Start-Sleep -Seconds 1
Write-Host "[*] Starting threat in background..."
$threatProc = Start-Process -FilePath ".\target\debug\boaz-test-threat.exe" -PassThru -WindowStyle Hidden

Start-Sleep -Seconds 5
Write-Host "[*] Running daemon --once..."
cargo build -p boaz-daemon -q 2>$null
$daemonOut = & .\target\debug\boaz-daemon.exe --once 2>&1 | Out-String
$daemonOut | ForEach-Object { Write-Host $_ }

Write-Host "[*] Stopping threat..."
taskkill /T /F /PID $threatProc.Id 2>$null

if ($daemonOut -match "THREAT|内鬼") {
    Write-Host "`n[OK] Threat detected!" -ForegroundColor Green
} else {
    Write-Host "`n[?] No threat detected. Try running as Administrator." -ForegroundColor Yellow
}
