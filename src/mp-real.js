// 真程序：临时联机（M5）——游戏开了局域网就提示邀请朋友、生成邀请码、用邀请码建实例并直接进。后端在 commands/mp.rs
import { $, toast, esc, ask } from './ui.js';
import { icon } from './icons.js';
import { invoke, listen, R } from './real.js';
import { realMpBody } from './views/server-real.js';

const KIND = { lan: '局域网', ipv6: '公网 IPv6', radmin: 'Radmin VPN', server: '你填的地址', p2p: 'P2P（不在同一个网络也行）' };
const HOURS = [[0, '一直有效'], [2, '2 小时'], [24, '24 小时'], [168, '7 天']];
const ico = (tone, name) => `<span class="ico ${tone}">${icon(name, 16)}</span>`;
const chip = (cls, text) => `<span class="chip ${cls}">${text}</span>`;
const running = (c, i) => {
    const r = c.RUN.get(i.id);
    return !!r && r.phase !== 'crashed';
};

// 「联机 → 现在一起玩」：没开游戏 → 开了游戏等对局域网开放 → 开了局域网 → 邀请码发出去、看谁进来了
export function mpNow(c) {
    return roomCard(c) + altCard(c);
}

function roomCard(c) {
    const i = c.inst;
    const M = c.R.mp || {};
    const port = M.ports?.[i.id];
    const inv = M.invite?.key === i.id ? M.invite : null;
    const join = `<button type="button" class="btn sm ghost push" data-act="mp-join">${icon('download', 13)}用邀请码加入</button>`;
    if (inv) return openRoom(c, i, inv, port);
    if (port) {
        return `<section class="card room"><h3>${ico('violet', 'users')}邀请朋友一起玩${chip('ok', '已对局域网开放')}${join}</h3>
            <p>端口 <b class="mono">${port}</b>，生成邀请码发给朋友就行。</p>
            <div class="actions"><button type="button" class="btn pri" data-act="mp-invite">${icon('link')}生成邀请码</button>${inviteOpts(c)}</div></section>`;
    }
    if (running(c, i)) {
        return `<section class="card room"><h3>${ico('violet', 'users')}邀请朋友一起玩${chip('ok', '游戏在运行')}${join}</h3>
            <p class="waitline"><i class="dot busy"></i>在游戏的暂停菜单里点「对局域网开放」</p>
            <div class="actions mp-manual"><input id="mpPort" type="text" inputmode="numeric" placeholder="或者手动填端口" aria-label="端口"><button type="button" class="btn" data-act="mp-invite">生成邀请码</button></div></section>`;
    }
    const mods = i.loader ? `同样的 ${i.mods.filter((m) => m.on).length} 个 Mod` : '同样的版本';
    return `<section class="card room"><h3>${ico('violet', 'users')}邀请朋友一起玩${chip('', '没开房间')}${join}</h3>
        <ol class="how">
            <li><b>进游戏</b><span>启动「${esc(i.name)}」，进一个世界</span></li>
            <li><b>对局域网开放</b><span>暂停菜单里点一下，启动器会认出来</span></li>
            <li><b>发邀请码</b><span>朋友粘贴后自动装好${mods}，直接进来</span></li>
        </ol>
        <div class="actions"><button type="button" class="btn pri" data-act="launch"${i.installing ? ' disabled' : ''}>${icon('play')}启动游戏</button></div></section>`;
}

// 生成邀请码时的选项：开不开 P2P、有效期
function inviteOpts(c) {
    const M = c.R.mp || {};
    return `<label class="tick" title="不在同一个网络（不同小区、学校）也能进；第一次要下载约 30 MB 的 EasyTier"><input type="checkbox" id="mpP2p"${M.p2p !== false ? ' checked' : ''}>P2P</label>
        <select id="mpHours" aria-label="邀请码有效期">${HOURS.map(([h, t]) => `<option value="${h}"${(M.hours || 0) === h ? ' selected' : ''}>${t}</option>`).join('')}</select>`;
}

function openRoom(c, i, inv, port) {
    const v = inv.invite;
    // 邀请码有两千来字，只把开头当「门牌号」显示，复制的是整串
    const head = inv.code.slice(0, 13);
    const code = [...head].map((ch, k) => `<span style="--k:${k}">${esc(ch)}</span>`).join('') + '<span><i>…</i></span>';
    const players = Object.keys(c.R.mp.players?.[i.id] || {});
    const LD = { fabric: 'Fabric', forge: 'Forge', neoforge: 'NeoForge', quilt: 'Quilt' };
    const ver = `${v.mc}${v.loader ? ` · ${LD[v.loader] || v.loader} ${v.lv}` : ''}${v.mods.length ? ` · ${v.mods.length} 个 Mod` : ''}`;
    return `<section class="card room open"><h3>${ico('violet', 'users')}房间开着${chip('ok', `<i class="dot run"></i>${players.length ? `${players.length} 个朋友在玩` : '等朋友加入'}`)}
            <button type="button" class="btn sm ghost push" data-act="mp-join">${icon('download', 13)}用邀请码加入</button></h3>
        <div class="invite">
            <div class="code mono${inv.fresh ? ' fresh' : ''}" aria-label="邀请码">${code}</div>
            <div class="actions">
                <button type="button" class="btn pri" data-act="mp-copy" data-v="code">${icon('copy')}复制邀请码</button>
                <button type="button" class="btn" data-act="mp-copy" data-v="link">${icon('link')}复制链接</button>
            </div>
        </div>
        <p class="carry">${icon('package', 15)}<span>带着实例清单：<b>${esc(ver)}</b>${v.cf?.length ? `（${v.cf.length} 个从 CurseForge 下）` : ''}${v.miss.length ? `，另有 ${v.miss.length} 个 Mod 要朋友自己装：${esc(v.miss.join('、'))}` : ''}</span></p>
        <div class="friends">${players.length ? players.map((n) => `<div class="friend">${headImg(n)}<span class="tx"><span class="nm">${esc(n)}</span></span><span class="chip ok">在玩</span></div>`).join('') : '<div class="friend empty"><i class="dot busy"></i>把邀请码发给朋友</div>'}</div>
        <p class="note">${v.et ? `${icon('globe', 13)} P2P 房间开着：朋友不在同一个网络也能进${c.R.mp.p2pOff ? '（已关掉）' : ''}` : inv.p2pError ? `P2P 没开起来：${esc(inv.p2pError)}` : '没开 P2P：只有同一个网络、公网 IPv6 或 Radmin 能进'}${v.exp ? ` · 邀请码 ${new Date(v.exp).toLocaleString()} 失效` : ''}</p>
        <div class="actions"><span class="note">局域网端口 ${port || v.addrs[0]?.p || ''} · 关掉游戏就失效</span><span class="sp"></span>
            ${v.et && !c.R.mp.p2pOff ? `<button type="button" class="btn ghost" data-act="mp-close" title="关掉以后这个邀请码就进不来了">${icon('x', 13)}关掉房间</button>` : ''}
            <button type="button" class="btn ghost" data-act="mp-invite">${icon('refresh', 13)}重新生成</button></div>
        <div class="actions">${inviteOpts(c)}
            <button type="button" class="btn sm ghost" data-act="pack-export" title="朋友用 PCL、HMCL 之类：把整合包发给他，他导入后用地址进">${icon('package', 13)}给用别的启动器的朋友：导出整合包</button>
            ${i.loader ? `<button type="button" class="btn sm ghost" data-act="mp-offline" title="原版的局域网世界总要正版验证，离线账户进不来；装上 Custom LAN 后开局域网时能关掉">${icon('users', 13)}朋友用离线账户？</button>` : ''}</div>
    </section>`;
}

// 朋友的头像：没有皮肤就用首字母
const headImg = (n) => `<span class="tile" style="--h:${[...n].reduce((a, ch) => (a * 31 + ch.charCodeAt(0)) % 360, 7)}">${esc(n[0] || '?')}</span>`;

function altCard(c) {
    const i = c.inst;
    const M = c.R.mp || {};
    const port = M.ports?.[i.id];
    const hosts = M.hosts;
    const TONE = { ipv6: ['ok', 'zap'], lan: ['sky', 'wifi'], radmin: ['violet', 'link'] };
    const TAG = { ipv6: '最快', lan: '同一个 Wi-Fi', radmin: '已连上' };
    const rows = !hosts ? '<div class="addr dim"><span></span><span class="tx"><span class="sb">正在看这台电脑的地址…</span></span></div>'
        : hosts.length ? hosts.map((h) => {
            const [tone, ic] = TONE[h.kind] || ['sky', 'globe'];
            const addr = port ? (h.host.includes(':') ? `[${h.host}]:${port}` : `${h.host}:${port}`) : '';
            return `<div class="addr${addr ? '' : ' dim'}"><span class="ico ${tone}">${icon(ic === 'wifi' ? 'globe' : ic, 16)}</span>
                <span class="tx"><span class="nm">${KIND[h.kind] || h.kind}${chip(h.kind === 'ipv6' ? 'ok' : '', TAG[h.kind] || '')}</span></span>
                ${addr ? `<span class="av mono">${esc(addr)}</span><button type="button" class="iconbtn sm" data-act="mp-copy" data-v="addr" data-a="${esc(addr)}" aria-label="复制地址" title="复制">${icon('copy', 14)}</button>` : '<span class="av note">开了局域网才有地址</span>'}
            </div>`;
        }).join('')
            : '<div class="addr dim"><span class="ico">' + icon('alert', 16) + '</span><span class="tx"><span class="nm">没有朋友能直接连的地址</span></span><span class="av"></span></div>';
    return `<section class="card alt"><h3>${ico('sky', 'globe')}朋友没装 LeafStarter？<button type="button" class="iconbtn sm push" data-act="mp-hosts" aria-label="重新看地址" title="重新看地址">${icon('refresh', 14)}</button></h3>
        <p>给他地址就行。Mod 要他自己装对，可以把整合包一起发过去。</p>
        <div class="addrs">${rows}</div>
        ${i.loader ? `<div class="actions"><button type="button" class="btn" data-act="pack-export">${icon('package')}导出整合包</button></div>` : ''}
    </section>`;
}

const inst = (key) => esc(key.split('/').slice(1).join('/'));

// 邀请码带的 Mod：从哪装、几个要找主人要
function modLine(inv) {
    const parts = [];
    if (inv.mods.length) parts.push(`${inv.mods.length} 个从 Modrinth 装`);
    if (inv.cf?.length) parts.push(`${inv.cf.length} 个从 CurseForge 装`);
    if (inv.miss.length) parts.push(`${inv.miss.length} 个要找主人要（${esc(inv.miss.join('、'))}）`);
    return parts.join('；') || '没有 Mod';
}

// 本机能怎么进：一样的实例直接进 > 别的组正好是这套就切过去 > 版本一样的实例里新建一组 Mod（不用重下游戏）
function joinChoices(r) {
    const out = [];
    let pri = true;
    const cls = () => (pri ? ((pri = false), 'btn pri') : 'btn');
    if (r.same.length) {
        out.push(`<button type="button" class="${cls()}" data-act="mj-go" data-v="${esc(r.same[0])}">${icon('play', 13)}用「${inst(r.same[0])}」直接进</button>`);
    }
    for (const g of (r.groups || []).slice(0, 2)) {
        out.push(`<button type="button" class="${cls()}" data-act="mj-group" data-v="${esc(g.key)}" data-g="${esc(g.group)}">${icon('layers', 13)}切到「${esc(g.name)}」的「${esc(g.groupName)}」组再进</button>`);
    }
    for (const f of (r.fit || []).slice(0, 2)) {
        out.push(`<button type="button" class="${cls()}" data-act="mj-group" data-v="${esc(f.key)}" data-g="">${icon('plus', 13)}在「${esc(f.name)}」里新建一组 Mod（${f.need ? `下 ${f.need} 个` : '都有了'}${f.have ? `，${f.have} 个本机已有` : ''}）</button>`);
    }
    if (!out.length) return '';
    const tip = r.groups?.length || r.fit?.length ? '版本一样就不用重下游戏：Mod 单独放一组，玩完在实例的 Mod 页一键换回来。' : '版本、加载器、Mod 都一样。';
    return `<div class="actions mj-choices">${out.join('')}</div><p class="note">${tip}</p>`;
}

export function mpJoinDrawer(c) {
    const d = c.S.drawer;
    const r = d.read;
    const dirs = c.DIRS.filter((x) => x.access === 'direct');
    let body = `<label class="field" for="mjCode"><span>邀请码</span><textarea id="mjCode" rows="3" class="mono" placeholder="LS1.……">${esc(d.code || '')}</textarea></label>
        <div class="actions"><button type="button" class="btn" data-act="mj-read"${d.busy ? ' disabled' : ''}>${d.busy ? '正在看…' : '看看是什么'}</button></div>`;
    if (d.error) body += `<p class="callout warn">${icon('alert', 14)}<span>${esc(d.error)}</span></p>`;
    if (r) {
        const inv = r.invite;
        const ok = r.reach.filter((x) => x.ok);
        body += `<dl class="facts"><dt>实例</dt><dd>${esc(inv.name)}</dd><dt>游戏</dt><dd>${esc(inv.mc)} ${inv.loader ? `${esc(inv.loader)} ${esc(inv.lv)}` : '原版'}</dd><dt>Mod</dt><dd>${modLine(inv)}</dd>
            <dt>地址</dt><dd>${r.reach.map((x) => `${x.ok ? '✅' : '✖'} ${esc(KIND[x.kind] || x.kind)} <span class="mono">${esc(x.addr)}</span>`).join('<br>')}</dd></dl>
            ${r.hours >= 24 ? `<p class="callout warn">${icon('alert', 14)}<span>这个邀请码是 ${r.hours} 小时前生成的，主人可能已经关了游戏。</span></p>` : ''}
            ${ok.length ? '' : `<p class="callout warn">${icon('alert', 14)}<span>现在一个地址都连不上：主人可能没开着局域网，或者你们不在一个网络里。建好实例后可以等主人开了再进。</span></p>`}
            ${joinChoices(r)}
            <label class="field" for="mjDir"><span>新建在哪个游戏目录</span><select id="mjDir">${dirs.map((x) => `<option value="${esc(x.id)}"${x.id === d.dir ? ' selected' : ''}>${esc(x.name)} · ${esc(x.path)}</option>`).join('')}</select></label>
            <label class="field" for="mjName"><span>实例名</span><input id="mjName" type="text" value="${esc(d.name || `${inv.name}（朋友的）`)}"></label>
            <div class="actions"><button type="button" class="btn ${r.same.length || r.groups?.length || r.fit?.length ? '' : 'pri'}" data-act="mj-build">${icon('download', 13)}照着建实例，建好直接进</button></div>
            ${r.fit?.length ? '<p class="note">要重下整个游戏，比上面在已有实例里建一组 Mod 慢得多。</p>' : ''}`;
    }
    return `<section class="drawer" role="dialog" aria-label="用邀请码加入"><header class="dr-head"><h2>用邀请码加入</h2><span class="sp"></span><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭">${icon('x')}</button></header><div class="dr-body">${body}</div></section>`;
}

export function createMp(api) {
    const { S } = api;
    R.mp = { hosts: null, ports: {}, invite: null, extra: '', pending: null, players: {} };
    const repaint = () => S.page === 'home' && S.tab === 'servers' && api.paintBody();
    async function hosts() {
        R.mp.hosts = null;
        repaint();
        R.mp.hosts = await invoke('mp_hosts').catch(() => []);
        repaint();
    }
    hosts();
    // 点了邀请链接（leafstarter://join/LS1.…）打开的：直接弹「用邀请码加入」并读出来
    const openLink = (l) => {
        const code = decodeURIComponent(String(l).replace(/^leafstarter:\/\/join\//i, ''));
        if (!code.startsWith('LS1.')) return toast('这个链接不是 LeafStarter 的邀请');
        api.openDrawer({ kind: 'mpjoin', dir: api.defaultDir(), code });
        requestAnimationFrame(() => actions['mj-read']());
    };
    setTimeout(() => invoke('mp_take_link').then((l) => l && openLink(l)).catch(() => {}), 600);
    listen('deep-link', (e) => {
        invoke('mp_take_link').catch(() => {});
        openLink(e.payload);
    });
    // 游戏开了局域网：记下端口，右下角问一句
    listen('lan-open', (e) => {
        const { key, port } = e.payload;
        if (api.plugOff?.('mp') || R.mp.ports[key] === port) return;
        R.mp.ports[key] = port;
        const i = api.byId(key);
        toast(`「${i?.name || '游戏'}」对局域网开放了（端口 ${port}）。邀请朋友？`, { ms: 15000, action: { label: '生成邀请码', run: () => {
            if (i) api.selectGo(i.id);
            api.setTab('servers');
            requestAnimationFrame(() => actions['mp-invite']());
        } } });
        repaint();
    });
    // 朋友进出：开了局域网以后，游戏日志里的 joined / left the game
    listen('lan-player', (e) => {
        const { key, name, joined } = e.payload;
        if (!R.mp.ports[key]) return;
        const p = (R.mp.players[key] ||= {});
        if (joined) {
            if (p[name]) return;
            p[name] = Date.now();
            toast(`${name} 进来了`);
        } else delete p[name];
        repaint();
    });
    // 游戏关了：房间跟着没了
    listen('launch-state', (e) => {
        const { key, phase } = e.payload;
        if (!['exited', 'crashed', 'failed'].includes(phase)) return;
        delete R.mp.ports[key];
        delete R.mp.players[key];
        if (R.mp.invite?.key === key) {
            if (R.mp.invite.invite?.et) invoke('mp_p2p_stop', { key, room: null }).catch(() => {});
            R.mp.invite = null;
        }
        if (R.mp.joined?.key === key) {
            invoke('mp_p2p_stop', { key: null, room: R.mp.joined.room }).catch(() => {});
            R.mp.joined = null;
        }
        repaint();
    });
    // 联机进不去：后端从日志里认出了原因
    listen('mp-hint', (e) => {
        const i = api.byId(e.payload.key);
        toast(`${i ? `「${i.name}」` : ''}${e.payload.text}`, { ms: 15000 });
    });
    // 照邀请码建的实例装好了：直接进
    listen('task-update', (e) => {
        const p = R.mp.pending;
        if (!p || e.payload.id !== p.task || e.payload.state === 'running') return;
        R.mp.pending = null;
        if (e.payload.state !== 'done') return;
        setTimeout(() => {
            const i = api.byId(p.key);
            if (i) {
                api.selectGo(i.id);
                go(i, p.addr, p.code);
            } else toast('实例建好了，在左边找到它，启动后从多人游戏里进');
        }, 2500);
    });
    // 进朋友的世界：直连地址直接进；P2P 先起 EasyTier、等转发通了再进
    async function go(i, addr, code) {
        if (addr !== 'p2p') return api.launchJoin(i, addr);
        toast('正在通过 P2P 连主人（最多等 45 秒）…', { ms: 6000 });
        try {
            const r = await invoke('mp_p2p_join', { code });
            R.mp.joined = { key: i.id, room: r.room };
            api.launchJoin(i, r.addr);
        } catch (e) {
            toast(String(e), { ms: 12000 });
        }
    }
    const actions = {
        'mp-hosts': () => hosts(),
        'mp-invite': async () => {
            const i = api.cur();
            const port = Number($('#mpPort')?.value || R.mp.ports[i.id]);
            if (!port || port < 1024 || port > 65535) return toast('先在游戏里「对局域网开放」，或者填一下端口');
            R.mp.ports[i.id] = port;
            if ($('#mpP2p')) R.mp.p2p = $('#mpP2p').checked;
            if ($('#mpHours')) R.mp.hours = Number($('#mpHours').value) || 0;
            const p2p = R.mp.p2p !== false;
            if (p2p) toast('正在开 P2P 房间（第一次要先下 EasyTier）…', { ms: 4000 });
            try {
                const r = await invoke('mp_invite', { dir: i.dir, name: i.name, port, extra: R.mp.extra || null, p2p, hours: R.mp.hours || null });
                R.mp.p2pOff = false;
                if (r.p2pError) toast(`P2P 没开起来，邀请码里只有直连地址：${r.p2pError}`, { ms: 9000 });
                // fresh：邀请码头几个字落下来的动画只播这一次
                R.mp.invite = { key: i.id, ...r, fresh: true };
                repaint();
                R.mp.invite.fresh = false;
                navigator.clipboard.writeText(r.code).then(() => toast('邀请码复制好了，发给朋友'), () => {});
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        // 关掉 P2P 房间：这个邀请码就进不来了（已经在玩的朋友会掉线）
        'mp-close': async () => {
            const i = api.cur();
            if (!(await ask('关掉 P2P 房间？已经在玩的朋友会掉线，这个邀请码也就进不来了。'))) return;
            await invoke('mp_p2p_stop', { key: i.id, room: null });
            R.mp.p2pOff = true;
            repaint();
        },
        // 朋友用离线账户：给主人的实例装 Custom LAN（开局域网时能关正版验证）
        'mp-offline': async () => {
            const i = api.cur();
            if (!(await ask(`给「${i.name}」装 Custom LAN？\n装好后重开游戏，「对局域网开放」的界面里能把「正版验证」关掉，朋友用离线账户就能进。只在你信得过的朋友之间这么做。`))) return;
            try {
                const plan = await invoke('content_plan', { dir: i.dir, name: i.name, kind: 'mod', project: 'custom-lan', version: null, source: 'modrinth' });
                await invoke('content_install', { dir: i.dir, name: i.name, kind: 'mod', items: plan.items || plan });
                toast('开始装 Custom LAN，装好重开游戏', { ms: 6000 });
            } catch (e) {
                toast(`没装上：${e}`, { ms: 8000 });
            }
        },
        'mp-copy': (el) => {
            const v = R.mp.invite;
            const k = el.dataset.v;
            const text = k === 'addr' ? el.dataset.a : !v ? '' : k === 'link' ? `leafstarter://join/${v.code}` : v.code;
            if (!text) return;
            navigator.clipboard.writeText(text).then(() => toast(k === 'addr' ? '地址复制好了' : '复制好了，发给朋友'), () => toast('没复制上，再点一次'));
        },
        // 「现在一起玩 / 常驻服务器」：指示条滑过去，只换下面的内容
        'mp-mode': (el) => {
            S.mpMode = el.dataset.v;
            api.save?.();
            const m = $('.modes');
            const body = $('#mpBody');
            if (!m || !body) return api.paintBody();
            m.dataset.on = S.mpMode;
            m.querySelectorAll('.mode').forEach((b) => {
                const on = b.dataset.v === S.mpMode;
                b.classList.toggle('on', on);
                b.setAttribute('aria-selected', on);
            });
            body.innerHTML = realMpBody(api.ctx(), S.mpMode);
            body.classList.remove('swap');
            void body.offsetWidth;
            body.classList.add('swap');
        },
        'mp-join': () => api.openDrawer({ kind: 'mpjoin', dir: api.defaultDir() }),
        'mj-read': async () => {
            const d = S.drawer;
            d.code = $('#mjCode')?.value.trim();
            if (!d.code) return toast('先粘贴邀请码');
            d.busy = true;
            d.error = '';
            d.read = null;
            api.paintDrawer();
            try {
                d.read = await invoke('mp_invite_read', { code: d.code });
            } catch (e) {
                d.error = String(e);
            }
            d.busy = false;
            api.paintDrawer();
        },
        'mj-go': async (el) => {
            const d = S.drawer;
            const i = api.byId(el.dataset.v);
            const addr = d.read.reach.find((x) => x.ok)?.addr || d.read.reach[0]?.addr;
            api.closeDrawer();
            if (i) {
                api.selectGo(i.id);
                go(i, addr, d.code);
            }
        },
        'mj-group': async (el) => {
            const d = S.drawer;
            const i = api.byId(el.dataset.v);
            if (!i) return toast('找不到这个实例，重新扫描一下');
            try {
                const task = await invoke('mp_join_group', { code: d.code, dir: i.dir, name: i.name, group: el.dataset.g || null });
                R.mp.pending = { task, key: i.id, addr: d.read.reach.find((x) => x.ok)?.addr || d.read.reach[0]?.addr, code: d.code };
                api.closeDrawer();
                toast(el.dataset.g ? '换好 Mod 组就自动启动并进朋友的世界' : '开始下朋友的 Mod，下好就自动启动并进朋友的世界；进度在「任务」页', { ms: 7000 });
            } catch (e) {
                toast(String(e), { ms: 7000 });
            }
        },
        'mj-build': async () => {
            const d = S.drawer;
            const dir = $('#mjDir')?.value;
            const name = $('#mjName')?.value.trim();
            if (!dir || !name) return toast('选一个目录、起个名字');
            try {
                const task = await invoke('mp_join_build', { code: d.code, dir, name });
                R.mp.pending = { task, key: `${dir}/${name}`, addr: d.read.reach.find((x) => x.ok)?.addr || d.read.reach[0]?.addr, code: d.code };
                api.closeDrawer();
                toast('开始建实例，装好就自动启动并进朋友的世界；进度在「任务」页', { ms: 7000 });
            } catch (e) {
                toast(String(e), { ms: 7000 });
            }
        },
    };
    return { actions };
}
