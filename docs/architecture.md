# Boaz 技术架构说明

更多内容看项目根目录 [README.md](../README.md) 及 [whitepaper-v2.md](whitepaper-v2.md)。  
作者 Samuel，2026-02-25。

---

## 架构概览

Boaz 面向 **V2.0** 演进：从离线审计（Phase 0）转向 AI 驱动的实时 EDR（Phase 1–3）。  
**当前已实现**：Phase 0 离线静态审计、Phase 1 进程与网络感知、Phase 2 部分（AI 研判、绞杀、白名单）。

---

## Phase 0：离线静态审计 ✅

### 检测与处置

- 扫的是 Windows 下的木马、后门、非法监控、病毒之类，靠离线读 Hive、算 Hash、跑 Yara 出结构化结果。
- 报告里的 `suggested_removals` 是可处置项（Run 键、恶意文件等）；用 `--remove` 或 UI 勾选后，可以真的删。注册表项得在 PE 里或进系统后自己清，文件可以在挂载点上直接删。
- `--hash-db` 指向一个「可信 Hash」文件（每行一个 SHA256 或 `文件名:hash`），内核哈希不在里头就标不信任、可给 RED。
- 除了 Run 还读服务（SYSTEM Hive）、计划任务（SOFTWARE TaskCache），核心文件完整性包含 ntoskrnl、winload、hal、bootmgr 等。
- `--remediate --yes` 会真的删报告里 type=file 的项；Run 键还是要人工在 PE 或系统里清。

---

## Phase 1：进程与网络感知（The Eye of Boaz）✅

- **boaz-daemon**：sysinfo 进程图谱 + netstat2 网络连接，按 PID 关联「哪个进程连了哪个 IP:端口」。
- 本地初筛：微软签名进程、白名单路径（System32、知名软件）直接放行。
- 嫌疑判定：AppData\Roaming 或 Temp 下无签名程序 + 外连可疑端口（4444、5555 等）或任意远程 IP。
- **boaz-ui 集成**：启动监控后 spawn daemon，stderr 通过 `daemon-log` 事件推送；解析 `[THREAT]` 行发出 `daemon-threat`，触发威胁弹窗。

---

## Phase 2：AI 研判与处置 🔄 部分完成

- **已实现**：AI 研判（Gemini/OpenAI/Qwen/Grok）、按 PID 绞杀（taskkill）、加入白名单、威胁弹窗（AI 研判/绞杀/白名单/忽略）。
- **待实现**：进程挂起（NtSuspendProcess）、Resume 解除冻结。

---

## V2.0 目标架构（Phase 3）

- **探针层**：ETW 实时捕获（当前为轮询）；微隔离沙箱（NtSuspendProcess）
- **智脑层**：本地 Hash/Yara 秒杀 + 大模型研判 ✅
- **交互层**：系统托盘 + 语义化告警弹窗 ✅
