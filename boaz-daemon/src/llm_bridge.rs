//! AI 桥接：将行为翻译给 Jachin / 大模型
//! Samuel, 2026-02-24
//! 遵循 .cursor/rules/02-llm-bridge.mdc：隐私脱敏、JSON 格式化

/// 对路径进行隐私脱敏，例如 C:\Users\Samuel\Desktop\secret.docx → [USER_DESKTOP]\secret.docx
#[allow(dead_code)]
pub fn sanitize_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    // 简化脱敏：将用户目录替换为占位符
    if path.contains("/Users/") || path.contains("\\Users\\") {
        if let Some(idx) = path.find("/AppData/") {
            return format!("[USER_APPDATA]{}", &path[idx + 9..]);
        }
        if let Some(idx) = path.find("/Desktop/") {
            return format!("[USER_DESKTOP]{}", &path[idx + 9..]);
        }
        return "[USER_HOME]/...".to_string();
    }
    path
}

/// 组装 Prompt 调用大模型（占位，Phase 2 实现）
#[allow(dead_code)]
pub fn build_prompt(_suspect: &boaz_shared::SuspectMapping) -> String {
    "TODO: Phase 2 - LLM Prompt".to_string()
}
