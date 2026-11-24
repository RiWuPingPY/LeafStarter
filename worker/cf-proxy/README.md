# CurseForge 代理

LeafStarter 只通过这个 Worker 访问 CurseForge，Key 放在 Worker 的密钥里，不进客户端、不进仓库（ROADMAP 5.6、D7）。

- 只转发启动器用得到的接口，只放行 Minecraft（gameId 432）
- 不缓存、不存储 API 返回的数据（CurseForge 第三方条款）
- 文件由用户的客户端直接从 CurseForge 的 CDN 下载，不经过这里
- 每个 IP 每分钟 120 次

## 部署（e 总自己来）

```
cd worker/cf-proxy
npx wrangler secret put CF_API_KEY      # 粘贴 CurseForge 控制台「API keys」页的 Key
npx wrangler deploy
```

部署后把地址填进 `src-tauri/src/content/curseforge.rs` 的 `PROXY`（比如 `https://<启动器域名>/cf`），
或者先用环境变量 `LEAFSTARTER_CF_PROXY` 试。没填时启动器只用 Modrinth，CurseForge 那一栏说明「还没接上」。
