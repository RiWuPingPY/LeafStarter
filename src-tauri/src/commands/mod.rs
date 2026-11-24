//! 前端能调用的命令，按页面拆分。
//! 加新命令：写进对应的文件（没有合适的就新建一个），再到 lib.rs 的 `generate_handler!` 里登记。

pub mod app;
pub mod install;
pub mod instance;
pub mod settings;
pub mod targets;
pub mod tasks;
pub mod versions;
