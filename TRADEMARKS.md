# 名称、图标与署名政策

LeafStarter 的代码按 Apache-2.0 开放：任何人都可以用、改、再发布，包括商用（见 `LICENSE`）。这份政策管的是代码以外的东西：**名字、图标，以及怎么署名**。Apache-2.0 第 6 条本来就写明，许可证不授予名称和商标的使用权，所以两者不冲突。

我们对你只有一个要求：**说清楚它来自 LeafStarter。**

## 这份政策管哪些

- 名称「LeafStarter」、「LeafHub」，以及看起来像它们、容易混淆的名字；
- LeafStarter 的叶子图标和应用图标；
- LeafStarter 的微软应用（Azure 客户端 ID）、更新服务器、LeafHub 同步服务、CurseForge 代理等在线服务的身份。

这些名称目前没有注册商标，但它们代表这个项目，请按下面的方式使用。

## 不用问就可以

- 在文章、视频、教程、评测里提到 LeafStarter，配上它的图标和截图。
- 说你的东西「基于 LeafStarter」「为 LeafStarter 制作」「支持 LeafStarter」。
- 写 LeafStarter 的插件，名字叫「某某 for LeafStarter」。
- 用 LeafStarter 的定制版功能，做你自己服务器或整合包的专用启动器，改显示名称、图标、主题、功能。

## 改源码另发一个启动器时

Apache-2.0 允许你这样做。这时请：

1. **换掉名字和图标**：不叫 LeafStarter，也不要用容易混淆的名字和叶子图标。
2. **写明来源**：在「关于」页或者 README 里写「基于 LeafStarter（https://github.com/RiWuPingPY/LeafStarter）」，并按 Apache-2.0 保留 `NOTICE` 文件。
3. **换成你自己的在线身份**：
   - 微软登录：源码里的客户端 ID 是公开信息、不是密钥，但它代表的是 LeafStarter 这个应用。请注册你自己的 Azure 应用，按 Mojang 的要求申请 Minecraft 接口权限后再用。
   - CurseForge：用你自己申请的 API Key，不要连我们的代理。
   - 自动更新、同步、下载统计：用你自己的服务器，不要连我们的。
4. **不要暗示是官方版本**，也不要说经过了我们的认可。

## 定制版

定制版用的是我们发布、我们签名的程序，加上你写的定制包，所以：

- 可以改显示名称、图标（窗口和快捷方式里的）、主题、字体，开关任何功能。
- 「基于 LeafStarter」由程序固定显示在「关于」页和启动画面上，定制包里关不掉。
- 定制包必须签名；「关于」页会显示定制包作者的信息。

## 插件

- 名字建议用「某某 for LeafStarter」，不要用「LeafStarter 某某」（看起来像官方插件）。
- 不要用 LeafStarter 的图标当插件图标。
- 「官方」「已审核」标记只能由插件市场给，不能自己写进插件介绍。

## 任何情况下都不行

- 冒充官方发布 LeafStarter，或者冒充官方渠道提供下载。
- 把 LeafStarter 的名字、图标用在恶意软件、盗号工具、钓鱼页面上。
- 让人以为 LeafStarter 是 Minecraft 官方产品。LeafStarter 和基于它的作品都不是 Minecraft 官方产品，与 Mojang 和 Microsoft 无关，请照样写明。

## 署名写法示例

> 基于 LeafStarter（https://github.com/RiWuPingPY/LeafStarter），按 Apache-2.0 许可使用。

拿不准的情况，在 GitHub 上开一个 Issue 问我们。

---

## In English, briefly

LeafStarter's code is licensed under Apache-2.0. This policy covers the names "LeafStarter" and "LeafHub", the leaf logo, and the identity of our online services, none of which the license grants (see section 6 of Apache-2.0). You may mention, review, and build plugins or custom editions for LeafStarter freely. If you fork and redistribute it, please use a different name and logo, state "based on LeafStarter" with a link, keep the `NOTICE` file, and use your own Microsoft (Azure) client ID with your own Minecraft API approval, your own CurseForge API key, and your own update and sync servers. Custom editions built on our signed binary always show "based on LeafStarter". LeafStarter is not an official Minecraft product and is not affiliated with Mojang or Microsoft.
