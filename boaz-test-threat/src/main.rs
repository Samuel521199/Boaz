//! 模拟威胁测试程序 - 用于验证 Boaz 监控能否捕获
//!
//! 行为：在 AppData\Roaming 下运行，建立到 127.0.0.1:4444 的外连，
//! 符合 daemon 的「内鬼」判定条件（AppData + 可疑端口）。
//!
//! 用法：
//!   1. 先启动 Boaz 监控（boaz-ui 中点击「启动监控」）
//!   2. 运行: cargo run -p boaz-test-threat
//!   3. 等待约 10 秒，监控应弹出威胁告警
//!
//! Samuel, 2026-02-23

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const SUSPICIOUS_PORT: u16 = 4444;
const HOLD_SECS: u64 = 90;

fn appdata_roaming() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_child = args.len() > 1 && args[1] == "--child";

    if is_child {
        run_as_suspect_client();
    } else {
        run_as_launcher();
    }
}

/// 启动器：复制自身到 AppData，起服务器，再启动子进程
fn run_as_launcher() {
    let exe = std::env::current_exe().expect("无法获取当前可执行文件路径");
    let target_dir = appdata_roaming().join("boaz-test");
    let target_exe = target_dir.join("fake-threat.exe");

    println!("[*] Boaz 威胁模拟测试");
    println!("[*] 目标路径: {}", target_exe.display());

    std::fs::create_dir_all(&target_dir).expect("创建目录失败");
    std::fs::copy(&exe, &target_exe).expect("复制可执行文件失败");

    // 后台启动 TCP 服务器（4444 端口）
    let _server_handle = thread::spawn(|| {
        let listener = TcpListener::bind(("127.0.0.1", SUSPICIOUS_PORT)).expect("绑定端口失败");
        println!("[*] 监听 127.0.0.1:{} 等待连接...", SUSPICIOUS_PORT);
        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                let _ = s.write_all(b"ok");
                let _ = s.flush();
                // 保持连接不关闭，让子进程的 netstat 能看见
                thread::sleep(Duration::from_secs(HOLD_SECS));
            }
        }
    });

    thread::sleep(Duration::from_millis(500));

    // 启动子进程（从 AppData 运行，连接 4444）
    let mut child = Command::new(&target_exe)
        .arg("--child")
        .current_dir(&target_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("启动子进程失败");

    println!("[*] 已启动模拟威胁进程 PID={}", child.id());
    println!("[*] 请确保 Boaz 监控已运行，约 10 秒内应弹出告警");
    println!("[*] 按 Ctrl+C 可提前结束");

    let _ = child.wait();
    println!("[*] 测试结束");
}

/// 子进程：作为「可疑进程」连接 4444 并保持
fn run_as_suspect_client() {
    println!("[*] 模拟威胁进程启动，连接 127.0.0.1:{}...", SUSPICIOUS_PORT);

    match TcpStream::connect(("127.0.0.1", SUSPICIOUS_PORT)) {
        Ok(mut stream) => {
            let mut buf = [0u8; 2];
            let _ = stream.read(&mut buf);
            println!("[*] 已连接，保持 {} 秒（等待监控扫描）", HOLD_SECS);
            thread::sleep(Duration::from_secs(HOLD_SECS));
        }
        Err(e) => {
            eprintln!("[!] 连接失败: {}（请确保先运行不带 --child 的启动器）", e);
            std::process::exit(1);
        }
    }
}
