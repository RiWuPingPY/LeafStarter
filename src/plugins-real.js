// 真程序：插件宿主的前端（M15–M17）——套用主题、命令面板条目、脚本插件的沙箱和接口
import { $, toast, esc } from './ui.js';
import { invoke, listen, R } from './real.js';

// 脚本插件页面前面加的一小段：只能 postMessage 给宿主；CSP 禁止它自己联网
const PRELUDE = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:">
<script>(() => {
  let seq = 0; const wait = new Map(); const subs = {};
  addEventListener('message', (e) => {
    if (e.source !== parent) return;
    const d = e.data || {};
    if (d.__ls === 'reply' && wait.has(d.id)) { const [ok, no] = wait.get(d.id); wait.delete(d.id); d.error ? no(new Error(d.error)) : ok(d.result); }
    if (d.__ls === 'event') (subs[d.name] || []).forEach((f) => { try { f(d.data); } catch (err) { console.error(err); } });
  });
  window.leafstarter = {
    call(method, params) { return new Promise((ok, no) => { const id = ++seq; wait.set(id, [ok, no]); parent.postMessage({ __ls: 'call', id, method, params }, '*'); }); },
    on(name, fn) { (subs[name] = subs[name] || []).push(fn); },
  };
  const size = () => parent.postMessage({ __ls: 'size', h: Math.ceil(document.documentElement.scrollHeight) }, '*');
  addEventListener('load', size);
  new ResizeObserver(size).observe(document.documentElement);
})();<\/script>
<style>html{font:14px/1.6 system-ui,"Microsoft YaHei",sans-serif;color:CanvasText;background:transparent}body{margin:0}</style>`;

// 主题只收 CSS 变量，值里不许有能跳出去的东西
const safeVar = (k, v) => /^--[a-z0-9-]{1,40}$/i.test(k) && typeof v === 'string' && v.length < 120 && !/[;{}<>]|url\s*\(|expression|@import/i.test(v);

export function createPluginHost(api) {
    const { S } = api;
    const scriptOf = (id) => (R.plug?.contrib?.scripts || []).find((s) => s.plugin === id);

    async function load() {
        R.plug = await invoke('plugin_list').catch(() => ({ plugins: [], contrib: {} }));
        applyTheme();
        if (S.page === 'plugins' || (S.page === 'home' && S.tab === 'overview')) api.paintView();
    }

    function applyTheme() {
        let el = document.getElementById('plugTheme');
        if (!el) {
            el = document.createElement('style');
            el.id = 'plugTheme';
            document.head.append(el);
        }
        const css = [];
        for (const t of R.plug?.contrib?.themes || []) {
            const vars = (o) => Object.entries(o || {}).filter(([k, v]) => safeVar(k, v)).map(([k, v]) => `${k}:${v};`).join('');
            if (t.light) css.push(`:root{${vars(t.light)}}`);
            if (t.dark) css.push(`@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){${vars(t.dark)}}} :root[data-theme="dark"]{${vars(t.dark)}}`);
            if (t.font === 'serif') css.push(':root{--font: var(--serif);}');
        }
        el.textContent = css.join('\n');
    }

    // ---------- 脚本插件：iframe 出现在页面上就装内容 ----------
    const frames = new Map(); // contentWindow → 插件 id
    async function fill(f) {
        f.dataset.loaded = '1';
        const id = f.dataset.plugin;
        const s = scriptOf(id);
        if (!s) return;
        const list = R.plug?.plugins || [];
        const m = list.find((p) => p.id === id)?.manifest;
        try {
            const html = await invoke('plugin_file', { id, name: m?.main || 'index.html' });
            f.srcdoc = PRELUDE + html;
        } catch (e) {
            f.srcdoc = `<p style="color:#b33">插件「${esc(s.name)}」打不开：${esc(String(e))}</p>`;
        }
        frames.set(f.contentWindow, id);
    }
    new MutationObserver(() => {
        document.querySelectorAll('iframe.plug-frame:not([data-loaded])').forEach(fill);
        for (const [w] of frames) if (![...document.querySelectorAll('iframe.plug-frame')].some((f) => f.contentWindow === w)) frames.delete(w);
    }).observe(document.body, { childList: true, subtree: true });

    const need = (id, perm) => {
        if (!scriptOf(id)?.permissions?.includes(perm)) throw new Error(`插件没申请 ${perm} 权限`);
    };
    const instInfo = (i) => i && { id: i.id, name: i.name, mc: i.mc, loader: i.loader || null, loaderVersion: i.loaderVer || null, mods: (i.mods || []).map((m) => ({ name: m.name, file: m.file, enabled: m.on })) };
    const METHODS = {
        'ui.toast': (id, p) => toast(`「${scriptOf(id).name}」：${String(p?.text || '').slice(0, 120)}`),
        'instances.list': (id) => {
            need(id, 'instances:read');
            return api.INSTANCES.filter((i) => i.real).map((i) => ({ id: i.id, name: i.name, mc: i.mc, loader: i.loader || null }));
        },
        'instance.current': (id) => {
            need(id, 'instances:read');
            return instInfo(api.cur());
        },
        'instance.launch': (id, p) => {
            need(id, 'launch');
            const i = api.byId(p?.id) || api.cur();
            if (!i || !confirm(`插件「${scriptOf(id).name}」想启动「${i.name}」，允许吗？`)) throw new Error('用户没有允许');
            api.launch(i.id);
            return true;
        },
        'net.fetch': (id, p) => invoke('plugin_fetch', { id, url: String(p?.url || '') }),
        'storage.get': (id, p) => invoke('plugin_storage', { id, key: String(p?.key || ''), value: null }),
        'storage.set': (id, p) => invoke('plugin_storage', { id, key: String(p?.key || ''), value: p?.value ?? null }),
        'native.call': (id, p) => invoke('plugin_native', { id, method: String(p?.method || ''), params: p?.params ?? {} }),
    };
    window.addEventListener('message', async (e) => {
        const id = frames.get(e.source);
        const d = e.data;
        if (!id || !d || typeof d !== 'object') return;
        if (d.__ls === 'size') {
            const f = [...document.querySelectorAll('iframe.plug-frame')].find((x) => x.contentWindow === e.source);
            if (f && !f.classList.contains('tall')) f.style.height = Math.min(600, Math.max(40, Number(d.h) || 0)) + 'px';
            return;
        }
        if (d.__ls !== 'call') return;
        const reply = (r) => e.source.postMessage({ __ls: 'reply', id: d.id, ...r }, '*');
        const fn = METHODS[d.method];
        if (!fn) return reply({ error: `没有这个接口：${d.method}` });
        try {
            reply({ result: await fn(id, d.params) });
        } catch (err) {
            reply({ error: String(err?.message || err) });
        }
    });
    // 事件：游戏启动、退出、崩溃（要 events 权限）
    listen('launch-state', (e) => {
        for (const [w, id] of frames) {
            if (scriptOf(id)?.permissions?.includes('events')) w.postMessage({ __ls: 'event', name: 'launch', data: { instance: e.payload.key, phase: e.payload.phase } }, '*');
        }
    });

    // ---------- 动作 ----------
    async function consent(info, tier, path, entry) {
        api.openDrawer({ kind: 'plugconsent', info, tier, path, entry });
    }
    const actions = {
        'pl-tab': (el) => {
            S.ptab = el.dataset.v;
            if (S.ptab === 'market' && !R.plugMarket) actions['pl-market']();
            api.paintView();
        },
        'pl-market': async () => {
            R.plugMarket = { loading: true };
            api.paintView();
            try {
                R.plugMarket = await invoke('plugin_index');
                await load();
            } catch (e) {
                R.plugMarket = { error: String(e) };
            }
            if (S.page === 'plugins') api.paintView();
        },
        'pl-file': async () => {
            const p = await invoke('plugin_pick', { folder: false });
            if (!p) return;
            try {
                await consent(await invoke('plugin_inspect', { path: p }), 'community', p, null);
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        'pl-get': async (el) => {
            const x = R.plugMarket?.plugins?.[Number(el.dataset.k)];
            if (!x) return;
            el.disabled = true;
            try {
                await consent(await invoke('plugin_market_inspect', { entry: x }), x.tier, null, x);
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
            el.disabled = false;
        },
        'pl-yes': async () => {
            const d = S.drawer;
            const granted = d.info.manifest.permissions || [];
            try {
                const m = d.entry ? await invoke('plugin_market_install', { entry: d.entry, granted }) : await invoke('plugin_install', { path: d.path, granted });
                api.closeDrawer();
                await load();
                toast(`装好了「${m.name}」，不用重启`);
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
        },
        'pl-del': async (el) => {
            const p = R.plug.plugins.find((x) => x.id === el.dataset.id);
            if (!p || !confirm(`卸载插件「${p.manifest.name}」？`)) return;
            await invoke('plugin_remove', { id: p.id }).catch((e) => toast(String(e)));
            await load();
        },
        'pl-open': (el) => {
            S.plugPage = el.dataset.id;
            S.ptab = 'page';
            api.paintView();
        },
        'pl-devdir': async () => {
            const dir = await invoke('plugin_pick', { folder: true });
            if (!dir) return;
            await invoke('plugin_dev', { on: true, dir });
            await load();
        },
        'pl-reload': async () => {
            await load();
            document.querySelectorAll('iframe.plug-frame').forEach((f) => {
                delete f.dataset.loaded;
                fill(f);
            });
            toast('重新加载了插件');
        },
    };

    return {
        load,
        actions,
        change(t) {
            if (t.dataset.plug) {
                invoke('plugin_enable', { id: t.dataset.plug, on: t.checked }).then(load, (e) => {
                    toast(String(e));
                    load();
                });
                return true;
            }
            if (t.id === 'plDev') {
                invoke('plugin_dev', { on: t.checked, dir: null }).then(load);
                return true;
            }
            return false;
        },
        // 命令面板：插件加的条目（打开网页）
        palette() {
            return (R.plug?.contrib?.palette || []).filter((x) => /^https:\/\//.test(x.url || '')).map((x) => ({ g: '插件', ic: 'link', t: String(x.title || '').slice(0, 40), sub: x.pluginName, run: () => invoke('open_path', { path: x.url }) }));
        },
    };
}
