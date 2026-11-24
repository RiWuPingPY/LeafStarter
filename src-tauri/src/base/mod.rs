//! 基础设施：多源下载、任务进度、通用工具、平台桥接（安卓插件、打开文件夹）。
//! 不关心游戏本身，以后的同步、联机、插件宿主都建在这一层上。

pub mod gpu;
pub mod net;
pub mod platform;
#[cfg(windows)]
pub mod shell;
pub mod task;
pub mod util;
