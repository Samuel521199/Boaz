// 命令行入口：读挂载点、可选规则路径，跑完输出 JSON（可接 Lark 等）

use anyhow::Result;
use boaz_core::{hash_checker, hive_reader};
#[cfg(feature = "yara")]
use boaz_core::fs_parser;
#[cfg(feature = "yara")]
mod yara_engine;

use boaz_core::hive_reader::{ScheduledTaskEntry, ServiceEntry, SuspiciousRunKey};
use clap::Parser;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author = "Samuel", version, about = "Boaz Offline Audit Engine", long_about = None)]
struct Args {
    /// 目标嫌疑机器 C 盘的挂载点路径 (例如 Linux 下的 /mnt/windows 或 WinPE 下的 D:\)
    #[arg(short, long)]
    mount_point: PathBuf,

    /// Yara 规则文件或规则目录路径；提供后会扫描 System32/SysWOW64 下可执行文件并输出命中项
    #[arg(short, long)]
    rules: Option<PathBuf>,

    /// 检测到问题后，在报告中标记可删除/可处置项；配合 UI 或脚本可执行删除（如可疑自启动项、恶意文件）
    #[arg(long)]
    remove: bool,

    /// 可信 Hash 列表文件路径（每行一个 SHA256，或 文件名:hash）；内核哈希若不在列表中则报告 kernel_trusted: false
    #[arg(long)]
    hash_db: Option<PathBuf>,

    /// 对报告中 type=file 的可处置项执行删除（需同时传 --remove 产出报告）；加 --yes 跳过确认
    #[arg(long)]
    remediate: bool,

    /// 与 --remediate 同用：不确认直接删除
    #[arg(long)]
    yes: bool,

    /// ESP/EFI 分区挂载点；提供则对引导文件做完整性校验并写入报告 esp_integrity
    #[arg(long)]
    esp_mount_point: Option<PathBuf>,

    /// 将删除 Run 键的 reg 命令写入该脚本（.cmd 或 .ps1），供进系统或 PE 下 reg load 后执行
    #[arg(long)]
    output_reg_script: Option<PathBuf>,

    /// 控制台友好输出：显示统计、分类汇总（供 Scan-Core-Only.bat 等使用）；默认不输出 JSON
    #[arg(long)]
    human: bool,

    /// 与 --human 同用：同时输出 JSON 到 stdout（供管道/脚本）
    #[arg(long)]
    json: bool,
}

/// 控制台友好输出：统计、分类、简要明细（--human 时调用）
fn print_human_summary(
    status: &str,
    run_keys: &[SuspiciousRunKey],
    risky_services: &[ServiceEntry],
    risky_tasks: &[ScheduledTaskEntry],
    yara_matches: &[Value],
    kernel_trusted: bool,
    kernel_hash: &str,
    suggested_removals: &[Value],
) {
    let stderr = std::io::stderr();
    let mut w = stderr.lock();
    use std::io::Write;

    let _ = writeln!(w, "");
    let _ = writeln!(w, "========================================");
    let _ = writeln!(w, "  Boaz 离线审计 - 扫描报告");
    let _ = writeln!(w, "========================================");
    let _ = writeln!(w, "");

    // 状态
    let (status_str, status_desc) = match status {
        "GREEN" => ("[ 安全 ]", "未发现明显风险"),
        "YELLOW" => ("[ 需关注 ]", "存在可疑项，建议查看详情"),
        _ => ("[ 风险 ]", "发现严重问题，请处置"),
    };
    let _ = writeln!(w, "  总体状态: {} {}", status_str, status_desc);
    let _ = writeln!(w, "");

    // 统计
    let _ = writeln!(w, "  ---------- 统计 ----------");
    let _ = writeln!(w, "  可疑自启动 (Run):     {:>4} 项", run_keys.len());
    let _ = writeln!(w, "  风险服务:             {:>4} 项", risky_services.len());
    let _ = writeln!(w, "  风险计划任务:         {:>4} 项", risky_tasks.len());
    let _ = writeln!(w, "  Yara 命中:            {:>4} 项", yara_matches.len());
    let _ = writeln!(w, "  内核完整性:           {}", if kernel_hash == "FILE_NOT_FOUND" { "缺失" } else if kernel_trusted { "正常" } else { "未在可信库" });
    let _ = writeln!(w, "  可处置项合计:         {:>4} 项", suggested_removals.len());
    let _ = writeln!(w, "");

    // 明细（最多各 5 条）
    fn trunc(s: &str, max: usize) -> String {
        if s.len() <= max { s.to_string() } else { format!("{}...", &s[..max]) }
    }

    if !run_keys.is_empty() {
        let _ = writeln!(w, "  ---------- 可疑自启动 ----------");
        for k in run_keys.iter().take(5) {
            let _ = writeln!(w, "    {} -> {}", k.name, trunc(&k.command_path, 60));
        }
        if run_keys.len() > 5 {
            let _ = writeln!(w, "    ... 还有 {} 项", run_keys.len() - 5);
        }
        let _ = writeln!(w, "");
    }

    if !risky_services.is_empty() {
        let _ = writeln!(w, "  ---------- 风险服务 ----------");
        for s in risky_services.iter().take(5) {
            let _ = writeln!(w, "    {} -> {}", s.name, trunc(&s.image_path, 55));
        }
        if risky_services.len() > 5 {
            let _ = writeln!(w, "    ... 还有 {} 项", risky_services.len() - 5);
        }
        let _ = writeln!(w, "");
    }

    if !risky_tasks.is_empty() {
        let _ = writeln!(w, "  ---------- 风险计划任务 ----------");
        for t in risky_tasks.iter().take(5) {
            let _ = writeln!(w, "    {}", trunc(&t.path, 65));
        }
        if risky_tasks.len() > 5 {
            let _ = writeln!(w, "    ... 还有 {} 项", risky_tasks.len() - 5);
        }
        let _ = writeln!(w, "");
    }

    if !yara_matches.is_empty() {
        let _ = writeln!(w, "  ---------- Yara 命中 ----------");
        for m in yara_matches.iter().take(5) {
            let path = m.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let rule = m.get("rule_id").and_then(|v| v.as_str()).unwrap_or("?");
            let _ = writeln!(w, "    {} [{}]", trunc(path, 50), rule);
        }
        if yara_matches.len() > 5 {
            let _ = writeln!(w, "    ... 还有 {} 项", yara_matches.len() - 5);
        }
        let _ = writeln!(w, "");
    }

    let _ = writeln!(w, "========================================");
    let _ = writeln!(w, "  加 --json 可同时输出完整报告供管道使用");
    let _ = writeln!(w, "========================================");
    let _ = writeln!(w, "");
    let _ = w.flush();
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[boaz-core] 错误: {}", e);
        for (i, cause) in e.chain().skip(1).enumerate() {
            eprintln!("[boaz-core] 原因#{}: {}", i + 1, cause);
        }
        eprintln!("[boaz-core] 调试详情: {:?}", e);
        let _ = std::io::Write::flush(&mut std::io::stderr());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();

    if args.human {
        eprintln!("[*] Boaz 离线审计 | 目标: {}", args.mount_point.display());
        eprintln!("[*] 正在扫描，请稍候...");
    } else {
        eprintln!("[*] 初始化 Boaz 零信任底层环境审计引擎...");
        eprintln!("[*] 目标挂载点: {}", args.mount_point.display());
    }

    // 1. 猎杀持久化驻留项（Run / 服务 / 计划任务）
    if !args.human {
        eprintln!("[*] 正在读取 Hive: Windows/System32/config/SOFTWARE (Run 键)");
    }
    let run_keys = hive_reader::hunt_startup_keys(&args.mount_point)?;
    if !args.human {
        eprintln!("[*] 正在读取 Hive: Windows/System32/config/SYSTEM (服务)");
    }
    let services = hive_reader::hunt_services(&args.mount_point).unwrap_or_default();
    if !args.human {
        eprintln!("[*] 正在读取 Hive: Windows/System32/config/SOFTWARE (计划任务)");
    }
    let scheduled_tasks = hive_reader::hunt_scheduled_tasks(&args.mount_point).unwrap_or_default();
    if !args.human {
        for k in &run_keys {
            let trunc = if k.command_path.len() > 55 { format!("{}...", &k.command_path[..52]) } else { k.command_path.clone() };
            eprintln!("[*] 发现可疑自启动: {} -> {}", k.name, trunc);
        }
        for s in &services {
            if s.risky {
                let trunc = if s.image_path.len() > 55 { format!("{}...", &s.image_path[..52]) } else { s.image_path.clone() };
                eprintln!("[*] 发现风险服务: {} -> {}", s.name, trunc);
            }
        }
        for t in &scheduled_tasks {
            if t.risky {
                let trunc = if t.path.len() > 60 { format!("{}...", &t.path[..57]) } else { t.path.clone() };
                eprintln!("[*] 发现风险任务: {}", trunc);
            }
        }
        eprintln!("[*] 自启动/服务/计划任务解析完成");
    }

    let risky_services: Vec<_> = services.iter().filter(|s| s.risky).cloned().collect();
    let risky_tasks: Vec<_> = scheduled_tasks.iter().filter(|t| t.risky).cloned().collect();

    // 2. 校验核心文件（内核 + 引导等）
    const CORE_FILES: &[&str] = &[
        "Windows/System32/ntoskrnl.exe",
        "Windows/System32/winload.exe",
        "Windows/System32/hal.dll",
        "bootmgr",
    ];
    let mut core_integrity = Vec::new();
    let mut kernel_hash = String::new();
    for rel in CORE_FILES {
        let name = rel.split('/').last().unwrap_or(rel);
        if !args.human {
            eprintln!("[*] 正在校验: {}", name);
        }
        let p = args.mount_point.join(rel);
        let hash = if p.exists() {
            hash_checker::verify_file_integrity(&p).unwrap_or_else(|_| "HASH_ERROR".to_string())
        } else {
            "FILE_NOT_FOUND".to_string()
        };
        if name == "ntoskrnl.exe" {
            kernel_hash = hash.clone();
        }
        core_integrity.push(serde_json::json!({ "file": name, "path": rel, "sha256": hash }));
    }
    if kernel_hash.is_empty() {
        kernel_hash = "FILE_NOT_FOUND".to_string();
    }
    if !args.human {
        eprintln!("[*] 核心文件完整性校验完成");
    }

    // 2b. 可选：ESP 引导分区完整性
    let esp_integrity: Vec<serde_json::Value> = if let Some(ref esp) = args.esp_mount_point {
        const ESP_FILES: &[&str] = &[
            "EFI/Microsoft/Boot/bootmgfw.efi",
            "EFI/Boot/bootx64.efi",
            "EFI/Microsoft/Boot/BCD",
        ];
        let mut list = Vec::new();
        for rel in ESP_FILES {
            let p = esp.join(rel);
            let hash = if p.exists() {
                hash_checker::verify_file_integrity(&p).unwrap_or_else(|_| "HASH_ERROR".to_string())
            } else {
                "FILE_NOT_FOUND".to_string()
            };
            let name = rel.split('/').last().unwrap_or(rel);
            list.push(json!({ "file": name, "path": rel, "sha256": hash }));
        }
        list
    } else {
        Vec::new()
    };

    // 3. 可处置项（自启动）
    #[cfg_attr(not(feature = "yara"), allow(unused_mut))]
    let mut suggested_removals: Vec<serde_json::Value> = run_keys
        .iter()
        .map(|k| {
            json!({
                "type": "run_key",
                "name": k.name,
                "command_path": k.command_path,
                "description": "可疑自启动项，可在 PE 下编辑注册表或通过处置模块删除"
            })
        })
        .collect();

    // 4. Yara 扫描（若启用 feature 且提供规则路径）
    let yara_matches: Vec<serde_json::Value> = {
        #[cfg(feature = "yara")]
        {
            if let Some(ref rules_path) = args.rules {
                if rules_path.exists() {
                    if !args.human {
                        eprintln!("[*] 正在枚举关键可执行文件并加载 Yara 规则: {}", rules_path.display());
                    }
                    let files = fs_parser::list_critical_files_for_scan(&args.mount_point)?;
                    let human = args.human;
                    match yara_engine::scan_files_with_rules(rules_path, &files, |i, total, p| {
                        if !human {
                            let name = p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
                            eprintln!("[*] Yara 扫描: {}/{} {}", i, total, name);
                        }
                    }) {
                        Ok(m) => {
                            if !args.human {
                                eprintln!("[*] Yara 扫描完成，命中 {} 条规则。", m.len());
                            }
                            for rm in &m {
                                suggested_removals.push(json!({
                                    "type": "file",
                                    "path": rm.path,
                                    "rule_id": rm.rule_id,
                                    "namespace": rm.namespace,
                                    "description": "Yara 规则命中，建议隔离或删除后复核"
                                }));
                            }
                            m.into_iter()
                                .map(|rm| {
                                    json!({
                                        "path": rm.path,
                                        "rule_id": rm.rule_id,
                                        "namespace": rm.namespace
                                    })
                                })
                                .collect()
                        }
                        Err(e) => {
                            eprintln!("[!] Yara 扫描失败（已跳过）: {}", e);
                            Vec::new()
                        }
                    }
                } else {
                    eprintln!("[!] 规则路径不存在，已跳过 Yara 扫描: {}", rules_path.display());
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        }
        #[cfg(not(feature = "yara"))]
        {
            if args.rules.is_some() {
                eprintln!("[!] 本构建未包含 Yara 支持，已跳过规则扫描（可用 cargo build --release 启用 yara feature 重新编译）");
            }
            Vec::new()
        }
    };

    let remediation_requested = args.remove;

    // 5. 可选：与可信 Hash 库比对
    let kernel_trusted = if let Some(ref db_path) = args.hash_db {
        let content = std::fs::read_to_string(db_path).unwrap_or_default();
        let trusted: std::collections::HashSet<String> = content
            .lines()
            .map(|l| {
                let s = l.trim().to_lowercase();
                if s.contains(':') {
                    s.split_once(':').map(|(_, h)| h.to_string()).unwrap_or_default()
                } else {
                    s
                }
            })
            .filter(|s| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()))
            .collect();
        kernel_hash != "FILE_NOT_FOUND" && trusted.contains(&kernel_hash.to_lowercase())
    } else {
        true
    };

    let core_missing = core_integrity
        .iter()
        .any(|c| c.get("sha256").and_then(|v| v.as_str()).unwrap_or("") == "FILE_NOT_FOUND");
    let esp_missing = esp_integrity
        .iter()
        .any(|c| c.get("sha256").and_then(|v| v.as_str()).unwrap_or("") == "FILE_NOT_FOUND");
    // 6. 状态判定：Yara/核心/ESP/内核 -> RED；可疑 Run 或风险服务 -> YELLOW；否则 GREEN
    let status = if !yara_matches.is_empty()
        || kernel_hash == "FILE_NOT_FOUND"
        || !kernel_trusted
        || core_missing
        || esp_missing
    {
        "RED"
    } else if !run_keys.is_empty() || !risky_services.is_empty() || !risky_tasks.is_empty() {
        "YELLOW"
    } else {
        "GREEN"
    };

    // 7. 处置：对 type=file 的可处置项执行删除（需 --remediate，可选 --yes 跳过确认）
    if args.remediate {
        for item in &suggested_removals {
            let ty = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if ty != "file" {
                continue;
            }
            let path_str = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path_str.is_empty() {
                continue;
            }
            let path = std::path::Path::new(path_str);
            if !path.exists() {
                eprintln!("[!] 跳过不存在的文件: {}", path_str);
                continue;
            }
            if !args.yes {
                eprintln!("[?] 确认删除: {} (无 --yes 则跳过)", path_str);
                continue;
            }
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("[!] 删除失败 {}: {}", path_str, e);
            } else {
                eprintln!("[+] 已删除: {}", path_str);
            }
        }
    }

    // 8. 可选：输出删除 Run 键的 reg 脚本（进系统或 PE 下 reg load 后执行）
    if let Some(ref script_path) = args.output_reg_script {
        let mut lines = Vec::new();
        lines.push("@echo off".to_string());
        lines.push("REM 删除 HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run 下报告的可疑项".to_string());
        lines.push("REM 在 PE 下需先: reg load HKLM\\OfflineSoft C:\\Windows\\System32\\config\\SOFTWARE".to_string());
        lines.push("REM 再将下方 HKLM\\SOFTWARE 改为 HKLM\\OfflineSoft 后执行".to_string());
        lines.push("".to_string());
        for k in &run_keys {
            let name_escaped = k.name.replace('"', "\\\"");
            lines.push(format!(
                "reg delete \"HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run\" /v \"{}\" /f 2>nul",
                name_escaped
            ));
        }
        if let Err(e) = std::fs::write(script_path, lines.join("\r\n")) {
            eprintln!("[!] 写入 reg 脚本失败: {}", e);
        } else {
            eprintln!("[+] 已写入 Run 键删除脚本: {}", script_path.display());
        }
    }

    // 9. 控制台友好输出（--human 时）
    if args.human {
        print_human_summary(
            &status,
            &run_keys,
            &risky_services,
            &risky_tasks,
            &yara_matches,
            kernel_trusted,
            &kernel_hash,
            &suggested_removals,
        );
    }

    // 10. 结构化输出
    let mut audit_report = json!({
        "status": status,
        "suspicious_run_keys": run_keys,
        "risky_services": risky_services,
        "risky_scheduled_tasks": risky_tasks,
        "services": services,
        "scheduled_tasks": scheduled_tasks,
        "kernel_integrity": {
            "file": "ntoskrnl.exe",
            "sha256": kernel_hash,
            "trusted": kernel_trusted
        },
        "core_integrity": core_integrity,
        "yara_matches": yara_matches,
        "suggested_removals": suggested_removals,
        "remediation_requested": remediation_requested
    });
    if !esp_integrity.is_empty() {
        if let Some(obj) = audit_report.as_object_mut() {
            obj.insert("esp_integrity".to_string(), serde_json::json!(esp_integrity));
        }
    }

    // 输出 JSON：UI 调用时必输出；--human 时默认不输出
    if !args.human || args.json {
        eprintln!("[*] 扫描完成，正在生成报告…");
        println!("{}", serde_json::to_string(&audit_report)?);
    }

    if remediation_requested && !suggested_removals.is_empty() && !args.remediate {
        eprintln!(
            "[!] 已标记 {} 项为可处置；若需自动删除文件请使用 --remediate --yes；Run 键可传 --output-reg-script 生成删除脚本。",
            suggested_removals.len()
        );
    }

    Ok(())
}
