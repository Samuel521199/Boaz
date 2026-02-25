//! 执法模块：挂起、杀死进程、删文件
//! Samuel, 2026-02-24
//! 遵循 .cursor/rules/03-windows-api.mdc：先挂起后杀

#[allow(dead_code)]
#[cfg(windows)]
pub fn suspend_process(_pid: u32) -> anyhow::Result<()> {
    // TODO: 调用 NtSuspendProcess 或 DebugActiveProcess
    Ok(())
}

#[allow(dead_code)]
#[cfg(windows)]
pub fn terminate_process(_pid: u32) -> anyhow::Result<()> {
    // TODO: 调用 TerminateProcess
    Ok(())
}

#[cfg(not(windows))]
pub fn suspend_process(_pid: u32) -> anyhow::Result<()> {
    anyhow::bail!("仅支持 Windows")
}

#[cfg(not(windows))]
pub fn terminate_process(_pid: u32) -> anyhow::Result<()> {
    anyhow::bail!("仅支持 Windows")
}
