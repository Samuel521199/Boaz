#Requires -RunAsAdministrator
<#
.SYNOPSIS
  将 Boaz 可执行文件与规则库复制到已安装 Ventoy 的 U 盘上的 BOAZ 目录，便于从 WinPE/Linux Live 直接运行。
.PARAMETER DriveLetter
  Ventoy U 盘盘符（如 E:），不要带反斜杠结尾。
.EXAMPLE
  .\prepare-ventoy-usb.ps1 -DriveLetter E:
#>
param(
    [Parameter(Mandatory = $true)]
    [string] $DriveLetter
)

$DriveLetter = $DriveLetter.TrimEnd('\')
if (-not $DriveLetter.EndsWith(':')) { $DriveLetter = $DriveLetter + ':' }
$ventoyRoot = $DriveLetter + '\'

if (-not (Test-Path $ventoyRoot)) {
    Write-Error "找不到驱动器 $ventoyRoot，请插入 Ventoy U 盘并指定正确盘符。"
    exit 1
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir

$boazDest = Join-Path $ventoyRoot "BOAZ"
New-Item -ItemType Directory -Force -Path $boazDest | Out-Null

# 复制 boaz-core（Windows 为 .exe）
$coreExe = Join-Path $projectRoot "boaz-core\target\release\boaz-core.exe"
if (Test-Path $coreExe) {
    Copy-Item -Path $coreExe -Destination $boazDest -Force
    Write-Host "[+] 已复制 boaz-core.exe 到 $boazDest"
} else {
    Write-Warning "未找到 $coreExe，请先在项目根执行: cd boaz-core; cargo build --release"
}

# 复制规则库（若存在）
$rulesSrc = Join-Path $projectRoot "rules"
if (Test-Path $rulesSrc) {
    $rulesDest = Join-Path $boazDest "rules"
    if (-not (Test-Path $rulesDest)) { New-Item -ItemType Directory -Force -Path $rulesDest | Out-Null }
    Copy-Item -Path (Join-Path $rulesSrc "*") -Destination $rulesDest -Recurse -Force
    Write-Host "[+] 已复制规则库到 $rulesDest"
}

# 可选：boaz-ui
$uiExe = Join-Path $projectRoot "boaz-ui\src-tauri\target\release\boaz-ui.exe"
if (Test-Path $uiExe) {
    Copy-Item -Path $uiExe -Destination $boazDest -Force
    Write-Host "[+] 已复制 boaz-ui.exe 到 $boazDest"
}

Write-Host ""
Write-Host "[*] 完成。请用此 U 盘启动目标机，在 Ventoy 菜单选择 WinPE 或 Linux Live，进入系统后打开 U 盘下的 BOAZ 文件夹运行 Boaz。"
Write-Host "[*] Ventoy 下载: https://github.com/ventoy/Ventoy/releases"
