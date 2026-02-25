#!/usr/bin/env bash
# 将 Boaz 可执行文件与规则库复制到已挂载的 Ventoy U 盘上的 BOAZ 目录（Linux/macOS）
# 用法: ./prepare-ventoy-usb.sh /media/user/Ventoy  或  ./prepare-ventoy-usb.sh /Volumes/Ventoy

set -e
VENTOY_MOUNT="${1:?用法: $0 <Ventoy 分区挂载点>}"

if [[ ! -d "$VENTOY_MOUNT" ]]; then
    echo "错误: 找不到目录 $VENTOY_MOUNT，请先挂载 Ventoy U 盘。"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BOAZ_DEST="${VENTOY_MOUNT%/}/BOAZ"
mkdir -p "$BOAZ_DEST"

# boaz-core（Linux 无 .exe）
CORE_BIN="$PROJECT_ROOT/boaz-core/target/release/boaz-core"
if [[ -f "$CORE_BIN" ]]; then
    cp -f "$CORE_BIN" "$BOAZ_DEST/"
    echo "[+] 已复制 boaz-core 到 $BOAZ_DEST"
else
    echo "警告: 未找到 $CORE_BIN，请先在项目根执行: cd boaz-core && cargo build --release"
fi

# 规则库
if [[ -d "$PROJECT_ROOT/rules" ]]; then
    mkdir -p "$BOAZ_DEST/rules"
    cp -Rf "$PROJECT_ROOT/rules/"* "$BOAZ_DEST/rules/"
    echo "[+] 已复制规则库到 $BOAZ_DEST/rules"
fi

# 可选 boaz-ui（Tauri Linux 产物可能在不同路径）
UI_BIN="$PROJECT_ROOT/boaz-ui/src-tauri/target/release/boaz-ui"
[[ -f "$UI_BIN" ]] && cp -f "$UI_BIN" "$BOAZ_DEST/" && echo "[+] 已复制 boaz-ui 到 $BOAZ_DEST"

echo ""
echo "[*] 完成。用此 U 盘启动目标机，在 Ventoy 菜单选择 WinPE 或 Linux Live，进入系统后打开 U 盘下的 BOAZ 文件夹运行 Boaz。"
echo "[*] Ventoy: https://github.com/ventoy/Ventoy/releases"
