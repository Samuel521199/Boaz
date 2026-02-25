# Boaz UI / PE 白屏问题：全面分析与对策

作者 Samuel · 2026-02-23

本文档分析 boaz-ui 在 WinPE、精简系统或离线环境中出现白屏的原因，并给出系统化对策，力求从根本上减少白屏发生。

---

## 一、白屏可能原因概览

| 原因 | 表现 | 对策 |
|------|------|------|
| **1. Asset 协议失效** | PE 下 Tauri 的 asset/tauri 协议无法正确解析路径 | 设置 BOAZ_USE_FILE_PROTOCOL=1 用 file:// 加载（注意：file:// 会导致 invoke 报 Origin 错误，扫描/监控可能不可用） |
| **2. WebView2 运行时缺失** | 无固定版或路径错误 | 打包 webview2 固定版，路径 `./webview2` |
| **3. WebView2 用户数据目录** | 默认路径在 PE 下不可写或异常 | 用 Run-Boaz-UI.bat 设置 `WEBVIEW2_USER_DATA_FOLDER` |
| **4. app/ 目录缺失** | file:// 回退找不到 index.html | build-and-pack 必须复制 app/ |
| **5. 工作目录异常** | 相对路径解析错误 | 优先用 exe 所在目录计算路径 |
| **6. 依赖 DLL 缺失** | 如 VCRUNTIME140.dll | 静态链接 CRT |

---

## 二、技术原理与当前实现

### 2.1 加载流程

```
启动 boaz-ui.exe
    ↓
setup() 中 make_initial_url()
    ↓
BOAZ_USE_FILE_PROTOCOL=1 ？
    ├─ 否 → WebviewUrl::App("index.html")  ← 默认 asset 协议（IPC 正常）
    └─ 是 → 检查 app/index.html 是否存在
        ├─ 存在 → WebviewUrl::External(file:///.../app/index.html)  ← PE 白屏时权宜之计（invoke 可能失败）
        └─ 不存在 → WebviewUrl::App("index.html")
    ↓
WebviewWindowBuilder 创建窗口并加载 URL
    ↓
Tauri 向页面注入 __TAURI__ API
    ↓
前端调用 invoke('run_audit', ...) 等
```

### 2.2 为何 file:// 更可靠

- **Asset 协议**：Tauri 通过自定义协议（如 `tauri://localhost/`）从打包资源加载。在 PE 中，当前目录、临时目录、资源解析可能与完整 Windows 不同，导致协议处理失败，返回空内容 → 白屏。
- **file:// 协议**：直接使用文件系统路径，不依赖 Tauri 内部资源解析。只要 `app/index.html` 与 exe 同目录且存在，即可加载。
- **前端自包含**：`index.html` 使用内联 CSS 和 JS，无外部 `<script src>`、`<link href>`，file:// 加载时不会触发跨域或路径问题。

### 2.3 WebView2 固定版

- **tauri.conf.json**：`bundle.windows.webviewInstallMode: { type: "fixedRuntime", path: "./webview2" }`
- **路径解析**：`./webview2` 相对于可执行文件所在目录。
- **目录结构**：`webview2/` 下需包含 `msedgewebview2.exe` 等运行时文件（由 setup-webview2.ps1 解压得到）。

### 2.4 用户数据目录

- WebView2 需要可写的用户数据目录（缓存、配置等）。
- PE 下默认路径（如 `%LOCALAPPDATA%`）可能不可用或权限异常。
- **Run-Boaz-UI.bat** 设置 `WEBVIEW2_USER_DATA_FOLDER=%~dp0webview2_data`，在 exe 同目录创建 `webview2_data`，确保可写。

---

## 三、已实施对策汇总

| 对策 | 实现位置 | 说明 |
|------|----------|------|
| asset 优先（修复 Origin） | lib.rs `make_initial_url()` | 默认 asset；仅 BOAZ_USE_FILE_PROTOCOL=1 时用 file://（PE 白屏权宜之计） |
| 复制 app/ | build-and-pack.ps1 | 将 boaz-ui/src 复制到 release/Boaz/app/ |
| WebView2 固定版 | setup-webview2.ps1 + tauri.conf | 打包 webview2，离线/PE 可用 |
| 用户数据目录 | Run-Boaz-UI.bat | 设置 WEBVIEW2_USER_DATA_FOLDER |
| 静态链接 CRT | .cargo/config.toml | 避免 VCRUNTIME140.dll 依赖 |
| 窗口 create:false | tauri.conf | 在 setup 中手动创建窗口，便于控制 URL |

---

## 四、排查清单（白屏时按顺序检查）

### 4.1 目录结构

确认 `release\Boaz` 下至少包含：

```
Boaz\
├── boaz-ui.exe
├── boaz-core.exe
├── webview2\           ← 固定版运行时（含 msedgewebview2.exe）
├── app\                 ← 必须存在
│   └── index.html
├── Run-Boaz-UI.bat
├── Scan-Core-Only.bat
└── README.txt
```

若缺少 `app\` 或 `app\index.html`，file:// 回退失效，会退回 asset，易白屏。

### 4.2 启动方式

1. **优先**：用 `Run-Boaz-UI.bat` 启动，而非直接双击 boaz-ui.exe。
2. 若仍白屏：改用 `Scan-Core-Only.bat` 做纯命令行扫描。

### 4.3 WebView2 固定版

- 确认 `webview2` 文件夹存在且含 `msedgewebview2.exe`。
- 若缺失：按 [WebView2-PE.md](WebView2-PE.md) 执行 setup-webview2.ps1 后重新 build-and-pack。

### 4.4 控制台输出

- 启动时会有控制台窗口（attach_console）。
- 若有 `[boaz-ui]` 开头的错误，可据此判断是启动 boaz-core 失败、JSON 解析失败等。

### 4.5 权限与路径

- PE 下 exe 所在盘（如 U 盘）需可读。
- `webview2_data` 会在 exe 同目录创建，需可写；若 U 盘只读，可能异常。

---

## 五、进一步加固建议（可选）

以下为可选增强，用于进一步降低白屏概率：

### 5.1 多路径回退

在 `make_initial_url` 中增加回退顺序，例如：

1. `exe_dir/app/index.html`（当前已实现）
2. `exe_dir/index.html`（若 app 未复制成功时的备用）
3. `WebviewUrl::App("index.html")`（最后回退到 asset）

### 5.2 启动时校验

在 setup 中检查 `app_index.exists()`，若不存在则弹窗或写日志提示「app 目录缺失，请重新打包」，避免静默白屏。

### 5.3 环境变量文档

在 README 或本文档中明确写出：

- `WEBVIEW2_USER_DATA_FOLDER`：用户数据目录，Run-Boaz-UI.bat 已设置。
- `BOAZ_CORE_PATH`：boaz-core 路径，默认同目录。

### 5.4 前端降级

若 `window.__TAURI__` 不存在，在 index.html 中显示「未在 Tauri 环境中运行，请直接双击 Boaz 程序」等提示，避免空白页。

---

## 六、与浏览器的区别

| 对比项 | 浏览器（如 Chrome） | Boaz UI（Tauri + WebView2） |
|--------|----------------------|-----------------------------|
| 运行环境 | 完整系统，网络可用 | PE/离线，可能无网络 |
| 加载方式 | 通常 http/https | asset 或 file:// |
| 扩展/插件 | 可装 | 无 |
| 数据目录 | 用户配置目录 | 需显式指定（WEBVIEW2_USER_DATA_FOLDER） |
| 白屏原因 | 多为网络/跨域 | 多为协议、路径、运行时 |

Boaz 不依赖浏览器，而是用 WebView2 作为内嵌渲染引擎，因此必须保证：

1. WebView2 运行时存在且路径正确；
2. 页面来源（asset 或 file）可被正确加载；
3. 用户数据目录可写。

---

## 七、总结

白屏主要来自 **Asset 协议在 PE 下不可靠** 和 **WebView2 环境不完整**。当前通过 file:// 回退、app/ 复制、固定版 WebView2、Run-Boaz-UI.bat 设置用户数据目录，已覆盖大部分场景。若仍白屏，按第四节排查清单逐项检查，并优先使用 Scan-Core-Only.bat 完成扫描。
