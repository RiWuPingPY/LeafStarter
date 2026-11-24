// 世界工具：补全存档信息、画地图（去过的地方 + 按种子推算的大概）、推算建筑、局外修改、指令台
import { $, toast, FACES } from './ui.js';
import { seedOf } from './scene.js';
import { STYPES, RULES, confirmCard } from './views/world.js';

// ---------- 噪声：整数哈希 + 值噪声 + 分形叠加，同一个种子永远画出同一张图 ----------
const H = (a, b, s) => {
    let h = Math.imul(a | 0, 374761393) ^ Math.imul(b | 0, 668265263) ^ s;
    h = Math.imul(h ^ (h >>> 13), 1274126177);
    return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
};
function vnoise(x, y, s) {
    const ix = Math.floor(x);
    const iy = Math.floor(y);
    const fx = x - ix;
    const fy = y - iy;
    const u = fx * fx * (3 - 2 * fx);
    const v = fy * fy * (3 - 2 * fy);
    const a = H(ix, iy, s);
    const b = H(ix + 1, iy, s);
    const c = H(ix, iy + 1, s);
    const d = H(ix + 1, iy + 1, s);
    return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}
function fbm(x, y, s, oct = 4) {
    let t = 0;
    let amp = 0.5;
    let f = 1;
    let n = 0;
    for (let k = 0; k < oct; k++) {
        t += vnoise(x * f, y * f, s + k * 1013) * amp;
        n += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    return t / n;
}

const BIO = {
    deep: ['#24457E', '深海'], ocean: ['#3462A8', '海洋'], beach: ['#D8CB8D', '沙滩'], river: ['#3F6CC0', '河流'],
    mount: ['#8E8E8A', '山地'], peaks: ['#E8EDF1', '雪峰'], snowy: ['#E1E9EE', '雪原'], taiga: ['#4E7A5C', '针叶林'],
    desert: ['#DDC57A', '沙漠'], jungle: ['#3A9A2C', '丛林'], savanna: ['#ADA758', '热带草原'], swamp: ['#4F6340', '沼泽'],
    dark: ['#2F5A27', '黑森林'], forest: ['#4C8A39', '森林'], plains: ['#7DB35A', '平原'],
    void: ['#0D111C', '虚空'], island: ['#69AA4B', '空岛'],
    cave: ['#1B1B21', '洞穴'], lava: ['#E0631F', '岩浆'], diamond: ['#5ED6D0', '钻石矿'], iron: ['#D8AF93', '铁矿'], red: ['#B8321F', '红石矿'], slate: ['#50505A', '深板岩'],
    lavasea: ['#D9541E', '岩浆海'], soul: ['#51413A', '灵魂沙峡谷'], warped: ['#2C7C74', '诡异森林'], crimson: ['#8F2230', '绯红森林'], basalt: ['#3B3A40', '玄武岩三角洲'], waste: ['#6F2B2B', '下界荒地'],
};

function surface(x, z, s) {
    const e = fbm(x / 1100, z / 1100, s, 5);
    if (e < 0.38) return ['deep', e];
    if (e < 0.44) return ['ocean', e];
    const t = fbm(x / 1900 + 31, z / 1900 - 17, s ^ 0x5bd1, 3);
    const m = fbm(x / 1500 - 7, z / 1500 + 23, s ^ 0x1b56, 3);
    if (e < 0.455 && t > 0.4) return ['beach', e];
    const r = Math.abs(fbm(x / 800 + 11, z / 800 + 5, s ^ 0x77, 3) - 0.5);
    if (r < 0.011 && e < 0.6) return ['river', e];
    if (e > 0.655) return [t < 0.52 ? 'peaks' : 'mount', e];
    if (e > 0.61) return ['mount', e];
    if (t < 0.42) return [m > 0.52 ? 'taiga' : 'snowy', e];
    if (t > 0.58) return [m < 0.45 ? 'desert' : m > 0.57 ? 'jungle' : 'savanna', e];
    if (m > 0.6) return [e < 0.5 ? 'swamp' : 'dark', e];
    if (m > 0.52) return ['forest', e];
    return ['plains', e];
}
function under(x, z, s) {
    const c = Math.abs(fbm(x / 70, z / 70, s ^ 0x3c, 3) - 0.5);
    if (c < 0.035) return ['cave', 0.4];
    if (fbm(x / 260, z / 260, s ^ 0x4d, 2) > 0.68) return ['lava', 0.5];
    const o = H(Math.floor(x / 3), Math.floor(z / 3), s ^ 0x9e);
    if (o < 0.002) return ['diamond', 0.5];
    if (o < 0.007) return ['iron', 0.5];
    if (o < 0.011) return ['red', 0.5];
    return ['slate', 0.45 + c];
}
function nether(x, z, s) {
    const e = fbm(x / 500, z / 500, s ^ 0x2a, 4);
    if (e < 0.41) return ['lavasea', e];
    const t = fbm(x / 700, z / 700, s ^ 0x2b, 3);
    const m = fbm(x / 650 + 9, z / 650, s ^ 0x2c, 3);
    if (t < 0.42) return ['soul', e];
    if (m > 0.58) return ['warped', e];
    if (m < 0.42) return ['crimson', e];
    if (t > 0.6) return ['basalt', e];
    return ['waste', e];
}
function sky(x, z, s) {
    // 空岛：出生点附近几块小岛，其余是虚空
    const d = Math.hypot(x, z) / (0.75 + 0.5 * fbm(x / 14, z / 14, s, 2));
    if (d < 22) return ['island', 0.48 + d / 200];
    if (Math.hypot(x - 70, z + 40) / (0.8 + 0.4 * fbm(x / 10, z / 10, s ^ 5, 2)) < 13) return ['island', 0.5];
    return ['void', 0.5];
}

const hex = (c) => [parseInt(c.slice(1, 3), 16), parseInt(c.slice(3, 5), 16), parseInt(c.slice(5, 7), 16)];
const RGB = Object.fromEntries(Object.entries(BIO).map(([k, [c]]) => [k, hex(c)]));
const ZOOMS = [1, 2, 4, 8, 16, 32];
const T = 64;

export function createWorld(api) {
    const { S } = api;
    S.world = null;
    S.wtab = 'info';
    S.findType = 'all';
    S.map = { layer: 'surface', z: 3, cx: 0, cz: 0, hi: null };
    S.wdraft = null;
    S.wc = null;
    S.console = api.store.get('console', false);

    const cur = () => api.cur();
    const curWorld = () => cur()?.worlds.find((x) => x.id === S.world) || null;

    // ---------- 存档信息：示例数据里没写的项，按世界 id 算一份固定的 ----------
    function meta(w) {
        if (w._m) return w;
        const i = cur();
        const h = seedOf(w.id + (i?.id || ''));
        const r = (k) => H(h, k, 7);
        if (w.skyblock) Object.assign(w, { spawn: [-6, 64, 4], pos: [70, 65, -40], reach: 160, deaths: [[34, -14]], inv: [['树苗', '#4E8B3A', 2], ['岩浆桶', '#E0631F'], ['冰', '#A9D3F5', 2], ['骨粉', '#E8E4D6', 9]] });
        w.seed ??= (r(1) < 0.5 ? '-' : '') + String(Math.floor(1e9 + r(2) * 8.9e9)) + String(Math.floor(r(3) * 1e9)).padStart(9, '0');
        w.playMin ??= Math.round(30 + r(4) * 900);
        w.spawn ??= [Math.round(r(5) * 240 - 120), 64 + Math.round(r(6) * 10), Math.round(r(7) * 240 - 120)];
        w.pos ??= [w.spawn[0] + Math.round(r(8) * 700 - 350), 63 + Math.round(r(9) * 12), w.spawn[2] + Math.round(r(10) * 700 - 350)];
        w.deaths ??= r(11) < 0.5 ? [[w.spawn[0] + Math.round(r(12) * 500 - 250), w.spawn[2] + Math.round(r(13) * 500 - 250)]] : [];
        w.reach ??= Math.min(1800, 300 + (parseInt(w.size, 10) || 50) * 3);
        w.hp ??= 20;
        w.food ??= 18 + Math.round(r(14) * 2);
        w.lvl ??= Math.round(r(15) * 18);
        w.rules ??= { keepInventory: false, doDaylightCycle: true, mobGriefing: true, doFireTick: true };
        w.cheats ??= w.mode === '创造';
        w.inv ??= [['石剑', '#9A9A9A'], ['火把', '#F2B33D', 16], ['面包', '#C9903D', 6], ['圆石', '#7E7E7E', 64]];
        w._m = true;
        return w;
    }

    // ---------- 去过的地方：出生点、现在的位置、两者之间的路、死过的地方，边缘不规则 ----------
    function reachOf(w, layer) {
        const k = layer === 'nether' ? 8 : 1;
        const pts = [w.spawn, w.pos].map((p) => [p[0] / k, p[2] / k]).concat(w.deaths.map((d) => [d[0] / k, d[1] / k]));
        const R = w.reach * (layer === 'nether' ? 0.22 : layer === 'under' ? 0.45 : 1);
        const [a, b] = pts;
        const s = seedOf(w.seed) ^ 99;
        return (x, z) => {
            let d = Infinity;
            for (const p of pts) d = Math.min(d, Math.hypot(x - p[0], z - p[1]));
            const vx = b[0] - a[0];
            const vz = b[1] - a[1];
            const L = vx * vx + vz * vz || 1;
            const t = Math.max(0, Math.min(1, ((x - a[0]) * vx + (z - a[1]) * vz) / L));
            d = Math.min(d, Math.hypot(x - a[0] - t * vx, z - a[1] - t * vz) * 1.6);
            return d < R * (0.78 + 0.45 * fbm(x / 350, z / 350, s, 2));
        };
    }

    function sampler(w, layer) {
        const s = seedOf(w.seed);
        if (w.skyblock && layer === 'surface') return (x, z) => sky(x, z, s);
        return layer === 'under' ? (x, z) => under(x, z, s) : layer === 'nether' ? (x, z) => nether(x, z, s) : (x, z) => surface(x, z, s);
    }

    // ---------- 地图：按 64×64 的块缓存，拖动时只画新露出来的块 ----------
    let M = null; // { wid, cv, tb, tiles, W, H, css: [w, h], dpr }
    const tiles = new Map();
    let tilesFor = '';

    function tile(w, layer, bpp, tx, tz) {
        const key = `${layer}:${bpp}:${tx}:${tz}`;
        const hit = tiles.get(key);
        if (hit) return hit;
        const cv = document.createElement('canvas');
        cv.width = cv.height = T;
        const g = cv.getContext('2d');
        const img = g.createImageData(T, T);
        const f = sampler(w, layer);
        const seen = reachOf(w, layer);
        const x0 = tx * T * bpp;
        const z0 = tz * T * bpp;
        for (let py = 0; py < T; py++) {
            const z = z0 + (py + 0.5) * bpp;
            let prev = f(x0 - 0.5 * bpp, z)[1];
            for (let px = 0; px < T; px++) {
                const x = x0 + (px + 0.5) * bpp;
                const [b, e] = f(x, z);
                const c = RGB[b];
                let k = 1 + Math.max(-0.16, Math.min(0.16, (e - prev) * (bpp < 4 ? 40 : 14)));
                if (b === 'deep' || b === 'ocean') k = 0.9 + (e - 0.3) * 0.8;
                prev = e;
                let [r, gg, bb] = [c[0] * k, c[1] * k, c[2] * k];
                if (!seen(x, z)) {
                    // 没去过：灰掉，再压一层斜纹，一眼能分出来
                    const gray = (r + gg + bb) / 3;
                    const st = ((px + py) & 3) === 0 ? 0.82 : 1;
                    r = (gray * 0.6 + r * 0.4) * 0.62 * st + 40;
                    gg = (gray * 0.6 + gg * 0.4) * 0.62 * st + 40;
                    bb = (gray * 0.6 + bb * 0.4) * 0.62 * st + 44;
                }
                const o = (py * T + px) * 4;
                img.data[o] = r;
                img.data[o + 1] = gg;
                img.data[o + 2] = bb;
                img.data[o + 3] = 255;
            }
        }
        g.putImageData(img, 0, 0);
        if (tiles.size > 600) tiles.clear();
        tiles.set(key, cv);
        return cv;
    }

    const bppOf = () => ZOOMS[S.map.z];

    function render() {
        const w = curWorld();
        if (!M || !w || !M.cv.isConnected) return;
        if (tilesFor !== w.id + w.seed) {
            tiles.clear();
            tilesFor = w.id + w.seed;
        }
        const { layer, cx, cz } = S.map;
        const bpp = bppOf();
        const tg = M.tb.getContext('2d');
        const left = cx - (M.W / 2) * bpp;
        const top = cz - (M.H / 2) * bpp;
        const span = T * bpp;
        for (let tx = Math.floor(left / span); tx * span < left + M.W * bpp; tx++) {
            for (let tz = Math.floor(top / span); tz * span < top + M.H * bpp; tz++) {
                tg.drawImage(tile(w, layer, bpp, tx, tz), Math.round((tx * span - left) / bpp), Math.round((tz * span - top) / bpp));
            }
        }
        const g = M.cv.getContext('2d');
        const [cw, ch] = M.css;
        g.setTransform(M.dpr, 0, 0, M.dpr, 0, 0);
        g.imageSmoothingEnabled = false;
        g.drawImage(M.tb, 0, 0, M.W * 2, M.H * 2);
        marks(g, w, cw, ch);
        const sc = $('#mapScale');
        if (sc) sc.textContent = `每像素 ${bpp / 2} 格 · ${layer === 'nether' ? '下界坐标' : '主世界坐标'}`;
    }

    const toScreen = (x, z) => {
        const [cw, ch] = M.css;
        return [((x - S.map.cx) / bppOf()) * 2 + cw / 2, ((z - S.map.cz) / bppOf()) * 2 + ch / 2];
    };
    const toWorld = (sx, sy) => {
        const [cw, ch] = M.css;
        return [S.map.cx + ((sx - cw / 2) / 2) * bppOf(), S.map.cz + ((sy - ch / 2) / 2) * bppOf()];
    };

    function marks(g, w, cw, ch) {
        const L = S.map.layer;
        const k = L === 'nether' ? 8 : 1;
        const inView = ([x, y]) => x > -20 && y > -20 && x < cw + 20 && y < ch + 20;
        // 建筑：小方块
        if (L === 'surface' && bppOf() <= 16) {
            for (const s of structs(w)) {
                const p = toScreen(s.x, s.z);
                if (!inView(p)) continue;
                g.fillStyle = `hsl(${STYPES[s.type][1]} 60% 50%)`;
                g.strokeStyle = '#FFFFFF';
                g.lineWidth = 1.5;
                g.fillRect(p[0] - 4, p[1] - 4, 8, 8);
                g.strokeRect(p[0] - 4, p[1] - 4, 8, 8);
            }
        }
        // 死过的地方：红叉
        if (L !== 'nether') {
            for (const d of w.deaths) {
                const [x, y] = toScreen(d[0], d[1]);
                if (!inView([x, y])) continue;
                g.strokeStyle = '#FFFFFF';
                g.lineWidth = 4;
                cross(g, x, y);
                g.strokeStyle = '#E11D48';
                g.lineWidth = 2;
                cross(g, x, y);
            }
        }
        // 出生点：白色星
        const sp = toScreen(w.spawn[0] / k, w.spawn[2] / k);
        if (inView(sp)) star(g, sp[0], sp[1]);
        // 你：头像
        {
            const me = toScreen(w.pos[0] / k, w.pos[2] / k);
            if (inView(me)) {
                const f = FACES[api.face()] || FACES.steve;
                const s = 2.25;
                const x0 = Math.round(me[0] - 4 * s);
                const y0 = Math.round(me[1] - 4 * s);
                g.fillStyle = '#FFFFFF';
                g.fillRect(x0 - 2, y0 - 2, 8 * s + 4, 8 * s + 4);
                f.rows.forEach((row, y) => [...row].forEach((chr, x) => {
                    g.fillStyle = f.pal[chr];
                    g.fillRect(x0 + x * s, y0 + y * s, Math.ceil(s), Math.ceil(s));
                }));
            }
        }
        // 选中的建筑 / 地点：一圈
        const hi = S.map.hi;
        if (hi && L === 'surface') {
            const [x, y] = toScreen(hi.x, hi.z);
            g.strokeStyle = '#FFFFFF';
            g.lineWidth = 3;
            g.beginPath();
            g.arc(x, y, 13, 0, Math.PI * 2);
            g.stroke();
            g.strokeStyle = '#7C3AED';
            g.lineWidth = 2;
            g.stroke();
            g.font = '600 12px system-ui, sans-serif';
            const tw = g.measureText(hi.label).width;
            g.fillStyle = 'rgba(20,18,14,.78)';
            g.fillRect(x + 16, y - 10, tw + 12, 20);
            g.fillStyle = '#FFFFFF';
            g.fillText(hi.label, x + 22, y + 4);
        }
    }
    function cross(g, x, y) {
        g.beginPath();
        g.moveTo(x - 5, y - 5);
        g.lineTo(x + 5, y + 5);
        g.moveTo(x + 5, y - 5);
        g.lineTo(x - 5, y + 5);
        g.stroke();
    }
    function star(g, x, y) {
        g.beginPath();
        for (let k = 0; k < 10; k++) {
            const r = k % 2 ? 3.4 : 8;
            const a = (k / 10) * Math.PI * 2 - Math.PI / 2;
            g.lineTo(x + Math.cos(a) * r, y + Math.sin(a) * r);
        }
        g.closePath();
        g.fillStyle = '#FFFFFF';
        g.strokeStyle = '#1F1D26';
        g.lineWidth = 2;
        g.stroke();
        g.fill();
    }

    // 画完界面后调用：有地图就接上
    function after() {
        const cv = $('#wmap');
        if (!cv) {
            M = null;
            return;
        }
        const wrap = cv.parentElement;
        const cw = wrap.clientWidth;
        const ch = wrap.clientHeight;
        const dpr = Math.min(2, window.devicePixelRatio || 1);
        cv.width = Math.round(cw * dpr);
        cv.height = Math.round(ch * dpr);
        const tb = document.createElement('canvas');
        tb.width = Math.ceil(cw / 2);
        tb.height = Math.ceil(ch / 2);
        M = { cv, tb, W: tb.width, H: tb.height, css: [cw, ch], dpr };
        bind(cv);
        render();
    }

    let raf = 0;
    const later = () => {
        if (!raf) raf = requestAnimationFrame(() => { raf = 0; render(); });
    };

    function bind(cv) {
        let drag = null;
        cv.addEventListener('pointerdown', (e) => {
            drag = { x: e.clientX, y: e.clientY, cx: S.map.cx, cz: S.map.cz };
            try { cv.setPointerCapture(e.pointerId); } catch {}
            cv.classList.add('drag');
        });
        cv.addEventListener('pointermove', (e) => {
            if (drag) {
                S.map.cx = drag.cx - ((e.clientX - drag.x) / 2) * bppOf();
                S.map.cz = drag.cz - ((e.clientY - drag.y) / 2) * bppOf();
                later();
            }
            readout(e);
        });
        const end = () => {
            drag = null;
            cv.classList.remove('drag');
        };
        cv.addEventListener('pointerup', end);
        cv.addEventListener('pointercancel', end);
        cv.addEventListener('pointerleave', () => {
            const r = $('#mapRead');
            if (r && !drag) r.textContent = '拖动平移，滚轮缩放';
        });
        cv.addEventListener('wheel', (e) => {
            e.preventDefault();
            const r = cv.getBoundingClientRect();
            zoom(e.deltaY > 0 ? 1 : -1, e.clientX - r.left, e.clientY - r.top);
        }, { passive: false });
    }

    function readout(e) {
        const w = curWorld();
        const el = $('#mapRead');
        if (!w || !el || !M) return;
        const r = M.cv.getBoundingClientRect();
        const [x, z] = toWorld(e.clientX - r.left, e.clientY - r.top);
        const [b] = sampler(w, S.map.layer)(x, z);
        const seen = reachOf(w, S.map.layer)(x, z);
        el.textContent = `X ${Math.round(x)} · Z ${Math.round(z)} · ${BIO[b][1]} · ${seen ? '去过' : '没去过，按种子推算'}`;
    }

    function zoom(d, sx, sy) {
        const nz = Math.max(0, Math.min(ZOOMS.length - 1, S.map.z + d));
        if (nz === S.map.z || !M) return;
        const [cw, ch] = M.css;
        const ax = sx ?? cw / 2;
        const ay = sy ?? ch / 2;
        const before = toWorld(ax, ay);
        S.map.z = nz;
        const afterW = toWorld(ax, ay);
        S.map.cx += before[0] - afterW[0];
        S.map.cz += before[1] - afterW[1];
        render();
    }

    // ---------- 建筑：按区域网格撒点，落在对的群系里才算 ----------
    const SPEC = {
        village: [544, ['plains', 'savanna', 'desert', 'taiga', 'snowy'], 6],
        outpost: [704, ['plains', 'savanna', 'desert', 'taiga', 'snowy'], 3],
        desert_temple: [512, ['desert'], 3],
        jungle_temple: [512, ['jungle'], 2],
        swamp_hut: [512, ['swamp'], 2],
        monument: [512, ['deep'], 3],
        mansion: [1280, ['dark'], 1],
        shipwreck: [384, ['ocean', 'beach'], 3],
        ruined_portal: [640, null, 3],
        ancient_city: [1536, ['mount', 'peaks'], 2],
    };
    const scache = new Map();
    function structs(w) {
        if (w.skyblock) return [];
        const key = w.id + w.seed + w.reach;
        if (scache.has(key)) return scache.get(key);
        const s = seedOf(w.seed);
        const [sx, , sz] = w.spawn;
        const R = 3200;
        const seen = reachOf(w, 'surface');
        const out = [];
        let n = 0;
        Object.entries(SPEC).forEach(([type, [sp, ok, keep]], ti) => {
            const found = [];
            for (let gx = Math.floor((sx - R) / sp); gx <= Math.floor((sx + R) / sp); gx++) {
                for (let gz = Math.floor((sz - R) / sp); gz <= Math.floor((sz + R) / sp); gz++) {
                    const x = Math.round(gx * sp + H(gx, gz, s ^ (ti * 7919 + 1)) * sp * 0.75);
                    const z = Math.round(gz * sp + H(gz, gx, s ^ (ti * 104729 + 3)) * sp * 0.75);
                    const dist = Math.hypot(x - sx, z - sz);
                    if (dist > R) continue;
                    const [b] = surface(x, z, s);
                    if (ok ? !ok.includes(b) : ['deep', 'ocean', 'river'].includes(b)) continue;
                    found.push({ type, x, z, dist, y: type === 'ancient_city' ? -51 : null });
                }
            }
            found.sort((a, b) => a.dist - b.dist).slice(0, keep).forEach((f) => out.push(f));
        });
        // 要塞：在离原点 1280–2816 格的环上，三个，角度均分
        const a0 = H(1, 2, s) * Math.PI * 2;
        for (let k = 0; k < 3; k++) {
            const r = 1280 + H(3, k, s) * 1536;
            const x = Math.round(Math.cos(a0 + (k * Math.PI * 2) / 3) * r);
            const z = Math.round(Math.sin(a0 + (k * Math.PI * 2) / 3) * r);
            out.push({ type: 'stronghold', x, z, y: 30 + Math.round(H(k, 9, s) * 12), dist: Math.hypot(x - sx, z - sz) });
        }
        out.forEach((o) => {
            o.id = 's' + n++;
            o.gen = seen(o.x, o.z);
        });
        out.sort((a, b) => a.dist - b.dist);
        scache.set(key, out);
        return out;
    }

    // ---------- 局外修改 ----------
    const draftOf = (w) => ({ mode: w.mode, diff: w.diff, cheats: !!w.cheats, rules: { ...w.rules }, time: 'keep', weather: 'keep' });
    function wdirty(w) {
        const d = S.wdraft;
        if (!d) return false;
        return d.mode !== w.mode || d.diff !== w.diff || d.cheats !== !!w.cheats || d.time !== 'keep' || d.weather !== 'keep' || RULES.some(([k]) => d.rules[k] !== w.rules[k]);
    }
    function wdChange(el) {
        const d = S.wdraft;
        const k = el.dataset.wd;
        const v = el.type === 'checkbox' ? el.checked : el.value;
        if (k.startsWith('r:')) d.rules[k.slice(2)] = v;
        else d[k] = v;
        api.paintBody();
    }
    function wdSave() {
        const w = curWorld();
        const d = S.wdraft;
        if (!w || !wdirty(w)) return;
        const before = { mode: w.mode, diff: w.diff, cheats: w.cheats, rules: { ...w.rules } };
        const what = [];
        if (d.mode !== w.mode) what.push(`模式改成${d.mode}`);
        if (d.diff !== w.diff) what.push(`难度改成${d.diff}`);
        if (d.cheats !== !!w.cheats) what.push(d.cheats ? '允许作弊' : '不许作弊');
        RULES.forEach(([k, t]) => { if (d.rules[k] !== w.rules[k]) what.push(`${t}${d.rules[k] ? '开' : '关'}`); });
        if (d.time !== 'keep') what.push({ day: '改成白天', noon: '改成正午', night: '改成夜晚' }[d.time]);
        if (d.weather !== 'keep') what.push({ clear: '天晴', rain: '下雨', thunder: '雷雨' }[d.weather]);
        Object.assign(w, { mode: d.mode, diff: d.diff, cheats: d.cheats, rules: { ...d.rules } });
        S.wdraft = draftOf(w);
        api.paintBody();
        const stampName = new Date().toTimeString().slice(0, 5).replace(':', '');
        toast(`「${w.name}」${what.join('、')}。改之前备份到了 backups\\${w.name}-${stampName}.zip`, {
            undo: () => {
                Object.assign(w, before);
                S.wdraft = draftOf(w);
                if (curWorld() === w) api.paintBody();
            },
        });
    }

    // ---------- 指令台 ----------
    const wcOf = (w) => (w._wc ??= { state: 'off', ready: false, log: [], snaps: [], ask: null, pct: 0 });
    const esc2 = (s) => String(s).replace(/[&<>]/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[ch]));
    const tnow = () => new Date().toTimeString().slice(0, 8);

    function paintWc() {
        api.paintBody();
        const lg = $('#wcLog');
        if (lg) lg.scrollTop = lg.scrollHeight;
    }

    function aiCmds(text, w) {
        const [sx, sy, sz] = w.spawn;
        const me = api.acctName();
        const cmds = [];
        const say = [];
        if (/玻璃/.test(text) && /平台|地板|台子/.test(text)) {
            cmds.push(`/fill ${sx - 4} ${sy - 1} ${sz - 4} ${sx + 4} ${sy - 1} ${sz + 4} minecraft:glass`);
            say.push(`出生点在 ${sx}, ${sy}, ${sz}，在它脚下一格铺 9×9 的玻璃。`);
        }
        if (/白天|天亮/.test(text)) cmds.push('/time set day');
        if (/晴|雨停|别下雨/.test(text)) cmds.push('/weather clear');
        const dm = text.match(/(\d+)\s*个?钻石/);
        if (/钻石/.test(text)) cmds.push(`/give ${me} minecraft:diamond ${dm ? dm[1] : 64}`);
        if (/回出生点|传送.*出生|出生点.*传送/.test(text)) cmds.push(`/tp ${me} ${sx} ${sy} ${sz}`);
        if (/死亡不掉落|保留物品|不掉东西/.test(text)) cmds.push('/gamerule keepInventory true');
        if (/僵尸/.test(text)) cmds.push('/kill @e[type=minecraft:zombie]');
        return cmds.length ? { cmds, say: say.join(''), state: 'wait' } : null;
    }

    function output(cmd, w) {
        const c = cmd.trim().replace(/^\//, '');
        const [op, ...a] = c.split(/\s+/);
        switch (op) {
            case 'fill': {
                const n = a.slice(0, 6).map(Number);
                return n.every(Number.isFinite) ? `Successfully filled ${(Math.abs(n[3] - n[0]) + 1) * (Math.abs(n[4] - n[1]) + 1) * (Math.abs(n[5] - n[2]) + 1)} block(s)` : 'Incorrect argument for command';
            }
            case 'time': return a[0] === 'set' ? `Set the time to ${{ day: 1000, noon: 6000, night: 13000 }[a[1]] ?? a[1]}` : `The time is ${Math.round(w.playMin * 1200) % 24000}`;
            case 'weather': return `Set the weather to ${a[0] || 'clear'}`;
            case 'give': return `Gave ${a[2] || 1} [${(a[1] || '').replace('minecraft:', '').replace(/_/g, ' ').replace(/\b\w/g, (m) => m.toUpperCase())}] to ${a[0]}`;
            case 'tp': case 'teleport':
                if (a.length >= 4) w.pos = a.slice(1, 4).map(Number);
                return `Teleported ${a[0]} to ${a.slice(1).join(', ')}`;
            case 'gamerule':
                if (a[1] != null) {
                    w.rules[a[0]] = a[1] === 'true';
                    return `Gamerule ${a[0]} is now set to: ${a[1]}`;
                }
                return `Gamerule ${a[0]} is currently set to: ${w.rules[a[0]] ?? 'true'}`;
            case 'kill': return `Killed ${3 + (w.id.length % 5)} entities`;
            case 'seed': return `Seed: [${w.seed}]`;
            case 'difficulty': return a[0] ? `The difficulty has been set to ${a[0]}` : `The difficulty is ${w.diff}`;
            case 'help': return '原型只认：fill、give、gamerule、kill、seed、time、tp、weather、difficulty';
            default: return 'Unknown or incomplete command, see below for error';
        }
    }

    async function exec(cmds) {
        const w = curWorld();
        const i = cur();
        if (!w || !i) return;
        const W = wcOf(w);
        if (W.state !== 'off') return toast('上一条还在执行');
        if (api.RUN.get(i.id)?.phase === 'running') return toast('游戏正开着这个实例，先退出游戏');
        const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
        const push = (l) => { W.log.push(l); };
        if (!W.ready) {
            W.state = 'dl';
            push(`<span class="o">[${tnow()}] 第一次用：下载 ${esc2(i.mc)} 服务端（和开服用的是同一套）</span>`);
            for (let p = 0; p <= 100; p += 10) {
                W.pct = p;
                if (curWorld() === w && S.wtab === 'cmd') paintWc();
                await sleep(110);
            }
            W.ready = true;
        }
        W.state = 'boot';
        const srvMods = i.mods.filter((m) => m.on && !m.client).length;
        push(`<span class="o">[${tnow()}] 建快照：${esc2(w.name)} → snapshots\\${esc2(w.name)}-${tnow().replace(/:/g, '')}</span>`);
        push(`<span class="o">[${tnow()}] 起本地服务器：${esc2(i.mc)}${i.loader ? ` · ${esc2(i.loader)}，带 ${srvMods} 个服务端 Mod` : ''}（不开端口，只走 RCON）</span>`);
        paintWc();
        await sleep(650);
        push(`<span class="o">[Server] Preparing level "${esc2(w.name)}"</span>`);
        push(`<span class="o">[Server] Done (3.4s)!</span>`);
        W.state = 'on';
        paintWc();
        for (const c of cmds) {
            await sleep(380);
            push(`<span class="p">&gt;</span> ${esc2(c)}`);
            push(esc2(output(c, w)));
            paintWc();
        }
        await sleep(400);
        push(`<span class="o">[${tnow()}] 存档，关掉本地服务器</span>`);
        W.state = 'off';
        W.snaps.unshift({ at: tnow().slice(0, 5), cmds });
        W.snaps.splice(4);
        paintWc();
    }

    function startWorld(w) {
        meta(w);
        S.world = w.id;
        S.wtab = 'info';
        S.findType = 'all';
        S.map = { layer: 'surface', z: 3, cx: w.pos[0], cz: w.pos[2], hi: null };
        S.wdraft = draftOf(w);
        S.wc = wcOf(w);
    }

    function setTabW(t) {
        const w = curWorld();
        if (!w) return;
        S.wtab = t;
        if (t === 'edit') S.wdraft = draftOf(w);
        api.paintBody();
    }

    return {
        meta, structs, wdirty, after,
        reset: () => { S.world = null; },
        open: (w) => startWorld(w),
        wdChange,
        console: { exec, aiCmds },
        actions: {
            'world-open': (el) => {
                const w = cur()?.worlds.find((x) => x.id === el.dataset.v);
                if (!w) return;
                startWorld(w);
                api.paintBody();
            },
            'world-back': () => {
                S.world = null;
                api.paintBody();
            },
            wtab: (el) => setTabW(el.dataset.v),
            'find-type': (el) => {
                S.findType = el.dataset.v;
                api.paintBody();
            },
            'seed-copy': () => {
                const w = curWorld();
                navigator.clipboard?.writeText(w.seed).then(() => toast('种子复制好了'), () => toast('浏览器不让复制，选中后按 Ctrl C'));
            },
            'map-layer': (el) => {
                S.map.layer = el.dataset.v;
                const w = curWorld();
                const k = S.map.layer === 'nether' ? 8 : 1;
                S.map.cx = w.pos[0] / k;
                S.map.cz = w.pos[2] / k;
                api.paintBody();
            },
            'map-zoom': (el) => zoom(Number(el.dataset.v)),
            'map-home': () => {
                const w = curWorld();
                const k = S.map.layer === 'nether' ? 8 : 1;
                S.map.cx = w.spawn[0] / k;
                S.map.cz = w.spawn[2] / k;
                render();
            },
            'map-me': () => {
                const w = curWorld();
                const k = S.map.layer === 'nether' ? 8 : 1;
                S.map.cx = w.pos[0] / k;
                S.map.cz = w.pos[2] / k;
                render();
            },
            'map-goto': (el) => {
                const w = curWorld();
                const x = Number(el.dataset.x);
                const z = Number(el.dataset.z);
                const s = structs(w).find((o) => o.id === el.dataset.k);
                S.map = { ...S.map, layer: 'surface', cx: x, cz: z, z: Math.min(S.map.z, 2), hi: { x, z, label: s ? STYPES[s.type][0] : '死在这里' } };
                S.wtab = 'map';
                api.paintBody();
            },
            'wd-save': () => wdSave(),
            'wd-reset': () => {
                S.wdraft = draftOf(curWorld());
                api.paintBody();
            },
            'wc-enable': () => {
                S.console = true;
                api.store.set('console', true);
                api.paintBody();
                toast('指令台打开了。设置里可以再关上');
            },
            'wc-ask': () => {
                const w = curWorld();
                const inp = $('#wcAsk');
                const t = inp?.value.trim();
                if (!w || !t) return;
                const a = aiCmds(t, w);
                if (!a) return toast('原型里只认几句：造玻璃平台、改成白天、天晴、给我钻石、回出生点、死亡不掉落、清掉僵尸');
                W_ask(w, a);
                inp.value = '';
            },
            'wc-yes': () => {
                const W = wcOf(curWorld());
                if (!W.ask || W.ask.state !== 'wait') return;
                W.ask.state = 'yes';
                exec(W.ask.cmds);
            },
            'wc-no': () => {
                const W = wcOf(curWorld());
                if (!W.ask) return;
                W.ask.state = 'no';
                paintWc();
            },
            'wc-undo': (el) => {
                const w = curWorld();
                const W = wcOf(w);
                const s = W.snaps[Number(el.dataset.k)];
                if (!s || s.undone) return;
                s.undone = true;
                W.log.push(`<span class="o">[${tnow()}] 回滚到 ${s.at} 执行前的快照</span>`);
                paintWc();
                toast(`「${w.name}」回到了 ${s.at} 执行前的样子`);
            },
        },
    };

    function W_ask(w, a) {
        const W = wcOf(w);
        W.ask = a;
        const box = $('#wcConfirm');
        if (box) box.innerHTML = confirmCard(a);
        else paintWc();
        box?.querySelector('[data-act="wc-yes"]')?.focus({ preventScroll: true });
    }
}
