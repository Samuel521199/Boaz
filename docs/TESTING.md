# Boaz 测试说明

Samuel，2026-02-23。怎么跑单元测试和简单手工验证。

---

## 单元测试（不碰真实盘也行）

### boaz-core（Rust）

危险路径那套逻辑有单测，不读真实 Hive。

```bash
cd boaz-core
cargo test --lib
```

用 `--lib` 只测库，不链 Yara，省得环境缺东西。测的是 Run/服务/任务路径谁算危险、谁算安全，最后看到一溜 `ok` 和 `test result: ok` 就行。

---

### boaz-net（Node.js）

规则引擎那边测了危险端口、非常规端口会不会告警。

```bash
cd boaz-net
npm test
```

看到「rules_engine 测试全部通过」、退出码 0 就对了。

---

## 二、手工验证（需要对应环境）

### 1. boaz-core 完整流程（需挂载点）

**条件：** 有一块已挂载的 Windows 系统盘（如虚拟机磁盘、PE 下挂载的 C:、或 Linux 下挂载的 NTFS）。

**步骤：**

```bash
cd boaz-core
cargo build --release

# 仅扫描，输出 JSON（将路径改为你的挂载点）
./target/release/boaz-core --mount-point /mnt/windows

# 使用规则与 Hash 库（若你有）
./target/release/boaz-core -m /mnt/windows --rules ./rules --hash-db ./hashes.txt --remove
```

终端最后会打出一段 JSON，带 status、suspicious_run_keys、services、core_integrity 等，没 panic 就成。没 Windows 盘就跳过，只跑上面的 cargo test 也行。

---

### 2. boaz-net 抓包 / 快照

**Linux/macOS（需 tcpdump）：**

```bash
cd boaz-net
node src/index.js 5 any
```

跑几秒会打出 JSON（status、summary、alerts）。没装 tcpdump 会报错；Windows 下没 tcpdump 会改用 netstat 快照，一样出 JSON。

---

### 3. boaz-reporter（需飞书 Webhook）

**不推 Lark（仅本地检查 Markdown 生成）：**

```bash
echo '{"status":"YELLOW","suspicious_run_keys":[{"name":"Test","command_path":"C:\\Temp\\x.exe"}],"kernel_integrity":{"sha256":"abc","trusted":true},"core_integrity":[],"risky_services":[],"risky_scheduled_tasks":[],"yara_matches":[],"suggested_removals":[]}' > /tmp/p1.json
node boaz-reporter/push_to_lark.js /tmp/p1.json
```

没设 LARK_WEBHOOK_URL 会报错，说明脚本至少把 JSON 吃进去了；设好 Webhook 再跑就会真推。

---

### 4. 合并脚本（阶段一 + 阶段二）

```bash
# 仅阶段一
node scripts/merge-phase1-phase2.js phase1.json

# 阶段一 + 阶段二
node scripts/merge-phase1-phase2.js phase1.json phase2.json

# 从 stdin 读阶段一
cat phase1.json | node scripts/merge-phase1-phase2.js
```

用前面 boaz-core / boaz-net 生成的 JSON 作为 phase1.json、phase2.json，检查合并后的 JSON 与 Lark 推送内容是否包含 `risky_services`、`risky_scheduled_tasks`、`dangerous_ports` 等字段。

---

### 5. boaz-ui（Tauri）

**开发模式（需已安装 Tauri 依赖）：**

```bash
cd boaz-ui
# boaz-core 在 PATH 或设 BOAZ_CORE_PATH 都行
cargo tauri dev
```

界面里填挂载点（和可选的引擎路径、规则路径），点开始扫描。挂载点无效会报错，有效就会更状态环和报告。没盘可扫时也能打开界面点点看，扫会失败是正常的。

---

### 6. 监控与威胁弹窗（Phase 1）

**威胁检测诊断（一键验证 daemon 能否检测）：**

```powershell
.\scripts\test-threat-detection.ps1
```

输出 `[OK] Threat detected!` 即 daemon 工作正常。

**完整流程：**

1. 启动 `boaz-ui.exe`，点击「启动监控」（需 `boaz-daemon.exe` 与 UI 同目录）。
2. 另开终端运行 `cargo run -p boaz-test-threat --release`。
3. 约 10 秒内应弹出威胁告警，可测试 AI 研判、绞杀、白名单。
4. 点击「测试告警」按钮可模拟弹窗，用于诊断 UI。

---

## 三、建议的测试顺序

| 步骤 | 操作 | 目的 |
|------|------|------|
| 1 | `cd boaz-core && cargo test` | 确认危险路径与任务路径逻辑正确 |
| 2 | `cd boaz-net && npm test` | 确认规则引擎告警条件正确 |
| 3 | `.\scripts\test-threat-detection.ps1` | 验证 daemon 威胁检测 |
| 4 | （可选）在真实或虚拟机挂载点上跑一次 `boaz-core -m <path>` | 端到端验证引擎与 Hive 解析 |
| 5 | （可选）在 PE 或 Live 中跑 boaz-ui，指定 U 盘上的 boaz-core | 验证实际使用流程 |

跑完 1、2、3 就算核心、规则和监控过关；4、5 是发版或上现场前再摸一遍流程用的。
