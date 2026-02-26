# Boaz 启动 U 盘制作与快速上手

这里是一堆脚本和说明，不熟 WinPE / Linux Live 也能跟着做一块「插上就能用」的 Boaz U 盘。Samuel，2026-02-23。

---

## 推荐方式：Ventoy + Boaz（一个 U 盘搞定）

**Ventoy** 可让一块 U 盘同时容纳多个系统镜像（如 WinPE、Linux Live），并保留一个普通数据区放 Boaz 可执行文件和规则库。启动时选 WinPE 或 Linux Live，进入系统后从同一 U 盘的 **BOAZ** 文件夹运行 Boaz。

### 第一步：制作 Ventoy 启动 U 盘（只需做一次）

1. 下载 Ventoy：<https://github.com/ventoy/Ventoy/releases>，解压。
2. 将**空白 U 盘**插入电脑（注意：会清空 U 盘）。
3. **Windows**：运行 `Ventoy2Disk.exe`，选择该 U 盘，点击「安装」。
4. **Linux**：运行 `sudo ./Ventoy2Disk.sh -i /dev/sdX`（`sdX` 替换为你的 U 盘设备名，如 `sdb`）。
5. 安装完成后，U 盘会多出一个盘符（Windows）或挂载点（Linux），用于存放 ISO 和文件。

### 第二步：放入系统镜像与 Boaz

1. 将 **WinPE** 或 **Linux Live** 的 ISO 拷贝到 U 盘根目录（或任意子目录）：
   - WinPE：可从 [微 PE](https://www.wepe.com.cn/)、[Edgeless](https://edgeless.top/) 等获取，或按下方「仅 WinPE」脚本生成。
   - Linux Live：如 [SystemRescue](https://systemrescue.org/)、[Ubuntu Live](https://ubuntu.com/download/desktop)。
2. 使用本目录的**准备脚本**，把 Boaz 可执行文件和规则库复制到 U 盘的 **BOAZ** 文件夹：

**Windows（PowerShell，以管理员运行）：**

```powershell
# 将 E: 换成你的 Ventoy U 盘盘符
.\prepare-ventoy-usb.ps1 -DriveLetter E:
```

**Linux / macOS：**

```bash
# 将 /media/user/Ventoy 换成你的 Ventoy 分区挂载点
./prepare-ventoy-usb.sh /media/user/Ventoy
```

脚本会在 U 盘上创建 `BOAZ` 目录，并复制当前项目里已编译的 `boaz-core`（及可选 boaz-ui、规则库）。若尚未编译，请先在本项目中执行 `cargo build --release`（并构建 boaz-ui），再运行脚本。

### 第三步：日常使用

1. 用该 U 盘启动待审计电脑，在 Ventoy 菜单中选择 **WinPE** 或 **Linux Live**。
2. 进入系统后，打开 U 盘（在 WinPE 下多为某个盘符，Linux 下常自动挂载在 `/run/media/...`）。
3. 进入 **BOAZ** 文件夹，运行 `boaz-core` 或 `boaz-ui`，按主文档「使用指南」操作即可。

---

## 仅制作 WinPE + Boaz（Windows 本机）

若你只需要 WinPE、且本机为 Windows，可用脚本半自动生成 WinPE 并写入 U 盘。

- **已安装微软 ADK**：运行 `winpe-quick-setup.ps1`，按提示选择 U 盘盘符，脚本会生成 WinPE 并拷贝 Boaz 到 U 盘（若未装 ADK，脚本会提示下载链接）。
- **未安装 ADK**：脚本会给出「用第三方 PE（如微 PE、Edgeless）制作 U 盘后，如何手动把 Boaz 拷入」的简要步骤。

运行方式（PowerShell，管理员）：

```powershell
.\winpe-quick-setup.ps1 -DriveLetter E:
```

---

## 仅制作 Linux Live + Boaz（Linux / macOS 本机）

若你只需要 Linux Live，可用脚本下载小型 Live 镜像并指导写入 U 盘、放置 Boaz。

```bash
./linux-live-quick-setup.sh
```

脚本会提示下载 SystemRescue（或其它小体积 Live）的 ISO，并给出 `dd` 写入 U 盘及创建 **BOAZ** 目录的完整命令；你只需按顺序执行即可。

---

## 脚本一览

| 脚本 | 适用环境 | 作用 |
|------|----------|------|
| `build.bat` | Windows | 项目根目录一键构建：自动下载 WebView2、编译、打包 |
| `build-and-pack.ps1` | Windows | 同上（PowerShell 版） |
| `setup-webview2.ps1` | Windows | 准备 WebView2 固定版（无参数时自动从 NuGet 下载） |
| `prepare-release.ps1` | Windows | 构建 release 并复制 boaz-ui、boaz-daemon、boaz-test-threat 到 release/Boaz |
| `test-threat-detection.ps1` | Windows | 威胁检测诊断：启动模拟威胁，运行 daemon --once，验证能否检测 |
| `prepare-ventoy-usb.ps1` | Windows | 向已安装 Ventoy 的 U 盘写入 BOAZ 目录及可执行文件 |
| `prepare-ventoy-usb.sh` | Linux / macOS | 同上 |
| `winpe-quick-setup.ps1` | Windows | 使用 ADK 生成 WinPE 并写入 U 盘 + 复制 Boaz，或给出手动步骤 |
| `linux-live-quick-setup.sh` | Linux / macOS | 下载 Live ISO、给出 dd 与 BOAZ 目录制作步骤 |
| `bitlocker-unlock.ps1` | Windows / WinPE | 使用恢复密钥解锁 BitLocker 盘符（需管理员） |
| `bitlocker-unlock.sh` | Linux | 使用 dislocker 解锁 BitLocker 分区并挂载 |
| `merge-phase1-phase2.js` | Node.js | 合并阶段一（离线）与阶段二（网络）报告，可选推送到 Lark |

---

## 编译与打包（推荐一键）

在项目根目录**双击 `build.bat`** 或执行：

```powershell
.\scripts\build-and-pack.ps1
```

脚本会：自动下载 WebView2（若缺失）、编译 boaz-core、编译 boaz-ui、打包到 `release\Boaz`。无需手动准备 WebView2。

---

## 编译 Boaz 后再运行脚本

准备脚本会从**项目根目录**寻找已编译产物并复制到 U 盘。若未用 `build.bat`，可手动编译：

```bash
# 在项目根目录
cd boaz-core
cargo build --release
# 若已集成 boaz-ui，也需在 boaz-ui 下构建
```

脚本默认会复制 `boaz-core/target/release/boaz-core`（或 `.exe`）及 `rules` 等目录（若存在）。具体路径见各脚本内注释。
