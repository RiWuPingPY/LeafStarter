// 开发用的静态服务器：npm run dev 时由 Tauri 先启动它，窗口从这里加载 src/ 里的界面。
// 只用 Node 自带的模块；只监听本机；不缓存，改了代码在窗口里按 F5 就是新的。
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../src/', import.meta.url)));
const PORT = Number(process.env.LS_DEV_PORT) || 1420;
const TYPES = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.mjs': 'text/javascript; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.svg': 'image/svg+xml',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.ico': 'image/x-icon',
    '.woff2': 'font/woff2',
};

const server = createServer(async (req, res) => {
    let path = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    if (path.endsWith('/')) path += 'index.html';
    const file = normalize(join(ROOT, path));
    // 不许跑出 src/
    if (!file.startsWith(ROOT)) {
        res.writeHead(403).end();
        return;
    }
    try {
        const body = await readFile(file);
        res.writeHead(200, { 'Content-Type': TYPES[extname(file).toLowerCase()] || 'application/octet-stream', 'Cache-Control': 'no-store' });
        res.end(body);
    } catch {
        res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' }).end('找不到 ' + path);
    }
});

// 上次 npm run dev 没正常退出时，旧的服务器可能还占着端口：是我们自己的就接着用，别的程序占着才报错
server.on('error', async (e) => {
    if (e.code !== 'EADDRINUSE') throw e;
    const ours = await fetch(`http://127.0.0.1:${PORT}/index.html`).then((r) => r.text()).then((t) => t.includes('LeafStarter'), () => false);
    if (!ours) {
        console.error(`端口 ${PORT} 被别的程序占着。关掉它，或者设环境变量 LS_DEV_PORT 换一个端口（tauri.conf.json 的 devUrl 也要跟着改）`);
        process.exit(1);
    }
    console.log(`端口 ${PORT} 上已经有界面开发服务器（上次没关掉的），直接用它`);
    setInterval(() => {}, 1 << 30);
});
server.listen(PORT, '127.0.0.1', () => console.log(`界面开发服务器：http://127.0.0.1:${PORT}/`));
