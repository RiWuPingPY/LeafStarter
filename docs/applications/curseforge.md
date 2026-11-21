# CurseForge API 申请表

表单是英文审核，答案用英文写；每段下面附中文意思。「目标与范围」「商业模式」「如何回馈作者」「附加说明」在表单里是单行输入框，换行会丢，所以写成一整段（下面就是实际填进去的文字）。带 `【你填】` 的是个人信息或需要你决定的，我不代填。

依据：CurseForge 第三方 API 条款（2024-08-14 版）。和申请相关的几条：

- 申请人须年满 18 岁或有完全民事行为能力（4(a)），所填信息须真实，有变化要及时更新（4(b)）。
- Key 不可转让、不可给第三方（2.2）→ 不能打进客户端，放服务器。
- 不得保存或缓存经 API 获得的数据（3.1(e)）→ 服务器只转发，不缓存。
- 应用要有超出 API 本身的实质功能（1.1）→ 启动器满足。

---

**Project name**

```
LeafStarter
```

**Your nickname** — 【你填】（Git 提交署名是 `PingYi`，可以用这个）

**Your real name** — 【你填】

**Email** — 【你填】

**Your full Discord username** — 【你填】（现在的 Discord 用户名已经没有 `#1234` 后缀，填用户名本身即可）

---

**Describe the project goal and scope**

```
LeafStarter is a free launcher for Minecraft: Java Edition, for Windows first (macOS and Linux later), with an Android companion app. It covers what a player does day to day: installing game versions and mod loaders (Fabric, Quilt, Forge, NeoForge), Microsoft sign-in, launching the game, managing instances, fixing crashes, and managing mods, resource packs, shader packs and modpacks. Its main feature is keeping the same instance and world in sync between a PC and a phone. It is in development; its predecessor MC Supply (website below) is already released. We would use the CurseForge API only to: (1) search and browse mods, modpacks, resource packs and shader packs, filtered by game version and mod loader; (2) resolve required dependencies when a player installs a mod; (3) install CurseForge modpacks (manifest.json) as a new instance; (4) check installed mods for updates by file fingerprint. The API key stays on our server and is never shipped in our desktop or Android app; our backend forwards requests to the CurseForge API and does not cache or store responses. The player's app downloads files directly from CurseForge's CDN using the download URL returned by the API; we never mirror, re-host or modify files. If an author has disabled third-party distribution, we send the player to the project page on CurseForge to download the file there. Every project shows its author and links to its CurseForge page.
```

中文意思：LeafStarter 是免费的 Minecraft Java 版启动器，先做 Windows，之后做 macOS / Linux，另有安卓配套应用。功能覆盖日常的全部流程：装版本和加载器、微软账户登录、启动、管理实例、修复崩溃，以及管理 Mod、资源包、光影和整合包。主打电脑和手机之间同步同一个实例和世界；安卓应用负责把同步来的实例装进玩家用的安卓启动器。目前在开发中，前身「方块补给」已经发布。CurseForge API 只用于四件事：搜索浏览、装 Mod 时解析依赖、安装 CurseForge 整合包、按文件指纹检查更新。Key 只在服务器上，不打进任何客户端；服务器只转发，不缓存不存储 API 数据；文件由玩家的应用直接从 CurseForge 的 CDN 下载，我们不镜像、不转存、不改动；作者关闭了第三方分发的，引导到 CurseForge 项目页下载；每个项目都显示作者并链接回 CurseForge。

---

**Why are you building this project?**（上限 2000 字符，下面约 1100）

```
Many Minecraft players in China are students. They play on a modest laptop at home and on a phone at school, and existing launchers make them start over on each device. Downloads from overseas servers are often slow or fail.

We built MC Supply to fix the download part: it installs any Minecraft version and mod loader into existing launchers and picks the fastest download source for each file. LeafStarter turns it into a complete launcher built around three ideas:
1. The same world on PC and phone: instances and worlds sync between the desktop launcher and the player's Android launcher.
2. No friction: double-click the icon and you are in the game; the interface stays out of the way.
3. Fix problems instead of reporting them: crash analysis with one-click fixes, such as switching to the right Java version or installing a missing dependency.

Mods and modpacks are a big part of how our players play, and many of the best ones are on CurseForge. With API access, players can find and install them from inside the launcher, along with the right dependencies, and still see who made them, with a link back to CurseForge.
```

中文意思：国内很多 MC 玩家是学生，在家用一般的笔记本玩，在学校用手机玩，现有启动器在每台设备上都得从头来，海外服务器下载又慢又容易失败。方块补给先解决了下载；LeafStarter 把它做成完整的启动器，围绕三件事：电脑手机同一个世界；双击就进游戏、界面不碍事；出错直接修好（比如换对的 Java、补装缺的依赖）。Mod 和整合包是玩家玩法的重要部分，很多好作品在 CurseForge 上；有了 API，玩家在启动器里就能找到、连同依赖一起装好，同时能看到作者并链接回 CurseForge。

---

**Supported games**

```
Minecraft
```

**Website URL**

```
https://mcdownloader.pingyi.me
```

（LeafStarter 还没有官网，先填已上线的方块补给官网，附加说明里解释。若 D8 定了新域名并且已上线，可以换成新的。）

**Git URL** — 【你填】必填项。需要先有一个代码仓库地址（见 ROADMAP 的 D9）。审核方大概率会打开看，私有仓库对他们没用。

**I understand that file distribution through the API is subjected to the mod author's approval** — 【你勾】

---

**Are you looking to monetize the project?** — 【你选】取决于 D11：

- 如果云存档全免费 → 选 **No**，下面填：

```
None. LeafStarter is free with no ads, subscriptions or paid features.
```

- 如果打算「免费额度 + 超出收费」→ 选 **Yes**，下面填：

```
LeafStarter itself is free and has no ads. We plan to offer optional paid cloud storage for world and instance sync beyond a free quota, sold through our existing LeafHub account system. CurseForge content is never behind a paywall: searching, installing and updating mods and modpacks are free for everyone.
```

（中文意思：启动器本身免费、无广告；计划对超出免费额度的云同步存储收费，通过现有 LeafHub 账号体系购买；CurseForge 内容永远不收费。）

---

**If you're planning to distribute mods, how would you contribute to the mod authors?**

```
We will not redistribute CurseForge-hosted files: the player's app downloads every file directly from CurseForge's CDN using the URL the API provides, and we never re-host, mirror or modify mod files. In the launcher, every mod and modpack shows its author and links to its CurseForge project page; files whose authors have disabled third-party distribution are never downloaded through the launcher, and we send the player to CurseForge to get them instead; installed mods keep a record of where they came from, so players can always find the project again; and we never strip or change copyright notices, credits or CurseForge branding.
```

中文意思：我们不分发 CurseForge 上的文件，每个文件都由玩家的应用直接从 CurseForge 的 CDN 下载，不转存、不镜像、不改动。对作者：每个 Mod 和整合包都显示作者、链接到 CurseForge 项目页；作者关闭第三方分发的文件绝不经启动器下载，引导去 CurseForge；装好的 Mod 记录来源，玩家随时能找回项目页；从不删改版权声明、署名和 CurseForge 标识。

---

**I understand that above certain volumes I might be required to reduce API calls, or alternatively, pay associated bandwidth costs** — 【你勾】

**I have read and agree to the API's TOS** — 【你勾】

---

**Additional notes**

```
I also develop MC Supply (the website above); LeafStarter is its successor and shares its codebase and backend. Our players are mostly in mainland China, so the interface is in Simplified Chinese first, with English later. To keep the API key off players' devices, our backend forwards API requests and does not cache or store any response; if you prefer a different setup for desktop and mobile clients, we will follow your guidance. Usage will be small at first; we rate-limit each client and will contact you before any large release.
```

中文意思：我也是方块补给的开发者，LeafStarter 是它的后继，共用代码和后端；用户主要在中国大陆，界面先做简体中文；为了不让 Key 出现在玩家设备上，由后端转发 API 请求且不缓存任何返回数据，如果你们希望桌面和移动客户端用别的方式，我们照办；初期用量很小，每个客户端都限流，大规模发布前会先联系你们。
