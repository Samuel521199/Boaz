//! 进程图谱：获取存活进程 (PID、名称、执行文件绝对路径)
//! Samuel, 2026-02-24

use boaz_shared::SuspectProcess;
use sysinfo::System;

/// 获取当前系统所有活跃进程
pub fn get_process_map() -> Vec<SuspectProcess> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut out = Vec::new();
    for (pid, proc_) in sys.processes() {
        let path = proc_
            .exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| proc_.name().to_string_lossy().into_owned());

        let parent_pid = proc_.parent().map(|p| p.as_u32());

        out.push(SuspectProcess {
            pid: pid.as_u32(),
            name: proc_.name().to_string_lossy().into_owned(),
            path,
            parent_pid,
        });
    }
    out
}
