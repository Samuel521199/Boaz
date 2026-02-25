use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// 算文件 SHA-256，流式读，省内存。拿来校内核之类核心文件有没有被换。
pub fn verify_file_integrity(file_path: &Path) -> Result<String> {
    let mut file = File::open(file_path)
        .with_context(|| format!("无法读取核心文件以进行哈希校验: {}", file_path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192]; // 8KB 缓冲区，适合 PE 环境

    loop {
        let count = file
            .read(&mut buffer)
            .with_context(|| format!("读取文件流时发生 I/O 错误: {}", file_path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let result = hasher.finalize();
    Ok(hex::encode(result))
}
