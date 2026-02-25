<#
.SYNOPSIS
  在 WinPE 或本机 Windows 下使用 BitLocker 恢复密钥解锁指定驱动器，便于 Boaz 挂载后扫描。
.PARAMETER DriveLetter
  待解锁的盘符（如 C:）
.PARAMETER RecoveryPassword
  48 位恢复密码（可选）；未提供时会提示输入
.EXAMPLE
  .\bitlocker-unlock.ps1 -DriveLetter C:
  .\bitlocker-unlock.ps1 -DriveLetter C: -RecoveryPassword "123456-123456-..."
#>
param(
    [Parameter(Mandatory = $true)]
    [string] $DriveLetter,
    [string] $RecoveryPassword
)

$DriveLetter = $DriveLetter.TrimEnd('\')
if (-not $DriveLetter.EndsWith(':')) { $DriveLetter = $DriveLetter + ':' }

if (-not $RecoveryPassword) {
    $RecoveryPassword = Read-Host -Prompt "请输入 BitLocker 恢复密钥（48 位，含连字符）"
}
$RecoveryPassword = $RecoveryPassword -replace '\s+', ''

try {
    Unlock-BitLocker -MountPoint $DriveLetter -RecoveryPassword $RecoveryPassword
    Write-Host "[+] 已解锁 $DriveLetter"
} catch {
    Write-Host "[!] 解锁失败，请确认：1) 以管理员运行 2) 恢复密钥正确 3) 驱动器为 BitLocker 加密。"
    Write-Host "    也可使用: manage-bde -unlock $DriveLetter -RecoveryPassword ""恢复密钥"""
    exit 1
}
