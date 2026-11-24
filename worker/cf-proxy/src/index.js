// LeafStarter 的 CurseForge 代理（Cloudflare Worker）。
//
// 按 CurseForge 第三方 API 条款（ROADMAP 5.6、D7）：
// - Key 只放在这里（Worker 的密钥 CF_API_KEY），启动器里没有 Key
// - 不缓存、不存储任何 API 返回的数据：原样转发，回复带 no-store
// - 文件由用户的客户端直接从 CurseForge 的 CDN 下载，这里不转发文件
// - 只放行启动器用得到的几个接口，只放行 Minecraft（gameId 432）
//
// 部署：wrangler secret put CF_API_KEY，然后 wrangler deploy（见 README.md）

const API = 'https://api.curseforge.com';
const MC = '432';

// 允许的路径（方法 + 正则）
const ALLOW = [
    ['GET', /^\/v1\/mods\/search$/],
    ['GET', /^\/v1\/mods\/\d+$/],
    ['GET', /^\/v1\/mods\/\d+\/description$/],
    ['GET', /^\/v1\/mods\/\d+\/files$/],
    ['GET', /^\/v1\/mods\/\d+\/files\/\d+$/],
    ['GET', /^\/v1\/mods\/\d+\/files\/\d+\/download-url$/],
    ['POST', /^\/v1\/mods$/],
    ['POST', /^\/v1\/mods\/files$/],
    ['POST', /^\/v1\/fingerprints\/432$/],
    ['GET', /^\/v1\/games\/432\/versions$/],
    ['GET', /^\/v1\/categories$/],
];

const json = (status, body) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' } });

export default {
    async fetch(req, env) {
        const url = new URL(req.url);
        // 路径前缀 /cf 去掉：https://<域名>/cf/v1/mods/search → /v1/mods/search
        const path = url.pathname.replace(/^\/cf(?=\/)/, '');
        if (!ALLOW.some(([m, re]) => m === req.method && re.test(path))) return json(404, { error: 'not allowed' });
        if (!env.CF_API_KEY) return json(503, { error: 'CurseForge 代理还没配置 Key' });

        // 简单限流：同一个 IP 每分钟最多 120 次（用 Workers 的 Rate Limiting 绑定时更准）
        if (env.LIMITER) {
            const ip = req.headers.get('cf-connecting-ip') || 'unknown';
            const { success } = await env.LIMITER.limit({ key: ip });
            if (!success) return json(429, { error: '请求太频繁，等一分钟' });
        }

        const q = new URLSearchParams(url.search);
        if (path === '/v1/mods/search') q.set('gameId', MC);
        let body;
        if (req.method === 'POST') {
            const text = await req.text();
            if (text.length > 200_000) return json(413, { error: 'too large' });
            body = text;
        }
        const res = await fetch(`${API}${path}${q.toString() ? `?${q}` : ''}`, {
            method: req.method,
            body,
            headers: { 'x-api-key': env.CF_API_KEY, accept: 'application/json', 'content-type': 'application/json' },
            cf: { cacheTtl: 0, cacheEverything: false },
        });
        // 原样转发，不缓存
        return new Response(res.body, { status: res.status, headers: { 'content-type': res.headers.get('content-type') || 'application/json', 'cache-control': 'no-store' } });
    },
};
