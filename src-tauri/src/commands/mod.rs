//! 前端能调用的命令，按页面拆分。
//! 加新命令：写进对应的文件（没有合适的就新建一个），再到 lib.rs 的 `generate_handler!` 里登记。

pub mod accounts;
pub mod ai;
pub mod app;
pub mod bisect;
pub mod content;
pub mod direct;
pub mod i18n;
pub mod install;
pub mod instance;
pub mod lan;
pub mod launch;
pub mod server;
pub mod settings;
pub mod sync;
pub mod targets;
pub mod tasks;
pub mod versions;
