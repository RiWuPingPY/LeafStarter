// 真程序：定制版（M14）——套用定制包、设置里的定制版面板、制作器。后端在 commands/edition.rs
import { $, toast, esc } from './ui.js';
import { icon } from './icons.js';
import { invoke, R } from './real.js';

const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
const FEATS = [['download', '下载页'], ['ai', 'AI 助手'], ['skin', '皮肤工作台'], ['world', '世界工具'], ['mp', '常驻服务器'], ['sync', '手机电脑同步']];
const COLORS = ['#0E7C86', '#2F7D5B', '#6D4ED6', '#C0392B', '#E67E22', '#2E6FD8', '#1F1D26'];

// 定制包 → 原型里定制版用的那套状态（主题、留哪些功能、极简）
export function applyEdition(api) {
    const e = R.edition;
    const { S } = api;
    const on = !!e && !e.error;
    S.distOn = on;
    const app = $('#app');
    if (on) {
        S.dist = {
            name: e.name, author: e.author, icon: 'grass', color: e.theme.color, font: e.theme.font,
            keep: { ...e.features }, minimal: e.minimal, notice: e.home.notice,
            srv: e.servers[0]?.name || '', addr: e.servers[0]?.address || '', pack: '',
        };
        document.title = `${e.name}（基于 LeafStarter）`;
    } else {
        document.title = 'LeafStarter';
    }
    const word = $('.brand .word');
    if (word) word.textContent = on ? e.name : 'LeafStarter';
    const proto = $('.brand .proto');
    if (proto) {
        proto.textContent = on ? '基于 LeafStarter' : '';
        proto.title = on ? '这一行由程序固定显示，定制包里关不掉' : '';
    }
    for (const [k] of FEATS) app.classList.toggle(`d-no${k}`, on && e.features[k] === false);
    api.applyLook?.();
}

// 实例页顶上：公告、服务器、定制包带的整合包
export function editionCards(c) {
    const e = c.R.edition;
    if (!e || e.error) return '';
    const p = e.pack || {};
    const installed = p.name && c.INSTANCES.some((i) => i.name === p.name);
    return `${e.home.notice ? `<section class="card ed-notice"><h3>${icon('alert', 16)}公告</h3><p>${esc(e.home.notice)}</p></section>` : ''}
        ${e.servers.length ? `<section class="card"><h3>${icon('server', 16)}服务器</h3><div class="rows">${e.servers.map((s) => `<div class="row"><span class="tx"><span class="nm">${esc(s.name || s.address)}</span><span class="sb mono">${esc(s.address)}</span></span><button type="button" class="btn sm ghost" data-act="srv-copy" data-v="${esc(s.address)}">${icon('copy', 13)}复制地址</button></div>`).join('')}</div></section>` : ''}
        ${p.name && !installed && (e.hasPack || p.modrinth) ? `<section class="card"><h3>${icon('package', 16)}${esc(p.name)}</h3><p>${esc(e.name)} 的整合包（${esc(p.mc || '')} ${esc(p.loader || '')}），装好就是一个实例。</p><div class="actions"><button type="button" class="btn pri" data-act="ed-pack">${icon('download')}安装整合包</button></div></section>` : ''}`;
}

// 极简：只有公告、服务器和启动
export function editionHome(c) {
    return `${editionCards(c)}${c.inst ? `<section class="card"><h3>${esc(c.inst.name)}</h3><p class="note">${esc(c.inst.mc)} ${c.inst.loader ? esc(c.inst.loader) : ''}</p></section>` : ''}`;
}

export function editionPanel(c) {
    const e = c.R.edition;
    let cur;
    if (e?.error) cur = `<p class="callout warn">${icon('alert', 14)}<span>${esc(e.error)}</span></p>`;
    else if (e) {
        cur = `<div class="kv"><div class="k"><b>${esc(e.name)}</b><small>作者 ${esc(e.author || '没写')}${e.version ? ` · ${esc(e.version)}` : ''} · 签名指纹 <span class="mono">${esc(e.fingerprint)}</span> · ${e.beside ? '放在 LeafStarter 旁边' : '导入的'}</small></div>
            <div class="v">${e.update ? `<button type="button" class="btn sm" data-act="ed-update">${icon('refresh', 13)}检查更新</button>` : ''}${e.beside ? '' : `<button type="button" class="btn sm ghost" data-act="ed-remove">拿掉</button>`}</div></div>`;
    } else {
        cur = `<div class="kv"><div class="k"><b>现在是普通版</b><small>服主或整合包作者发的定制包（.lsedition）导入后，启动器会换成他们的名字、主题和整合包；也可以直接放在 LeafStarter.exe 旁边</small></div>
            <div class="v"><button type="button" class="btn sm" data-act="ed-import">${icon('upload', 13)}导入定制包</button></div></div>`;
    }
    return `<section class="panel" id="edPanel"><h2 class="ph">定制版</h2>${cur}
        <div class="kv"><div class="k"><b>做一个自家专用启动器</b><small>给服主和整合包作者：不写代码，填表导出一个签名的定制包。程序还是 LeafStarter 签名的那一个，不用重新编译；「关于」和顶栏固定写着「基于 LeafStarter」</small></div>
            <div class="v"><button type="button" class="btn sm" data-act="ed-make">${icon('package', 13)}制作定制包</button></div></div></section>`;
}

export function edMakerDrawer(c) {
    const d = c.S.drawer;
    const m = d.m;
    const f = (id, label, val, ph = '', cls = '') => `<label class="field" for="${id}"><span>${label}</span><input id="${id}" type="text" class="${cls}" value="${esc(val || '')}" placeholder="${esc(ph)}" autocomplete="off"></label>`;
    const base = (p) => (p ? p.split(/[\\/]/).pop() : '');
    const body = `<p class="note">导出的定制包用你自己的密钥签名（第一次自动生成，存在本机保管库）；以后更新定制包也要用同一台电脑上的这把密钥，玩家那边才认。你的签名指纹：<span class="mono">${esc(d.fp || '…')}</span></p>
        ${f('emName', '启动器名字', m.name, '比如 星辰服启动器')}
        <div class="field-row">${f('emAuthor', '作者', m.author, '比如 星辰服管理组')}${f('emVer', '版本', m.version, '1.0')}</div>
        <div class="field"><span>主题色</span><div class="picks-row">${COLORS.map((col) => `<button type="button" class="swatch${m.color === col ? ' on' : ''}" style="--c:${col}" data-act="em-color" data-v="${col}" aria-label="${col}"></button>`).join('')}</div></div>
        <div class="field"><span>字体</span>${seg('em-font', [['sans', '默认黑体'], ['serif', '衬线体']], m.font)}</div>
        <div class="field"><span>图标（PNG）</span><div class="inrow"><button type="button" class="btn sm" data-act="em-icon">${icon('image', 13)}${m.icon ? esc(base(m.icon)) : '选图标'}</button></div></div>
        <div class="field"><span>整合包</span><div class="inrow"><button type="button" class="btn sm" data-act="em-pack">${icon('package', 13)}${m.packFile ? esc(base(m.packFile)) : '带上一个 .mrpack'}</button><input id="emModrinth" type="text" class="mono" value="${esc(m.modrinth || '')}" placeholder="或者填 Modrinth 上的整合包（slug）"></div></div>
        <div class="field-row">${f('emPackName', '整合包装好后的实例名', m.packName, '比如 星辰服')}${f('emPackMc', '游戏版本 / 加载器（显示用）', m.packMc, '1.20.1 Fabric')}</div>
        <div class="field-row">${f('emSrv', '服务器名字', m.srv, '星辰服')}${f('emAddr', '服务器地址', m.addr, 'play.example.com', 'mono')}</div>
        <label class="field" for="emNotice"><span>主页公告</span><textarea id="emNotice" rows="2">${esc(m.notice || '')}</textarea></label>
        ${f('emUpdate', '更新地址（https，可以不填）', m.update, 'https://example.com/星辰服.lsedition', 'mono')}
        <div class="field"><span>留下哪些功能</span>${FEATS.map(([k, t]) => `<label class="tick"><input type="checkbox" data-emfeat="${k}"${m.features[k] !== false ? ' checked' : ''}>${t}</label>`).join('')}</div>
        <label class="tick"><input type="checkbox" id="emMin"${m.minimal ? ' checked' : ''}>极简：打开就一个启动按钮（没有左栏、标签页和设置页）</label>
        <div class="actions"><button type="button" class="btn pri" data-act="em-export">${icon('file')}导出定制包</button>${d.out ? `<button type="button" class="btn" data-act="em-try">${icon('play')}在这台电脑上试试</button>` : ''}</div>
        ${d.out ? `<p class="note">导出到了 <span class="mono">${esc(d.out)}</span>。发给玩家，让他们放在 LeafStarter.exe 旁边，或者在「设置 → 定制版」里导入。</p>` : ''}`;
    return `<section class="drawer wide" role="dialog" aria-label="制作定制包">
        <header class="dr-head"><h2>制作定制包</h2><span class="sp"></span><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">${body}</div></section>`;
}

export function createEdition(api) {
    const { S } = api;
    const d = () => (S.drawer?.kind === 'edmaker' ? S.drawer : null);
    // 重画前把输入框里的记下来
    function keep() {
        const x = d();
        if (!x) return;
        const v = (id) => $(id)?.value ?? undefined;
        const m = x.m;
        for (const [k, id] of [['name', '#emName'], ['author', '#emAuthor'], ['version', '#emVer'], ['modrinth', '#emModrinth'], ['packName', '#emPackName'], ['packMc', '#emPackMc'], ['srv', '#emSrv'], ['addr', '#emAddr'], ['notice', '#emNotice'], ['update', '#emUpdate']]) {
            const val = v(id);
            if (val !== undefined) m[k] = val;
        }
        document.querySelectorAll('[data-emfeat]').forEach((cb) => {
            m.features[cb.dataset.emfeat] = cb.checked;
        });
        const mn = $('#emMin');
        if (mn) m.minimal = mn.checked;
    }
    const repaint = () => {
        keep();
        api.paintDrawer();
    };
    async function reload() {
        R.edition = await invoke('edition_get').catch(() => null);
        applyEdition(api);
        api.paintView();
    }
    return {
        actions: {
            'ed-import': async () => {
                const p = await invoke('edition_pick', { kind: 'edition' });
                if (!p) return;
                try {
                    const e = await invoke('edition_import', { path: p });
                    await reload();
                    toast(`换成了「${e.name}」（作者 ${e.author || '没写'}，签名指纹 ${e.fingerprint}）`, { ms: 7000 });
                } catch (e) {
                    toast(String(e), { ms: 7000 });
                }
            },
            'ed-remove': async () => {
                if (!confirm('拿掉这个定制包，恢复成普通的 LeafStarter？实例和存档都不动。')) return;
                try {
                    await invoke('edition_remove');
                } catch (e) {
                    toast(String(e), { ms: 8000 });
                }
                await reload();
            },
            'ed-update': async () => {
                try {
                    const r = await invoke('edition_update');
                    if (r.updated) await reload();
                    toast(r.updated ? '定制包更新了' : '已经是最新的');
                } catch (e) {
                    toast(String(e), { ms: 7000 });
                }
            },
            'ed-pack': async () => {
                const e = R.edition;
                const dir = api.defaultDir();
                if (!dir) return toast('还没有能装的游戏目录，先在设置里加一个');
                try {
                    if (e.hasPack) await invoke('pack_install', { path: await invoke('edition_pack'), dir, name: e.pack.name });
                    else await invoke('pack_install_modrinth', { project: e.pack.modrinth, version: null, dir, name: e.pack.name });
                    toast('开始装整合包，进度在「任务」页');
                } catch (err) {
                    toast(String(err), { ms: 7000 });
                }
            },
            'ed-make': async () => {
                const e = R.edition && !R.edition.error ? R.edition : null;
                api.openDrawer({
                    kind: 'edmaker', out: '',
                    m: {
                        name: e?.name || '', author: e?.author || '', version: e?.version || '1.0', color: e?.theme.color || COLORS[0], font: e?.theme.font || 'sans',
                        icon: '', packFile: '', modrinth: e?.pack.modrinth || '', packName: e?.pack.name || '', packMc: e ? `${e.pack.mc} ${e.pack.loader}`.trim() : '',
                        srv: e?.servers[0]?.name || '', addr: e?.servers[0]?.address || '', notice: e?.home.notice || '', update: e?.update || '',
                        features: { ...(e?.features || {}) }, minimal: !!e?.minimal,
                    },
                });
                const fp = await invoke('edition_author').catch(() => '');
                if (d()) {
                    keep();
                    d().fp = fp;
                    api.paintDrawer();
                }
            },
            'em-color': (el) => {
                keep();
                d().m.color = el.dataset.v;
                api.paintDrawer();
            },
            'em-font': (el) => {
                keep();
                d().m.font = el.dataset.v;
                api.paintDrawer();
            },
            'em-icon': async () => {
                keep();
                const p = await invoke('edition_pick', { kind: 'icon' });
                if (p && d()) {
                    d().m.icon = p;
                    api.paintDrawer();
                }
            },
            'em-pack': async () => {
                keep();
                const p = await invoke('edition_pick', { kind: 'pack' });
                if (p && d()) {
                    d().m.packFile = p;
                    try {
                        const info = await invoke('pack_read', { path: p });
                        const m = d().m;
                        if (!m.packName) m.packName = info.name;
                        m.packMc = `${info.mc || ''} ${info.loader || ''}`.trim();
                    } catch {}
                    repaint();
                }
            },
            'em-export': async () => {
                keep();
                const x = d();
                const m = x.m;
                if (!m.name.trim()) return toast('起个名字');
                const out = await invoke('edition_pick', { kind: 'save' });
                if (!out) return;
                const [mc, loader] = (m.packMc || '').split(/\s+/);
                const manifest = {
                    name: m.name, author: m.author, version: m.version, theme: { color: m.color, font: m.font }, features: m.features, minimal: m.minimal,
                    pack: { name: m.packName, mc: mc || '', loader: loader || '', modrinth: m.packFile ? '' : m.modrinth },
                    servers: m.addr ? [{ name: m.srv, address: m.addr }] : [], home: { notice: m.notice }, update: m.update.trim(),
                };
                try {
                    const r = await invoke('edition_build', { manifest, icon: m.icon || null, pack: m.packFile || null, out });
                    x.out = r.path;
                    api.paintDrawer();
                    toast(`导出好了（${Math.max(1, Math.round(r.size / 1024))} KB），签名指纹 ${r.fingerprint}`, { ms: 7000 });
                } catch (e) {
                    toast(String(e), { ms: 7000 });
                }
            },
            'em-try': async () => {
                const x = d();
                if (!x?.out) return;
                try {
                    await invoke('edition_import', { path: x.out });
                    api.closeDrawer();
                    await reload();
                    toast('换成了刚做的定制版。想恢复：设置 → 定制版 → 拿掉');
                } catch (e) {
                    toast(String(e));
                }
            },
        },
    };
}
