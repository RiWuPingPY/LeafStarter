// 部署前把应用界面复制成官网的在线演示（/demo/，浏览器里会自动用演示数据），顺便放好图标
import { cpSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const site = join(dirname(fileURLToPath(import.meta.url)), '..');
const app = join(site, '..');
const pub = join(site, 'public');

rmSync(join(pub, 'demo'), { recursive: true, force: true });
mkdirSync(join(pub, 'demo'), { recursive: true });
cpSync(join(app, 'src'), join(pub, 'demo'), { recursive: true });
cpSync(join(app, 'src-tauri', 'icons', 'icon.png'), join(pub, 'icon-512.png'));
cpSync(join(app, 'src-tauri', 'icons', '64x64.png'), join(pub, 'favicon.png'));
console.log('demo + icons ready');
