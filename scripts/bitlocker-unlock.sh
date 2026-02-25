#!/usr/bin/env bash
# 在 Linux Live 下使用 dislocker 解锁 BitLocker 分区并挂载，便于 Boaz 扫描。
# 用法: ./bitlocker-unlock.sh <块设备> <恢复密钥> [挂载点]
# 例:   ./bitlocker-unlock.sh /dev/sda2 "123456-123456-..." /mnt/windows
# 依赖: dislocker (apt install dislocker 或从 dislocker 项目编译)

set -e
DEV="${1:?用法: $0 <块设备> <恢复密钥> [挂载点]}"
KEY="${2:?用法: $0 <块设备> <恢复密钥> [挂载点]}"
MNT="${3:-/mnt/windows}"

KEY_CLEAN=$(echo "$KEY" | tr -d ' \n-')
if [[ ${#KEY_CLEAN} -ne 48 ]]; then
  echo "[!] 恢复密钥应为 48 位数字（可含连字符）。"
  exit 1
fi

DISLOCKER_MNT="/tmp/dislocker_$$"
mkdir -p "$DISLOCKER_MNT" "$MNT"

echo "[*] 正在使用 dislocker 解锁 $DEV …"
dislocker -V "$DEV" -p"$KEY_CLEAN" -- "$DISLOCKER_MNT"

echo "[*] 挂载解密后的 NTFS 到 $MNT …"
mount -o ro "$DISLOCKER_MNT/dislocker-file" "$MNT"

echo "[+] 已挂载到 $MNT，可运行: boaz-core -m $MNT"
echo "[*] 卸载: umount $MNT; umount $DISLOCKER_MNT; rmdir $DISLOCKER_MNT"
