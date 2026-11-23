# LeafStarter

Minecraft Java 版启动器，开发中。由下面的「方块补给」（版本与加载器下载器）发展而来，规划见 [docs/ROADMAP.md](docs/ROADMAP.md)。

LeafStarter is a free launcher for Minecraft: Java Edition, currently in development. It grows out of MC Supply, the downloader documented below. The plan (in Chinese) is in [docs/ROADMAP.md](docs/ROADMAP.md).

## 前身：方块补给（MC Supply）

Minecraft 版本与加载器**下载器**（不是启动器）：下载所有 MC 版本和 Fabric / Quilt / Forge / NeoForge，自动挑最快的下载源，下完直接放进 PCL、HMCL、官方启动器、Zalith Launcher 2、FCL、PojavLauncher 等启动器的目录。

官网：<https://mcdownloader.pingyi.me>

## 目录

```
src/                前端（原生 HTML/CSS/JS，设计沿用 LeafHub 跨端传送 / 众声）
  mock.js           浏览器里直接打开时的演示数据（官网 /demo/ 用的就是它）
src-tauri/          Tauri 2 后端（Rust）
  src/net.rs        下载源：URL 改写、测速打分、按文件回退、限流冷却、SHA1 校验
  src/mojang.rs     版本清单、版本 JSON → 文件清单、加载器 JSON 合并成独立版本
  src/loaders.rs    Fabric / Quilt / Forge / NeoForge 版本列表
  src/forge.rs      解析 Forge/NeoForge 安装器、执行处理器（不跑安装器界面）
  src/java.rs       找本机 Java（含 PCL / HMCL / 官方启动器的运行时），没有就下 Mojang 官方运行时
  src/launchers.rs  识别启动器：PCL 注册表、hmcl.json、常用位置扫描；安卓按包名 + 授权目录
  src/install.rs    一次安装的完整流程
  src/platform.rs   安卓桥接（调用 Kotlin 插件）
  gen/android/…/McBridgePlugin.kt  安卓：查已装启动器、所有文件访问权限、系统文件授权、批量写入授权目录
site/               官网（Cloudflare Worker + 静态资源 + R2）
```

## 开发与构建

需要 Node、Rust；安卓另需 Android SDK / NDK 和 JDK 17+。

```bash
npm install
npm run dev                 # 桌面端开发
npm run build               # Windows 安装包 → src-tauri/target/release/bundle/nsis/
npm run android:build       # 安卓 APK（arm64）→ src-tauri/gen/android/app/build/outputs/apk/
```

不开窗口的命令行模式（调试用）：

```bash
mc-supply --cli sources                       # 测速
mc-supply --cli scan                          # 识别启动器
mc-supply --cli loaders --loader forge --mc 1.20.1
mc-supply --cli install --dir D:\test\.minecraft --mc 1.20.1 --loader forge --loader-version 47.4.28
```

### 安卓签名

发布签名在 `src-tauri/gen/android/mcsupply-release.jks`，密码在同目录 `keystore.properties`（两者都已加入 .gitignore）。**务必备份**：丢了就没法用同一个签名发布更新，用户只能卸载重装。

## 发布新版本

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
