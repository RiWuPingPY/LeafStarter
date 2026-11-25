//! Windows 外壳：桌面快捷方式、任务栏跳转列表、打开时是不是按着 Shift。

use std::path::Path;

use anyhow::Result;
use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_SHIFT};
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, SHStrDupW, ShellLink};

/// 跳转列表里的一项：点了就用 `args` 再打开一次自己
pub struct Item {
    pub title: String,
    pub args: String,
    pub desc: String,
}

/// 这个线程用 COM 期间保持初始化；别人已经按另一种模式初始化过也能用
struct Com(bool);

impl Com {
    fn init() -> Result<Com> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            return Ok(Com(false));
        }
        hr.ok()?;
        Ok(Com(true))
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() }
        }
    }
}

fn link(exe: &Path, args: &str, desc: &str, title: Option<&str>) -> Result<IShellLinkW> {
    unsafe {
        let l: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        l.SetPath(&HSTRING::from(exe.as_os_str()))?;
        l.SetArguments(&HSTRING::from(args))?;
        l.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0)?;
        if let Some(dir) = exe.parent() {
            l.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
        }
        l.SetDescription(&HSTRING::from(desc))?;
        // 跳转列表显示的名字要写在属性里，快捷方式的文件名不算
        if let Some(t) = title {
            let store: IPropertyStore = l.cast()?;
            let mut pv = PROPVARIANT::default();
            (*pv.Anonymous.Anonymous).vt = VT_LPWSTR;
            (*pv.Anonymous.Anonymous).Anonymous.pwszVal = SHStrDupW(&HSTRING::from(t))?;
            let r = store.SetValue(&PKEY_Title, &pv).and_then(|_| store.Commit());
            let _ = PropVariantClear(&mut pv);
            r?;
        }
        Ok(l)
    }
}

/// 建一个快捷方式（已经有了就覆盖）
pub fn create_shortcut(lnk: &Path, exe: &Path, args: &str, desc: &str) -> Result<()> {
    let _com = Com::init()?;
    unsafe {
        let l = link(exe, args, desc, None)?;
        let pf: IPersistFile = l.cast()?;
        pf.Save(&HSTRING::from(lnk.as_os_str()), true)?;
    }
    Ok(())
}

/// 换掉任务栏跳转列表里自己的分类。用户手动从列表里删掉的项目不再加回去（加回去整个分类都会失败）
pub fn set_jump_list(exe: &Path, category: &str, items: &[Item]) -> Result<()> {
    let _com = Com::init()?;
    unsafe {
        let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        let mut slots = 0u32;
        let removed: IObjectArray = list.BeginList(&mut slots)?;
        let mut gone = vec![];
        for k in 0..removed.GetCount()? {
            if let Ok(l) = removed.GetAt::<IShellLinkW>(k) {
                let mut buf = [0u16; 2048];
                if l.GetArguments(&mut buf).is_ok() {
                    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    gone.push(String::from_utf16_lossy(&buf[..n]));
                }
            }
        }
        let coll: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        let mut n = 0;
        for it in items.iter().filter(|it| !gone.contains(&it.args)).take(slots.max(1) as usize) {
            coll.AddObject(&link(exe, &it.args, &it.desc, Some(&it.title))?)?;
            n += 1;
        }
        if n > 0 {
            let arr: IObjectArray = coll.cast()?;
            list.AppendCategory(&HSTRING::from(category), &arr)?;
        }
        list.CommitList()?;
    }
    Ok(())
}

/// 现在是不是按着 Shift（双击图标时按住 Shift = 正常打开界面）
pub fn shift_down() -> bool {
    unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) < 0 }
}

/// 按住 Ctrl 打开：安全模式，不加载插件
pub fn ctrl_down() -> bool {
    unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::STGM_READ;

    #[test]
    fn shortcut_round_trip() {
        let dir = std::env::temp_dir().join(format!("ls-shell-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lnk = dir.join("空岛 生存.lnk");
        let exe = std::env::current_exe().unwrap();
        let args = r#"--launch "D:\mc\versions\空岛 生存""#;
        create_shortcut(&lnk, &exe, args, "测试").unwrap();
        assert!(lnk.is_file());
        let _com = Com::init().unwrap();
        unsafe {
            let l: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            let pf: IPersistFile = l.cast().unwrap();
            pf.Load(&HSTRING::from(lnk.as_os_str()), STGM_READ).unwrap();
            let mut buf = [0u16; 512];
            l.GetArguments(&mut buf).unwrap();
            let n = buf.iter().position(|&c| c == 0).unwrap();
            assert_eq!(String::from_utf16_lossy(&buf[..n]), args);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jump_list_accepts_items() {
        // 测试程序自己的跳转列表，和 LeafStarter 的不相干
        let exe = std::env::current_exe().unwrap();
        let items = [Item { title: "空岛生存".into(), args: r#"--launch "D:\mc\versions\a""#.into(), desc: "启动 空岛生存".into() }];
        // 有的电脑上 AppData 被驱动接管，跳转列表写不进去（0x80070003）；程序里只记日志，这里也只打印
        if let Err(e) = set_jump_list(&exe, "最近玩的", &items) {
            eprintln!("跳转列表写不进去：{e:#}");
        }
        let _ = set_jump_list(&exe, "最近玩的", &[]);
    }
}
