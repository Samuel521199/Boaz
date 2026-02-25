//! 按规则路径加载 Yara，扫一批文件，把命中的规则和文件路径吐出来。

use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;
use yara::Compiler;

/// 单次 Yara 命中：某文件命中某条规则
#[derive(Debug, Clone, Serialize)]
pub struct YaraMatch {
    pub path: String,
    pub rule_id: String,
    pub namespace: String,
}

/// 从目录或单文件加载规则并编译。规则路径为目录时，会加载其中所有 .yar / .yara 文件。
fn load_compiled_rules(rules_path: &Path) -> Result<yara::Rules> {
    let mut compiler = Compiler::new()
        .context("初始化 Yara 编译器失败（请确认系统已安装 libyara 或使用 vendored 特性）")?;

    if rules_path.is_file() {
        let content = fs::read_to_string(rules_path)
            .with_context(|| format!("读取规则文件失败: {}", rules_path.display()))?;
        compiler = compiler
            .add_rules_str(&content)
            .with_context(|| format!("解析规则文件失败: {}", rules_path.display()))?;
    } else if rules_path.is_dir() {
        let entries = fs::read_dir(rules_path)
            .with_context(|| format!("打开规则目录失败: {}", rules_path.display()))?;
        for entry in entries {
            let entry = entry.context("遍历规则目录项失败")?;
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("yar") || ext.eq_ignore_ascii_case("yara") {
                let content = fs::read_to_string(&p)
                    .with_context(|| format!("读取规则文件失败: {}", p.display()))?;
                compiler = compiler
                    .add_rules_str(&content)
                    .with_context(|| format!("解析规则文件失败: {}", p.display()))?;
            }
        }
    } else {
        anyhow::bail!("规则路径既非文件也非目录: {}", rules_path.display());
    }

    compiler
        .compile_rules()
        .context("编译 Yara 规则失败")
}

/// 对给定文件列表进行 Yara 扫描。单文件读取上限 20MB，避免 PE 环境下内存不足。
/// on_progress: (当前索引, 总数, 文件路径) 每扫描一个文件调用一次，用于 UI 输出。
pub fn scan_files_with_rules<F>(
    rules_path: &Path,
    files: &[std::path::PathBuf],
    mut on_progress: F,
) -> Result<Vec<YaraMatch>>
where
    F: FnMut(usize, usize, &Path),
{
    const MAX_FILE_SCAN_BYTES: usize = 20 * 1024 * 1024;

    if files.is_empty() {
        return Ok(Vec::new());
    }

    let rules = load_compiled_rules(rules_path)?;
    let mut matches = Vec::new();
    let total = files.len();

    for (i, path) in files.iter().enumerate() {
        on_progress(i + 1, total, path);
        if !path.exists() {
            continue;
        }
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            continue;
        }
        if meta.len() > MAX_FILE_SCAN_BYTES as u64 {
            continue; // 跳过过大文件
        }
        let content = match fs::read(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let scan_result = match rules.scan_mem(&content, 30) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for m in scan_result.iter() {
            let rule_id = m.identifier.to_string();
            let namespace = m.namespace.to_string();
            matches.push(YaraMatch {
                path: path.display().to_string(),
                rule_id,
                namespace,
            });
        }
    }

    Ok(matches)
}
