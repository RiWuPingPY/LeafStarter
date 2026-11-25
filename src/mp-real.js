// 真程序：临时联机（M5）——游戏开了局域网就提示邀请朋友、生成邀请码、用邀请码建实例并直接进。后端在 commands/mp.rs
import { $, toast, esc } from './ui.js';
import { icon } from './icons.js';
import { invoke, listen, R } from './real.js';
import { realMpBody } from './views/server-real.js';

const KIND = { lan: '局域网', ipv6: '公网 IPv6', radmin: 'Radmin VPN', server: '你填的地址' };
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
            <div class="actions"><button type="button" class="btn pri" data-act="mp-invite">${icon('link')}生成邀请码</button></div></section>`;
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
        <p class="carry">${icon('package', 15)}<span>带着实例清单：<b>${esc(ver)}</b>${v.miss.length ? `，另有 ${v.miss.length} 个 Mod 要朋友自己装` : ''}</span></p>
        <div class="friends">${players.length ? players.map((n) => `<div class="friend">${headImg(n)}<span class="tx"><span class="nm">${esc(n)}</span></span><span class="chip ok">在玩</span></div>`).join('') : '<div class="friend empty"><i class="dot busy"></i>把邀请码发给朋友</div>'}</div>
        <div class="actions"><span class="note">局域网端口 ${port || v.addrs[0]?.p || ''} · 关掉游戏就失效</span><span class="sp"></span><button type="button" class="btn ghost" data-act="mp-invite">${icon('refresh', 13)}重新生成</button></div>
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
        body += `<dl class="facts"><dt>实例</dt><dd>${esc(inv.name)}</dd><dt>游戏</dt><dd>${esc(inv.mc)} ${inv.loader ? `${esc(inv.loader)} ${esc(inv.lv)}` : '原版'}</dd><dt>Mod</dt><dd>${inv.mods.length} 个从 Modrinth 装${inv.miss.length ? `；${inv.miss.length} 个要找主人要` : ''}</dd>
            <dt>地址</dt><dd>${r.reach.map((x) => `${x.ok ? '✅' : '✖'} ${esc(KIND[x.kind] || x.kind)} <span class="mono">${esc(x.addr)}</span>`).join('<br>')}</dd></dl>
            ${r.hours >= 24 ? `<p class="callout warn">${icon('alert', 14)}<span>这个邀请码是 ${r.hours} 小时前生成的，主人可能已经关了游戏。</span></p>` : ''}
            ${ok.length ? '' : `<p class="callout warn">${icon('alert', 14)}<span>现在一个地址都连不上：主人可能没开着局域网，或者你们不在一个网络里。建好实例后可以等主人开了再进。</span></p>`}
            ${r.same.length ? `<div class="actions"><button type="button" class="btn pri" data-act="mj-go" data-v="${esc(r.same[0])}">${icon('play', 13)}用已有的「${esc(r.same[0].split('/').slice(1).join('/'))}」直接进</button></div><p class="note">版本、加载器一样${inv.mods.length ? '，Mod 也基本一样' : ''}；进不去的话就照着邀请码新建一个。</p>` : ''}
            <label class="field" for="mjDir"><span>新建在哪个游戏目录</span><select id="mjDir">${dirs.map((x) => `<option value="${esc(x.id)}"${x.id === d.dir ? ' selected' : ''}>${esc(x.name)} · ${esc(x.path)}</option>`).join('')}</select></label>
            <label class="field" for="mjName"><span>实例名</span><input id="mjName" type="text" value="${esc(d.name || `${inv.name}（朋友的）`)}"></label>
            <div class="actions"><button type="button" class="btn ${r.same.length ? '' : 'pri'}" data-act="mj-build">${icon('download', 13)}照着建实例，建好直接进</button></div>`;
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
        if (R.mp.invite?.key === key) R.mp.invite = null;
        repaint();
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
                api.launchJoin(i, p.addr);
            } else toast('实例建好了，在左边找到它，启动后从多人游戏里进');
        }, 2500);
    });
    const actions = {
        'mp-hosts': () => hosts(),
        'mp-invite': async () => {
            const i = api.cur();
            const port = Number($('#mpPort')?.value || R.mp.ports[i.id]);
            if (!port || port < 1024 || port > 65535) return toast('先在游戏里「对局域网开放」，或者填一下端口');
            R.mp.ports[i.id] = port;
            try {
                const r = await invoke('mp_invite', { dir: i.dir, name: i.name, port, extra: R.mp.extra || null });
                // fresh：邀请码头几个字落下来的动画只播这一次
                R.mp.invite = { key: i.id, ...r, fresh: true };
                repaint();
                R.mp.invite.fresh = false;
                navigator.clipboard.writeText(r.code).then(() => toast('邀请码复制好了，发给朋友'), () => {});
            } catch (e) {
                toast(String(e), { ms: 8000 });
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
        'mj-go': (el) => {
            const d = S.drawer;
            const i = api.byId(el.dataset.v);
            const addr = d.read.reach.find((x) => x.ok)?.addr || d.read.reach[0]?.addr;
            api.closeDrawer();
            if (i) {
                api.selectGo(i.id);
                api.launchJoin(i, addr);
            }
        },
        'mj-build': async () => {
            const d = S.drawer;
            const dir = $('#mjDir')?.value;
            const name = $('#mjName')?.value.trim();
            if (!dir || !name) return toast('选一个目录、起个名字');
            try {
                const task = await invoke('mp_join_build', { code: d.code, dir, name });
                R.mp.pending = { task, key: `${dir}/${name}`, addr: d.read.reach.find((x) => x.ok)?.addr || d.read.reach[0]?.addr };
                api.closeDrawer();
                toast('开始建实例，装好就自动启动并进朋友的世界；进度在「任务」页', { ms: 7000 });
            } catch (e) {
                toast(String(e), { ms: 7000 });
            }
        },
    };
    return { actions };
}
