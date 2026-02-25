#!/usr/bin/env bash
# 快速制作「Linux Live + Boaz」启动 U 盘：下载小型 Live ISO，并给出 dd 写入与 BOAZ 目录制作步骤。

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# 推荐小型 Live：SystemRescue 约 700MB，支持从 U 盘挂载同一 U 盘上的 BOAZ 目录
ISO_URL="https://sourceforge.net/projects/systemrescuecd/files/sysresccd-11.00/systemrescue-11.00-amd64.iso/download"
ISO_NAME="systemrescue-11.00-amd64.iso"
DOWNLOAD_DIR="${DOWNLOAD_DIR:-$HOME/Downloads}"

echo "[*] Linux Live 快速制作步骤（以 SystemRescue 为例）"
echo ""
echo "1) 下载 Live ISO（若尚未下载）："
echo "   wget -O $DOWNLOAD_DIR/$ISO_NAME '$ISO_URL'"
echo "   或从浏览器打开: https://systemrescue.org/Downloads/"
echo ""
echo "2) 插入 U 盘，确认设备名（如 /dev/sdb，勿选错盘）："
echo "   lsblk"
echo ""
echo "3) 将 ISO 写入 U 盘（会清空该 U 盘，/dev/sdX 请替换为实际设备）："
echo "   sudo dd if=$DOWNLOAD_DIR/$ISO_NAME of=/dev/sdX bs=4M status=progress oflag=sync"
echo ""
echo "4) 写入完成后，多数 Live 会只占一个分区，U 盘剩余空间可能未分区。"
echo "   若需在同一 U 盘放 BOAZ，可："
echo "   - 再插到已安装 Ventoy 的电脑，用 Ventoy 方案（见 scripts/README.md）；或"
echo "   - 在 U 盘上新建一个分区并格式化为 exFAT/FAT32，挂载后运行："
echo "     $SCRIPT_DIR/prepare-ventoy-usb.sh <该分区挂载点>"
echo ""
echo "5) 若使用两块 U 盘：一块为 Linux Live 启动盘，另一块为 Ventoy 数据盘存放 BOAZ，"
echo "   则先做 Ventoy 数据盘并运行: $SCRIPT_DIR/prepare-ventoy-usb.sh <Ventoy挂载点>"
echo ""
echo "[*] 编译 Boaz（在项目根目录）："
echo "   cd $PROJECT_ROOT/boaz-core && cargo build --release"
echo "   (Linux 下可执行文件为 target/release/boaz-core，无 .exe)"
