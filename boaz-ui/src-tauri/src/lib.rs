// 调 boaz-core 跑审计，把日志推到界面，最后把报告 JSON 给前端
// Samuel, 2026-02-23

use std::io::{BufRead, Read, Write};
use std::sync::Mutex;

/// 控制台输出句柄：直接写入 CONOUT$，绕过 Rust 已初始化的 stderr（GUI 进程下 stderr 可能无效）
#[cfg(windows)]
static CONSOLE_OUT: Mutex<Option<std::fs::File>> = Mutex::new(None);

#[cfg(windows)]
fn init_console() {
    use std::os::windows::io::FromRawHandle;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Foundation::GENERIC_WRITE;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Console::{AllocConsole, SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};

    if std::env::var("BOAZ_HIDE_CONSOLE").map(|v| v == "1").unwrap_or(false) {
        return;
    }
    unsafe {
        if AllocConsole().is_ok() {
            let name: Vec<u16> = "CONOUT$".encode_utf16().chain(std::iter::once(0)).collect();
            if let Ok(h) = CreateFileW(
                PCWSTR(name.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            ) {
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, h);
                let _ = SetStdHandle(STD_ERROR_HANDLE, h);
                let raw = std::mem::transmute::<_, std::os::windows::io::RawHandle>(h);
                let file = std::fs::File::from_raw_handle(raw);
                if let Ok(mut guard) = CONSOLE_OUT.lock() {
                    *guard = Some(file);
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn init_console() {}

/// 调试日志：直接写入 CONOUT$，确保控制台有输出
macro_rules! dbg_log {
    ($($arg:tt)*) => {{
        let msg = format!("[boaz-ui] {}\n", format!($($arg)*));
        #[cfg(windows)]
        {
            if let Ok(mut guard) = $crate::CONSOLE_OUT.lock() {
                if let Some(ref mut f) = *guard {
                    let _ = f.write_all(msg.as_bytes());
                    let _ = f.flush();
                }
            }
        }
        #[cfg(not(windows))]
        {
            eprintln!("{}", msg.trim());
        }
    }};
}

use std::path::PathBuf;
use tauri::Emitter;
use std::process::{Child, Command, Stdio};

#[derive(Default)]
struct AuditState {
    last_report: Mutex<Option<String>>,
}

/// 守护进程子进程句柄（用于停止监控）
struct DaemonState {
    child: Mutex<Option<Child>>,
}

/// 返回可扫描的盘符列表（Windows 下 C:\, D:\ 等），供下拉选择。
#[tauri::command]
fn get_drives() -> Vec<String> {
    dbg_log!("get_drives 被调用");
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        for c in b'A'..=b'Z' {
            let letter = c as char;
            let path = format!("{}:\\", letter);
            if std::path::Path::new(&path).exists() {
                out.push(path);
            }
        }
        dbg_log!("get_drives 返回 {} 个盘符: {:?}", out.len(), out);
        out
    }
    #[cfg(not(windows))]
    {
        let _ = ();
        Vec::new()
    }
}

/// 白名单文件路径（程序同目录）
fn whitelist_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("boaz-whitelist.json")))
        .unwrap_or_else(|| PathBuf::from("boaz-whitelist.json"))
}

/// 加入白名单：持久化到 boaz-whitelist.json，下次扫描时过滤
#[tauri::command]
fn add_to_whitelist(program_name: String, program_path: String) -> Result<(), String> {
    let path = whitelist_path();
    let mut entries: Vec<String> = if path.exists() {
        let s = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        Vec::new()
    };
    let name = program_name.trim().to_string();
    let path_str = program_path.trim().to_string();
    if !name.is_empty() && !entries.contains(&name) {
        entries.push(name);
    }
    if !path_str.is_empty() && path_str.len() > 10 && !entries.contains(&path_str) {
        entries.push(path_str);
    }
    let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// 获取白名单列表，供前端过滤显示
#[tauri::command]
fn get_whitelist() -> Result<Vec<String>, String> {
    let path = whitelist_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let s = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let entries: Vec<String> = serde_json::from_str(&s).unwrap_or_default();
    Ok(entries)
}

/// 处置项：删除自启动 / 停止服务 / 删除计划任务
#[tauri::command]
fn remediate_item(item_type: String, name: Option<String>, path: Option<String>) -> Result<String, String> {
    let t = item_type.to_lowercase();
    if t == "run_key" {
        let n = name.as_deref().filter(|s| !s.is_empty()).ok_or("自启动项缺少 name")?;
        #[cfg(windows)]
        {
            use winreg::enums::HKEY_LOCAL_MACHINE;
            use winreg::enums::HKEY_CURRENT_USER;
            use winreg::RegKey;
            let run_path = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
            let _ = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(run_path).and_then(|k| k.delete_value(n));
            let _ = RegKey::predef(HKEY_CURRENT_USER).open_subkey(run_path).and_then(|k| k.delete_value(n));
            Ok(format!("已删除自启动项: {}", n))
        }
        #[cfg(not(windows))]
        return Err("仅支持 Windows".to_string());
    } else if t == "service" {
        let n = name.as_deref().filter(|s| !s.is_empty()).ok_or("服务项缺少 name")?;
        #[cfg(windows)]
        {
            let out = std::process::Command::new("sc")
                .args(["stop", n])
                .output()
                .map_err(|e| e.to_string())?;
            let _ = std::process::Command::new("sc")
                .args(["config", n, "start= disabled"])
                .output();
            if out.status.success() {
                Ok(format!("已停止并禁用服务: {}", n))
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(format!("停止服务失败: {}", stderr.trim()))
            }
        }
        #[cfg(not(windows))]
        return Err("仅支持 Windows".to_string());
    } else if t == "task" {
        let p = path.as_deref().filter(|s| !s.is_empty()).ok_or("计划任务缺少 path")?;
        #[cfg(windows)]
        {
            let out = std::process::Command::new("schtasks")
                .args(["/delete", "/tn", p, "/f"])
                .output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(format!("已删除计划任务: {}", p))
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(format!("删除任务失败: {}", stderr.trim()))
            }
        }
        #[cfg(not(windows))]
        return Err("仅支持 Windows".to_string());
    } else {
        Err(format!("不支持的处置类型: {}", item_type))
    }
}

/// 最小化到托盘：隐藏主窗口，点击托盘图标可恢复
#[tauri::command]
fn minimize_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 显示并聚焦主窗口（威胁弹窗时唤起）
#[tauri::command]
fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}

/// 按 PID 终止进程（绞杀）
#[tauri::command]
fn kill_process_by_pid(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()
            .map_err(|e| format!("执行 taskkill 失败: {}", e))?;
        if out.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!("绞杀失败: {}", stderr.trim()))
        }
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        Err("仅支持 Windows".to_string())
    }
}

/// 程序所在目录（U 盘即 U 盘路径），用作默认日志目录。
#[tauri::command]
fn get_app_dir() -> Result<String, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "无法获取程序目录".to_string())?
        .to_path_buf()
        .into_os_string()
        .into_string()
        .map_err(|_| "路径含非法字符".to_string())
}

/// 把日志写入指定目录，文件名 boaz-log-{时间}.txt；目录内只保留最近 10 个日志，删最旧的。
#[tauri::command]
fn save_log_to_disk(content: String, custom_dir: Option<String>) -> Result<String, String> {
    use std::fs;
    use std::io::Write;
    let dir: PathBuf = match custom_dir.filter(|s| !s.trim().is_empty()) {
        Some(s) => PathBuf::from(s.trim()),
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or_else(|| "无法获取程序目录".to_string())?
            .to_path_buf(),
    };
    if !dir.is_dir() {
        return Err("所选路径不是目录".to_string());
    }
    let name = format!(
        "boaz-log-{}.txt",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let path = dir.join(&name);
    let mut f = fs::File::create(&path).map_err(|e| e.to_string())?;
    f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    // 只保留最近 10 个 boaz-log-*.txt
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.starts_with("boaz-log-") && n.ends_with(".txt"))
        })
        .collect();
    entries.sort_by(|a, b| {
        let at = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let bt = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        at.cmp(&bt)
    });
    let keep = 10;
    for e in entries.into_iter().rev().skip(keep) {
        let _ = fs::remove_file(e.path());
    }
    path.into_os_string()
        .into_string()
        .map_err(|_| "路径含非法字符".to_string())
}

/// 跑一次审计：起 boaz-core，stderr 按行 emit 到前端，最后把 stdout 里那坨 JSON 返回。
/// core_path 不填就用同目录的 boaz-core 或环境变量 BOAZ_CORE_PATH。
#[tauri::command]
async fn run_audit(
    app: tauri::AppHandle,
    mount_point: String,
    remove: bool,
    rules_path: Option<String>,
    core_path_override: Option<String>,
    state: tauri::State<'_, AuditState>,
) -> Result<String, String> {
    dbg_log!("run_audit 被调用: mount_point={}, remove={}", mount_point, remove);
    let core_path: PathBuf = core_path_override
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("BOAZ_CORE_PATH").ok().map(PathBuf::from))
        .or_else(|| same_dir_as_exe("boaz-core"))
        .unwrap_or_else(|| PathBuf::from("boaz-core"));

    let mut cmd = Command::new(&core_path);
    cmd.arg("--mount-point").arg(&mount_point)
        .stderr(Stdio::piped())
        .stdout(Stdio::piped());
    if remove {
        cmd.arg("--remove");
    }
    if let Some(ref r) = rules_path {
        if !r.is_empty() {
            cmd.arg("--rules").arg(r);
        }
    }

    dbg_log!("启动 boaz-core: {}", core_path.display());
    let mut child = cmd.spawn().map_err(|e| {
        let msg = format!("启动 boaz-core 失败: {}", e);
        dbg_log!("{}", msg);
        msg
    })?;
    // 立即向前端推送一条日志，让用户知道扫描已启动
    let _ = app.emit("audit-log", &format!("[*] 已启动 boaz-core，正在扫描 {}，请稍候…", mount_point));
    let stderr = child.stderr.take().ok_or_else(|| {
        let msg = "无法接管 stderr";
        dbg_log!("{}", msg);
        msg.to_string()
    })?;
    let mut stdout_handle = child.stdout.take().ok_or_else(|| {
        let msg = "无法接管 stdout";
        dbg_log!("{}", msg);
        msg.to_string()
    })?;

    // 在后台线程中逐行读取 stderr：推送到前端 + 收集以便失败时一并返回
    let app_clone = app.clone();
    let stderr_handle = std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        let mut collected = Vec::new();
        for line in reader.lines() {
            if let Ok(l) = line {
                let _ = app_clone.emit("audit-log", &l);
                collected.push(l);
            }
        }
        collected
    });

    // 读取 stdout 并等待进程结束
    let mut stdout_vec = Vec::new();
    let _ = stdout_handle.read_to_end(&mut stdout_vec);
    let status = child.wait().map_err(|e| format!("等待进程失败: {}", e))?;

    let stderr_lines = stderr_handle.join().unwrap_or_default();
    let stdout = String::from_utf8_lossy(&stdout_vec);

    if !status.success() {
        let base = format!("boaz-core 退出异常: {}", status);
        let msg = if stderr_lines.is_empty() {
            base
        } else {
            format!("{}\n\n调试输出:\n{}", base, stderr_lines.join("\n"))
        };
        dbg_log!("{}", msg);
        return Err(msg);
    }

    dbg_log!("boaz-core 扫描完成，解析报告");
    let report = extract_json_from_stdout(&stdout);
    if let Some(ref r) = report {
        *state.last_report.lock().map_err(|e| e.to_string())? = Some(r.clone());
    }
    report.ok_or_else(|| {
        let msg = "未解析到有效 JSON 报告".to_string();
        dbg_log!("{}", msg);
        msg
    })
}

/// 解析 daemon 输出的 [THREAT] 行，提取 PID、路径、连接信息
/// 格式: "  [THREAT] PID=1234 C:\\path\\to\\exe.exe -> 1.2.3.4:4444, 5.6.7.8:5555"
fn parse_threat_line(line: &str) -> Option<serde_json::Value> {
    let rest = line.find("[THREAT]").or_else(|| line.find("[内鬼]"))?;
    let after_marker = &line[rest..];
    let pid_start = after_marker.find("PID=")?;
    let pid_section = &after_marker[pid_start..];
    let arrow = pid_section.find(" -> ")?;
    let left = pid_section[..arrow].trim();
    let connections = pid_section[arrow + 4..].trim().to_string();
    let pid_part = left.strip_prefix("PID=")?.trim();
    let mut parts = pid_part.splitn(2, ' ');
    let pid_str = parts.next()?;
    let path = parts.next()?.trim();
    let pid: u32 = pid_str.parse().ok()?;
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知")
        .to_string();
    Some(serde_json::json!({
        "pid": pid,
        "name": name,
        "path": path,
        "connections": connections
    }))
}

/// 启动 The Eye of Boaz 守护进程监控（后台常驻，输出通过 daemon-log 事件推送）
#[tauri::command]
async fn run_daemon_monitor(
    app: tauri::AppHandle,
    drive_filter: Option<String>,
    interval_secs: Option<u64>,
    state: tauri::State<'_, DaemonState>,
) -> Result<(), String> {
    dbg_log!("run_daemon_monitor 被调用: drive_filter={:?}, interval={:?}", drive_filter, interval_secs);
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Err("监控已在运行，请先停止".to_string());
    }

    let daemon_path: PathBuf = std::env::var("BOAZ_DAEMON_PATH")
        .ok()
        .map(PathBuf::from)
        .or_else(|| same_dir_as_exe("boaz-daemon"))
        .unwrap_or_else(|| PathBuf::from("boaz-daemon"));

    if !daemon_path.exists() {
        return Err(format!(
            "未找到 boaz-daemon，请确保与程序同目录或设置 BOAZ_DAEMON_PATH。路径: {}",
            daemon_path.display()
        ));
    }

    let mut cmd = Command::new(&daemon_path);
    cmd.stderr(Stdio::piped()).stdout(Stdio::piped());

    if let Some(ref s) = drive_filter {
        if !s.trim().is_empty() {
            cmd.arg("--drive").arg(s.trim());
        }
    }
    if let Some(n) = interval_secs {
        if n > 0 {
            cmd.arg("--interval").arg(n.to_string());
        }
    }

    dbg_log!("启动 boaz-daemon: {}", daemon_path.display());
    let mut child = cmd.spawn().map_err(|e| format!("启动 boaz-daemon 失败: {}", e))?;

    let stderr = child.stderr.take().ok_or_else(|| "无法接管 stderr".to_string())?;
    let app_clone = app.clone();
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(l) = line {
                let _ = app_clone.emit("daemon-log", &l);
                // 解析 [内鬼] 格式，发出结构化威胁事件供弹窗使用
                if l.contains("[THREAT]") || l.contains("[内鬼]") {
                    if let Some(payload) = parse_threat_line(&l) {
                        let _ = app_clone.emit("daemon-threat", &payload);
                    }
                }
            }
        }
    });

    *guard = Some(child);
    dbg_log!("boaz-daemon 已启动");
    Ok(())
}

/// 调用 AI 研判：根据 provider 和 api_key 请求 Gemini/OpenAI/Grok，返回威胁分析结论
/// user_context 可选：用户补充说明（如「我是开发人员，这是我在用的工具/插件」），用于重新研判
#[tauri::command]
async fn invoke_ai_judgment(
    program_name: String,
    program_path: String,
    risk_type: String,
    provider: String,
    api_key: String,
    user_context: Option<String>,
) -> Result<String, String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("请先在「AI 研判配置」中填写 API Key".to_string());
    }
    let base = format!(
        "你是一名 Windows 系统安全专家。请对以下可疑程序进行威胁研判，用大白话给出结论（2-4 句话），并明确建议：安全可信可加入白名单，或建议立即隔离/绞杀。\n\n程序名：{}\n路径/类型：{}\n风险分类：{}\n",
        program_name, program_path, risk_type
    );
    let prompt = match user_context.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Some(ctx) => format!(
            "{}【用户补充说明】{}\n\n请结合用户说明重新研判，若用户确认为自用工具/开发插件等可信软件，可建议加入白名单。请直接给出研判结论。",
            base, ctx
        ),
        None => format!("{}\n请直接给出研判结论，不要重复问题。", base),
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let provider_lower = provider.to_lowercase();
    if provider_lower == "gemini" {
        // 按官方文档 https://ai.google.dev/gemini-api/docs/gemini-3 使用 x-goog-api-key 头
        // 模型顺序：优先有免费配额的 Flash，3.1 Pro 可能无免费层
        let models = [
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
            "gemini-2.0-flash",
            "gemini-1.5-flash",
            "gemini-3.1-pro-preview",
            "gemini-pro",
        ];
        let mut last_err = String::new();
        for model in models {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                model
            );
            let mut gen_config = serde_json::json!({"maxOutputTokens": 256});
            if model.starts_with("gemini-3") {
                gen_config["thinkingConfig"] = serde_json::json!({"thinkingLevel": "low"});
            } else {
                gen_config["temperature"] = serde_json::json!(0.3);
            }
            let body = serde_json::json!({
                "contents": [{"parts": [{"text": prompt}]}],
                "generationConfig": gen_config
            });
            let res = match client
                .post(&url)
                .header("x-goog-api-key", api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                {
                Ok(r) => r,
                Err(e) => {
                    last_err = format!("Gemini 请求失败: {}", e);
                    continue;
                }
            };
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            if !status.is_success() {
                let code = status.as_u16();
                if code == 404 {
                    last_err = format!("模型 {} 不可用", model);
                    continue;
                }
                if code == 429 {
                    last_err = "API 配额已用尽，请稍后重试或检查计费方案。详见：https://ai.google.dev/gemini-api/docs/rate-limits".to_string();
                    continue;
                }
                last_err = format!("Gemini API 错误 ({}): {}", status, text);
                break;
            }
            let json: serde_json::Value = match serde_json::from_str(&text) {
                Ok(j) => j,
                Err(e) => {
                    last_err = format!("解析 Gemini 响应失败: {}", e);
                    continue;
                }
            };
            let content = json
                .get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.get(0))
                .and_then(|p| p.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim();
            if !content.is_empty() {
                return Ok(format!("AI 研判：{}", content));
            }
            last_err = "Gemini 未返回有效内容".to_string();
        }
        Err(if last_err.is_empty() {
            "Gemini 未返回有效内容".to_string()
        } else {
            last_err
        })
    } else if provider_lower == "openai" || provider_lower == "chatgpt" {
        let url = "https://api.openai.com/v1/chat/completions";
        let body = serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 256
        });
        let res = client.post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send().await
            .map_err(|e| format!("OpenAI 请求失败: {}", e))?;
        let status = res.status();
        let text = res.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
        if !status.is_success() {
            return Err(format!("OpenAI API 错误 ({}): {}", status, text));
        }
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("解析 OpenAI 响应失败: {}", e))?;
        let content = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim();
        if content.is_empty() {
            return Err("OpenAI 未返回有效内容".to_string());
        }
        Ok(format!("AI 研判：{}", content))
    } else if provider_lower == "qwen" || provider_lower == "aliyun" || provider_lower == "dashscope" {
        // 阿里云百炼 DashScope，OpenAI 兼容接口 https://help.aliyun.com/zh/model-studio
        let url = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
        let body = serde_json::json!({
            "model": "qwen-turbo",
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 256
        });
        let res = client.post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await
            .map_err(|e| format!("Qwen 请求失败: {}", e))?;
        let status = res.status();
        let text = res.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
        if !status.is_success() {
            return Err(format!("Qwen API 错误 ({}): {}", status, text));
        }
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("解析 Qwen 响应失败: {}", e))?;
        let content = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim();
        if content.is_empty() {
            return Err("Qwen 未返回有效内容".to_string());
        }
        Ok(format!("AI 研判：{}", content))
    } else if provider_lower == "grok" || provider_lower == "xai" {
        let url = "https://api.x.ai/v1/chat/completions";
        let body = serde_json::json!({
            "model": "grok-2-1212",
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 256
        });
        let res = client.post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send().await
            .map_err(|e| format!("Grok 请求失败: {}", e))?;
        let status = res.status();
        let text = res.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
        if !status.is_success() {
            return Err(format!("Grok API 错误 ({}): {}", status, text));
        }
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("解析 Grok 响应失败: {}", e))?;
        let content = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim();
        if content.is_empty() {
            return Err("Grok 未返回有效内容".to_string());
        }
        Ok(format!("AI 研判：{}", content))
    } else {
        Err(format!("不支持的 AI 提供商: {}，请选择 gemini、openai、qwen 或 grok", provider))
    }
}

/// 停止守护进程监控
#[tauri::command]
async fn stop_daemon_monitor(state: tauri::State<'_, DaemonState>) -> Result<(), String> {
    dbg_log!("stop_daemon_monitor 被调用");
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

/// 与当前可执行文件同目录下的程序（U 盘/同一文件夹拷贝即用）
/// 开发模式下若同目录没有，则尝试 workspace target/debug
fn same_dir_as_exe(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    #[cfg(windows)]
    let path = dir.join(format!("{}.exe", name));
    #[cfg(not(windows))]
    let path = dir.join(name);
    if path.exists() {
        return Some(path);
    }
    // 开发模式回退：cargo tauri dev 时 exe 可能在 target/debug，boaz-daemon 也在同一 target
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let manifest_path = PathBuf::from(&manifest_dir);
    let workspace_root = manifest_path.parent()?.parent()?;
    #[cfg(windows)]
    let fallback = workspace_root.join("target").join("debug").join(format!("{}.exe", name));
    #[cfg(not(windows))]
    let fallback = workspace_root.join("target").join("debug").join(name);
    if fallback.exists() {
        Some(fallback)
    } else {
        None
    }
}

/// 从混合了日志的 stdout 中取最后一个完整 JSON 对象。
/// 正确跳过字符串内的 { }，避免路径等含 } 时被错误截断导致 "Unterminated string"。
fn extract_json_from_stdout(stdout: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut start = None;
    let mut in_string = false;
    let mut escape_next = false;
    let mut quote_char = '\0';
    for (pos, c) in stdout.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if in_string {
            if c == '\\' {
                escape_next = true;
            } else if c == quote_char {
                in_string = false;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            in_string = true;
            quote_char = c;
            continue;
        }
        if c == '{' {
            if depth == 0 {
                start = Some(pos);
            }
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    return Some(stdout[s..pos + c.len_utf8()].to_string());
                }
            }
        }
    }
    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_console();
    dbg_log!("Boaz UI 启动");
    tauri::Builder::default()
        .setup(|app| {
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
            use tauri::Manager;
            let icon = app.default_window_icon().cloned().or_else(|| {
                tauri::image::Image::from_bytes(include_bytes!("../icons/icon.ico"))
                    .ok()
                    .map(Into::into)
            });
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            if let Some(icon) = icon {
                let _tray = TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Boaz 零信任安全")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    if event.id.as_ref() == "show" {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    } else if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click { button, button_state, .. } = event {
                        if button == MouseButton::Left && button_state == MouseButtonState::Up {
                            if let Some(w) = tray.app_handle().get_webview_window("main") {
                                let _ = if w.is_visible().unwrap_or(true) {
                                    w.hide()
                                } else {
                                    w.show()
                                };
                            }
                        }
                    }
                })
                .build(app)?;
            }
            Ok(())
        })
        .manage(AuditState::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().ok();
                api.prevent_close();
            }
        })
        .manage(DaemonState {
            child: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_drives,
            get_app_dir,
            save_log_to_disk,
            run_audit,
            run_daemon_monitor,
            stop_daemon_monitor,
            invoke_ai_judgment,
            add_to_whitelist,
            get_whitelist,
            remediate_item,
            minimize_to_tray,
            show_main_window,
            kill_process_by_pid
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
