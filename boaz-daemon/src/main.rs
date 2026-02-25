//! Boaz Daemon - The Eye of Boaz
//! 进程与网络感知融合：找出「哪个文件正在连接哪个外部 IP 的什么端口」
//! Samuel, 2026-02-24

mod killer;
mod llm_bridge;
mod monitor_net;
mod monitor_proc;

use anyhow::Result;
use boaz_shared::{NetworkConnection, SuspectMapping, SuspectProcess};
use clap::Parser;
use std::time::Duration;
use tracing::{info, warn};

/// 默认扫描间隔（秒），遵循性能红线 CPU < 1%
const DEFAULT_INTERVAL_SECS: u64 = 10;

/// 白名单：微软官方签名进程名（简化版，Phase 1 仅做初筛）
const WHITELIST_PROCESS_NAMES: &[&str] = &[
    "svchost.exe",
    "csrss.exe",
    "lsass.exe",
    "services.exe",
    "wininit.exe",
    "winlogon.exe",
    "dwm.exe",
    "explorer.exe",
    "SearchHost.exe",
    "RuntimeBroker.exe",
    "SystemSettings.exe",
    "conhost.exe",
    "fontdrvhost.exe",
];

/// 白名单：常见合法软件（可扩展）
const WHITELIST_SOFTWARE: &[&str] = &[
    "WeChat", "wechat", "chrome", "firefox", "msedge", "Teams", "Slack",
];

/// 可疑端口（RAT/木马常用）
const SUSPICIOUS_PORTS: &[u16] = &[4444, 5555, 6666, 8080, 8888, 9999];

fn is_whitelisted(proc_: &SuspectProcess) -> bool {
    let name_lower = proc_.name.to_lowercase();
    let path_lower = proc_.path.to_lowercase();

    if WHITELIST_PROCESS_NAMES
        .iter()
        .any(|&w| name_lower == w.to_lowercase())
    {
        return true;
    }

    if WHITELIST_SOFTWARE
        .iter()
        .any(|&w| path_lower.contains(&w.to_lowercase()))
    {
        return true;
    }

    // 系统目录下的可执行文件通常可信
    if path_lower.contains("\\windows\\system32")
        || path_lower.contains("\\windows\\syswow64")
        || path_lower.contains("/windows/system32")
        || path_lower.contains("/windows/syswow64")
    {
        return true;
    }

    false
}

fn is_suspicious(proc_: &SuspectProcess, conns: &[NetworkConnection]) -> bool {
    // 位于 AppData\Roaming 下的未签名程序 + 外连非常规端口
    let path_lower = proc_.path.to_lowercase();
    let in_appdata = path_lower.contains("appdata\\roaming")
        || path_lower.contains("appdata/roaming")
        || path_lower.contains("appdata\\local\\temp")
        || path_lower.contains("appdata/local/temp");

    let has_suspicious_port = conns.iter().any(|c| {
        c.remote_port > 0 && SUSPICIOUS_PORTS.contains(&c.remote_port)
    });

    let has_remote_connection = conns
        .iter()
        .any(|c| !c.remote_addr.is_empty() && c.remote_addr != "*");

    in_appdata && (has_suspicious_port || has_remote_connection)
}

/// 进程路径是否匹配指定盘符（drive_letters 为空表示全盘）
fn path_matches_drives(path: &str, drive_letters: &[char]) -> bool {
    if drive_letters.is_empty() {
        return true;
    }
    let path_upper = path.to_uppercase();
    for &letter in drive_letters {
        let prefix = format!("{}:\\", letter).to_uppercase();
        if path_upper.starts_with(&prefix) {
            return true;
        }
    }
    false
}

fn run_eye(drive_filter: &[char]) -> Result<Vec<SuspectMapping>> {
    let processes = monitor_proc::get_process_map();
    let conns_by_pid = monitor_net::get_connections_by_pid()?;

    let mut suspects = Vec::new();

    for proc_ in &processes {
        if !path_matches_drives(&proc_.path, drive_filter) {
            continue;
        }

        let Some(conns) = conns_by_pid.get(&proc_.pid) else {
            continue;
        };

        // 过滤：只关心有外连的进程
        let remote_conns: Vec<_> = conns
            .iter()
            .filter(|c| !c.remote_addr.is_empty() && c.remote_addr != "*")
            .cloned()
            .collect();

        if remote_conns.is_empty() {
            continue;
        }

        if is_whitelisted(proc_) {
            continue;
        }

        if is_suspicious(proc_, &remote_conns) {
            suspects.push(SuspectMapping {
                process: proc_.clone(),
                connections: remote_conns,
            });
        }
    }

    Ok(suspects)
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Boaz Daemon - The Eye of Boaz")]
struct Args {
    /// 仅扫描一次后退出（默认：常驻循环监控）
    #[arg(long)]
    once: bool,

    /// 扫描间隔（秒），默认 10
    #[arg(short, long, default_value_t = DEFAULT_INTERVAL_SECS)]
    interval: u64,

    /// 仅监控指定盘符上的进程（如 D 或 C,D）；不指定则全盘
    #[arg(short, long)]
    drive: Option<String>,
}

fn report_suspects(suspects: &[SuspectMapping]) {
    if suspects.is_empty() {
        info!("未发现可疑进程");
        return;
    }
    warn!("发现 {} 个嫌疑进程:", suspects.len());
    for s in suspects {
        let conns = s.connections
            .iter()
            .map(|c| format!("{}:{}", c.remote_addr, c.remote_port))
            .collect::<Vec<_>>()
            .join(", ");
        // [THREAT] ASCII 标记，避免子进程 stderr 编码导致前端无法解析
        warn!("  [THREAT] PID={} {} -> {}", s.process.pid, s.process.path, conns);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse()?),
        )
        .init();

    let args = Args::parse();

    let drive_filter: Vec<char> = args
        .drive
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().chars().next())
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_uppercase().next().unwrap_or(c))
        .collect();
    let scope = if drive_filter.is_empty() {
        "全盘".to_string()
    } else {
        drive_filter
            .iter()
            .map(|c| format!("{}:\\", c))
            .collect::<Vec<_>>()
            .join(", ")
    };

    info!("Boaz Daemon - The Eye of Boaz 启动（范围: {}）", scope);

    if args.once {
        let suspects = run_eye(&drive_filter)?;
        report_suspects(&suspects);
        return Ok(());
    }

    info!("常驻监控模式，间隔 {} 秒（Ctrl+C 退出）", args.interval);
    let interval = Duration::from_secs(args.interval);

    loop {
        match run_eye(&drive_filter) {
            Ok(suspects) => report_suspects(&suspects),
            Err(e) => warn!("扫描异常: {}", e),
        }
        tokio::time::sleep(interval).await;
    }
}
