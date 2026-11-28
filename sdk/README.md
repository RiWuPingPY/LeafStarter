# LeafStarter 插件 SDK

插件分四层（详见 `docs/ROADMAP.md` 5.18）：

| 层 | `layer` | 能做什么 |
|---|---|---|
| 外观 | `look` | `contributes.theme`：`light` / `dark` 里写 CSS 变量（只收 `--名字`），`font: "serif"` |
| 声明式 | `decl` | `crashRules`（`match` 里的字都出现在日志或崩溃报告里就命中）、`homeCards`、`palette`（命令面板条目，打开 https 网址）、`mirrors`、`yggPresets` |
| 脚本 | `script` | 一个 HTML 入口（`main`），可以放在：实例概览的卡片（`contributes.card`）、插件页里的独立页面（`contributes.page`）、实例页的一个标签（`contributes.tab`，最多 4 个插件）、启动按钮旁边的小按钮（`contributes.launch`，点了打开 `page`）；还能给 AI 助手加工具（`contributes.tools`，见下）。接口见 `leafstarter.d.ts` |
| 原生 | `native` | 脚本插件 + 自带程序（`bin`：`{"windows-x64": "bin/x.exe"}`），一行一条 JSON-RPC 走标准输入输出；要认证开发者的证书（`cert`） |

## 三步写一个

1. 复制 `plugin-template/`，改 `plugin.json` 里的 `id`、`name`、`permissions`。
2. LeafStarter 里打开「插件 → 开发者」，打开开发者模式、选这个文件夹；改了文件点「重新加载」。
3. 打包并签名：`leafstarter --cli plugin-pack <文件夹> <输出.lsplugin>`（用这台电脑上的作者密钥签；`--cli plugin-key` 看公钥）。以后升级必须用同一把密钥签。

## 权限

`instances:read`、`launch`、`events`、`storage`、`net:<域名>`（支持 `net:*.example.com`）、`native`、`ai:tools`。安装时用大白话给用户看；升级时多要了权限要重新同意。

## 给 AI 助手加工具（`ai:tools`）

```json
"contributes": { "tools": [ { "name": "server_tps", "title": "看服务器 TPS", "description": "Read the current TPS of my server", "inputSchema": { "type": "object", "properties": {} } } ] }
```

AI 要用时（每次都会先问用户），插件页面会收到 `tool` 事件；做完用 `ai.result` 交回结果（30 秒内）：

```js
leafstarter.on('tool', async ({ call, tool, input }) => {
  const r = await leafstarter.call('net.fetch', { url: 'https://example.com/tps' });
  leafstarter.call('ai.result', { call, result: r.text });
});
```

插件页面没开着的时候，启动器会在后台（看不见）打开它来接这个事件。工具名会变成 `plg_<插件 id>_<name>`；返回的内容 AI 只当数据看。

## 定制包里带插件

服主做定制包（设置 → 定制版 → 制作定制包）时可以带上几个 `.lsplugin`：玩家打开定制版就自动装好（插件自己的签名照常检查，`native` 权限不会自动给）。定制包还能锁住设置（主题、下载源、Java、新建删除实例、内存和 JVM 参数）、换启动画面。

## 插件市场

索引在主仓库的 `market/index.json`，插件包放在 `market/files/` 或者作者自己的 https 地址，按 SHA-256 校验。档位「官方 / 审核过」要有官方根密钥的二次签名（`leafstarter --cli market-sign`），没有的一律按「社区」显示。想上架就提一个 PR，把插件包和索引项一起加进来。

## 原生插件的开发者认证

原生插件要带官方发的证书（`cert`）。申请办法见 `docs/plugin-cert.md`。

## 隔离

脚本插件跑在 `sandbox="allow-scripts"` 的 iframe 里，页面带 CSP（`default-src 'none'`），不能自己联网；实测（Windows / WebView2）iframe 里调启动器后端会被拒，只能走 `leafstarter.call()`，宿主按权限放行。

`sdk/` 下的文件按 MIT 许可。
