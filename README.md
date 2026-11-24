# LeafStarter

Minecraft Java 版启动器，开发中。由下面的「方块补给」（版本与加载器下载器）发展而来，规划见 [docs/ROADMAP.md](docs/ROADMAP.md)。

LeafStarter is a free launcher for Minecraft: Java Edition, currently in development. It grows out of MC Supply, the downloader documented below. The plan (in Chinese) is in [docs/ROADMAP.md](docs/ROADMAP.md).

## 前身：方块补给（MC Supply）

Minecraft 版本与加载器**下载器**（不是启动器）：下载所有 MC 版本和 Fabric / Quilt / Forge / NeoForge，自动挑最快的下载源，下完直接放进 PCL、HMCL、官方启动器、Zalith Launcher 2、FCL、PojavLauncher 等启动器的目录。

官网：<https://mcdownloader.pingyi.me>

## 目录

```
src/                    前端（原生 HTML/CSS/JS，ES 模块，不用构建），从 proto/ 的原型搬过来
  app.js                入口：状态、绘制、动作表、快捷键
  real.js               真程序的数据层：在 LeafStarter 里运行时用后端的真数据换掉示例数据
  data.js               示例数据：浏览器里直接打开（线上原型、官网演示）时用
  views/                界面，只出 HTML；views/real.js 是真程序里和原型不一样的几块
scripts/dev-server.mjs  开发用的静态服务器，npm run dev 时自动启动（127.0.0.1:1420，不缓存）
src-tauri/src/          Tauri 2 后端（Rust）
  lib.rs                只做启动准备和命令登记
  state.rs              全局状态和设置
  instance.rs           实例里的 Mod、存档、截图，启用 / 禁用 Mod；实例设置；改名、复制、删到回收站
  launch/               启动核心：resolve（inheritsFrom 合并）files（补文件）jvm（挑 Java、内存）natives args
                        process（进程和日志）logfmt（log4j XML 转普通日志）unixsock（Java 本机管道放哪个目录）
                        crash（崩溃规则和一键修复）
  auth/                 账户：msa（微软设备代码登录）vault（DPAPI 加密令牌）region（D6 地区判断）
  migrate.rs            第一次打开时把方块补给的设置搬过来
  cli.rs                命令行模式
  commands/             给前端的命令，按页面分：app  versions  targets  install  instance  launch  accounts  tasks  settings
  base/net.rs           下载源：URL 改写、测速打分、按文件回退、限流冷却、SHA1 校验
  base/task.rs          任务进度推送
  base/platform.rs      安卓桥接（调用 Kotlin 插件）、打开文件夹
  game/mojang.rs        版本清单、版本 JSON → 文件清单、加载器 JSON 合并成独立版本
  game/loaders.rs       Fabric / Quilt / Forge / NeoForge 版本列表
  game/forge.rs         解析 Forge/NeoForge 安装器、执行处理器（不跑安装器界面）
  game/java.rs          找本机 Java（含 PCL / HMCL / 官方启动器的运行时），没有就下 Mojang 官方运行时
  game/launchers.rs     识别启动器：PCL 注册表、hmcl.json、常用位置扫描；安卓按包名 + 授权目录
  game/install.rs       一次安装的完整流程
src-tauri/gen/android/…/McBridgePlugin.kt  安卓：查已装启动器、所有文件访问权限、系统文件授权、批量写入授权目录
proto/                  交互原型（设计用，全是示例数据；真程序的界面在 src/）
site/                   方块补给的官网（Cloudflare Worker + 静态资源 + R2），LeafStarter 的官网等域名（D8）定了再改
```

## 开发与构建

需要 Node、Rust；安卓另需 Android SDK / NDK 和 JDK 17+。

```bash
npm install
npm run dev                 # 桌面端开发（改了前端在窗口里按 F5；改了 Rust 会自动重新编译）
npm run build               # Windows 安装包 → src-tauri/target/release/bundle/nsis/
npm run android:build       # 安卓 APK（arm64）→ src-tauri/gen/android/app/build/outputs/apk/
```

不开窗口的命令行模式（调试用）：

```bash
leafstarter --cli sources                     # 测速
leafstarter --cli scan                        # 识别启动器
leafstarter --cli loaders --loader forge --mc 1.20.1
leafstarter --cli install --dir D:\test\.minecraft --mc 1.20.1 --loader forge --loader-version 47.4.28
leafstarter --cli args --dir <.minecraft> --name <版本名>        # 只打印启动用的 Java 和完整参数（会先补文件）
leafstarter --cli launch --dir <.minecraft> --name <版本名> --user Steve   # 用离线账户真的启动，日志打在终端里
leafstarter --cli unixsock <目录> [目录…]          # 查这些目录能不能放 Java 的本机管道文件
```

### 编译报 D8050 或程序写不了文件

如果编译 `ring` 时报 `D8050 … 未能将命令行放入调试记录中`，或者编出来的程序写不了 `%TEMP%`、`%APPDATA%`，多半是仓库所在的文件夹被打了「低完整性」标签（一些 AI 编程工具的 Windows 沙盒会这么做），那里面编出来的程序一运行就被降级。查：`icacls <仓库的上级目录>`，看有没有 `Mandatory Label\Low Mandatory Level`。两种办法：去掉这个标签；或者把编译输出放到别处，比如设环境变量 `CARGO_TARGET_DIR`。

### 安卓签名

LeafStarter 的应用标识是 `com.leafhub.leafstarter`（方块补给是 `com.leafhub.mcsupply`，两个可以同时装），沿用方块补给的发布签名：`src-tauri/gen/android/mcsupply-release.jks`，密码在同目录 `keystore.properties`（两者都已加入 .gitignore）。**务必备份**：丢了就没法用同一个签名发布更新，用户只能卸载重装。

## 发布新版本

> 下面这套发布脚本和 R2 存储桶还是**方块补给**的。LeafStarter 的发布（自动更新、签名、安装版 / 便携版）在 M6 做，在那之前**不要从这个仓库发布**，否则会把 LeafStarter 当成方块补给的新版本发出去。

```bash
cd site
npm install
node scripts/release.mjs --version 0.1.1 --windows <setup.exe> --android <app.apk> --notes "改动一;改动二"
npm run deploy              # 只有网站页面或 Worker 改了才需要
```

R2 存储桶 `mcdownloader-releases` 的结构：`releases/latest.json`、`releases/<版本>/release.json`、`releases/<版本>/<安装包>`、`stats/downloads.json`（下载次数，读改写，只是近似值）。

## 已知限制

- 安卓端没法运行 Forge / NeoForge 安装器（没有 JVM）：会先装原版，并把安装器 jar 放到启动器目录的 `installers/`，再在启动器里「执行 jar」。
- Windows 安装包没有代码签名，首次运行会有 SmartScreen 提示。
- Quilt 不在 BMCLAPI 镜像上，只走官方源。

## 许可 / License

LeafStarter 的代码按 [Apache-2.0](LICENSE) 发布：任何人都可以用、改、再发布，包括商用，只要保留版权声明和 [NOTICE](NOTICE)。名称「LeafStarter」、叶子图标和我们的在线服务不随代码授权，改源码另发时请换名字，并写明「基于 LeafStarter」，详见 [名称、图标与署名政策](TRADEMARKS.md)。仓库里用到的第三方作品见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。

Copyright 2026 PingYi. Licensed under the [Apache License 2.0](LICENSE). The LeafStarter name and logo are covered separately by the [name, logo and attribution policy](TRADEMARKS.md).

LeafStarter 不是 Minecraft 官方产品，与 Mojang 和 Microsoft 无关。LeafStarter is not an official Minecraft product and is not affiliated with Mojang or Microsoft.
