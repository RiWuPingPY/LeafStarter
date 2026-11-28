// 发布版不弹黑色控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    leafstarter_lib::started();
    let args: Vec<String> = std::env::args().collect();
    // 自动更新换文件的帮手、换完后等旧进程退出
    if let Some(code) = leafstarter_lib::pre_start(&args) {
        std::process::exit(code);
    }
    if let Some(code) = leafstarter_lib::cli(&args) {
        std::process::exit(code);
    }
    leafstarter_lib::run()
}
