// ls-guard：LeafStarter 源站接口的防刷守护（root 运行，systemd 单元 ls-guard.service）
//
// 能管什么、不能管什么：
// - 管不了「流量型」DDoS：几 G 的流量在到达这台机器之前就把带宽占满了，腾讯云会把 IP 拉进黑洞，
//   服务器上的程序无能为力。也不做「关机」：关机等于自己把自己打挂，还会带走同机的抽卡、叶脉号、MC。
// - 管的是应用层的刷接口和连接洪水，并且优先保住同机别的项目：
//   1. 单个 IP 狂刷 /ls/（一分钟上千次，远超任何正常使用，学校机房共用一个出口 IP 也到不了）→ 在防火墙里临时封它，
//      10 分钟 → 1 小时 → 24 小时逐级加长；
//   2. 整体被刷（分布式）或者出口带宽快满、而且 LeafStarter 占了一大块 → 「防护模式」：只把 /ls/ 关掉（nginx 直接断开），
//      启动器自动改走 Cloudflare Worker，玩家只是慢一点；15 分钟 → 1 小时 → 4 小时后自动解除；
//   3. 内核层：每个 IP 每秒新建连接（SYN）限 50 个（突发 100），多出来的丢掉，防连接洪水；不用 conntrack，不会把连接表撑满。
// 防火墙规则放在单独的表 inet ls_guard 里：默认放行、只管 8443 端口，不碰 SSH 和别的端口，重启后由本程序重建。
//
// 命令：node ls-guard.mjs            守护（systemd 用）
//       node ls-guard.mjs status     看现状：防护模式、封了谁、最近一分钟的量
//       node ls-guard.mjs unban <ip> 解封
//       node ls-guard.mjs shield on|off  手动开关防护模式
import { readFileSync, writeFileSync, existsSync, mkdirSync, statSync, openSync, readSync, closeSync, rmSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { isIP } from 'node:net';

const CONF_FILE = process.env.LS_GUARD_CONF || '/etc/leafstarter/guard.json';
const DEF = {
    log: '/var/log/nginx/ls.access.log', // nginx 给 /ls/ 单独记的日志：$msec $remote_addr $status $body_bytes_sent $request_time
    iface: 'eth0',
    port: 8443,
    linkMbps: 6, // 这台服务器的出口带宽（腾讯云控制台「带宽」那一栏），判断带宽是不是快被吃满
    banPerMin: 1200, // 一个 IP 一分钟打到 /ls/ 多少次（含被限速、被断开的）就封
    banSteps: [600, 3600, 86400], // 封禁时长（秒），6 小时内再犯升一级
    shieldPerMin: 6000, // /ls/ 全部加起来一分钟多少次就进防护模式（约每秒 100 次）
    shieldTxRatio: 0.85, // 出口带宽用到 85%……
    shieldLsShare: 0.4, // ……而且 LeafStarter 占了四成以上，连续 30 秒 → 防护模式，把带宽让给同机别的项目
    shieldSteps: [900, 3600, 14400], // 防护模式时长（秒），6 小时内再触发升一级
    synPerSec: 50, // 每个 IP 每秒新建连接
    synBurst: 100,
    allow: ['127.0.0.1', '::1'], // 永远不封（可以加上自己家里的 IP）
    dryRun: false, // true 时只记日志不动手
};
const conf = { ...DEF, ...(existsSync(CONF_FILE) ? JSON.parse(readFileSync(CONF_FILE, 'utf8')) : {}) };
const RUN = '/run/ls-guard';
const SHIELD = `${RUN}/shield`;
const STATUS = `${RUN}/status.json`;
const STATE = process.env.LS_GUARD_STATE || '/var/lib/ls-guard/state.json';
const TICK = 5; // 秒

const log = (...a) => console.log(new Date().toLocaleString('zh-CN', { hour12: false }), ...a);
const nft = (args, input) => execFileSync('nft', args, { input, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });

// ---------- 防火墙表 ----------
function setupNft() {
    // add 是幂等的：已有的封禁名单保留，规则链清空重建
    nft(['-f', '-'], `
add table inet ls_guard
add set inet ls_guard ban4 { type ipv4_addr; flags timeout; }
add set inet ls_guard ban6 { type ipv6_addr; flags interval, timeout; }
add set inet ls_guard syn4 { type ipv4_addr; size 65535; flags dynamic, timeout; timeout 60s; }
add set inet ls_guard syn6 { type ipv6_addr; size 65535; flags dynamic, timeout; timeout 60s; }
add chain inet ls_guard input { type filter hook input priority -10; policy accept; }
flush chain inet ls_guard input
add rule inet ls_guard input tcp dport ${conf.port} ip saddr @ban4 counter drop
add rule inet ls_guard input tcp dport ${conf.port} ip6 saddr @ban6 counter drop
add rule inet ls_guard input tcp dport ${conf.port} tcp flags & (fin|syn|rst|ack) == syn update @syn4 { ip saddr limit rate over ${conf.synPerSec}/second burst ${conf.synBurst} packets } counter drop
add rule inet ls_guard input tcp dport ${conf.port} tcp flags & (fin|syn|rst|ack) == syn update @syn6 { ip6 saddr limit rate over ${conf.synPerSec}/second burst ${conf.synBurst} packets } counter drop
`);
}

// IPv6 按 /64 封（一个家庭 / 一台云服务器通常拿一整段）
const v6net = (ip) => {
    const full = ip.includes('::') ? (() => {
        const [a, b] = ip.split('::');
        const x = a ? a.split(':') : [];
        const y = b ? b.split(':') : [];
        return [...x, ...Array(8 - x.length - y.length).fill('0'), ...y];
    })() : ip.split(':');
    return `${full.slice(0, 4).join(':')}::/64`;
};
const isPrivate = (ip) => /^(10\.|127\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|169\.254\.|::1$|fe80:|fc|fd)/i.test(ip);

function ban(ip, secs, why) {
    const v = isIP(ip);
    if (!v || isPrivate(ip) || conf.allow.includes(ip)) return false;
    log(`封 ${ip} ${secs / 60} 分钟：${why}${conf.dryRun ? '（只记录）' : ''}`);
    if (conf.dryRun) return true;
    try {
        if (v === 4) nft(['add', 'element', 'inet', 'ls_guard', 'ban4', `{ ${ip} timeout ${secs}s }`]);
        else nft(['add', 'element', 'inet', 'ls_guard', 'ban6', `{ ${v6net(ip)} timeout ${secs}s }`]);
    } catch (e) {
        log('封禁失败', e.stderr || e.message);
    }
    return true;
}
function unban(ip) {
    const v = isIP(ip);
    if (!v) throw new Error('不是 IP');
    try {
        nft(['delete', 'element', 'inet', 'ls_guard', v === 4 ? 'ban4' : 'ban6', `{ ${v === 4 ? ip : v6net(ip)} }`]);
    } catch (e) {
        if (!/No such file|does not exist/i.test(e.stderr || '')) throw e;
    }
}

// ---------- 状态（再犯升级要记住，重启不丢） ----------
let state = { offenders: {}, shieldLevel: 0, shieldLast: 0, shieldUntil: 0, shieldWhy: '' };
try {
    state = { ...state, ...JSON.parse(readFileSync(STATE, 'utf8')) };
} catch {}
const saveState = () => {
    try {
        mkdirSync(STATE.replace(/\/[^/]+$/, ''), { recursive: true });
        writeFileSync(STATE, JSON.stringify(state));
    } catch (e) {
        log('存状态失败', e.message);
    }
};
const SIX_H = 6 * 3600 * 1000;

function shieldOn(why, now) {
    const level = now - state.shieldLast < SIX_H ? Math.min(state.shieldLevel + 1, conf.shieldSteps.length - 1) : 0;
    const secs = conf.shieldSteps[level];
    state = { ...state, shieldLevel: level, shieldLast: now, shieldUntil: now + secs * 1000, shieldWhy: why };
    log(`防护模式开 ${secs / 60} 分钟：${why}${conf.dryRun ? '（只记录）' : ''}`);
    if (!conf.dryRun) writeFileSync(SHIELD, JSON.stringify({ why, until: new Date(state.shieldUntil).toISOString() }));
    saveState();
}
function shieldOff(why) {
    if (existsSync(SHIELD)) rmSync(SHIELD);
    state.shieldUntil = 0;
    log(`防护模式关：${why}`);
    saveState();
}

// ---------- 读 nginx 日志（接着上次读到的地方；日志轮转后从头读新文件） ----------
let pos = 0;
let ino = 0;
function readNew() {
    let st;
    try {
        st = statSync(conf.log);
    } catch {
        return [];
    }
    if (st.ino !== ino || st.size < pos) {
        // 第一次启动从文件末尾开始（不把旧日志当成新流量）；轮转后的新文件从头读
        pos = ino === 0 ? st.size : 0;
        ino = st.ino;
    }
    if (st.size <= pos) return [];
    const len = Math.min(st.size - pos, 64 * 1024 * 1024);
    const buf = Buffer.alloc(len);
    const fd = openSync(conf.log, 'r');
    try {
        readSync(fd, buf, 0, len, pos);
    } finally {
        closeSync(fd);
    }
    const text = buf.toString('utf8');
    const cut = text.lastIndexOf('\n') + 1; // 半行留到下次
    pos += Buffer.byteLength(text.slice(0, cut));
    return text.slice(0, cut).split('\n').filter(Boolean);
}

// 网卡出口字节数
function txBytes() {
    const line = readFileSync('/proc/net/dev', 'utf8').split('\n').find((l) => l.trim().startsWith(`${conf.iface}:`));
    return line ? Number(line.split(':')[1].trim().split(/\s+/)[8]) : 0;
}

// ---------- 守护 ----------
function daemon() {
    mkdirSync(RUN, { recursive: true });
    if (!conf.dryRun) setupNft();
    // 上次的防护模式还没到期就接着开
    if (state.shieldUntil > Date.now() && !conf.dryRun) writeFileSync(SHIELD, JSON.stringify({ why: state.shieldWhy, until: new Date(state.shieldUntil).toISOString() }));
    else if (existsSync(SHIELD)) rmSync(SHIELD);
    log(`ls-guard 启动：封禁阈值 ${conf.banPerMin}/分钟，防护阈值 ${conf.shieldPerMin}/分钟，出口 ${conf.linkMbps} Mbps${conf.dryRun ? '，只记录不动手' : ''}`);

    const win = []; // 最近 12 个 tick（60 秒）：{ per: Map(ip → n), n, bytes, tx }
    let lastTx = txBytes();
    const tick = () => {
        const now = Date.now();
        const per = new Map();
        let n = 0;
        let bytes = 0;
        for (const l of readNew()) {
            const f = l.split(' ');
            if (f.length < 4) continue;
            const ip = f[1];
            per.set(ip, (per.get(ip) || 0) + 1);
            n++;
            bytes += Number(f[3]) || 0;
        }
        const t = txBytes();
        const tx = Math.max(0, t - lastTx);
        lastTx = t;
        win.push({ per, n, bytes, tx });
        if (win.length > 12) win.shift();

        // 最近一分钟每个 IP 的量
        const minute = new Map();
        let total = 0;
        for (const w of win) {
            total += w.n;
            for (const [ip, c] of w.per) minute.set(ip, (minute.get(ip) || 0) + c);
        }
        // 1. 单个 IP
        for (const [ip, c] of minute) {
            if (c < conf.banPerMin) continue;
            const o = state.offenders[ip];
            const level = o && now - o.last < SIX_H ? Math.min(o.level + 1, conf.banSteps.length - 1) : 0;
            if (o && now - o.last < conf.banSteps[o.level] * 1000) continue; // 还在封着（封禁前排队的请求会继续进日志）
            if (ban(ip, conf.banSteps[level], `一分钟 ${c} 次`)) {
                state.offenders[ip] = { level, last: now };
                saveState();
            }
        }
        // 2. 防护模式
        const last6 = win.slice(-6);
        const mbps = (b, secs) => (b * 8) / secs / 1e6;
        const txFull = last6.length === 6 && last6.every((w) => mbps(w.tx, TICK) >= conf.linkMbps * conf.shieldTxRatio);
        const lsShare = last6.reduce((s, w) => s + w.bytes, 0) / Math.max(1, last6.reduce((s, w) => s + w.tx, 0));
        // 防护模式以标记文件为准（手动 shield on 写的也算）
        let sh = null;
        try {
            sh = JSON.parse(readFileSync(SHIELD, 'utf8'));
        } catch {}
        if (!sh) {
            if (win.length >= 12 && total >= conf.shieldPerMin) shieldOn(`一分钟 ${total} 次，来自 ${minute.size} 个 IP`, now);
            else if (txFull && lsShare >= conf.shieldLsShare) shieldOn(`出口带宽连续 30 秒超过 ${Math.round(conf.shieldTxRatio * 100)}%，LeafStarter 占 ${Math.round(lsShare * 100)}%`, now);
        } else if (now >= (Date.parse(sh.until) || 0) && total < conf.shieldPerMin / 2 && !txFull) {
            shieldOff('到时间了，流量也降下来了');
        }
        // 3. 清掉过期的再犯记录
        for (const [ip, o] of Object.entries(state.offenders)) if (now - o.last > SIX_H + conf.banSteps[conf.banSteps.length - 1] * 1000) delete state.offenders[ip];

        writeFileSync(STATUS, JSON.stringify({
            at: new Date(now).toISOString(), perMinute: total, ips: minute.size,
            top: [...minute].sort((a, b) => b[1] - a[1]).slice(0, 5),
            txMbps: +mbps(last6.reduce((s, w) => s + w.tx, 0), last6.length * TICK || 1).toFixed(2),
            lsMbps: +mbps(last6.reduce((s, w) => s + w.bytes, 0), last6.length * TICK || 1).toFixed(2),
            shield: existsSync(SHIELD) ? JSON.parse(readFileSync(SHIELD, 'utf8')) : null,
        }));
    };
    setInterval(() => {
        try {
            tick();
        } catch (e) {
            log('出错', e.message);
        }
    }, TICK * 1000);
}

// ---------- 命令 ----------
const [cmd, arg] = process.argv.slice(2);
if (!cmd) daemon();
else if (cmd === 'status') {
    const s = existsSync(STATUS) ? JSON.parse(readFileSync(STATUS, 'utf8')) : null;
    console.log(s ? `最近一分钟 /ls/ ${s.perMinute} 次（${s.ips} 个 IP），出口 ${s.txMbps} Mbps，其中 LeafStarter ${s.lsMbps} Mbps` : '守护没在跑');
    if (s?.top?.length) console.log('最多的几个 IP：' + s.top.map(([ip, c]) => `${ip} ${c}`).join('，'));
    console.log(s?.shield ? `防护模式开着：${s.shield.why}，到 ${s.shield.until}` : '防护模式没开');
    for (const set of ['ban4', 'ban6']) {
        try {
            const j = JSON.parse(nft(['-j', 'list', 'set', 'inet', 'ls_guard', set]));
            const els = j.nftables.find((x) => x.set)?.set.elem || [];
            for (const e of els) {
                const v = e.elem || e;
                const ip = typeof v.val === 'object' ? `${v.val.prefix?.addr}/${v.val.prefix?.len}` : v.val;
                console.log(`封着：${ip}，还剩 ${Math.round((v.expires || 0) / 60)} 分钟`);
            }
        } catch {}
    }
} else if (cmd === 'unban') {
    unban(arg);
    delete state.offenders[arg];
    saveState();
    console.log(`解封了 ${arg}`);
} else if (cmd === 'shield') {
    if (arg === 'on') shieldOn('手动打开', Date.now());
    else shieldOff('手动关闭');
} else {
    console.log('用法：node ls-guard.mjs [status | unban <ip> | shield on|off]');
    process.exit(1);
}
