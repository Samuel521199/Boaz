# WebView2 固定版：PE / 离线环境支持

作者 Samuel · 2026-02-23

boaz-ui 使用 Tauri + WebView2 做界面。普通 Windows 一般自带 WebView2，但 **WinPE、精简系统、离线环境** 往往没有。为此项目会打包 **WebView2 固定版运行时**，随 Boaz 一起分发，插 U 盘就能在 PE 里用界面。

---

## 打包（推荐：一键完成）

在项目根目录执行：

```batch
build.bat
```

或：

```powershell
.\scripts\build-and-pack.ps1
```

**首次构建时**：若缺少 WebView2 固定版，脚本会**自动从 NuGet 下载**并解压，无需手动操作。完成后 `release\Boaz` 含 webview2、app、exe 等，整份拷到 U 盘即可。

---

## 手动准备（仅当自动下载失败时）

若网络受限或 NuGet 不可用，可手动下载并指定路径：

1. **下载**：<https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section> → Fixed Version → x64 → Download，得到 `.cab` 文件。
2. **运行**：
   ```powershell
   .\scripts\setup-webview2.ps1 -CabPath "path\to\Microsoft.WebView2.FixedVersionRuntime.xxx.x64.cab"
   ```
3. **再打包**：`.\scripts\build-and-pack.ps1` 或 `build.bat`

---

## 输出结构

```
release\Boaz\
  boaz-ui.exe
  boaz-core.exe
  webview2\          ← 固定版 WebView2 运行时（约 150MB）
    msedgewebview2.exe
    ...
  Scan-Core-Only.bat
  README.txt
```

整份拷到 U 盘，在 PE 或离线环境里双击 `boaz-ui.exe` 即可用界面。

---

## 手动解压（脚本失败时）

`.cab` 是 Microsoft Cabinet 格式，可用系统自带的 `expand` 解压：

```cmd
mkdir boaz-ui\src-tauri\webview2
expand "D:\Microsoft.WebView2.FixedVersionRuntime.145.0.3800.70.x64.cab" -F:* boaz-ui\src-tauri\webview2
```

解压后会生成版本号子文件夹，把里面的内容（含 `msedgewebview2.exe` 的目录）移到 `webview2` 根目录即可。或安装 7-Zip 后重新运行脚本。

---

## 常见问题

- **打包失败**：提示找不到 WebView2。先执行 `setup-webview2.ps1` 再打包。
- **Scan-Core-Only.bat 报 VCRUNTIME140.dll 缺失**：重新执行 `build-and-pack.ps1` 打包（已改为静态链接 CRT，无需该 DLL）。
- **界面白屏（PE）**：打包后 `release\Boaz` 含 `app\` 文件夹，程序会优先从 `app/index.html` 用 file:// 加载（绕过 asset 协议）。确认 `app` 文件夹与 exe 同目录；或改用 `Run-Boaz-UI.bat` 启动。
- **界面仍起不来**：改用 `Scan-Core-Only.bat` 纯命令行。
- **体积变大**：固定版约 150MB，换来的是 PE/离线可用。
