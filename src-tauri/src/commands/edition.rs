//! 定制版（M14）：读当前定制包、导入、拿掉、制作、取出带的整合包、按作者填的地址更新

use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::plugin::{edition, sign};
use crate::state::{err, AppState, CmdResult};

#[tauri::command]
pub fn edition_get(state: State<'_, AppState>) -> Value {
    match edition::find(&state.data_dir) {
        Some(p) => match edition::load(&p) {
            Ok(e) => {
                let mut v = e.view();
                let exe_dir = std::env::current_exe().ok().and_then(|x| x.parent().map(|p| p.to_path_buf()));
                v["beside"] = json!(exe_dir.is_some_and(|d| p.starts_with(d)));
                v
            }
            Err(e) => json!({ "error": format!("定制包「{}」用不了：{e:#}", p.file_name().unwrap_or_default().to_string_lossy()) }),
        },
        None => Value::Null,
    }
}

/// 选文件：定制包、图标、整合包；save 是选导出的位置
#[tauri::command]
pub async fn edition_pick(app: AppHandle, kind: String) -> CmdResult<Option<String>> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let d = app.dialog().file();
        let send = move |f: Option<tauri_plugin_dialog::FilePath>| {
            let _ = tx.send(f.map(|p| p.to_string()));
        };
        match kind.as_str() {
            "edition" => d.set_title("选择定制包").add_filter("LeafStarter 定制包", &[edition::EXT]).pick_file(send),
            "icon" => d.set_title("选择图标").add_filter("PNG 图片", &["png"]).pick_file(send),
            "pack" => d.set_title("选择整合包").add_filter("Modrinth 整合包", &["mrpack"]).pick_file(send),
            "save" => d.set_title("导出定制包").set_file_name(format!("定制包.{}", edition::EXT)).add_filter("LeafStarter 定制包", &[edition::EXT]).save_file(send),
            _ => return Err("不认识".into()),
        }
        Ok(rx.await.ok().flatten())
    }
    #[cfg(target_os = "android")]
    {
        let _ = (app, kind);
        Ok(None)
    }
}

fn current_path(st: &AppState) -> PathBuf {
    st.data_dir.join("edition").join(format!("current.{}", edition::EXT))
}

/// 导入定制包（先校验签名）
#[tauri::command]
pub fn edition_import(state: State<'_, AppState>, path: String) -> CmdResult<Value> {
    let e = edition::load(std::path::Path::new(&path)).map_err(err)?;
    let dst = current_path(&state);
    std::fs::create_dir_all(dst.parent().unwrap()).map_err(|x| x.to_string())?;
    std::fs::copy(&path, &dst).map_err(|x| format!("复制失败：{x}"))?;
    Ok(e.view())
}

/// 拿掉导入的定制包（exe 旁边的要自己删文件）
#[tauri::command]
pub fn edition_remove(state: State<'_, AppState>) -> CmdResult<()> {
    let p = current_path(&state);
    if p.is_file() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    if let Some(f) = edition::find(&state.data_dir) {
        return Err(format!("LeafStarter 旁边还有定制包「{}」，把它删掉或挪走就恢复成普通版", f.display()));
    }
    Ok(())
}

/// 作者自己的签名密钥：第一次做定制包时生成，存在保管库里，以后都用它（更新时玩家那边才认）
fn author_key(st: &AppState) -> anyhow::Result<Vec<u8>> {
    let p = st.data_dir.join("edition").join("author.key");
    if let Ok(s) = std::fs::read_to_string(&p) {
        return crate::auth::vault::open(s.trim());
    }
    let k = sign::new_key()?;
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, crate::auth::vault::seal(&k)?)?;
    Ok(k)
}

/// 做一个定制包
#[tauri::command]
pub async fn edition_build(state: State<'_, AppState>, manifest: Value, icon: Option<String>, pack: Option<String>, out: String) -> CmdResult<Value> {
    let k = author_key(&state).map_err(err)?;
    let out = PathBuf::from(out);
    tauri::async_runtime::spawn_blocking(move || {
        let public = edition::build(&manifest, icon.as_deref().map(std::path::Path::new), pack.as_deref().map(std::path::Path::new), &out, &k).map_err(err)?;
        Ok(json!({ "path": out.to_string_lossy(), "fingerprint": sign::fingerprint(&public), "size": std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 作者密钥的指纹（制作器里显示）
#[tauri::command]
pub fn edition_author(state: State<'_, AppState>) -> CmdResult<String> {
    let k = author_key(&state).map_err(err)?;
    Ok(sign::fingerprint(&sign::public_of(&k).map_err(err)?))
}

/// 定制包带的整合包：取出来，返回文件路径（界面接着用 pack_install 装）
#[tauri::command]
pub fn edition_pack(state: State<'_, AppState>) -> CmdResult<String> {
    let p = edition::find(&state.data_dir).ok_or("没有定制包")?;
    let e = edition::load(&p).map_err(err)?;
    Ok(edition::extract_pack(&e, &state.data_dir).map_err(err)?.to_string_lossy().into_owned())
}

/// 按作者填的地址更新定制包：新包必须是同一把密钥签的
#[tauri::command]
pub async fn edition_update(state: State<'_, AppState>) -> CmdResult<Value> {
    let p = edition::find(&state.data_dir).ok_or("没有定制包")?;
    let cur = edition::load(&p).map_err(err)?;
    let url = cur.manifest["update"].as_str().filter(|u| !u.is_empty()).ok_or("这个定制包没写更新地址")?.to_string();
    let b = reqwest::Client::builder()
        .user_agent(concat!("LeafStarter/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("连不上更新地址：{e}"))?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    let tmp = state.data_dir.join("edition").join("update.part");
    std::fs::create_dir_all(tmp.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, &b).map_err(|e| e.to_string())?;
    let new = edition::load(&tmp).map_err(err)?;
    if new.key != cur.key {
        let _ = std::fs::remove_file(&tmp);
        return Err("新的定制包不是原作者签的，没有更新".into());
    }
    if new.manifest == cur.manifest && new.icon == cur.icon {
        let _ = std::fs::remove_file(&tmp);
        return Ok(json!({ "updated": false }));
    }
    std::fs::rename(&tmp, &p).or_else(|_| std::fs::copy(&tmp, &p).map(|_| ())).map_err(|e| format!("写不进 {}：{e}", p.display()))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(json!({ "updated": true, "edition": new.view() }))
}
