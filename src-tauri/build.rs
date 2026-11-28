fn main() {
    // 应用清单自己用链接参数嵌：程序和测试程序都带上。tauri-build 只给程序嵌，测试程序没有清单，
    // 托盘菜单要用的公共控件 6.0 就找不到，测试一跑就报 STATUS_ENTRYPOINT_NOT_FOUND
    let mut attrs = tauri_build::Attributes::new();
    #[cfg(windows)]
    {
        let m = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows").join("app.manifest");
        println!("cargo:rerun-if-changed={}", m.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", m.display());
        attrs = attrs.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    }
    tauri_build::try_build(attrs).expect("tauri-build 失败");
}
