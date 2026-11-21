// 发布新版本：把安装包传到 R2，并更新 releases/<版本>/release.json 和 releases/latest.json
//
//   node scripts/release.mjs --version 0.1.0 --windows <setup.exe> --android <app.apk> --notes "第一版;支持安卓"
//   加 --local 写到 wrangler dev 的本地模拟存储（调试用）
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, statSync, writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const BUCKET = 'mcdownloader-releases';
const args = process.argv.slice(2);
const opt = (name) => {
    const i = args.indexOf('--' + name);
    return i >= 0 ? args[i + 1] : undefined;
};
const local = args.includes('--local');
const version = opt('version');
if (!version) throw new Error('缺少 --version');

const PLATFORMS = {
    windows: { key: `MCSupply_${version}_x64-setup.exe`, name: `方块补给_${version}_x64-setup.exe`, type: 'application/vnd.microsoft.portable-executable' },
    android: { key: `MCSupply_${version}_arm64.apk`, name: `方块补给_${version}_arm64.apk`, type: 'application/vnd.android.package-archive' },
};

function put(key, file, contentType) {
    const cmdArgs = ['wrangler', 'r2', 'object', 'put', `${BUCKET}/${key}`, '--file', file, '--content-type', contentType, local ? '--local' : '--remote'];
    if (local && opt('persist-to')) cmdArgs.push('--persist-to', opt('persist-to'));
    console.log(`上传 ${key}`);
    // Windows 上 npx 是 .cmd，只能经 shell 调用，参数自己加引号（路径里可能有空格）
    const win = process.platform === 'win32';
    const quoted = win ? cmdArgs.map((a) => `"${a.replace(/"/g, '\\"')}"`) : cmdArgs;
    execFileSync(win ? `npx ${quoted.join(' ')}` : 'npx', win ? [] : cmdArgs, { stdio: 'inherit', shell: win });
}

const files = {};
for (const [plat, info] of Object.entries(PLATFORMS)) {
    const path = opt(plat);
    if (!path) continue;
    const buf = readFileSync(path);
    const key = `releases/${version}/${info.key}`;
    put(key, path, info.type);
    files[plat] = { key, name: info.name, size: statSync(path).size, sha256: createHash('sha256').update(buf).digest('hex') };
}
if (!Object.keys(files).length) throw new Error('至少给一个 --windows 或 --android');

const release = {
    version,
    date: new Date().toISOString(),
    notes: (opt('notes') || '').split(/[;；]/).map((s) => s.trim()).filter(Boolean),
    files,
};
const dir = mkdtempSync(join(tmpdir(), 'mcs-release-'));
const relFile = join(dir, 'release.json');
writeFileSync(relFile, JSON.stringify(release, null, 2));
put(`releases/${version}/release.json`, relFile, 'application/json');
put('releases/latest.json', relFile, 'application/json');
console.log(JSON.stringify(release, null, 2));
