// 真程序：自动更新（M6）——像 PCL 一样，后台下好，关掉启动器再打开就是新版本，不弹安装窗口。后端在 update.rs
import { toast, esc } from './ui.js';
import { icon } from './icons.js';
import { invoke, listen, R } from './real.js';

const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${cur === v ? 'on' : ''}" aria-pressed="${cur === v}">${t}</button>`).join('')}</div>`;

function line(u) {
    const s = u.status;
    switch (s.phase) {
        case 'checking': return '正在看有没有新版本…';
        case 'latest': return '已经是最新版本';
        case 'downloading': return `正在下 ${esc(s.latest)}（${s.pct}%）`;
        case 'ready': return `${esc(s.latest)} 已下好，重启生效`;
        case 'manual': return `有新版本 ${esc(s.latest)}。${esc(s.error)}`;
        case 'error': return `没看成：${esc(s.error)}`;
        default: return u.auto ? '' : '自动更新关着';
    }
}

// 设置页「更新」
export function updatePanel(c) {
    const u = c.R.update;
    if (!u) return '';
    const s = u.status;
    const busy = s.phase === 'checking' || s.phase === 'downloading';
    return `<section class="panel" id="updPanel"><h2 class="ph">更新</h2>
        <div class="kv"><div class="k"><b>LeafStarter ${esc(u.version)}${u.portable ? ' · 便携版' : ''}</b><small>${line(u)}</small>
            ${s.phase === 'downloading' ? `<div class="bar" role="progressbar" aria-valuenow="${s.pct}" aria-valuemin="0" aria-valuemax="100"><i style="width:${s.pct}%"></i></div>` : ''}
            ${(s.phase === 'ready' || s.phase === 'manual') && s.notes ? `<small class="upd-notes">${esc(s.notes)}</small>` : ''}</div>
            <div class="v">${s.phase === 'ready' ? `<button type="button" class="btn sm pri" data-act="upd-restart">${icon('refresh', 13)}现在重启</button>` : `<button type="button" class="btn sm" data-act="upd-check"${busy ? ' disabled' : ''}>${icon('refresh', 13)}检查更新</button>`}</div></div>
        <div class="kv"><div class="k"><b>自动更新</b><small>下好后，下次打开就是新版本</small></div>
            <div class="v"><input type="checkbox" class="switch" id="updAuto"${u.auto ? ' checked' : ''} aria-label="自动更新"></div></div>
        <div class="kv"><div class="k"><b>更新频道</b></div>
            <div class="v">${seg('upd-chan', [['stable', '正式版'], ['beta', '测试版']], u.channel)}</div></div>
    </section>`;
}

export function createUpdate(api) {
    const { S } = api;
    API = api;
    let told = '';
    const repaint = () => S.page === 'settings' && api.paintView();
    async function load() {
        R.update = await invoke('update_status').catch(() => null);
        const s = R.update?.status;
        if (s?.updatedFrom) toast(`已经更新到 ${R.update.version}${s.notes ? `：${s.notes}` : ''}`, { ms: 9000 });
        if (s?.phase === 'ready') told = s.latest;
        repaint();
    }
    load();
    listen('update-state', (e) => {
        if (!R.update) return;
        R.update.status = e.payload;
        const s = e.payload;
        // 下好了：右下角说一声（同一个版本只说一次）
        if (s.phase === 'ready' && told !== s.latest) {
            told = s.latest;
            toast(`新版本 ${s.latest} 下好了，关掉启动器再打开就是新版本`, { ms: 12000, action: { label: '现在重启', run: () => actions['upd-restart']() } });
        }
        if (S.page === 'settings') {
            // 下载中只改进度条，别整页重画
            const bar = document.querySelector('#updPanel .bar i');
            if (s.phase === 'downloading' && bar) bar.style.width = `${s.pct}%`;
            else repaint();
        }
    });
    const save = async () => {
        try {
            await invoke('update_set', { auto: R.update.auto, channel: R.update.channel });
        } catch (e) {
            toast(String(e));
        }
        repaint();
    };
    const actions = {
        'upd-check': async () => {
            try {
                await invoke('update_check');
            } catch {
                // 错误已经在状态里了
            }
            R.update = await invoke('update_status').catch(() => R.update);
            repaint();
        },
        'upd-restart': async () => {
            try {
                await invoke('update_restart');
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        'upd-chan': (el) => {
            R.update.channel = el.dataset.v;
            save();
        },
    };
    const change = (t) => {
        if (t.id !== 'updAuto' || !R.update) return false;
        R.update.auto = t.checked;
        save();
        return true;
    };
    return { actions, change };
}

// ---------- 关于（真程序）：版本、许可、第三方作品（scripts/licenses.mjs 生成的 third-party.json，用到时才读） ----------
let LIC = null;
let licLoading = null;
function loadLic(api) {
    if (licLoading) return;
    licLoading = fetch('./third-party.json')
        .then((r) => r.json())
        .then((j) => { LIC = j; })
        .catch(() => { LIC = { error: true }; })
        .finally(() => api?.S.drawer?.kind === 'about' && api.paintDrawer());
}
let API = null;

// 搜依赖的结果（输入时只换这一块，搜索框不丢焦点）
function licRows(query) {
    const q = (query || '').trim().toLowerCase();
    if (!q || !LIC?.crates) return '';
    const shown = LIC.crates.filter((x) => x[0].toLowerCase().includes(q) || x[2].toLowerCase().includes(q));
    return `<div class="mini-rows lic-list">${shown.slice(0, 60).map(([n, ver, l, r]) => `<div class="mrow"><span class="tx"><span class="nm">${esc(n)} <span class="mono">${esc(ver)}</span></span><span class="sb">${esc(l)}${r ? ` · <span class="mono">${esc(r)}</span>` : ''}</span></span></div>`).join('') || '<div class="mrow"><span class="note">没找到</span></div>'}${shown.length > 60 ? `<div class="mrow"><span class="note">还有 ${shown.length - 60} 个，再多打几个字</span></div>` : ''}</div>`;
}

export function realAboutDrawer(c) {
    const e = c.R.edition && !c.R.edition.error ? c.R.edition : null;
    const v = c.R.info?.version || '';
    if (!LIC) loadLic(API);
    const d = c.S.drawer;
    let deps = '<span class="note">正在读清单…</span>';
    if (LIC?.error) deps = '<span class="note">清单没读到，安装目录里的 THIRD-PARTY-LICENSES.txt 是一样的内容</span>';
    else if (LIC) {
        const by = {};
        for (const [, , l] of LIC.crates) by[l] = (by[l] || 0) + 1;
        const top = Object.entries(by).sort((a, b) => b[1] - a[1]);
        deps = `<div class="mini-rows">${LIC.web.map((w) => `<div class="mrow"><span class="tx"><span class="nm">${esc(w.name)}</span><span class="sb">${esc(w.license)}</span></span></div>`).join('')}
            <div class="mrow"><span class="tx"><span class="nm">Rust 依赖 ${LIC.crates.length} 个</span><span class="sb wrap">${top.map(([k, n]) => `${esc(k)} ${n}`).join(' · ')}</span></span></div></div>
            <label class="field" for="licQ"><span>找一个依赖</span><input id="licQ" type="search" value="${esc(d.q || '')}" placeholder="名字或许可，比如 tokio、MPL" autocomplete="off"></label>
            <div id="licList">${licRows(d.q)}</div>
            <details class="lic-texts"><summary>许可原文（${LIC.texts.length} 份）</summary>${LIC.texts.map((t) => `<details><summary>${esc(t.name)} · ${t.users.length} 个依赖</summary><p class="note">${esc(t.users.join('、'))}</p><pre class="mono">${esc(t.text)}</pre></details>`).join('')}</details>
            <small class="note">清单生成于 ${esc(LIC.generated)}，安装目录里也有一份 THIRD-PARTY-LICENSES.txt</small>`;
    }
    return `<section class="drawer" role="dialog" aria-label="关于">
        <header class="dr-head"><h2>关于</h2><span class="sp"></span><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">
            ${e ? `<div class="about-h">${e.icon ? `<img src="${esc(e.icon)}" alt="" width="44" height="44">` : '<i class="leaf big" aria-hidden="true"></i>'}<span class="tx"><b>${esc(e.name)}</b><small>定制包作者：${esc(e.author || '没写')} · 签名指纹 <span class="mono">${esc(e.fingerprint)}</span></small></span></div>
                <div class="based"><i class="leaf" aria-hidden="true"></i><span><b>基于 LeafStarter ${esc(v)}</b><small>这一行由程序固定显示，定制包里关不掉</small></span></div>` : `<div class="about-h"><i class="leaf big" aria-hidden="true"></i><span class="tx"><b>LeafStarter</b><small>${esc(v)}${c.R.update?.portable ? ' · 便携版' : ''}</small></span></div>`}
            <dl class="facts">
                <dt>许可</dt><dd>Apache-2.0 · Copyright 2026 PingYi</dd>
                <dt>源码</dt><dd class="mono">github.com/RiWuPingPY/LeafStarter</dd>
                <dt>官网</dt><dd class="mono">leafstarter.pingyi.me</dd>
                <dt>名称与署名</dt><dd>改源码另发请换名字，写明「基于 LeafStarter」（TRADEMARKS.md）</dd>
            </dl>
            <div class="field"><span>第三方作品</span>${deps}</div>
            <p class="note">LeafStarter${e ? ` 和「${esc(e.name)}」` : ''}不是 Minecraft 官方产品，与 Mojang 和 Microsoft 无关。</p>
        </div>
    </section>`;
}

export function aboutInput(t) {
    if (t.id !== 'licQ' || !API) return false;
    API.S.drawer.q = t.value;
    const box = document.getElementById('licList');
    if (box) box.innerHTML = licRows(t.value);
    return true;
}
