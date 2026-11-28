// 部署前：把启动器界面复制成官网的在线演示（/demo/，浏览器里自动用演示数据；首页的像素风景也用它的 scene.js），放好图标
import { cpSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const site = join(dirname(fileURLToPath(import.meta.url)), '..');
const app = join(site, '..');
const pub = join(site, 'public');

rmSync(join(pub, 'demo'), { recursive: true, force: true });
mkdirSync(join(pub, 'demo'), { recursive: true });
// 许可清单 400 多 KB，演示里用不到
cpSync(join(app, 'src'), join(pub, 'demo'), { recursive: true, filter: (p) => !p.endsWith('third-party.json') });
cpSync(join(app, 'src-tauri', 'icons', 'icon.png'), join(pub, 'icon-512.png'));
// 标签页图标：和启动器一样的「方块芽」
cpSync(join(app, 'src', 'logo.svg'), join(pub, 'favicon.svg'));
console.log('demo + icons ready');
