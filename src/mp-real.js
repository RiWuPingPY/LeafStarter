// 真程序：临时联机（M5）——游戏开了局域网就提示邀请朋友、生成邀请码、用邀请码建实例并直接进。后端在 commands/mp.rs
import { $, toast, esc } from './ui.js';
import { icon } from './icons.js';
import { invoke, listen, R } from './real.js';

const KIND = { lan: '局域网（同一个 Wi-Fi）', ipv6: '公网 IPv6（直连，最快）', radmin: 'Radmin VPN', server: '你填的地址' };

// 「联机」标签顶上的临时联机卡片
export function mpCard(c) {
    const i = c.inst;
    const M = c.R.mp || {};
    const port = M.ports?.[i.id] || '';
    const hosts = M.hosts;
    const inv = M.invite?.key === i.id ? M.invite : null;
    return `<section class="card mp-card"><h3>${icon('users', 18)}临时联机<span class="chip">你开着游戏，朋友进来一起玩</span></h3>
        <p class="note">进游戏 → 暂停 → 「对局域网开放」，启动器会认出端口、在右下角提示你。也可以自己填端口。朋友用 LeafStarter 时，发邀请码最省事：他那边自动建一样的实例、装好 Mod 直接进。</p>
        <div class="mp-hosts">${!hosts ? '<span class="note">正在看这台电脑的地址…</span>' : hosts.length ? hosts.map((h) => `<span class="chip ${h.kind === 'ipv6' ? 'ok' : ''}">${KIND[h.kind] || h.kind}：<span class="mono">${esc(h.host)}</span></span>`).join('') : '<span class="chip warn">没有朋友能直接连的地址</span>'}</div>
        <div class="mp-row"><label class="field" for="mpPort"><span>游戏端口</span><input id="mpPort" type="text" inputmode="numeric" value="${esc(String(port))}" placeholder="开局域网后自动填"></label>
            <label class="field grow" for="mpExtra"><span>另外的地址（樱花穿透、自己的服务器，可以不填）</span><input id="mpExtra" type="text" class="mono" value="${esc(M.extra || '')}" placeholder="比如 frp-xxx.sakurafrp.com:12345"></label></div>
        <div class="actions"><button type="button" class="btn pri" data-act="mp-invite">${icon('link', 13)}生成邀请码</button><button type="button" class="btn" data-act="mp-join">${icon('download', 13)}用邀请码加入</button><button type="button" class="btn ghost" data-act="mp-hosts">${icon('refresh', 13)}重新看地址</button></div>
        ${inv ? `<pre class="cmdbox mono mp-code" id="mpCode">${esc(inv.code)}</pre>
            <div class="actions"><button type="button" class="btn sm" data-act="mp-copy" data-v="code">${icon('copy', 13)}复制邀请码</button><button type="button" class="btn sm ghost" data-act="mp-copy" data-v="addr">${icon('copy', 13)}只复制地址（朋友用别的启动器）</button></div>
            <p class="note">邀请码里有：${inv.invite.addrs.map((a) => esc(KIND[a.k] || a.k)).join('、')}；${inv.invite.mods.length} 个 Mod 从 Modrinth 下${inv.invite.miss.length ? `，${inv.invite.miss.length} 个 Modrinth 上没有的要朋友自己装` : ''}。公网 IPv6 第一次连时，Windows 可能弹防火墙提示，点「允许」。</p>` : ''}
        <p class="note">P2P 组网（EasyTier，两边都在 NAT 后面也能连）和樱花穿透一键开隧道还在做。</p>
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
    R.mp = { hosts: null, ports: {}, invite: null, extra: '', pending: null };
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
        if (R.mp.ports[key] === port) return;
        R.mp.ports[key] = port;
        const i = api.byId(key);
        toast(`「${i?.name || '游戏'}」对局域网开放了（端口 ${port}）。邀请朋友？`, { ms: 15000, action: { label: '生成邀请码', run: () => {
            if (i) api.selectGo(i.id);
            api.setTab('servers');
            requestAnimationFrame(() => actions['mp-invite']());
        } } });
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
            R.mp.extra = $('#mpExtra')?.value || '';
            if (!port) return toast('先在游戏里「对局域网开放」，或者填一下端口');
            R.mp.ports[i.id] = port;
            try {
                const r = await invoke('mp_invite', { dir: i.dir, name: i.name, port, extra: R.mp.extra || null });
                R.mp.invite = { key: i.id, ...r };
                repaint();
                navigator.clipboard.writeText(r.code).then(() => toast('邀请码复制好了，发给朋友'), () => {});
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        'mp-copy': (el) => {
            const v = R.mp.invite;
            if (!v) return;
            const text = el.dataset.v === 'code' ? v.code : v.invite.addrs.map((a) => (a.h.includes(':') ? `[${a.h}]:${a.p}` : `${a.h}:${a.p}`)).join('\n');
            navigator.clipboard.writeText(text).then(() => toast('复制好了'));
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
