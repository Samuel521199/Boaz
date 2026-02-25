# Boaz 技术架构说明

更多内容看项目根目录 [README.md](../README.md) 及 [whitepaper-v2.md](whitepaper-v2.md)。  
作者 Samuel，2026-02-24。

---

## 架构概览

Boaz 面向 **V2.0** 演进：从离线审计（Phase 0）转向 AI 驱动的实时 EDR（Phase 1–3）。当前已实现 Phase 0 的离线静态审计能力。

---

## Phase 0：离线静态审计（当前实现）

### 检测与处置

- 扫的是 Windows 下的木马、后门、非法监控、病毒之类，靠离线读 Hive、算 Hash、跑 Yara 出结构化结果。
- 报告里的 `suggested_removals` 是可处置项（Run 键、恶意文件等）；用 `--remove` 或 UI 勾选后，可以真的删。注册表项得在 PE 里或进系统后自己清，文件可以在挂载点上直接删。
- `--hash-db` 指向一个「可信 Hash」文件（每行一个 SHA256 或 `文件名:hash`），内核哈希不在里头就标不信任、可给 RED。
- 除了 Run 还读服务（SYSTEM Hive）、计划任务（SOFTWARE TaskCache），核心文件完整性包含 ntoskrnl、winload、hal、bootmgr 等。
- `--remediate --yes` 会真的删报告里 type=file 的项；Run 键还是要人工在 PE 或系统里清。

---

## V2.0 目标架构（Phase 1–3）

- **探针层**：ETW 实时捕获进程、注册表、网络事件；微隔离沙箱（NtSuspendProcess）
- **智脑层**：本地 Hash/Yara 秒杀 + 大模型研判
- **交互层**：系统托盘 + 语义化告警弹窗，人机决策
