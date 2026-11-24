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

createServer(async (req, res) => {
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
}).listen(PORT, '127.0.0.1', () => console.log(`界面开发服务器：http://127.0.0.1:${PORT}/`));
