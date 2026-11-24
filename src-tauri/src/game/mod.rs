//! 游戏本身：版本清单与版本 JSON、加载器、Forge 处理器、安装流程、Java、识别别的启动器的游戏目录。
//! M2 的启动核心（launch/）和账户（auth/）会和它并列，见 docs/ROADMAP.md 4.2。

pub mod forge;
pub mod install;
pub mod java;
pub mod launchers;
pub mod loaders;
pub mod modmeta;
pub mod mojang;
pub mod nbt;
pub mod options;
pub mod world;
