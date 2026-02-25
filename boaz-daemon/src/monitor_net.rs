//! 网络外连嗅探：实时获取 TCP/UDP 连接表 (类似 netstat -ano)
//! Samuel, 2026-02-24

use boaz_shared::NetworkConnection;
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};
use std::collections::HashMap;

/// 获取当前系统所有 TCP/UDP 连接，按 PID 分组
/// 返回: HashMap<PID, Vec<NetworkConnection>>
pub fn get_connections_by_pid() -> anyhow::Result<HashMap<u32, Vec<NetworkConnection>>> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;
    let sockets_info = get_sockets_info(af_flags, proto_flags)?;

    let mut by_pid: HashMap<u32, Vec<NetworkConnection>> = HashMap::new();

    for si in sockets_info {
        let pids: Vec<u32> = si.associated_pids.iter().copied().collect();
        if pids.is_empty() {
            continue;
        }

        let conn = match &si.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp) => NetworkConnection {
                local_addr: tcp.local_addr.to_string(),
                remote_addr: tcp.remote_addr.to_string(),
                remote_port: tcp.remote_port,
                protocol: "TCP".to_string(),
                state: format!("{:?}", tcp.state),
            },
            ProtocolSocketInfo::Udp(udp) => NetworkConnection {
                local_addr: udp.local_addr.to_string(),
                remote_addr: "*".to_string(),
                remote_port: 0,
                protocol: "UDP".to_string(),
                state: "UDP".to_string(),
            },
        };

        for pid in pids {
            by_pid.entry(pid).or_default().push(conn.clone());
        }
    }

    Ok(by_pid)
}
