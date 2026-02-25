use anyhow::{Context, Result};
use nt_hive::{Hive, KeyNode};
use serde::Serialize;
use std::path::Path;

#[cfg(windows)]
use winreg::RegKey;

// 木马/后门爱放的可写目录，用来筛「可疑」Run/服务
const RISKY_PATH_SEGMENTS: &[&str] = &[
    "\\temp\\", "\\tmp\\", "\\appdata\\", "\\programdata\\",
    "\\local\\temp\\", "\\roaming\\", "\\local low\\", "$recycle.bin",
];

/// 从 Run 键值里抠出可执行路径（去参数、去引号，带空格的 Program Files 也能认）
fn extract_executable_path(command: &str) -> String {
    let s = command.trim();
    let path = if s.starts_with('"') {
        s[1..].find('"').map(|i| &s[1..1 + i]).unwrap_or(s).to_string()
    } else {
        const EXTS: &[&str] = &[".exe", ".com", ".bat", ".cmd", ".dll", ".scr"];
        let lower = s.to_lowercase();
        // 取最右边一个扩展名结尾，防止路径里带 .exe 被截错
        let end = EXTS
            .iter()
            .filter_map(|ext| lower.rfind(ext).map(|i| i + ext.len()))
            .max();
        if let Some(end) = end {
            s[..end].trim_end().to_string()
        } else {
            s.split_whitespace().next().unwrap_or(s).to_string()
        }
    };
    path.replace('/', "\\").to_lowercase()
}

/// 路径在可写/非常规目录才算危险，系统目录不算
pub fn is_run_key_dangerous(command_path: &str) -> bool {
    let path = extract_executable_path(command_path);
    if path.is_empty() {
        return true;
    }
    let in_trusted = path.contains("\\windows\\system32\\")
        || path.contains("\\program files\\")
        || path.contains("(x86)")  // Program Files (x86)
        || path.contains("%windir%")
        || path.contains("%systemroot%");
    let in_risky = RISKY_PATH_SEGMENTS
        .iter()
        .any(|s| path.contains(&s.to_lowercase()));
    in_risky || !in_trusted
}

#[derive(Debug, Serialize)]
pub struct SuspiciousRunKey {
    pub name: String,
    pub command_path: String,
}

/// 服务映像路径在可写或非系统目录就标风险
/// 扩展可信路径：%SystemRoot%、System32、DriverStore、Program Files 等
pub fn is_service_path_risky(image_path: &str) -> bool {
    let path = image_path.trim().replace('/', "\\").to_lowercase();
    if path.is_empty() {
        return false;
    }
    let in_trusted = path.contains("\\windows\\system32\\")
        || path.contains("systemroot\\system32\\")
        || path.contains("%systemroot%")
        || path.contains("%windir%")
        || path.contains("system32\\")      // System32\drivers\*.sys 等
        || path.contains("\\system32\\")
        || path.contains("syswow64\\")
        || path.contains("driverstore\\filerepository")  // 微软驱动库
        || path.contains("\\program files\\")
        || path.contains("\\program files (x86)\\")
        || path.contains("%programfiles%")
        || path.contains("programdata\\microsoft\\windows defender");  // Defender
    let in_risky = RISKY_PATH_SEGMENTS
        .iter()
        .any(|s| path.contains(&s.to_lowercase()));
    in_risky || !in_trusted
}

#[derive(Clone, Debug, Serialize)]
pub struct ServiceEntry {
    pub name: String,
    pub image_path: String,
    /// 路径不在系统目录，可能被动手脚
    pub risky: bool,
}

/// 任务不在 \Microsoft\Windows* 下就标风险（根下或杂牌路径常被滥用）
pub(crate) fn is_task_path_risky(task_path: &str) -> bool {
    let p = task_path.trim().replace('/', "\\").to_lowercase();
    if p.is_empty() {
        return true;
    }
    let trusted = p.starts_with("\\microsoft\\windows\\") || p.starts_with("\\microsoft\\windows nt\\");
    !trusted
}

#[derive(Clone, Debug, Serialize)]
pub struct ScheduledTaskEntry {
    pub path: String,
    pub guid: String,
    /// 非系统任务路径，可能被滥用
    pub risky: bool,
}

/// Windows 下 Hive 被锁时，用注册表 API 读 Run 键（在线模式）
#[cfg(windows)]
fn hunt_startup_keys_live() -> Result<Vec<SuspiciousRunKey>> {
    let hklm = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let run_key = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run")
        .with_context(|| "无法打开 Run 注册表键（在线回退）")?;
    let mut suspicious = Vec::new();
    for (name, _) in run_key.enum_values().filter_map(|e| e.ok()) {
        if let Ok(cmd) = run_key.get_value::<String, _>(&name) {
            if is_run_key_dangerous(&cmd) {
                suspicious.push(SuspiciousRunKey { name, command_path: cmd });
            }
        }
    }
    Ok(suspicious)
}

/// 离线读 SOFTWARE Hive，把 Run 里「可疑」的拎出来；Hive 被锁时 Windows 下回退到注册表 API
pub fn hunt_startup_keys(mount_point: &Path) -> Result<Vec<SuspiciousRunKey>> {
    let hive_path = mount_point.join("Windows/System32/config/SOFTWARE");

    let bytes = match std::fs::read(&hive_path) {
        Ok(b) => b,
        Err(e) => {
            #[cfg(windows)]
            {
                let use_live = e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.kind() == std::io::ErrorKind::Other
                    || e.raw_os_error().map_or(false, |c| c == 32 || c == 5);
                if use_live {
                    eprintln!("[*] Hive 文件被锁定，改用注册表 API 读取（在线模式）");
                    return hunt_startup_keys_live();
                }
            }
            let hint = if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "该路径下未找到 Windows 安装（无 {}）。请选择已安装 Windows 的系统盘（如 C:\\），或从 PE/Linux Live 挂载目标盘后再扫描。",
                    hive_path.display()
                )
            } else {
                format!(
                    "无法读取: {}。若当前在目标系统上运行，Hive 会被锁定，请从 PE 或 Linux Live 挂载后再扫描。",
                    hive_path.display()
                )
            };
            return Err(anyhow::Error::from(e)).context(hint);
        }
    };

    let hive = Hive::<&[u8]>::new(bytes.as_slice())
        .with_context(|| "解析 Hive 二进制结构失败，可能文件已损坏或被底层篡改")?;

    let root = hive
        .root_key_node()
        .with_context(|| "获取 Hive 根键失败")?;

    let run_key_path = "Microsoft\\Windows\\CurrentVersion\\Run";

    let mut suspicious_keys = Vec::new();

    // subpath 返回 Option<Result<KeyNode>>
    let run_key_opt = root.subpath(run_key_path);

    if let Some(run_key_result) = run_key_opt {
        let run_key: KeyNode<'_, &[u8]> = run_key_result
            .with_context(|| format!("解析 Run 键路径失败: {}", run_key_path))?;

        if let Some(values_result) = run_key.values() {
            let values = values_result.with_context(|| "遍历 Run 键值失败")?;

            for value_result in values {
                let value = match value_result {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let name_nt = match value.name() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let name = name_nt.to_string_lossy();

                if let Ok(cmd_string) = value.string_data() {
                    if is_run_key_dangerous(&cmd_string) {
                        suspicious_keys.push(SuspiciousRunKey {
                            name,
                            command_path: cmd_string,
                        });
                    }
                }
            }
        }
    } else {
        // 路径不存在，可能是极度精简系统或安全策略隐藏，仅记录不视为致命错误
        eprintln!(
            "未找到常规 Run 路径: {}，可能已被安全策略隐藏或系统精简",
            run_key_path
        );
    }

    Ok(suspicious_keys)
}

/// Windows 下 Hive 被锁时，用注册表 API 读服务列表（在线模式）
#[cfg(windows)]
fn hunt_services_live() -> Result<Vec<ServiceEntry>> {
    let hklm = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let services = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Services")
        .with_context(|| "无法打开 Services 注册表键（在线回退）")?;
    let mut list = Vec::new();
    for name in services.enum_keys().filter_map(|e| e.ok()) {
        if let Ok(svc) = services.open_subkey(&name) {
            let image_path = svc.get_value::<String, _>("ImagePath").unwrap_or_default();
            if !image_path.is_empty() {
                list.push(ServiceEntry {
                    name,
                    image_path: image_path.clone(),
                    risky: is_service_path_risky(&image_path),
                });
            }
        }
    }
    Ok(list)
}

/// 解析离线 SYSTEM Hive，提取 ControlSet001\\Services 下的服务及 ImagePath（优先尝试 ControlSet001）
pub fn hunt_services(mount_point: &Path) -> Result<Vec<ServiceEntry>> {
    let hive_path = mount_point.join("Windows/System32/config/SYSTEM");
    let bytes = match std::fs::read(&hive_path) {
        Ok(b) => b,
        Err(e) => {
            #[cfg(windows)]
            {
                let use_live = e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.kind() == std::io::ErrorKind::Other
                    || e.raw_os_error().map_or(false, |c| c == 32 || c == 5);
                if use_live {
                    eprintln!("[*] SYSTEM Hive 被锁定，改用注册表 API 读取服务（在线模式）");
                    return hunt_services_live();
                }
            }
            let hint = if e.kind() == std::io::ErrorKind::NotFound {
                format!("该路径下未找到 Windows 安装（无 {}）。请选择系统盘或从 PE/Linux Live 挂载。", hive_path.display())
            } else {
                format!("无法读取 SYSTEM Hive: {}", hive_path.display())
            };
            return Err(anyhow::Error::from(e)).context(hint);
        }
    };
    let hive = Hive::<&[u8]>::new(bytes.as_slice())
        .with_context(|| "解析 SYSTEM Hive 失败")?;
    let root = hive.root_key_node().with_context(|| "获取 SYSTEM 根键失败")?;

    for control_set in ["ControlSet001", "ControlSet002"] {
        let services_path = format!("{}\\Services", control_set);
        let Some(services_result) = root.subpath(&services_path) else { continue };
        let services_key: KeyNode<'_, &[u8]> = match services_result {
            Ok(k) => k,
            Err(_) => continue,
        };
        let Some(subkeys_result) = services_key.subkeys() else { continue };
        let subkeys = match subkeys_result {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut list = Vec::new();
        for sk_result in subkeys {
            let sk: KeyNode<'_, &[u8]> = match sk_result {
                Ok(k) => k,
                Err(_) => continue,
            };
            let name_nt = match sk.name() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let name = name_nt.to_string_lossy();
            let image_path = sk
                .value("ImagePath")
                .and_then(|v| v.ok())
                .and_then(|v| v.string_data().ok())
                .unwrap_or_default();
            if !image_path.is_empty() {
                let risky = is_service_path_risky(&image_path);
                list.push(ServiceEntry {
                    name: name.to_string(),
                    image_path,
                    risky,
                });
            }
        }
        return Ok(list);
    }
    Ok(Vec::new())
}

/// Windows 下从 Tasks 目录枚举任务（在线模式，路径格式简化）
#[cfg(windows)]
fn hunt_scheduled_tasks_live(mount_point: &Path) -> Result<Vec<ScheduledTaskEntry>> {
    let tasks_dir = mount_point.join("Windows/System32/Tasks");
    let mut list = Vec::new();
    if !tasks_dir.is_dir() {
        return Ok(list);
    }
    for e in walkdir::WalkDir::new(&tasks_dir).min_depth(1).into_iter().filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_file() && p.extension().map_or(false, |e| e == "xml") {
            let rel = p.strip_prefix(&tasks_dir).unwrap_or(p);
            let path_str = format!("\\{}", rel.display().to_string().replace('/', "\\"));
            let risky = is_task_path_risky(&path_str);
            list.push(ScheduledTaskEntry {
                path: path_str,
                guid: String::new(),
                risky,
            });
        }
    }
    Ok(list)
}

/// 解析离线 SOFTWARE Hive，提取计划任务路径（TaskCache\\Tree 与 TaskCache\\Tasks）
pub fn hunt_scheduled_tasks(mount_point: &Path) -> Result<Vec<ScheduledTaskEntry>> {
    let hive_path = mount_point.join("Windows/System32/config/SOFTWARE");
    let bytes = match std::fs::read(&hive_path) {
        Ok(b) => b,
        Err(e) => {
            #[cfg(windows)]
            {
                let use_live = e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.kind() == std::io::ErrorKind::Other
                    || e.raw_os_error().map_or(false, |c| c == 32 || c == 5);
                if use_live {
                    eprintln!("[*] SOFTWARE Hive 被锁定，从 Tasks 目录枚举任务（在线模式）");
                    return hunt_scheduled_tasks_live(mount_point);
                }
            }
            let hint = if e.kind() == std::io::ErrorKind::NotFound {
                format!("该路径下未找到 Windows 安装（无 {}）。请选择系统盘或从 PE/Linux Live 挂载。", hive_path.display())
            } else {
                format!("无法读取 SOFTWARE Hive: {}", hive_path.display())
            };
            return Err(anyhow::Error::from(e)).context(hint);
        }
    };
    let hive = Hive::<&[u8]>::new(bytes.as_slice())
        .with_context(|| "解析 SOFTWARE Hive 失败")?;
    let root = hive.root_key_node().with_context(|| "获取根键失败")?;

    let task_cache = "Microsoft\\Windows NT\\CurrentVersion\\Schedule\\TaskCache";
    let tree_path = format!("{}\\Tree", task_cache);
    let tasks_path = format!("{}\\Tasks", task_cache);

    let Some(tree_result) = root.subpath(&tree_path) else { return Ok(Vec::new()) };
    let tree_key: KeyNode<'_, &[u8]> = tree_result.with_context(|| "打开 TaskCache\\Tree 失败")?;
    let Some(tasks_result) = root.subpath(&tasks_path) else { return Ok(Vec::new()) };
    let tasks_key: KeyNode<'_, &[u8]> = tasks_result.with_context(|| "打开 TaskCache\\Tasks 失败")?;

    let mut task_entries = Vec::new();
    collect_tree_tasks(&tree_key, "", &tasks_key, &mut task_entries);
    Ok(task_entries)
}

fn collect_tree_tasks<'h>(
    tree_key: &KeyNode<'h, &[u8]>,
    prefix: &str,
    tasks_key: &KeyNode<'h, &[u8]>,
    out: &mut Vec<ScheduledTaskEntry>,
) {
    let Some(subkeys_result) = tree_key.subkeys() else { return };
    let subkeys = match subkeys_result {
        Ok(s) => s,
        Err(_) => return,
    };
    for sk_result in subkeys {
        let sk: KeyNode<'h, &[u8]> = match sk_result {
            Ok(k) => k,
            Err(_) => continue,
        };
        let name_nt = match sk.name() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let name = name_nt.to_string_lossy();
        let path = if prefix.is_empty() {
            format!("\\{}", name)
        } else {
            format!("{}\\{}", prefix, name)
        };
        if let Some(id_result) = sk.value("Id") {
            if let Ok(id_val) = id_result {
                if let Ok(guid) = id_val.string_data() {
                    let task_path = tasks_key
                        .subkey(&guid)
                        .and_then(|r| r.ok())
                        .and_then(|t| t.value("Path").and_then(|v| v.ok()).and_then(|v| v.string_data().ok()))
                        .unwrap_or_else(|| path.clone());
                    let risky = is_task_path_risky(&task_path);
                    out.push(ScheduledTaskEntry {
                        path: task_path,
                        guid,
                        risky,
                    });
                }
            }
        }
        collect_tree_tasks(&sk, &path, tasks_key, out);
    }
}

#[cfg(test)]
mod tests {
    use super::{is_run_key_dangerous, is_service_path_risky, is_task_path_risky};

    #[test]
    fn run_key_dangerous_risky_paths() {
        assert!(is_run_key_dangerous("C:\\Users\\x\\AppData\\Roaming\\evil.exe"));
        assert!(is_run_key_dangerous("C:\\Windows\\Temp\\x.exe"));
        assert!(is_run_key_dangerous("D:\\ProgramData\\malware\\a.exe"));
    }

    #[test]
    fn run_key_safe_trusted_paths() {
        assert!(!is_run_key_dangerous("C:\\Windows\\System32\\SecurityHealthSystray.exe"));
        assert!(!is_run_key_dangerous("\"C:\\Program Files\\Defender\\foo.exe\" -arg"));
        assert!(!is_run_key_dangerous("C:\\Program Files (x86)\\Vendor\\app.exe"));
    }

    #[test]
    fn run_key_unknown_path_not_trusted() {
        assert!(is_run_key_dangerous("C:\\Unknown\\Folder\\x.exe"));
    }

    #[test]
    fn service_risky_paths() {
        assert!(is_service_path_risky("C:\\Users\\x\\AppData\\Local\\svc.exe"));
        assert!(is_service_path_risky("D:\\ProgramData\\bad.sys"));
    }

    #[test]
    fn service_safe_paths() {
        assert!(!is_service_path_risky("\\SystemRoot\\System32\\drivers\\ntfs.sys"));
        assert!(!is_service_path_risky("C:\\Windows\\System32\\svchost.exe -k netsvcs"));
    }

    #[test]
    fn task_path_risky() {
        assert!(is_task_path_risky("\\MyTask"));
        assert!(is_task_path_risky("\\Vendor\\Update"));
    }

    #[test]
    fn task_path_trusted() {
        assert!(!is_task_path_risky("\\Microsoft\\Windows\\WindowsUpdate\\Auto Update"));
        assert!(!is_task_path_risky("\\Microsoft\\Windows NT\\DiskDiagnostic\\Microsoft-Windows-DiskDiagnosticDataCollector"));
    }
}
