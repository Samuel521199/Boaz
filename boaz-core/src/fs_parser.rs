//! 在挂载点底下枚举要扫的文件（System32/SysWOW64 等），给 Hash 和 Yara 用，不调 Windows API。

use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// 待扫描扩展名（可执行与常见载荷）
const SCAN_EXTENSIONS: &[&str] = &["exe", "dll", "sys", "bat", "cmd", "ps1", "vbs", "scr"];

/// 枚举挂载点下关键目录中的可执行类文件，用于完整性校验与 Yara 扫描。
/// 包含：Windows\\System32（部分）、Windows\\SysWOW64、用户 ProgramData 等（可选）。
/// 为控制扫描时间，仅枚举第一级子目录中的目标扩展名文件，不无限递归。
pub fn list_critical_files_for_scan(mount_point: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let system32 = mount_point.join("Windows/System32");
    let syswow64 = mount_point.join("Windows/SysWOW64");

    for root in [system32, syswow64] {
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .max_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if SCAN_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                    out.push(path.to_path_buf());
                }
            }
        }
    }

    Ok(out)
}

