# Boaz V2.0：AI 驱动的零信任端点智能哨兵系统

**Whitepaper** · 作者：Samuel · 文档更新：2026-02-25 · v0.2.1

---

## 1. 执行摘要 (Executive Summary)

传统的离线审计虽然具备极高的置信度，但缺乏实效性与交互性。**Boaz V2.0** 完成了从「被动取证」到「主动防御（EDR）」的范式转移。

它以极其轻量级的 Rust 系统服务常驻于 Windows 环境，实时监听文件系统、注册表和网络连接的底层事件。结合大语言模型（LLM）的深度推理能力，Boaz 能够在一微秒内冻结可疑进程，并将复杂的进程树、网络外连等晦涩的机器行为翻译为人类可读的「黑客攻击意图」，交由系统管理员或用户进行最终裁决。

---

## 2. 核心架构与设计哲学 (Core Philosophy)

Boaz V2.0 遵循 **「白盒化监控 + AI 研判 + 人机共生」** 的设计哲学：

- **不越俎代庖**：系统只做拦截和建议，最高决策权（Kill or Allow）始终掌握在人类（用户/CTO）手中。
- **知其然，知其所以然**：告别传统杀毒软件只会报「发现 Trojan.Win32」的黑盒模式。Boaz 会通过大模型告诉你：「该程序在后台伪装成 svchost，正尝试将你桌面上的文档打包，并准备发送至一个俄罗斯的未知 IP。这符合典型的勒索/窃密软件行为模式。」

---

## 3. 系统宏观架构 (System Architecture)

系统被重构为三个高度解耦的层级：

### 3.1 探针感知层 (The Sensor - Ring 0 / Ring 3)

- **ETW (Event Tracing for Windows) 引擎**：摒弃容易导致蓝屏的传统底层驱动（Minifilter），使用微软官方的 ETW 机制。以极低的性能损耗，实时捕获进程创建、模块加载（DLL 注入）、网络连接建立等核心事件。
- **微隔离沙箱**：当探针发现未知程序的异常行为时，第一时间调用 `NtSuspendProcess` 将目标进程挂起（冻结），而不是直接删除，为 AI 研判争取时间。

### 3.2 混合智脑层 (The Brain - Local + Cloud AI)

- **本地敏捷引擎 (Local Heuristic)**：基于 Yara 规则和云端黑白名单 Hash 库，对已知威胁进行微秒级秒杀，拦截 90% 的已知噪音。
- **大模型研判中枢 (LLM Analyzer)**：对于本地无法判定的行为，提取上下文（进程名、父进程是谁、写入了什么注册表、连了什么 IP），构建 Prompt，通过 API 发送给大模型进行深度逻辑分析，甚至预测其所属的木马家族（如 AsyncRAT, Cobalt Strike）。

### 3.3 交互与决策层 (The UI - User in the Loop)

- **系统托盘守护**：启动监控后最小化到托盘，点击图标恢复；支持「显示窗口」「退出」右键菜单。
- **语义化告警弹窗**：监控发现可疑进程时自动唤起主窗口并弹出告警，支持【✨ AI 研判】【🔴 绞杀】【🟢 加入白名单】【忽略】。

---

## 4. 核心执行流程图 (Execution Flow)

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  Windows 系统底层 (ETW)                                                           │
│         │                                                                        │
│         ▼ 触发事件: 新建进程/修改注册表/发起网络连接                               │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │ Boaz 后台服务 (Rust)                                                       │   │
│  │    → 本地极速校验 (Hash/Yara 白名单过滤)                                    │   │
│  │    ├─ 命中白名单 → 放行 (无感知)                                           │   │
│  │    └─ 行为可疑/未知 →                                                       │   │
│  │         ├─ 发送 Suspend 指令 (瞬间冻结嫌疑进程)                             │   │
│  │         ├─ 收集进程树、网络抓包、文件上下文                                 │   │
│  │         ├─ 发送分析请求 → 大模型 API                                         │   │
│  │         └─ 收到 AI 诊断报告 → 触发告警弹窗                                 │   │
│  └──────────────────────────────────────────────────────────────────────────┘   │
│         │                                                                        │
│         ▼ 用户决策界面                                                           │
│  ┌──────────────────────────────────────────────────────────────────────────┐   │
│  │ Boaz 桌面弹窗 (Tauri)                                                      │   │
│  │    【拦截并清除】 or 【允许运行】 or 【加入白名单】                          │   │
│  └──────────────────────────────────────────────────────────────────────────┘   │
│         │                                                                        │
│         ├─ 用户选择【拦截】 → Kill & Clean（终止进程、删除源文件与注册表残留）   │
│         └─ 用户选择【允许】 → 加入白名单、Resume Process（解除冻结）             │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. 技术栈与工程结构 (Technology Stack V2.0)

| 模块 | 语言/技术 | 说明 |
|------|------------|------|
| **boaz-daemon** 核心守护进程 | Rust | `sysinfo` 监控进程树；`netstat2` 底层网络连接；Phase 1 已实现「进程与网络感知融合」(The Eye of Boaz) |
| **boaz-shared** IPC 数据结构 | Rust | Daemon 与 UI 通信的共享类型（SuspectMapping、AlertPayload 等） |
| **boaz-ui** 用户交互端 | Tauri | 离线扫描 + 实时监控；系统托盘 + 威胁告警弹窗；通过 stderr 管道与 Daemon 通信 |
| **boaz-test-threat** 威胁模拟 | Rust | 用于测试监控捕获的模拟威胁程序 |

### 目录结构 (Monorepo V2.0)

```
boaz/
├── .cursor/rules/           # Cursor 规则 (01-global-live-security.mdc 等)
├── boaz-daemon/             # Rust: Windows 守护进程 (Ring 3)
│   ├── src/
│   │   ├── main.rs          # 服务入口与循环监听
│   │   ├── monitor_net.rs   # 实时网络连接嗅探
│   │   ├── monitor_proc.rs  # 进程创建与行为监控
│   │   ├── killer.rs        # 执法模块 (挂起、杀死、删文件)
│   │   └── llm_bridge.rs    # AI 桥接
│   └── Cargo.toml
├── boaz-shared/             # Rust: Daemon 与 UI 通信的 IPC 模型
├── boaz-ui/                 # Tauri: 托盘图标与告警弹窗
├── boaz-core/               # Rust: Phase 0 离线审计引擎 (保留)
└── scripts/
```

---

## 6. 演进路线与具体目标 (Evolution Roadmap)

| 阶段 | 目标 | 状态 |
|------|------|------|
| **Phase 0** | 离线静态审计 | ✅ 已完成 |
| **Phase 1** | 流量与进程感知（The Eye of Boaz） | ✅ 已完成 |
| **Phase 2** | 挂起与 AI 审讯 | 🔄 部分完成 |
| **Phase 3** | 联动与自愈 | 规划中 |

### Phase 0：离线静态审计 ✅

- 基于 `boaz-core` 的离线 Hive 解析、Yara 规则、Hash 校验。
- `boaz-ui` 图形界面与 `Scan-Core-Only.bat` 命令行扫描。
- 适用于 WinPE / Linux Live 的冷启动场景。

### Phase 1：流量与进程感知（The Eye of Boaz）✅

- **boaz-daemon**：融合进程图谱（sysinfo）与网络连接表（netstat2），将「哪个文件正在连接哪个外部 IP 的什么端口」映射出来。
- 本地初筛过滤微软签名与白名单，锁定 AppData 下无签名程序外连非常规端口的嫌疑进程。
- **boaz-ui 集成**：点击「启动监控」后最小化到系统托盘，daemon 输出通过 `daemon-log` 事件推送。
- **威胁弹窗**：检测到可疑进程时自动唤起主窗口并弹出告警，支持 AI 研判、绞杀、加入白名单。
- **运行**：`cargo run -p boaz-daemon`（常驻）；`--once` 单次扫描；`--interval 5` 指定间隔；`--drive C,D` 指定盘符。
- **测试**：`cargo run -p boaz-test-threat --release` 模拟威胁；`.\scripts\test-threat-detection.ps1` 诊断脚本。

### Phase 2：挂起与 AI 审讯 🔄 部分完成

- **已实现**：AI 研判（Gemini/OpenAI/Qwen/Grok）、按 PID 绞杀、加入白名单、威胁弹窗交互。
- **待实现**：进程挂起（NtSuspendProcess）、Resume 解除冻结。

### Phase 3：联动与自愈

- 用户点击「拦截」后，自动回溯其行为，删除其创建的隐藏文件和启动项。

---

## 7. 当前实现：快速上手

### 打包

1. 安装 [Rust](https://www.rust-lang.org/tools/install)。
2. 在项目根目录**双击 `build.bat`** 或执行 `.\scripts\build-and-pack.ps1`。
3. 首次构建会自动下载 WebView2 固定版（约 150MB）。
4. 输出在 `release\Boaz`，整份拷到 U 盘即可。**监控功能需 `boaz-daemon.exe` 与 `boaz-ui.exe` 同目录**。

### Phase 0：离线扫描

1. 将 **Boaz** 文件夹拷到 U 盘。
2. 在目标电脑或 PE 中打开该文件夹。
3. 双击 **`boaz-ui.exe`** 或 **`Run-Boaz-UI.bat`**（PE 下白屏时优先用后者）。
4. 选择要扫描的盘符（如 `C:\`、`D:\`），点「开始扫描」。
5. 界面起不来时，用 **`Scan-Core-Only.bat`** 或命令行：`boaz-core.exe --mount-point D:\ --human`。

### Phase 1：实时监控

1. 启动 `boaz-ui.exe`，点击「启动监控」。
2. 窗口自动最小化到托盘，daemon 每约 10 秒扫描一次。
3. 发现可疑进程时弹出威胁告警，可进行 AI 研判、绞杀、加入白名单。
4. 测试威胁检测：`cargo run -p boaz-test-threat --release` 或 `.\scripts\test-threat-detection.ps1`。

### 命令行示例

```batch
boaz-core.exe --mount-point D:\ --human
boaz-core.exe --mount-point D:\ --rules path\to\rules
boaz-core.exe --mount-point D:\ --remove --remediate --yes
boaz-daemon.exe --once
boaz-daemon.exe --interval 5 --drive C,D
```

---

## 8. 文档索引

| 文档 | 说明 |
|------|------|
| [docs/PRODUCT-VISION.md](docs/PRODUCT-VISION.md) | **产品愿景**：咽喉点狙击、AI 仪表盘、三级风险卡片、前端重构 Prompt |
| [docs/whitepaper-v2.md](docs/whitepaper-v2.md) | V2.0 完整白皮书（含 Mermaid 流程图） |
| [docs/architecture.md](docs/architecture.md) | 技术架构与检测逻辑 |
| [docs/WebView2-PE.md](docs/WebView2-PE.md) | WebView2 固定版与 PE 支持 |
| [docs/UI-PE-白屏问题分析与对策.md](docs/UI-PE-白屏问题分析与对策.md) | 界面白屏排查 |
| [docs/安装Yara.md](docs/安装Yara.md) | Yara 规则扫描配置 |
| [scripts/README.md](scripts/README.md) | 启动 U 盘制作与脚本说明 |

---

## 9. 版本与许可

- **v0.2.1**（2026-02-25）：系统托盘、监控威胁弹窗、AI 研判/绞杀/白名单、boaz-test-threat 测试工具
- 作者：Samuel
