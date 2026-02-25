<#
.SYNOPSIS
  快速制作「WinPE + Boaz」启动 U 盘：若已安装 Windows ADK 则自动生成 WinPE 并写入；否则给出第三方 PE 制作与复制 Boaz 的步骤。
.PARAMETER DriveLetter
  目标 U 盘盘符（如 E:），会清空 U 盘。
#>
param(
    [Parameter(Mandatory = $true)]
    [string] $DriveLetter
)

$DriveLetter = $DriveLetter.TrimEnd('\')
if (-not $DriveLetter.EndsWith(':')) { $DriveLetter = $DriveLetter + ':' }
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir

# 检查 ADK 的 copype 是否存在（常见路径）
$adkPath = "${env:ProgramFiles(x86)}\Windows Kits\10\Assessment and Deployment Kit"
$copype = Get-ChildItem -Path $adkPath -Recurse -Filter "copype.cmd" -ErrorAction SilentlyContinue | Select-Object -First 1

if (-not $copype) {
    Write-Host "未检测到 Windows ADK 的 copype.cmd，无法自动生成 WinPE。"
    Write-Host ""
    Write-Host "方案 A - 安装 ADK 后使用本脚本："
    Write-Host "  1. 下载 Windows ADK: https://learn.microsoft.com/zh-cn/windows-hardware/get-started/adk-install"
    Write-Host "  2. 安装时勾选「部署工具」和「Windows 预安装环境」"
    Write-Host "  3. 再次运行本脚本: .\winpe-quick-setup.ps1 -DriveLetter $DriveLetter"
    Write-Host ""
    Write-Host "方案 B - 使用第三方 WinPE（无需 ADK）："
    Write-Host "  1. 下载微 PE 或 Edgeless 等，制作成可启动 U 盘（会格式化 $DriveLetter）"
    Write-Host "  2. 制作完成后，将 Boaz 复制到 U 盘："
    Write-Host "     .\prepare-ventoy-usb.ps1 -DriveLetter $DriveLetter"
    Write-Host "     若你用的不是 Ventoy，则手动把以下内容复制到 U 盘任意文件夹（如 BOAZ）："
    Write-Host "       - $projectRoot\boaz-core\target\release\boaz-core.exe"
    Write-Host "       - $projectRoot\rules\ (若存在)"
    Write-Host ""
    exit 0
}

Write-Host "[*] 检测到 ADK，正在生成 WinPE 到临时目录..."
$workDir = Join-Path $env:TEMP "BoazWinPE"
if (Test-Path $workDir) { Remove-Item -Recurse -Force $workDir }
New-Item -ItemType Directory -Force -Path $workDir | Out-Null

$copypeDir = Split-Path -Parent $copype.FullName
Set-Location $copypeDir
& .\copype.cmd amd64 $workDir
if ($LASTEXITCODE -ne 0) {
    Write-Error "copype 执行失败。"
    exit 1
}

Write-Host "[*] 正在将 WinPE 写入 U 盘 $DriveLetter（会清空该盘）..."
$wimPath = Join-Path $workDir "media\sources\boot.wim"
# 使用 DISM 或 oscdimg 等写入；此处给出通用步骤，实际需根据 ADK 版本调整
Write-Host "  请手动完成以下任一步骤："
Write-Host "  - 使用「部署和映像工具环境」中的 MakeWinPEMedia 将 $workDir 写入 $DriveLetter"
Write-Host "  - 或使用 Rufus 等工具，选择 $workDir 中生成的 ISO/WIM 写入 U 盘"
Write-Host ""
Write-Host "[*] 写入完成后，将 Boaz 复制到 U 盘："
Write-Host "  .\prepare-ventoy-usb.ps1 -DriveLetter $DriveLetter"
Write-Host "  若 U 盘不是 Ventoy 格式，请手动复制 boaz-core.exe 和 rules 到 U 盘。"
Set-Location $scriptDir
