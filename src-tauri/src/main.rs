// 发布版不弹黑色控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = mc_supply_lib::cli(&args) {
        std::process::exit(code);
    }
    mc_supply_lib::run()
}
