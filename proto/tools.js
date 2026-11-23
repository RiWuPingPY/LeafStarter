// 工具类的逻辑：补全中文、设置模板、局外新建世界、渲染器、显卡、Mod 外链
import { $, toast } from './ui.js';
import { makeMod, renderers, PC } from './data.js';
import { zhState, linksPop, TMPL_ITEMS, canMakeWorld } from './views/tools.js';
import { i18nDrawer } from './views/tools.js';

const BAD = { 'Glimmer Render': 1, 'Tech Reborn': 3 };

export function createTools(api) {
    const { S } = api;
    S.tmplLast = null;

    // ---------- 补全中文 ----------
    function zhOpen(i) {
        const none = i.mods.filter((m) => zhState(m, i) === 'none');
        if (!none.length) return toast(`「${i.name}」的 Mod 都有中文了`);
        api.openDrawer({ kind: 'i18n', id: i.id, engine: api.aiFor('tr') ? 'ai' : 'mt', pick: none.map((m) => m.id), phase: 'plan', pct: 0 });
    }

    function zhRun() {
        const d = S.drawer;
        const i = api.byId(d.id);
        const list = i.mods.filter((m) => d.pick.includes(m.id) && zhState(m, i) === 'none');
        if (!list.length) return;
        d.phase = 'run';
        d.run = list.map((m) => ({ id: m.id, name: m.name, h: m.h, lines: m.lines || 40, done: 0, bad: BAD[m.name] || 0, state: 'wait' }));
        const total = d.run.reduce((a, r) => a + r.lines, 0);
        const t = api.addTask(`补全中文 · ${i.name}`, `${d.run.length} 个 Mod · ${total} 条`);
        api.paintDrawer();
        let k = 0;
        let doneLines = 0;
        const h = setInterval(() => {
            const r = d.run[k];
            if (!r) return;
            if (r.state === 'wait') {
                r.state = 'run';
                paintRow(d, r, true);
            }
            const step = Math.max(4, Math.ceil(r.lines / 14));
            r.done = Math.min(r.lines, r.done + step);
            doneLines += step;
            d.pct = Math.min(100, Math.round((Math.min(doneLines, total) / total) * 100));
            t.pct = d.pct;
            t.stage = `${r.name}：${r.done} / ${r.lines}`;
            api.paintTasks();
            if (r.done >= r.lines) {
                r.state = 'done';
                paintRow(d, r, true);
                k += 1;
            } else paintRow(d, r, false);
            const go = S.drawer === d && $('#drCreate .fill');
            if (go) go.style.setProperty('--p', d.pct + '%');
            if (k >= d.run.length) {
                clearInterval(h);
                t.state = 'done';
                t.at = Date.now();
                t.stage = '';
                api.paintTasks();
                api.paintTop();
                const n = d.run.reduce((a, x) => a + x.lines - x.bad, 0);
                for (const m of list) m.zh = d.engine;
                i.zhPack = { on: true, n: (i.zhPack?.n || 0) + n, engine: d.engine };
                d.phase = 'done';
                if (S.drawer === d) api.paintDrawer();
                if (api.cur() === i && S.page === 'home') api.paintBody();
                toast(`「${i.name}」的中文补好了：${n} 条，${d.run.reduce((a, x) => a + x.bad, 0)} 条格式符对不上，留着英文`);
            }
        }, 70);
    }

    // 翻译进行中只改那一行，不整块重画
    function paintRow(d, r, whole) {
        if (S.drawer !== d) return;
        const el = document.querySelectorAll('#zhBody .zrow')[d.run.indexOf(r)];
        if (!el) return;
        if (whole) {
            const tmp = document.createElement('div');
            tmp.innerHTML = i18nDrawer(api.ctx());
            const fresh = tmp.querySelectorAll('#zhBody .zrow')[d.run.indexOf(r)];
            if (fresh) el.replaceWith(fresh);
            return;
        }
        el.querySelector('.bar i')?.style.setProperty('--p', Math.round((r.done / r.lines) * 100) + '%');
        const sb = el.querySelector('.sb');
        if (sb) sb.textContent = `正在翻 ${r.done} / ${r.lines}`;
    }

    // ---------- 设置模板 ----------
    function tmplOpen(dir) {
        const i = api.cur();
        const from = dir === 'in' ? (api.INSTANCES.find((x) => x.id !== i.id) || i).id : i.id;
        const to = dir === 'in' ? [i.id] : [];
        api.openDrawer({ kind: 'tmpl', from, to, items: { keys: true, fov: true, sens: true, view: true, sound: false, gui: false, lang: false }, asDefault: false });
    }
    function tmplApply() {
        const d = S.drawer;
        const src = api.byId(d.from);
        const to = d.to.map(api.byId).filter(Boolean).filter((x) => x !== src);
        const items = TMPL_ITEMS.filter(([k]) => d.items[k]).map(([, t]) => t);
        if (!to.length || !items.length) return;
        const before = S.tmplLast;
        api.closeDrawer();
        S.tmplLast = { from: src.name, n: to.length };
        if (S.page === 'home' && S.tab === 'config') api.paintBody();
        toast(`把${items.join('、')}从「${src.name}」套用到了${to.map((x) => `「${x.name}」`).join('')}${d.asDefault ? '，也存成了默认模板' : ''}`, {
            undo: () => {
                S.tmplLast = before;
                if (S.page === 'home' && S.tab === 'config') api.paintBody();
            },
        });
    }

    // ---------- 局外新建世界 ----------
    function nwOpen() {
        const i = api.cur();
        let n = '新的世界';
        for (let k = 2; i.worlds.some((w) => w.name === n); k++) n = `新的世界 (${k})`;
        api.openDrawer({ kind: 'world', id: i.id, name: n, seed: '', mode: '生存', diff: '普通', type: '默认', cheats: false, structs: true });
    }
    function nwCreate() {
        const d = S.drawer;
        const i = api.byId(d.id);
        if (!canMakeWorld(i.mc) || !d.name.trim()) return;
        const seed = d.seed.trim() || String(Math.floor(Math.random() * 9e17) - 4.5e17);
        const w = {
            id: 'w' + Date.now().toString(36), name: d.name.trim(), mode: d.mode === '极限' ? '极限' : d.mode, diff: d.mode === '极限' ? '困难' : d.diff, last: Date.now(), size: '—', history: [],
            seed, fresh: true, type: d.type, cheats: d.cheats, playMin: 0, spawn: [0, 64, 0], pos: [0, 64, 0], deaths: [], reach: 0, lvl: 0,
            rules: { keepInventory: false, doDaylightCycle: true, mobGriefing: true, doFireTick: true }, inv: [], _m: true,
        };
        i.worlds.unshift(w);
        api.closeDrawer();
        S.world = null;
        api.setTab('worlds');
        api.arrive(w.id);
        toast(`建好了「${w.name}」。第一次进去时游戏会生成地形`, {
            undo: () => {
                i.worlds.splice(i.worlds.indexOf(w), 1);
                if (api.cur() === i) api.paintBody();
            },
        });
    }

    // ---------- 渲染器 ----------
    function rdAdd(name, force) {
        const i = api.cur();
        const r = renderers(i.loader).find((x) => x.name === name);
        if (!r || i.mods.some((m) => m.name === name)) return;
        const off = force ? i.mods.filter((m) => r.conflicts?.includes(m.name) && m.on) : [];
        off.forEach((m) => { m.on = false; });
        const m = makeMod(r.name, r.ver, 'Modrinth', { client: true });
        i.mods.push(m);
        api.paintView();
        toast(off.length ? `禁用了 ${off.map((x) => x.name).join('、')}，装上了 ${name}` : `装上了 ${name} ${r.ver}`, {
            undo: () => {
                i.mods.splice(i.mods.indexOf(m), 1);
                off.forEach((x) => { x.on = true; });
                if (api.cur() === i) api.paintView();
            },
        });
    }

    return {
        zhOpen,
        primary() {
            const d = S.drawer;
            if (d.kind === 'i18n') {
                if (d.phase === 'plan') return zhRun();
                if (d.phase === 'done') {
                    api.closeDrawer();
                    api.setTab('mods');
                }
                return;
            }
            if (d.kind === 'tmpl') return tmplApply();
            if (d.kind === 'world') return nwCreate();
        },
        actions: {
            'zh-open': () => zhOpen(api.cur()),
            'zh-eng': (el) => {
                S.drawer.engine = el.dataset.v;
                api.paintDrawer();
            },
            'tmpl-open': (el) => tmplOpen(el.dataset.v),
            'nw-open': () => nwOpen(),
            'nw-set': (el) => {
                S.drawer[el.dataset.k] = el.dataset.v;
                api.paintDrawer();
            },
            'nw-dice': () => {
                S.drawer.seed = String(Math.floor(Math.random() * 9e17) - 4.5e17);
                api.paintDrawer();
            },
            'rd-add': (el) => rdAdd(el.dataset.v),
            'rd-clash': (el) => {
                const i = api.cur();
                const r = renderers(i.loader).find((x) => x.name === el.dataset.v);
                const on = i.mods.filter((m) => r.conflicts.includes(m.name) && m.on);
                api.openPop(el, `<div class="pop-h">${r.name} 和这几个不能一起装</div>${on.map((m) => `<div class="pop-row"><span class="tile sm" style="--h:${m.h}">${m.name[0]}</span><span class="tx"><span class="nm">${m.name}</span><span class="sb">已启用</span></span></div>`).join('')}<p class="pop-note big">装之前按规则查冲突。要装 ${r.name}，就得先禁用它们（能撤销）。${r.exp ? '它是实验性的，和很多 Mod 不兼容，更适合原版或很轻的实例。' : ''}</p><div class="pop-act"><button type="button" class="btn sm" data-act="rd-force" data-v="${r.name}">禁用它们，装上 ${r.name}</button><button type="button" class="btn sm ghost" data-act="pop-close">先不</button></div>`, 'below');
            },
            'rd-force': (el) => {
                api.closePop(false);
                rdAdd(el.dataset.v, true);
            },
            'gpu-fix': () => {
                const i = api.cur();
                const had = 'gpu' in i.cfg ? i.cfg.gpu : undefined;
                i.cfg.gpu = 'high';
                api.paintView();
                toast(`改好了：「${i.name}」下次启动用 ${PC.gpus.d}`, {
                    undo: () => {
                        if (had === undefined) delete i.cfg.gpu;
                        else i.cfg.gpu = had;
                        if (api.cur() === i) api.paintView();
                    },
                });
            },
            'mod-links': (el) => {
                const m = api.cur()?.mods.find((x) => x.id === el.dataset.id);
                if (m) api.openPop(el, linksPop(m), 'below');
            },
            'link-open': (el) => {
                api.closePop(false);
                toast(`正式版会用浏览器打开：${el.dataset.v}（示例链接）`);
            },
        },
        change(t) {
            const d = S.drawer;
            if (t.dataset.zhpick && d?.kind === 'i18n') {
                d.pick = t.checked ? [...d.pick, t.dataset.zhpick] : d.pick.filter((x) => x !== t.dataset.zhpick);
                api.paintDrawer();
                return true;
            }
            if (t.id === 'zhPackSw') {
                const i = api.cur();
                i.zhPack.on = t.checked;
                api.paintBody();
                toast(t.checked ? '汉化补全资源包开着，盖在最上面' : '关掉了汉化补全资源包，这几个 Mod 回到英文');
                return true;
            }
            if (d?.kind === 'tmpl') {
                if (t.id === 'tmFrom') {
                    d.from = t.value;
                    d.to = d.to.filter((x) => x !== t.value);
                } else if (t.dataset.tmitem) d.items[t.dataset.tmitem] = t.checked;
                else if (t.dataset.tmto) d.to = t.checked ? [...d.to, t.dataset.tmto] : d.to.filter((x) => x !== t.dataset.tmto);
                else if (t.id === 'tmDefault') d.asDefault = t.checked;
                else return false;
                api.paintDrawer();
                return true;
            }
            if (d?.kind === 'world') {
                if (t.id === 'nwCheats') d.cheats = t.checked;
                else if (t.id === 'nwStruct') d.structs = t.checked;
                else return false;
                return true;
            }
            return false;
        },
        input(t) {
            const d = S.drawer;
            if (d?.kind === 'world' && (t.id === 'nwName' || t.id === 'nwSeed')) {
                d[t.id === 'nwName' ? 'name' : 'seed'] = t.value;
                const b = $('#drCreate');
                if (b) b.disabled = !d.name.trim() || !canMakeWorld(api.byId(d.id).mc);
                return true;
            }
            return false;
        },
    };
}
