//! 显卡（ROADMAP 5.8 性能调度）：列出显卡；把某个 java(w).exe 设成「高性能」（Windows「图形设置」里那一项，用户级），
//! 调游戏进程的优先级和后台节流。

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    /// nvidia / amd / intel / 其他
    pub vendor: String,
    pub vram_mb: u64,
}

#[cfg(windows)]
pub fn list() -> Vec<Gpu> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
    let mut out = vec![];
    let Ok(f) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else { return out };
    for i in 0..16 {
        let Ok(a) = (unsafe { f.EnumAdapters1(i) }) else { break };
        let Ok(d) = (unsafe { a.GetDesc1() }) else { continue };
        if d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let n = d.Description.iter().position(|&c| c == 0).unwrap_or(d.Description.len());
        let name = String::from_utf16_lossy(&d.Description[..n]);
        let vendor = match d.VendorId {
            0x10de => "nvidia",
            0x1002 | 0x1022 => "amd",
            0x8086 => "intel",
            _ => "other",
        };
        if !out.iter().any(|g: &Gpu| g.name == name) {
            out.push(Gpu { name, vendor: vendor.into(), vram_mb: d.DedicatedVideoMemory as u64 / 1048576 });
        }
    }
    out
}

#[cfg(not(windows))]
pub fn list() -> Vec<Gpu> {
    vec![]
}

const KEY: &str = r"Software\Microsoft\DirectX\UserGpuPreferences";
const HIGH: &str = "GpuPreference=2;";

/// 把这个 Java（java.exe 和旁边的 javaw.exe）设成用高性能显卡；`on = false` 时只撤掉我们设的那条
#[cfg(windows)]
pub fn prefer_high(java: &Path, on: bool) -> anyhow::Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    let (key, _) = winreg::RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(KEY, KEY_READ | KEY_WRITE)?;
    let dir = java.parent().unwrap_or(Path::new(""));
    for exe in ["java.exe", "javaw.exe"] {
        let p = dir.join(exe);
        if !p.is_file() {
            continue;
        }
        let name = p.to_string_lossy().into_owned();
        if on {
            key.set_value(&name, &HIGH)?;
        } else if key.get_value::<String, _>(&name).ok().as_deref() == Some(HIGH) {
            key.delete_value(&name)?;
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn prefer_high(_java: &Path, _on: bool) -> anyhow::Result<()> {
    Ok(())
}

/// 游戏进程：优先级「高于正常」、关掉 Windows 11 切到后台时的效率模式节流
#[cfg(windows)]
pub fn tune(handle: std::os::windows::io::RawHandle, priority: bool, no_throttle: bool) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Threading::{ProcessPowerThrottling, SetPriorityClass, SetProcessInformation, ABOVE_NORMAL_PRIORITY_CLASS, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE};
    let h = HANDLE(handle);
    unsafe {
        if priority {
            let _ = SetPriorityClass(h, ABOVE_NORMAL_PRIORITY_CLASS);
        }
        if no_throttle {
            let s = PROCESS_POWER_THROTTLING_STATE { Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION, ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED, StateMask: 0 };
            let _ = SetProcessInformation(h, ProcessPowerThrottling, &s as *const _ as *const _, std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32);
        }
    }
}

/// 把游戏和它拉起的所有子进程（有的 Mod 会另起一个 Java 进程盯着游戏）放进同一个作业对象；
/// 用户点「结束」时整组一起结束，不留下没人管的进程
#[cfg(windows)]
pub struct Job(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for Job {}
#[cfg(windows)]
unsafe impl Sync for Job {}

#[cfg(windows)]
impl Job {
    pub fn adopt(process: std::os::windows::io::RawHandle) -> Option<Job> {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW};
        unsafe {
            let job = CreateJobObjectW(None, None).ok()?;
            if AssignProcessToJobObject(job, HANDLE(process)).is_err() {
                let _ = windows::Win32::Foundation::CloseHandle(job);
                return None;
            }
            Some(Job(job))
        }
    }

    pub fn kill_all(&self) {
        unsafe {
            let _ = windows::Win32::System::JobObjects::TerminateJobObject(self.0, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // 只关句柄，不结束进程：真崩溃时 Mod 弹的提示窗口要留给用户看
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
