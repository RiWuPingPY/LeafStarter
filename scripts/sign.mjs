// 代码签名：tauri build 打包时对 leafstarter.exe 和安装程序各调一次（release.mjs 配好 signCommand 才会调）。
//
//   node scripts/sign.mjs <要签的文件>
//
// 证书三选一，用环境变量告诉它（都没有就跳过，不算失败）：
//   LS_SIGN_THUMBPRINT=<证书指纹>                装在本机证书库里的证书（比如 Certum 开源证书、硬件 U 盾）
//   LS_SIGN_PFX=<pfx 文件> LS_SIGN_PFX_PASSWORD=<密码>
//   LS_SIGN_AZURE_DLIB=<Azure.CodeSigning.Dlib.dll> LS_SIGN_AZURE_METADATA=<metadata.json>   Azure 受信任签名
// 可选：SIGNTOOL=<signtool.exe 路径>，LS_SIGN_TIMESTAMP=<时间戳服务器>（默认 DigiCert）
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const file = process.argv[2];
if (!file || !existsSync(file)) {
    console.error(`要签的文件不存在：${file}`);
    process.exit(1);
}
const env = process.env;
const ts = env.LS_SIGN_TIMESTAMP || 'http://timestamp.digicert.com';

let how;
if (env.LS_SIGN_AZURE_DLIB && env.LS_SIGN_AZURE_METADATA) how = ['/dlib', env.LS_SIGN_AZURE_DLIB, '/dmdf', env.LS_SIGN_AZURE_METADATA];
else if (env.LS_SIGN_THUMBPRINT) how = ['/sha1', env.LS_SIGN_THUMBPRINT];
else if (env.LS_SIGN_PFX) how = ['/f', env.LS_SIGN_PFX, ...(env.LS_SIGN_PFX_PASSWORD ? ['/p', env.LS_SIGN_PFX_PASSWORD] : [])];
if (!how) {
    console.log(`没配代码签名证书，跳过：${file}`);
    process.exit(0);
}

function signtool() {
    if (env.SIGNTOOL) return env.SIGNTOOL;
    const base = 'C:\\Program Files (x86)\\Windows Kits\\10\\bin';
    const cands = [join(base, 'x64', 'signtool.exe')];
    if (existsSync(base)) {
        for (const d of readdirSync(base).filter((x) => /^10\./.test(x)).sort().reverse()) cands.push(join(base, d, 'x64', 'signtool.exe'));
    }
    const hit = cands.find(existsSync);
    if (!hit) {
        console.error('找不到 signtool.exe：装 Windows SDK，或者用 SIGNTOOL 环境变量指过去');
        process.exit(1);
    }
    return hit;
}

const args = ['sign', ...how, '/fd', 'sha256', '/tr', ts, '/td', 'sha256', '/d', 'LeafStarter', '/du', 'https://leafstarter.pingyi.me', file];
// 时间戳服务器偶尔连不上：多试两次
for (let k = 1; k <= 3; k++) {
    try {
        execFileSync(signtool(), args, { stdio: 'inherit' });
        console.log(`签好了：${file}`);
        process.exit(0);
    } catch (e) {
        if (k === 3) {
            console.error(`签名失败：${file}`);
            process.exit(1);
        }
        console.log(`签名没成，第 ${k + 1} 次再试…`);
    }
}
