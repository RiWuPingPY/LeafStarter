// 真程序：世界工具（M13）——地图、查找已经生成的建筑、不进游戏改设置。后端在 commands/worldtools.rs
import { $, toast, esc } from './ui.js';
import { icon } from './icons.js';
import { invoke } from './real.js';

const seg = (act, list, cur) => `<div class="seg" role="group">${list.map(([v, t]) => `<button type="button" data-act="${act}" data-v="${v}" class="${String(cur) === v ? 'on' : ''}" aria-pressed="${String(cur) === v}">${t}</button>`).join('')}</div>`;

// 常见建筑的中文名
const NAMES = {
    village: '村庄', mineshaft: '废弃矿井', stronghold: '要塞', ruined_portal: '废弃传送门', shipwreck: '沉船', ocean_ruin: '海底废墟', trial_chambers: '试炼密室',
    monument: '海底神殿', mansion: '林地府邸', desert_pyramid: '沙漠神殿', jungle_pyramid: '丛林神庙', swamp_hut: '女巫小屋', igloo: '雪屋', pillager_outpost: '掠夺者前哨站',
    ancient_city: '远古城市', trail_ruins: '古迹废墟', buried_treasure: '埋藏的宝藏', fortress: '下界要塞', bastion_remnant: '堡垒遗迹', end_city: '末地城', nether_fossil: '下界化石',
};
export const structName = (id) => {
    const n = id.split(':').pop();
    const k = Object.keys(NAMES).find((x) => n === x || n.startsWith(x + '_'));
    return k ? NAMES[k] + (n !== k ? `（${n.slice(k.length + 1)}）` : '') : id.startsWith('minecraft:') ? n : id;
};
const DIMS = [['overworld', '主世界'], ['nether', '下界'], ['end', '末地']];
const MODES = [['survival', '生存'], ['creative', '创造'], ['adventure', '冒险'], ['spectator', '旁观']];
const DIFFS = [['peaceful', '和平'], ['easy', '简单'], ['normal', '普通'], ['hard', '困难']];
// 老写法（level.dat 里）和 26.x 起的新写法（game_rules.dat 里）
const RULES = [
    ['keepInventory', '死亡不掉落'], ['minecraft:keep_inventory', '死亡不掉落'], ['doDaylightCycle', '昼夜交替'], ['minecraft:advance_time', '昼夜交替'],
    ['doWeatherCycle', '天气变化'], ['minecraft:advance_weather', '天气变化'], ['doMobSpawning', '生成生物'], ['minecraft:spawn_mobs', '生成生物'],
    ['mobGriefing', '生物破坏方块'], ['minecraft:mob_griefing', '生物破坏方块'], ['doFireTick', '火会蔓延'], ['minecraft:fire_damage', '火焰伤害'],
    ['doInsomnia', '幻翼'], ['minecraft:spawn_phantoms', '幻翼'], ['minecraft:fall_damage', '摔落伤害'], ['naturalRegeneration', '自然回血'], ['minecraft:natural_health_regeneration', '自然回血'],
];

export function worldToolDrawer(c) {
    const d = c.S.drawer;
    const w = c.inst.worlds.find((x) => x.folder === d.world);
    const f = w?.info || {};
    let body;
    if (d.tab === 'find') {
        const list = d.found;
        const types = list && !list.error ? [...new Set(list.map((s) => structName(s.id).split('（')[0]))] : [];
        const shown = list && !list.error ? list.filter((s) => !d.type || structName(s.id).startsWith(d.type)) : [];
        body = `${seg('wt-dim', DIMS, d.dim)}
            <p class="note">列的是已经生成过的区块里记着的建筑（走到过附近才有）。还没去过的地方要按种子推算，这个还在做。</p>
            ${!list ? '<p class="note">正在翻区块…大世界要等一会儿</p>' : list.error ? `<p class="callout warn">${icon('alert', 14)}<span>${esc(list.error)}</span></p>` : `
            <div class="chips-row"><button type="button" class="chip${d.type ? '' : ' acc'}" data-act="wt-type" data-v="">全部 ${list.length}</button>${types.map((t) => `<button type="button" class="chip${d.type === t ? ' acc' : ''}" data-act="wt-type" data-v="${esc(t)}">${esc(t)}</button>`).join('')}</div>
            <div class="rows">${shown.slice(0, 300).map((s) => `<div class="row wt-row"><span class="tx"><span class="nm">${esc(structName(s.id))}</span><span class="sb mono">${s.x}, ${s.z} · 离出生点 ${s.dist} 格</span></span>
                <button type="button" class="btn sm ghost" data-act="wt-goto" data-x="${s.x}" data-z="${s.z}">${icon('globe', 13)}地图</button>
                <button type="button" class="btn sm ghost" data-act="wt-tp" data-v="/tp @s ${s.x} ~ ${s.z}" title="复制传送命令">${icon('copy', 13)}</button></div>`).join('') || '<div class="row empty">没有</div>'}</div>`}`;
    } else if (d.tab === 'edit') {
        const rules = d.rules || {};
        const sp = f.spawn || ['', '', ''];
        body = `<p class="note">改之前自动存一份快照，不满意在「历史」里退回。游戏开着这个实例时不能改。</p>
            <label class="field" for="wtMode"><span>游戏模式</span><select id="wtMode">${MODES.map(([v, t]) => `<option value="${v}"${f.mode === v ? ' selected' : ''}>${t}</option>`).join('')}</select></label>
            <label class="field" for="wtDiff"><span>难度</span><select id="wtDiff">${DIFFS.map(([v, t]) => `<option value="${v}"${f.difficulty === v ? ' selected' : ''}>${t}</option>`).join('')}</select></label>
            <label class="tick"><input type="checkbox" id="wtCheats"${f.cheats ? ' checked' : ''}>允许作弊（能用命令）</label>
            <div class="field"><span>时间（不改天数）</span>${seg('wt-time', [['', '不改'], ['1000', '早上'], ['6000', '中午'], ['12000', '傍晚'], ['18000', '午夜']], d.time ?? '')}</div>
            <div class="field"><span>天气</span>${seg('wt-weather', [['', '不改'], ['clear', '晴'], ['rain', '下雨'], ['thunder', '雷暴']], d.weather ?? '')}</div>
            <div class="field"><span>出生点</span><div class="field-row3"><input id="wtSx" type="text" inputmode="numeric" value="${sp[0]}"><input id="wtSy" type="text" inputmode="numeric" value="${sp[1]}"><input id="wtSz" type="text" inputmode="numeric" value="${sp[2]}"></div></div>
            ${Object.keys(rules).length ? `<div class="field"><span>游戏规则</span>${RULES.filter(([k]) => k in rules).map(([k, t]) => `<label class="tick"><input type="checkbox" data-wtrule="${k}"${rules[k] === 'true' ? ' checked' : ''}>${t} <small class="mono">${k}</small></label>`).join('')}</div>` : '<p class="note">这个版本的游戏规则不在 level.dat 里，进游戏用 /gamerule 改。</p>'}
            <div class="actions"><button type="button" class="btn pri" data-act="wt-save">${icon('check')}保存</button></div>`;
    } else {
        body = `${seg('wt-dim', DIMS, d.dim)}
            <div class="wt-map" id="wtMap"><canvas id="wtCv"></canvas><span class="wt-coord mono" id="wtCoord"></span></div>
            <p class="note">拖动平移，滚轮缩放。${icon('home', 12)} 出生点 · 紫点是上次退出的位置。只画了生成过的区块；1.12 及以前的存档画不了。</p>`;
    }
    return `<section class="drawer wide" role="dialog" aria-label="世界工具">
        <header class="dr-head"><h2>${esc(w?.name || d.world)}</h2>${seg('wt-tab', [['map', '地图'], ['find', '查找'], ['edit', '设置']], d.tab)}<span class="sp"></span><button type="button" class="iconbtn" data-act="dr-close" aria-label="关闭" title="关闭（Esc）">${icon('x')}</button></header>
        <div class="dr-body">${body}</div></section>`;
}

export function createWorldTool(api) {
    const { S } = api;
    // 地图状态：中心（方块坐标）、每格几像素、已经拿到的图
    const M = { key: '', regions: [], tiles: new Map(), cx: 0, cz: 0, scale: 0.5, spawn: null, player: null, mark: null };
    const d = () => (S.drawer?.kind === 'worldtool' ? S.drawer : null);
    const keyOf = (x) => `${api.cur().id}|${x.world}|${x.dim}`;

    async function loadMap() {
        const x = d();
        if (!x || x.tab !== 'map') return;
        const k = keyOf(x);
        if (M.key !== k) {
            M.key = k;
            M.tiles = new Map();
            M.regions = [];
            M.mark = null;
            const i = api.cur();
            try {
                const r = await invoke('world_map_regions', { dir: i.dir, name: i.name, world: x.world, dim: x.dim });
                if (M.key !== k) return;
                M.regions = r.regions;
                M.spawn = r.spawn;
                M.player = r.player;
                const c = r.player || r.spawn || (r.regions.length ? [r.regions.reduce((s, q) => s + q[0], 0) / r.regions.length * 512 + 256, r.regions.reduce((s, q) => s + q[1], 0) / r.regions.length * 512 + 256] : [0, 0]);
                [M.cx, M.cz] = c;
                M.scale = 0.5;
            } catch (e) {
                toast(String(e));
            }
        }
        bind();
        draw();
    }

    function tile(rx, rz) {
        const k = `${rx},${rz}`;
        if (M.tiles.has(k)) return M.tiles.get(k);
        const t = { img: null };
        M.tiles.set(k, t);
        const x = d();
        const i = api.cur();
        const mk = M.key;
        invoke('world_map_tile', { dir: i.dir, name: i.name, world: x.world, dim: x.dim, x: rx, z: rz }).then((b64) => {
            const im = new Image();
            im.onload = () => {
                if (M.key !== mk) return;
                t.img = im;
                draw();
            };
            im.src = 'data:image/png;base64,' + b64;
        }, () => {
            t.failed = true;
        });
        return t;
    }

    function draw() {
        const cv = $('#wtCv');
        if (!cv) return;
        const box = cv.parentElement;
        const W = box.clientWidth;
        const H = box.clientHeight;
        const dpr = window.devicePixelRatio || 1;
        if (cv.width !== W * dpr || cv.height !== H * dpr) {
            cv.width = W * dpr;
            cv.height = H * dpr;
            cv.style.width = W + 'px';
            cv.style.height = H + 'px';
        }
        const g = cv.getContext('2d');
        g.setTransform(dpr, 0, 0, dpr, 0, 0);
        g.imageSmoothingEnabled = false;
        g.fillStyle = getComputedStyle(box).getPropertyValue('--wt-bg') || '#1b1b21';
        g.fillRect(0, 0, W, H);
        const s = M.scale;
        const toX = (bx) => (bx - M.cx) * s + W / 2;
        const toY = (bz) => (bz - M.cz) * s + H / 2;
        const have = new Set(M.regions.map(([a, b]) => `${a},${b}`));
        const r0x = Math.floor((M.cx - W / 2 / s) / 512);
        const r1x = Math.floor((M.cx + W / 2 / s) / 512);
        const r0z = Math.floor((M.cz - H / 2 / s) / 512);
        const r1z = Math.floor((M.cz + H / 2 / s) / 512);
        for (let rx = r0x; rx <= r1x; rx++) {
            for (let rz = r0z; rz <= r1z; rz++) {
                if (!have.has(`${rx},${rz}`)) continue;
                const t = tile(rx, rz);
                if (t.img) g.drawImage(t.img, toX(rx * 512), toY(rz * 512), 512 * s, 512 * s);
            }
        }
        const dot = (p, col, r) => {
            if (!p) return;
            g.beginPath();
            g.arc(toX(p[0]), toY(p[1]), r, 0, Math.PI * 2);
            g.fillStyle = col;
            g.fill();
            g.lineWidth = 2;
            g.strokeStyle = '#ffffff';
            g.stroke();
        };
        dot(M.spawn, '#E9C12E', 6);
        dot(M.player, '#7D4ED6', 6);
        dot(M.mark, '#E0631F', 7);
    }

    let bound = null;
    function bind() {
        const cv = $('#wtCv');
        if (!cv || bound === cv) return;
        bound = cv;
        let drag = null;
        cv.addEventListener('pointerdown', (e) => {
            drag = { x: e.clientX, y: e.clientY, cx: M.cx, cz: M.cz };
            cv.setPointerCapture(e.pointerId);
        });
        cv.addEventListener('pointermove', (e) => {
            const r = cv.getBoundingClientRect();
            const bx = Math.floor(M.cx + (e.clientX - r.left - r.width / 2) / M.scale);
            const bz = Math.floor(M.cz + (e.clientY - r.top - r.height / 2) / M.scale);
            const c = $('#wtCoord');
            if (c) c.textContent = `${bx}, ${bz}`;
            if (!drag) return;
            M.cx = drag.cx - (e.clientX - drag.x) / M.scale;
            M.cz = drag.cz - (e.clientY - drag.y) / M.scale;
            draw();
        });
        cv.addEventListener('pointerup', () => {
            drag = null;
        });
        cv.addEventListener('wheel', (e) => {
            e.preventDefault();
            const k = e.deltaY < 0 ? 1.25 : 0.8;
            M.scale = Math.min(8, Math.max(1 / 16, M.scale * k));
            draw();
        }, { passive: false });
    }

    async function loadFound() {
        const x = d();
        if (!x || x.tab !== 'find' || x.found) return;
        const i = api.cur();
        try {
            x.found = await invoke('world_structures', { dir: i.dir, name: i.name, world: x.world, dim: x.dim });
        } catch (e) {
            x.found = { error: String(e) };
        }
        if (d() === x) api.paintDrawer();
    }

    async function loadRules() {
        const x = d();
        if (!x || x.tab !== 'edit' || x.rules) return;
        const i = api.cur();
        x.rules = await invoke('world_rules', { dir: i.dir, name: i.name, world: x.world }).catch(() => ({}));
        if (d() === x) api.paintDrawer();
    }

    function after() {
        loadMap();
        loadFound();
        loadRules();
    }
    window.addEventListener('resize', () => d()?.tab === 'map' && draw());

    return {
        after,
        actions: {
            'wt-open': (el) => {
                api.openDrawer({ kind: 'worldtool', world: el.dataset.v, tab: el.dataset.tab || 'map', dim: 'overworld' });
                requestAnimationFrame(after);
            },
            'wt-tab': (el) => {
                S.drawer.tab = el.dataset.v;
                api.paintDrawer();
                requestAnimationFrame(after);
            },
            'wt-dim': (el) => {
                S.drawer.dim = el.dataset.v;
                S.drawer.found = null;
                S.drawer.type = '';
                api.paintDrawer();
                requestAnimationFrame(after);
            },
            'wt-type': (el) => {
                S.drawer.type = el.dataset.v;
                api.paintDrawer();
            },
            'wt-goto': (el) => {
                S.drawer.tab = 'map';
                api.paintDrawer();
                requestAnimationFrame(async () => {
                    await loadMap();
                    M.cx = Number(el.dataset.x);
                    M.cz = Number(el.dataset.z);
                    M.mark = [M.cx, M.cz];
                    M.scale = Math.max(M.scale, 1);
                    draw();
                });
            },
            'wt-tp': (el) => navigator.clipboard.writeText(el.dataset.v).then(() => toast(`复制了 ${el.dataset.v}`)),
            'wt-time': (el) => {
                S.drawer.time = el.dataset.v;
                api.paintDrawer();
            },
            'wt-weather': (el) => {
                S.drawer.weather = el.dataset.v;
                api.paintDrawer();
            },
            'wt-save': async () => {
                const x = d();
                const i = api.cur();
                const num = (id) => ($(id)?.value.trim() === '' ? null : Number($(id).value));
                const patch = { mode: $('#wtMode')?.value, difficulty: $('#wtDiff')?.value, cheats: !!$('#wtCheats')?.checked };
                if (x.time) patch.time = Number(x.time);
                if (x.weather) patch.weather = x.weather;
                const sp = [num('#wtSx'), num('#wtSy'), num('#wtSz')];
                if (sp.every((v) => Number.isFinite(v))) patch.spawn = sp;
                const rules = {};
                document.querySelectorAll('[data-wtrule]').forEach((cb) => {
                    if (String(cb.checked) !== x.rules?.[cb.dataset.wtrule]) rules[cb.dataset.wtrule] = cb.checked;
                });
                if (Object.keys(rules).length) patch.rules = rules;
                try {
                    const r = await invoke('world_edit', { dir: i.dir, name: i.name, world: x.world, patch });
                    toast(r.changed.length ? `改好了：${[...new Set(r.changed)].join('、')}。改之前的样子在「历史」里` : '没有要改的');
                    x.rules = null;
                    api.refreshWorlds?.(i);
                    api.paintDrawer();
                    requestAnimationFrame(after);
                } catch (e) {
                    toast(String(e));
                }
            },
        },
    };
}
