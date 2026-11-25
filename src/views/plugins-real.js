// 真程序的「插件」页（M15–M17）
import { esc } from '../ui.js';
import { icon } from '../icons.js';

const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;
export const TIER = { official: ['ok', '官方'], reviewed: ['acc', '已审核'], community: ['warn', '社区'], dev: ['warn', '本地开发'] };
export const LAYER = { look: '外观', decl: '声明式', script: '脚本', native: '原生' };

// 官方插件：官方功能也按插件算，用不着就关掉，入口跟着消失（「万物皆插件」先用在自己身上）
export const OFFICIAL_REAL = [
    ['mp', 'users', '联机', '邀请码、开局域网提示、照邀请码加入'],
    ['server', 'server', '常驻服务器', '云服务器一键开服、面板服'],
    ['world', 'globe', '世界工具', '地图、查找建筑、不进游戏改设置'],
    ['ai', 'sparkles', 'AI 助手', '查崩溃、MCP'],
    ['zh', 'file', '补全中文', '社区汉化 + AI 补齐'],
    ['skin', 'user', '皮肤工作台', '画皮肤、换上正版皮肤'],
    ['sync', 'smartphone', '设备同步', '局域网里互传实例'],
];

export function realPluginsView(c) {
    const P = c.R.plug || {};
    const tab = c.S.ptab || 'installed';
    let body;
    if (tab === 'market') body = market(c);
    else if (tab === 'dev') body = dev(c);
    else if (tab === 'page') body = page(c);
    else body = installed(c);
    return `<div class="page" id="page">
        <div class="page-h"><h1>插件</h1>${tab === 'page' ? '' : seg('pl-tab', [['installed', '已装'], ['market', '插件市场'], ['dev', '开发者']], tab)}<span class="sp"></span>
            ${tab === 'page' ? `<button type="button" class="btn ghost" data-act="pl-tab" data-v="installed">${icon('chevron', 14)}返回</button>` : `<button type="button" class="btn" data-act="pl-file">${icon('upload')}从文件安装</button>`}</div>
        ${P.safe ? `<div class="banner" role="status">${icon('shield', 20)}<div class="tx"><b>安全模式</b><p>这次是按住 Ctrl 打开的，没有加载任何插件。正常打开就恢复。</p></div></div>` : ''}
        ${body}
    </div>`;
}

function permList(perms) {
    return perms?.length ? `<ul class="perm-list">${perms.map((p) => `<li${p.new ? ' class="new"' : ''}>${icon(p.id === 'native' || p.id === 'launch' ? 'alert' : 'check', 13)}${esc(p.text || p.id)}${p.new ? ' <span class="chip warn">新要的</span>' : ''}</li>`).join('')}</ul>` : '<p class="note">不要任何权限（只是外观或数据）。</p>';
}

// 照概念版：左边图标方块，名字后面「官方」，开关在右边
function official(c) {
    return `<h3 class="sh">${icon('check', 14)}官方插件</h3>
    <div class="plist">${OFFICIAL_REAL.map(([id, ic, name, desc]) => {
        const on = !c.plugOff(id);
        return `<div class="prow${on ? '' : ' off'}"><span class="ptile official">${icon(ic, 17)}</span>
            <span class="tx"><span class="nm">${name}<span class="tier t-official">${icon('check', 12)}官方</span><span class="chip xs">内置 · 能关</span></span><span class="sb">${desc}</span></span>
            <span class="tags"><input type="checkbox" class="switch" data-poff="${id}"${on ? ' checked' : ''} aria-label="启用${name}"></span></div>`;
    }).join('')}</div>`;
}

function installed(c) {
    const list = c.R.plug?.plugins || [];
    if (!list.length) {
        return `${official(c)}<h3 class="sh">${icon('plug', 14)}第三方插件</h3><section class="card"><p class="note">还没装。去「插件市场」看看，或者从文件安装 .lsplugin</p></section>`;
    }
    return `${official(c)}<h3 class="sh">${icon('plug', 14)}第三方插件</h3><div class="rows">${list.map((p) => {
        if (p.error) return `<div class="row"><span class="tx"><span class="nm">本地开发的插件</span><span class="sb">${esc(p.error)}</span></span></div>`;
        const m = p.manifest;
        const [tc, tt] = TIER[p.record?.tier] || TIER.community;
        const script = ['script', 'native'].includes(m.layer) && m.contributes?.page;
        return `<div class="row plug-row${p.active ? '' : ' off'}">
            ${p.dev ? '<span></span>' : `<input type="checkbox" class="switch" data-plug="${esc(p.id)}"${p.record?.enabled ? ' checked' : ''}${p.revoked ? ' disabled' : ''} aria-label="启用 ${esc(m.name)}">`}
            <span class="tx"><span class="nm">${esc(m.name)} <small class="mono">${esc(m.version)}</small></span>
                <span class="sb">${esc(m.description || '')}${m.author ? ` · ${esc(m.author)}` : ''}${p.record?.key ? ` · 签名 <span class="mono">${esc(p.record.key)}</span>` : ''}</span>
                ${p.perms?.length ? `<span class="sb">要的权限：${p.perms.map((x) => esc(x.text || x.id)).join('、')}</span>` : ''}
                ${p.revoked ? `<span class="sb bad">${icon('alert', 12)}已吊销：${esc(p.revoked)}</span>` : ''}</span>
            <span class="tags"><span class="chip">${LAYER[m.layer] || m.layer}</span><span class="chip ${tc}">${tt}</span></span>
            ${script && p.active ? `<button type="button" class="btn sm" data-act="pl-open" data-id="${esc(p.id)}">打开</button>` : '<span></span>'}
            ${p.dev ? `<button type="button" class="btn sm ghost" data-act="pl-reload">${icon('refresh', 13)}重新加载</button>` : `<button type="button" class="btn sm ghost danger" data-act="pl-del" data-id="${esc(p.id)}" aria-label="卸载">${icon('trash', 13)}</button>`}
        </div>`;
    }).join('')}</div>
    <p class="note">插件拿不到账户令牌和密钥。出问题时按住 Ctrl 打开，进安全模式</p>`;
}

function market(c) {
    const M = c.R.plugMarket;
    if (!M || M.loading) return '<p class="note">正在读插件市场…</p>';
    if (M.error && /404/.test(M.error)) return '<section class="card"><h3>插件市场还没开张</h3><p class="note">可以先从文件安装作者发给你的 .lsplugin</p></section>';
    if (M.error) return `<section class="card"><h3>插件市场还连不上</h3><p class="note">${esc(M.error.split(' for url')[0])}</p><div class="actions"><button type="button" class="btn" data-act="pl-market">${icon('refresh')}再试一次</button></div></section>`;
    const have = new Set((c.R.plug?.plugins || []).map((p) => p.id));
    const list = M.plugins || [];
    return list.length ? `<div class="rows">${list.map((x, k) => {
        const [tc, tt] = TIER[x.tier] || TIER.community;
        return `<div class="row plug-row"><span></span><span class="tx"><span class="nm">${esc(x.name)} <small class="mono">${esc(x.version)}</small></span><span class="sb">${esc(x.description || '')} · ${esc(x.author || '')}</span></span>
            <span class="tags"><span class="chip">${LAYER[x.layer] || x.layer}</span><span class="chip ${tc}">${tt}</span></span><span></span>
            <button type="button" class="btn sm" data-act="pl-get" data-k="${k}"${have.has(x.id) ? ' disabled' : ''}>${have.has(x.id) ? '装了' : '安装'}</button></div>`;
    }).join('')}</div>` : '<section class="card"><p>市场里还没有插件。</p></section>';
}

function dev(c) {
    const P = c.R.plug || {};
    return `<section class="panel"><h2 class="ph">开发者模式</h2>
        <div class="kv"><div class="k"><b>加载本地插件</b><small>从一个文件夹（里面有 plugin.json）直接加载，不用签名，标「本地开发」。改了文件点「重新加载」</small></div>
            <div class="v"><input type="checkbox" class="switch" id="plDev"${P.dev ? ' checked' : ''} aria-label="开发者模式"></div></div>
        <div class="kv"><div class="k"><b>文件夹</b><small class="mono">${esc(P.devDir || '还没选')}</small></div><div class="v"><button type="button" class="btn sm" data-act="pl-devdir">${icon('folder', 13)}选文件夹</button><button type="button" class="btn sm ghost" data-act="pl-reload">${icon('refresh', 13)}重新加载</button></div></div>
        <div class="kv stack"><div class="k"><b>脚本插件能调的接口</b><small>在插件页面里用 <span class="mono">leafstarter.call(方法, 参数)</span>，返回 Promise；<span class="mono">leafstarter.on('launch', fn)</span> 收事件（要 events 权限）</small></div>
            <pre class="cmdbox mono">ui.toast { text }
instances.list                  需要 instances:read
instance.current                需要 instances:read
instance.launch { id }          需要 launch，每次都问用户
net.fetch { url }               需要 net:域名，只能 GET https
storage.get { key } / storage.set { key, value }   需要 storage
native.call { method, params }  需要 native（原生插件自带的程序）</pre></div>
        <div class="kv stack"><div class="k"><b>plugin.json 示例</b></div>
            <pre class="cmdbox mono">{
  "id": "server-status", "name": "服务器状态卡片", "version": "1.0.0", "author": "你",
  "layer": "script", "main": "index.html",
  "permissions": ["instances:read", "net:api.mcsrvstat.us"],
  "contributes": { "card": { "title": "服务器在线人数" }, "page": { "title": "服务器状态" } }
}</pre></div>
    </section>`;
}

function page(c) {
    const id = c.S.plugPage;
    const s = (c.R.plug?.contrib?.scripts || []).find((x) => x.plugin === id);
    if (!s) return '<section class="card"><p>这个插件没开着。</p></section>';
    return `<section class="card plug-page"><h3>${esc(s.page?.title || s.name)}<span class="chip">插件 · ${esc(s.name)}</span></h3>
        <iframe class="plug-frame tall" sandbox="allow-scripts" data-plugin="${esc(id)}" title="${esc(s.name)}"></iframe></section>`;
}

// 实例概览里：插件的主页卡片（声明式）和脚本卡片
export function pluginCards(c) {
    const C = c.R.plug?.contrib;
    if (!C) return '';
    const decl = (C.homeCards || []).map((x) => `<section class="card"><h3>${esc(x.title || '')}<span class="chip">插件 · ${esc(x.pluginName)}</span></h3><p>${esc(x.text || '')}</p>${/^https:\/\//.test(x.url || '') ? `<div class="actions"><button type="button" class="btn sm" data-act="open-url" data-v="${esc(x.url)}">${icon('link', 13)}打开</button></div>` : ''}</section>`).join('');
    const scripts = (C.scripts || []).filter((s) => s.card).map((s) => `<section class="card"><h3>${esc(s.card.title || s.name)}<span class="chip">插件 · ${esc(s.name)}</span></h3><iframe class="plug-frame" sandbox="allow-scripts" data-plugin="${esc(s.plugin)}" title="${esc(s.name)}"></iframe></section>`).join('');
    return decl + scripts;
}

// 装之前：要哪些权限
export function plugConsentDrawer(c) {
    const d = c.S.drawer;
    const x = d.info;
    const m = x.manifest;
    const [tc, tt] = TIER[d.tier] || TIER.community;
    const body = `<div class="kv"><div class="k"><b>${esc(m.name)} <small class="mono">${esc(m.version)}</small></b><small>${esc(m.description || '')}</small></div><div class="v"><span class="chip">${LAYER[m.layer]}</span><span class="chip ${tc}">${tt}</span></div></div>
        <dl class="facts"><dt>作者</dt><dd>${esc(m.author || '没写')}</dd><dt>签名指纹</dt><dd class="mono">${esc(x.fingerprint)}</dd>${x.upgrade ? `<dt>升级</dt><dd>${esc(x.upgrade.from)} → ${esc(m.version)}</dd>` : ''}</dl>
        ${x.revoked ? `<p class="callout warn">${icon('alert', 14)}<span>这个插件被吊销了：${esc(x.revoked)}</span></p>` : ''}
        ${d.tier !== 'official' && d.tier !== 'reviewed' ? `<p class="callout warn">${icon('alert', 14)}<span>${d.tier === 'dev' ? '本地开发' : '社区'}插件没有经过审核。只装你信任的作者发的。</span></p>` : ''}
        <h3 class="sub">它要这些权限</h3>${permList(x.perms)}
        <div class="actions"><button type="button" class="btn pri" data-act="pl-yes"${x.revoked ? ' disabled' : ''}>${x.upgrade ? '同意并升级' : '同意并安装'}</button><button type="button" class="btn ghost" data-act="dr-close">先不装</button></div>`;
    return `<section class="drawer" role="dialog" aria-label="安装插件"><header class="dr-head"><h2>安装插件</h2><span class="sp"></span><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭">${icon('x')}</button></header><div class="dr-body">${body}</div></section>`;
}
