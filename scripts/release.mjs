// 出一个版本：编译 → 签名 → 安装版、两种便携版、自动更新包 → 签好的更新信息 →（可选）传到 R2。
//
//   node scripts/release.mjs [--channel stable|beta] [--notes "修了…；加了…"] [--skip-build] [--upload]
//
// 用到的环境变量：
//   LS_BUILD_DIR     编译和产物放哪（默认仓库旁边的 ../../LeafStarter-build；别放在 D:\Document\HTML 里面，那里编出来的程序写不了临时目录）
//   LS_UPDATE_KEY    更新私钥文件（leafstarter --cli update-key-new 生成的）。没有就不出更新信息，老版本也就收不到这次更新
//   LS_SIGN_*        代码签名证书，见 scripts/sign.mjs。没有就不签
// 产物在 <LS_BUILD_DIR>/dist/<版本>/：
//   LeafStarter_<版本>_x64-setup.exe          安装版（开始菜单、leafstarter:// 邀请链接、自动更新）
//   LeafStarter_<版本>_x64_portable.exe       便携版 · 精简：一个 exe，数据放旁边的 data/
//   LeafStarter_<版本>_x64_portable_full.zip  便携版 · 全功能：解开就能用，带许可文件
//   LeafStarter_<版本>_x64.exe                自动更新下的就是它（和便携版是同一个程序，只是名字不带 portable）
//   update-<频道>.json  release.json  SHA256SUMS.txt
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const opt = (n, d) => {
    const i = args.indexOf('--' + n);
    return i >= 0 ? args[i + 1] : d;
};
const has = (n) => args.includes('--' + n);
const channel = opt('channel', 'stable');
if (!['stable', 'beta'].includes(channel)) throw new Error('频道只有 stable 和 beta');
const SITE = 'https://leafstarter.pingyi.me';
const BUCKET = 'leafstarter-releases';
const win = process.platform === 'win32';
const run = (cmd, a, o = {}) => execFileSync(win && cmd === 'npx' ? 'npx.cmd' : cmd, a, { stdio: 'inherit', cwd: root, shell: win && cmd === 'npx', ...o });

// ---------- 版本号三处要一样 ----------
const cargo = readFileSync(join(root, 'src-tauri', 'Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
const conf = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8')).version;
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
if (cargo !== conf || cargo !== pkg) throw new Error(`版本号对不上：Cargo.toml ${cargo}，tauri.conf.json ${conf}，package.json ${pkg}`);
const version = cargo;
if (channel === 'stable' && version.includes('-')) throw new Error(`${version} 带后缀，应该发到 beta`);
const notes = (opt('notes', '') || '').trim();

const build = resolve(process.env.LS_BUILD_DIR || join(root, '..', '..', 'LeafStarter-build'));
const target = join(build, 'release-target');
const dist = join(build, 'dist', version);
console.log(`LeafStarter ${version}（${channel}）→ ${dist}`);

// ---------- 第三方许可清单 ----------
try {
    run('node', ['scripts/licenses.mjs']);
} catch {
    console.log('cargo-about 没装或者没跑成，用仓库里现有的许可清单（cargo install cargo-about --locked --features cli）');
}

// ---------- 编译和打包（签名经 signCommand 在打包时做） ----------
if (!has('skip-build')) {
    const overlay = join(build, 'release-config.json');
    mkdirSync(build, { recursive: true });
    const signing = ['LS_SIGN_THUMBPRINT', 'LS_SIGN_PFX', 'LS_SIGN_AZURE_DLIB'].some((k) => process.env[k]);
    const cfg = { bundle: { windows: { signCommand: signing ? { cmd: 'node', args: [join(root, 'scripts', 'sign.mjs'), '%1'] } : null } } };
    writeFileSync(overlay, JSON.stringify(cfg));
    if (!signing) console.log('没配代码签名证书：这次的程序不签名（杀毒软件可能误报）');
    run('npx', ['tauri', 'build', '--config', overlay], { env: { ...process.env, CARGO_TARGET_DIR: target } });
}
const exe = join(target, 'release', 'leafstarter.exe');
const setup = join(target, 'release', 'bundle', 'nsis', `LeafStarter_${version}_x64-setup.exe`);
for (const f of [exe, setup]) if (!existsSync(f)) throw new Error(`没找到 ${f}`);

// ---------- 产物 ----------
rmSync(dist, { recursive: true, force: true });
mkdirSync(dist, { recursive: true });
const name = {
    setup: `LeafStarter_${version}_x64-setup.exe`,
    portable: `LeafStarter_${version}_x64_portable.exe`,
    portableFull: `LeafStarter_${version}_x64_portable_full.zip`,
    update: `LeafStarter_${version}_x64.exe`,
};
copyFileSync(setup, join(dist, name.setup));
copyFileSync(exe, join(dist, name.portable));
copyFileSync(exe, join(dist, name.update));

// 全功能便携版：LeafStarter/LeafStarter.exe + data/（有 data/.leafstarter 就是便携模式）+ 许可
const stage = join(build, 'portable-stage', 'LeafStarter');
rmSync(join(build, 'portable-stage'), { recursive: true, force: true });
mkdirSync(join(stage, 'data'), { recursive: true });
copyFileSync(exe, join(stage, 'LeafStarter.exe'));
writeFileSync(join(stage, 'data', '.leafstarter'), 'LeafStarter 便携版的数据，别删\n');
for (const f of ['LICENSE', 'NOTICE', 'THIRD-PARTY-LICENSES.txt']) if (existsSync(join(root, f))) copyFileSync(join(root, f), join(stage, f));
writeFileSync(
    join(stage, '使用说明.txt'),
    [
        `LeafStarter ${version} 便携版（全功能）`,
        '',
        '双击 LeafStarter.exe 就能用，不用安装。设置、账户、下载的 Java 都放在旁边的 data 文件夹里，',
        '整个文件夹拷到 U 盘，换一台电脑接着用。离线翻译模型不在里面，用到时再下。',
        '',
        '需要系统里有 WebView2：Windows 11 自带，Windows 10 绝大多数也有；没有的话程序会提示去装。',
        '新版本会自动下载，关掉程序再打开就是新的。',
        '',
        `官网：${SITE}`,
    ].join('\r\n') + '\r\n',
);
const zip = join(dist, name.portableFull);
if (win) {
    // 不用 Compress-Archive：PowerShell 5.1 写的路径是反斜杠、中文名不标 UTF-8，别的解压软件会解乱。
    // .NET 的 ZipFile 逐个加，名字用正斜杠（遇到非 ASCII 名字会自动标 UTF-8）
    const q = (x) => `'${x.replace(/'/g, "''")}'`;
    const ps = [
        'Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem',
        `$base = ${q(dirname(stage))}`,
        `$z = [IO.Compression.ZipFile]::Open(${q(zip)}, 'Create', [Text.Encoding]::UTF8)`,
        "Get-ChildItem -LiteralPath $base -Recurse -File -Force | ForEach-Object { $n = $_.FullName.Substring($base.Length + 1).Replace('\\', '/'); [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($z, $_.FullName, $n, 'Optimal') }",
        '$z.Dispose()',
    ].join('; ');
    run('powershell', ['-NoProfile', '-Command', ps]);
} else {
    run('zip', ['-r', '-q', zip, 'LeafStarter'], { cwd: dirname(stage) });
}

const sha = (p) => createHash('sha256').update(readFileSync(p)).digest('hex');
const info = (k) => ({ key: `releases/${version}/${name[k]}`, name: name[k], size: statSync(join(dist, name[k])).size, sha256: sha(join(dist, name[k])) });
const release = { version, channel, date: new Date().toISOString().slice(0, 10), notes, files: { setup: info('setup'), portable: info('portable'), portableFull: info('portableFull') } };
writeFileSync(join(dist, 'release.json'), JSON.stringify(release, null, 2));
writeFileSync(join(dist, 'SHA256SUMS.txt'), Object.values(name).map((n) => `${sha(join(dist, n))}  ${n}`).join('\n') + '\n');

// ---------- 自动更新信息（用更新私钥签） ----------
// 没给就用 e 总这台电脑上的默认位置（D:\Document\LeafStarter-keys，不在仓库里）
const key = process.env.LS_UPDATE_KEY || ['D:/Document/LeafStarter-keys/update.key'].find((p) => existsSync(p));
let manifest = null;
if (key && existsSync(key)) {
    const url = `${SITE}/update/${version}/${name.update}`;
    const out = execFileSync(exe, ['--cli', 'update-sign', key, join(dist, name.update), version, url], { encoding: 'utf8' });
    const item = JSON.parse(out.trim().split('\n').pop());
    manifest = { version, date: release.date, notes, files: { [item.platform]: item.file } };
    writeFileSync(join(dist, `update-${channel}.json`), JSON.stringify(manifest, null, 2));
    console.log(`更新信息签好了：update-${channel}.json`);
} else {
    console.log('没给 LS_UPDATE_KEY：这次不出更新信息（老版本收不到自动更新）');
}

// ---------- 传到 R2 ----------
if (has('upload')) {
    const put = (k, file, type) => {
        console.log(`上传 ${k}`);
        run('npx', ['wrangler', 'r2', 'object', 'put', `${BUCKET}/${k}`, '--file', file, '--content-type', type, '--remote'], { cwd: join(root, 'site') });
    };
    const EXE = 'application/vnd.microsoft.portable-executable';
    put(release.files.setup.key, join(dist, name.setup), EXE);
    put(release.files.portable.key, join(dist, name.portable), EXE);
    put(release.files.portableFull.key, join(dist, name.portableFull), 'application/zip');
    put(`releases/${version}/${name.update}`, join(dist, name.update), EXE);
    put(`releases/${version}/release.json`, join(dist, 'release.json'), 'application/json');
    if (channel === 'stable') put('releases/latest.json', join(dist, 'release.json'), 'application/json');
    put(`releases/latest-${channel}.json`, join(dist, 'release.json'), 'application/json');
    if (manifest) put(`update/${channel}.json`, join(dist, `update-${channel}.json`), 'application/json');
}

console.log(`\n好了：${dist}`);
for (const n of Object.values(name)) console.log(`  ${n}  ${(statSync(join(dist, n)).size / 1048576).toFixed(1)} MB`);
