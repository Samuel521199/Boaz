//! Boaz Daemon 与 UI 通信的 IPC 数据结构
//! Samuel, 2026-02-24

use serde::{Deserialize, Serialize};

/// 嫌疑进程信息（供 UI 展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspectProcess {
    pub pid: u32,
    pub name: String,
    pub path: String,
    pub parent_pid: Option<u32>,
}

/// 网络连接信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnection {
    pub local_addr: String,
    pub remote_addr: String,
    pub remote_port: u16,
    pub protocol: String,
    pub state: String,
}

/// 内鬼映射：进程 + 其外连
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspectMapping {
    pub process: SuspectProcess,
    pub connections: Vec<NetworkConnection>,
}

/// 告警消息（Daemon → UI）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPayload {
    pub suspect: SuspectMapping,
    pub severity: u8, // 0-10
    pub ai_reasoning: Option<String>,
    pub suggested_action: String, // KILL / IGNORE
}
