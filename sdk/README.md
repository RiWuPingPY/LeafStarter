# LeafStarter 插件 SDK

插件分四层（详见 `docs/ROADMAP.md` 5.18）：

| 层 | `layer` | 能做什么 |
|---|---|---|
| 外观 | `look` | `contributes.theme`：`light` / `dark` 里写 CSS 变量（只收 `--名字`），`font: "serif"` |
| 声明式 | `decl` | `crashRules`（`match` 里的字都出现在日志或崩溃报告里就命中）、`homeCards`、`palette`（命令面板条目，打开 https 网址）、`mirrors`、`yggPresets` |
| 脚本 | `script` | 一个 HTML 入口（`main`），放进实例概览的卡片（`contributes.card`）或独立页面（`contributes.page`）；接口见 `leafstarter.d.ts` |
| 原生 | `native` | 脚本插件 + 自带程序（`bin`：`{"windows-x64": "bin/x.exe"}`），一行一条 JSON-RPC 走标准输入输出；要认证开发者的证书（`cert`） |

## 三步写一个

1. 复制 `plugin-template/`，改 `plugin.json` 里的 `id`、`name`、`permissions`。
2. LeafStarter 里打开「插件 → 开发者」，打开开发者模式、选这个文件夹；改了文件点「重新加载」。
3. 打包并签名：`leafstarter --cli plugin-pack <文件夹> <输出.lsplugin>`（用这台电脑上的作者密钥签；`--cli plugin-key` 看公钥）。以后升级必须用同一把密钥签。

## 权限

`instances:read`、`launch`、`events`、`storage`、`net:<域名>`（支持 `net:*.example.com`）、`native`。安装时用大白话给用户看；升级时多要了权限要重新同意。

## 隔离

脚本插件跑在 `sandbox="allow-scripts"` 的 iframe 里，页面带 CSP（`default-src 'none'`），不能自己联网；实测（Windows / WebView2）iframe 里调启动器后端会被拒，只能走 `leafstarter.call()`，宿主按权限放行。

`sdk/` 下的文件按 MIT 许可。
