// 皮肤工作台：64×64 两层皮肤的展开图编辑、CSS 3D 预览、「描述 → 程序画」的 AI 生成、换上
import { $, toast, store, FACES } from './ui.js';
import { registerSkin } from './scene.js';
import { lvChip } from './views/agentView.js';
import { REAL, invoke } from './real.js';

// ---------- 皮肤的 UV 布局 ----------
export function parts(slim) {
    const a = slim ? 3 : 4;
    return [
        { k: 'head', name: '头', u: 0, v: 0, w: 8, h: 8, d: 8, ou: 32, ov: 0, x: 4, y: 0, on: '帽子层' },
        { k: 'body', name: '身体', u: 16, v: 16, w: 8, h: 12, d: 4, ou: 16, ov: 32, x: 4, y: 8, on: '外套' },
        { k: 'rarm', name: '右臂', u: 40, v: 16, w: a, h: 12, d: 4, ou: 40, ov: 32, x: 4 - a, y: 8, on: '右袖' },
        { k: 'larm', name: '左臂', u: 32, v: 48, w: a, h: 12, d: 4, ou: 48, ov: 48, x: 12, y: 8, on: '左袖' },
        { k: 'rleg', name: '右腿', u: 0, v: 16, w: 4, h: 12, d: 4, ou: 0, ov: 32, x: 4, y: 20, on: '右裤腿' },
        { k: 'lleg', name: '左腿', u: 16, v: 48, w: 4, h: 12, d: 4, ou: 0, ov: 48, x: 8, y: 20, on: '左裤腿' },
    ];
}
export const faces6 = (u, v, w, h, d) => ({
    top: [u + d, v, w, d], bottom: [u + d + w, v, w, d],
    right: [u, v + d, d, h], front: [u + d, v + d, w, h], left: [u + d + w, v + d, d, h], back: [u + d + w + d, v + d, w, h],
});
const faceOf = (p, layer) => faces6(layer ? p.ou : p.u, layer ? p.ov : p.v, p.w, p.h, p.d);

// 某个像素属于哪个部位、哪一层、哪个面
function locate(x, y, slim) {
    for (const p of parts(slim)) {
        for (const layer of [0, 1]) {
            for (const [f, r] of Object.entries(faceOf(p, layer))) {
                if (x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]) return { p, layer, f, r, fx: x - r[0], fy: y - r[1] };
            }
        }
    }
    return null;
}
const MIR = { head: 'head', body: 'body', rarm: 'larm', larm: 'rarm', rleg: 'lleg', lleg: 'rleg' };
const MIRF = { left: 'right', right: 'left' };
function mirrorOf(x, y, slim) {
    const L = locate(x, y, slim);
    if (!L) return null;
    const q = parts(slim).find((p) => p.k === MIR[L.p.k]);
    const f = MIRF[L.f] || L.f;
    const r = faceOf(q, L.layer)[f];
    return [r[0] + (r[2] - 1 - L.fx), r[1] + L.fy];
}

// ---------- 程序画：按一份「每个部位什么颜色、有什么配饰」的清单画出来 ----------
const shade = (hex, k) => {
    const n = parseInt(hex.slice(1), 16);
    const f = (c) => Math.max(0, Math.min(255, Math.round(c * k)));
    return `rgb(${f(n >> 16)} ${f((n >> 8) & 255)} ${f(n & 255)})`;
};
const dk = (hex, k) => '#' + [1, 3, 5].map((i) => Math.max(0, Math.min(255, Math.round(parseInt(hex.slice(i, i + 2), 16) * k))).toString(16).padStart(2, '0')).join('');
export const DEFAULT_SPEC = { skin: '#E6C3A5', hair: '#1F1D26', eyes: '#2E6B5E', top: '#6D4ED6', kind: 'hoodie', accent: '#EDEDED', pants: '#2C2B3F', shoes: '#1D1C26' };

// 返回一串「步骤」，生成时一步一步画，看得见在画哪
export function drawSteps(g, spec, slim) {
    const s = { ...DEFAULT_SPEC, ...spec };
    let seed = 7;
    const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
    const px = (x, y, c, j = 0.05) => {
        g.fillStyle = shade(c, 1 + (rnd() - 0.5) * j * 2);
        g.fillRect(x, y, 1, 1);
    };
    const P = Object.fromEntries(parts(slim).map((p) => [p.k, p]));
    const F = (k, layer = 0) => faceOf(P[k], layer);
    const rect = (r, c, rows = [0, r[3]], cols = [0, r[2]], j) => {
        for (let y = rows[0]; y < rows[1]; y++) for (let x = cols[0]; x < cols[1]; x++) px(r[0] + x, r[1] + y, c, j);
    };
    const allFaces = (k, c, rows, layer = 0) => Object.entries(F(k, layer)).forEach(([f, r]) => {
        if (f === 'top' || f === 'bottom') {
            if (!rows || rows[0] === 0) rect(r, c);
        } else rect(r, c, rows || undefined);
    });
    const front = (k, layer = 0) => F(k, layer).front;
    const hairDark = dk(s.hair, 0.8);
    const steps = [];

    steps.push(['头', () => {
        g.clearRect(0, 0, 64, 64);
        allFaces('head', s.skin, null);
        const f = front('head');
        px(f[0] + 1, f[1] + 4, '#FFFFFF', 0);
        px(f[0] + 2, f[1] + 4, s.eyes, 0);
        px(f[0] + 5, f[1] + 4, s.eyes, 0);
        px(f[0] + 6, f[1] + 4, '#FFFFFF', 0);
        rect(f, dk(s.skin, 0.88), [5, 6], [3, 5], 0);
        rect(f, '#9A5A4A', [6, 7], [3, 5], 0);
    }]);
    steps.push(['头发', () => {
        const fr = F('head');
        rect(fr.top, s.hair);
        rect(fr.front, s.hair, [0, 2]);
        px(fr.front[0], fr.front[1] + 2, s.hair);
        px(fr.front[0] + 7, fr.front[1] + 2, s.hair);
        if (!s.glasses) {
            rect(fr.front, hairDark, [3, 4], [1, 3], 0);
            rect(fr.front, hairDark, [3, 4], [5, 7], 0);
        }
        if (s.longHair) {
            rect(fr.front, s.hair, [2, 7], [0, 1]);
            rect(fr.front, s.hair, [2, 7], [7, 8]);
            rect(fr.right, s.hair);
            rect(fr.left, s.hair);
            rect(fr.back, s.hair);
        } else {
            rect(fr.right, s.hair, [0, 3]);
            rect(fr.right, s.hair, [3, 6], [0, 4]);
            rect(fr.left, s.hair, [0, 3]);
            rect(fr.left, s.hair, [3, 6], [4, 8]);
            rect(fr.back, s.hair, [0, 6]);
        }
    }]);
    steps.push(['上衣', () => {
        const k = s.kind;
        const b = F('body');
        if (k === 'coat' || k === 'chef') {
            allFaces('body', s.top, null);
            rect(b.front, s.accent, [0, 3], [3, 5], 0);
            rect(b.front, dk(s.top, 0.82), [3, 12], [4, 5], 0);
            if (k === 'chef') for (const y of [3, 5, 7]) { px(b.front[0] + 2, b.front[1] + y, s.accent, 0); px(b.front[0] + 5, b.front[1] + y, s.accent, 0); }
        } else if (k === 'suit') {
            allFaces('body', s.top, null);
            rect(b.front, '#F2F2F2', [0, 12], [3, 5], 0);
            rect(b.front, s.accent, [1, 8], [3, 5], 0);
            rect(b.front, s.pants, [10, 12]);
        } else if (k === 'dress') {
            allFaces('body', s.top, null);
        } else {
            allFaces('body', s.top, null);
            if (k === 'tee' || k === 'sweater') {
                rect(b.front, s.skin, [0, 1], [3, 5], 0);
                for (const f of ['front', 'back', 'right', 'left']) rect(b[f], s.pants, [10, 12]);
                rect(b.front, dk(s.pants, 0.6), [10, 11], [3, 5], 0);
            }
            if (k === 'hoodie') {
                rect(b.front, s.accent, [1, 5], [2, 3], 0);
                rect(b.front, s.accent, [1, 5], [5, 6], 0);
                rect(b.front, dk(s.top, 0.9), [7, 10], [1, 7]);
                rect(b.front, dk(s.top, 0.76), [7, 8], [1, 7], 0);
            }
            if (k === 'jacket') rect(b.front, dk(s.top, 0.7), [0, 12], [4, 5], 0);
        }
    }]);
    steps.push(['手臂', () => {
        const k = s.kind;
        for (const a of ['rarm', 'larm']) {
            const sleeve = k === 'tee' ? [0, 4] : k === 'dress' ? [0, 2] : [0, 11];
            allFaces(a, s.skin, null);
            allFaces(a, s.top, sleeve);
            if (k === 'suit') Object.entries(F(a)).forEach(([f, r]) => f !== 'top' && f !== 'bottom' && rect(r, '#F2F2F2', [10, 11], undefined, 0));
        }
    }]);
    steps.push(['裤子和鞋', () => {
        for (const l of ['rleg', 'lleg']) {
            allFaces(l, s.pants, null);
            if (s.kind === 'dress') {
                allFaces(l, s.top, [0, 6]);
                Object.entries(F(l)).forEach(([f, r]) => f !== 'top' && f !== 'bottom' && rect(r, s.skin, [6, 10]));
            }
            if (s.shorts) Object.entries(F(l)).forEach(([f, r]) => f !== 'top' && f !== 'bottom' && rect(r, s.skin, [5, 10]));
            Object.entries(F(l)).forEach(([f, r]) => (f === 'bottom' ? rect(r, s.shoes) : f !== 'top' && rect(r, s.shoes, [10, 12])));
        }
    }]);
    steps.push(['配饰', () => {
        const h = F('head', 1);
        if (s.glasses) {
            const f = h.front;
            const c = s.glasses;
            [[0, 3], [1, 3], [2, 3], [5, 3], [6, 3], [7, 3], [0, 4], [3, 4], [4, 4], [7, 4], [0, 5], [1, 5], [2, 5], [5, 5], [6, 5], [7, 5]].forEach(([x, y]) => px(f[0] + x, f[1] + y, c, 0));
            px(h.right[0] + 7, h.right[1] + 4, c, 0);
            px(h.right[0] + 6, h.right[1] + 4, c, 0);
            px(h.left[0], h.left[1] + 4, c, 0);
            px(h.left[0] + 1, h.left[1] + 4, c, 0);
        }
        if (s.hat) {
            const rows = s.hatKind === 'chef' ? [0, 3] : [0, 2];
            rect(h.top, s.hat);
            for (const f of ['front', 'back', 'right', 'left']) rect(h[f], s.hat, rows);
            if (s.hatKind === 'cap') rect(h.front, dk(s.hat, 0.8), [2, 3]);
        }
        if (s.phones) {
            rect(h.right, s.phones, [3, 6], [2, 5], 0);
            rect(h.left, s.phones, [3, 6], [3, 6], 0);
            rect(h.top, s.phones, [3, 5], [0, 8], 0);
        }
        const j = F('body', 1);
        if (s.scarf) {
            rect(j.top, s.scarf);
            for (const f of ['front', 'back', 'right', 'left']) rect(j[f], s.scarf, [0, 2]);
            rect(j.front, s.scarf, [2, 6], [5, 7]);
        }
        if (s.kind === 'hoodie') {
            rect(j.back, dk(s.top, 0.85), [0, 3]);
            rect(j.top, s.top);
        }
        if (s.kind === 'coat') for (const l of ['rleg', 'lleg']) for (const f of ['front', 'back', 'right', 'left']) rect(F(l, 1)[f], s.top, [0, 5]);
        if (s.mask) rect(h.front, s.mask, [5, 8]);
    }]);
    return steps;
}

// ---------- 把一句描述拆成清单（原型里用关键词模拟模型的拆解） ----------
const COLORS = [
    ['深蓝', '#22305A'], ['藏青', '#22305A'], ['天蓝', '#5FA8E0'], ['浅蓝', '#7FB2E5'], ['墨绿', '#1E4D3A'], ['深灰', '#3A3A42'], ['浅灰', '#B5B5BA'],
    ['咖啡', '#6B4A33'], ['卡其', '#B9A27A'], ['酒红', '#7A1F2B'], ['银', '#C8CCD2'], ['牛仔蓝', '#3B5A8C'],
    ['蓝', '#2E6FD8'], ['红', '#C0392B'], ['橙', '#E67E22'], ['黄', '#E9C12E'], ['金', '#D4A93A'], ['绿', '#2E8B57'], ['青', '#1AA59A'],
    ['紫', '#7D4ED6'], ['粉', '#F08DB6'], ['黑', '#26262C'], ['白', '#EDEDED'], ['灰', '#7A7A80'], ['棕', '#7A4E2D'], ['米', '#E3D5B8'],
];
const CALT = COLORS.map(([n]) => n).join('|');
const cname = (hex) => (COLORS.find(([, c]) => c === hex)?.[0] || '') + '色';
const cHex = (n) => COLORS.find(([k]) => k === n)?.[1];
function colorFor(text, items) {
    const m = text.match(new RegExp(`(${CALT})色?的?(?:短|长|宽松的|连帽)?(?:${items})`));
    return m ? cHex(m[1]) : null;
}

export function parseDesc(text) {
    const t = text || '';
    const s = { ...DEFAULT_SPEC, kind: 'tee', top: '#EDEDED', pants: '#3B5A8C', shoes: '#EDEDED', accent: '#EDEDED' };
    const said = [];
    // 先看职业，给一套底子
    if (/程序员|码农/.test(t)) Object.assign(s, { kind: 'hoodie', top: '#3E4A5C', glasses: '#1A1A1A' });
    if (/医生|大夫/.test(t)) Object.assign(s, { kind: 'coat', top: '#F4F4F4', accent: '#5FA8E0', pants: '#3A3A42', shoes: '#26262C' });
    if (/厨师/.test(t)) Object.assign(s, { kind: 'chef', top: '#F4F4F4', accent: '#26262C', hat: '#F4F4F4', hatKind: 'chef', pants: '#26262C', shoes: '#26262C' });
    if (/学生|校服/.test(t)) Object.assign(s, { kind: 'tee', top: '#F4F4F4', pants: '#22305A', shoes: '#EDEDED' });
    if (/忍者/.test(t)) Object.assign(s, { kind: 'jacket', top: '#1E1E24', accent: '#C0392B', pants: '#1E1E24', shoes: '#1E1E24', mask: '#1E1E24', scarf: '#C0392B' });
    if (/西装|西服|上班/.test(t)) Object.assign(s, { kind: 'suit', top: '#2B2E3A', accent: '#A8323E', pants: '#2B2E3A', shoes: '#1A1A1A' });
    // 衣服
    const tops = [['卫衣|连帽衫', 'hoodie'], ['T恤|短袖', 'tee'], ['毛衣', 'sweater'], ['衬衫', 'tee'], ['西装|西服', 'suit'], ['外套|夹克', 'jacket'], ['白大褂|大褂', 'coat'], ['裙子|连衣裙', 'dress']];
    for (const [w, kind] of tops) {
        if (new RegExp(w).test(t)) {
            s.kind = kind;
            s.top = colorFor(t, w) || (kind === 'coat' ? '#F4F4F4' : s.top);
        }
    }
    const pants = colorFor(t, '裤子|长裤|牛仔裤|短裤');
    if (pants) s.pants = pants;
    else if (/牛仔裤/.test(t)) s.pants = '#3B5A8C';
    if (/短裤/.test(t)) s.shorts = true;
    const shoes = colorFor(t, '鞋|运动鞋|靴子');
    if (shoes) s.shoes = shoes;
    else if (/靴子/.test(t)) s.shoes = '#5A3A24';
    // 头发
    const hm = t.match(new RegExp(`(${CALT})(?:色的?)?(?:长发|短发|头发|发)`));
    if (hm) s.hair = cHex(hm[1]);
    if (/长发/.test(t)) s.longHair = true;
    const em = colorFor(t, '眼睛|眼');
    if (em) s.eyes = em;
    if (/黑皮|深肤|小麦色/.test(t)) s.skin = '#A8754E';
    if (/白皙|白皮/.test(t)) s.skin = '#F1D3BD';
    // 配饰
    if (/眼镜/.test(t)) s.glasses = /金丝/.test(t) ? '#C9A13B' : /红框/.test(t) ? '#C0392B' : colorFor(t, '框眼镜|眼镜') || '#1A1A1A';
    if (/帽/.test(t) && !s.hat) {
        s.hat = colorFor(t, '帽子|鸭舌帽|棒球帽|毛线帽|贝雷帽|帽') || '#C0392B';
        s.hatKind = /鸭舌|棒球/.test(t) ? 'cap' : 'beanie';
    }
    if (/围巾/.test(t)) s.scarf = colorFor(t, '围巾') || '#C0392B';
    if (/耳机/.test(t)) s.phones = colorFor(t, '耳机') || '#26262C';
    const KN = { hoodie: '卫衣（带帽绳）', tee: 'T 恤', sweater: '毛衣', suit: '西装和领带', jacket: '外套', coat: '白大褂', chef: '厨师服', dress: '裙子' };
    said.push(`皮肤 ${s.skin === '#A8754E' ? '深肤色' : s.skin === '#F1D3BD' ? '白皙' : '浅肤色'}`);
    said.push(`头发 ${cname(s.hair) === '色' ? '黑色' : cname(s.hair)}${s.longHair ? '长发' : '短发'}`);
    if (s.glasses) said.push(`眼镜 ${s.glasses === '#C9A13B' ? '金丝' : s.glasses === '#1A1A1A' ? '黑框' : cname(s.glasses).replace(/色$/, '') + '框'}`);
    said.push(`上衣 ${cname(s.top)}${KN[s.kind]}`);
    said.push(`裤子 ${cname(s.pants)}${s.shorts ? '短裤' : ''}`);
    said.push(`鞋 ${cname(s.shoes)}`);
    if (s.hat) said.push(`帽子 ${cname(s.hat)}${s.hatKind === 'chef' ? '厨师帽' : s.hatKind === 'cap' ? '鸭舌帽' : '毛线帽'}`);
    if (s.scarf) said.push(`围巾 ${cname(s.scarf)}`);
    if (s.phones) said.push(`耳机 ${cname(s.phones)}`);
    if (s.mask) said.push('面罩');
    return { spec: s, said: said.map((x) => x.replace(/^(\S+) 色/, '$1 默认色')) };
}

// ---------- 3D 预览：每个部位一个盒子，六个面贴皮肤的对应区域 ----------
function boxHTML(p, layer) {
    const r = faceOf(p, layer);
    const { w, h, d } = p;
    const pxv = (n) => `calc(var(--px) * ${n})`;
    const f = (name, rc, left, top, tf) => `<i class="f f-${name}" style="width:${pxv(rc[2])};height:${pxv(rc[3])};left:${pxv(left)};top:${pxv(top)};background-position:${pxv(-rc[0])} ${pxv(-rc[1])};transform:${tf}"></i>`;
    const inf = layer ? (p.k === 'head' ? 1 : 0.5) : 0;
    const sc = layer ? `transform:scale3d(${(w + inf) / w},${(h + inf) / h},${(d + inf) / d})` : '';
    return `<span class="bx${layer ? ' outer' : ''}" style="width:${pxv(w)};height:${pxv(h)};${sc}">`
        + f('front', r.front, 0, 0, `translateZ(${pxv(d / 2)})`)
        + f('back', r.back, 0, 0, `rotateY(180deg) translateZ(${pxv(d / 2)})`)
        + f('right', r.right, (w - d) / 2, 0, `rotateY(-90deg) translateZ(${pxv(w / 2)})`)
        + f('left', r.left, (w - d) / 2, 0, `rotateY(90deg) translateZ(${pxv(w / 2)})`)
        + f('top', r.top, 0, (h - d) / 2, `rotateX(90deg) translateZ(${pxv(h / 2)})`)
        + f('bottom', r.bottom, 0, (h - d) / 2, `rotateX(-90deg) translateZ(${pxv(h / 2)})`)
        + '</span>';
}
export function rigHTML(slim) {
    return `<div class="rig" id="skinRig">${parts(slim).map((p) => `<span class="pt pt-${p.k}" style="left:calc(var(--px) * ${p.x});top:calc(var(--px) * ${p.y});width:calc(var(--px) * ${p.w});height:calc(var(--px) * ${p.h})">${boxHTML(p, 0)}${boxHTML(p, 1)}</span>`).join('')}</div>`;
}

// ---------- 控制器 ----------
export function createSkin(api) {
    const { S } = api;
    const K = { tool: 'brush', color: '#6D4ED6', layer: 0, part: 'all', mirror: true, slim: false, outer: true, pose: 'stand', ry: -28, rx: -10, route: 'draw', gen: null, busy: false, undo: [], hist: [], uploaded: 0, refs: [] };
    S.skin = K;
    const cv = document.createElement('canvas');
    cv.width = cv.height = 64;
    const g = cv.getContext('2d', { willReadFrequently: true });

    function loadDefault() {
        drawSteps(g, DEFAULT_SPEC, false).forEach(([, f]) => f());
    }
    // 画默认皮肤要用画布：冷启动时第一次用画布要等约 0.1 秒，不挡第一屏，空下来再画；
    // 用到皮肤的地方（皮肤页、动作、取图）先 ensure()
    let drawn = false;
    function ensure() {
        if (drawn) return;
        drawn = true;
        const saved = store.get('skin', null);
        if (saved) {
            const im = new Image();
            im.onload = () => {
                g.clearRect(0, 0, 64, 64);
                g.drawImage(im, 0, 0);
                preview();
            };
            im.src = saved;
        }
        loadDefault();
    }
    (window.requestIdleCallback || ((f) => setTimeout(f, 200)))(ensure, { timeout: 3000 });

    const url = () => (ensure(), cv.toDataURL());
    let raf = 0;
    function preview() {
        if (raf) return;
        raf = requestAnimationFrame(() => {
            raf = 0;
            const u = url();
            const st = $('#skinStage');
            if (st) st.style.setProperty('--skin', `url(${u})`);
            const ed = $('#skinCv');
            if (ed) {
                const eg = ed.getContext('2d');
                eg.clearRect(0, 0, 64, 64);
                eg.drawImage(cv, 0, 0);
            }
            store.set('skin', u);
        });
    }

    function paintMask() {
        const m = $('#skinMask');
        if (!m) return;
        const mg = m.getContext('2d');
        mg.clearRect(0, 0, 64, 64);
        mg.fillStyle = 'rgba(20,18,14,.55)';
        mg.fillRect(0, 0, 64, 64);
        for (const p of parts(K.slim)) {
            if (K.part !== 'all' && !(K.part === 'arm' ? p.k.endsWith('arm') : K.part === 'leg' ? p.k.endsWith('leg') : p.k === K.part)) continue;
            for (const r of Object.values(faceOf(p, K.layer))) mg.clearRect(r[0], r[1], r[2], r[3]);
        }
    }

    const editable = (x, y) => {
        const L = locate(x, y, K.slim);
        if (!L || L.layer !== K.layer) return null;
        if (K.part !== 'all' && !(K.part === 'arm' ? L.p.k.endsWith('arm') : K.part === 'leg' ? L.p.k.endsWith('leg') : L.p.k === K.part)) return null;
        return L;
    };
    const hexAt = (x, y) => {
        const d = g.getImageData(x, y, 1, 1).data;
        return d[3] ? '#' + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, '0')).join('') : null;
    };
    function put(x, y) {
        if (!editable(x, y)) return;
        const set = (a, b) => {
            if (K.tool === 'erase') g.clearRect(a, b, 1, 1);
            else {
                g.fillStyle = K.color;
                g.fillRect(a, b, 1, 1);
            }
        };
        set(x, y);
        if (K.mirror) {
            const m = mirrorOf(x, y, K.slim);
            if (m) set(m[0], m[1]);
        }
    }
    function fill(x, y) {
        const L = editable(x, y);
        if (!L) return;
        const [rx, ry, rw, rh] = L.r;
        const img = g.getImageData(rx, ry, rw, rh);
        const at = (a, b) => ((b - ry) * rw + (a - rx)) * 4;
        const o = at(x, y);
        const tgt = [img.data[o], img.data[o + 1], img.data[o + 2], img.data[o + 3]];
        const same = (k) => img.data[k] === tgt[0] && img.data[k + 1] === tgt[1] && img.data[k + 2] === tgt[2] && img.data[k + 3] === tgt[3];
        const n = parseInt(K.color.slice(1), 16);
        const col = [n >> 16, (n >> 8) & 255, n & 255, 255];
        if (col.every((v, k) => v === tgt[k])) return;
        const q = [[x, y]];
        const pts = [];
        while (q.length) {
            const [a, b] = q.pop();
            if (a < rx || b < ry || a >= rx + rw || b >= ry + rh) continue;
            const k = at(a, b);
            if (!same(k)) continue;
            col.forEach((v, j) => { img.data[k + j] = v; });
            pts.push([a, b]);
            q.push([a + 1, b], [a - 1, b], [a, b + 1], [a, b - 1]);
        }
        g.putImageData(img, rx, ry);
        if (K.mirror) {
            g.fillStyle = K.color;
            pts.forEach(([a, b]) => {
                const m = mirrorOf(a, b, K.slim);
                if (m) g.fillRect(m[0], m[1], 1, 1);
            });
        }
    }
    const snap = () => {
        K.undo.push(g.getImageData(0, 0, 64, 64));
        if (K.undo.length > 40) K.undo.shift();
    };

    function bindEditor() {
        const ed = $('#skinCv');
        if (!ed || ed.dataset.bound) return;
        ed.dataset.bound = '1';
        let down = false;
        let last = null;
        const pos = (e) => {
            const r = ed.getBoundingClientRect();
            return [Math.floor(((e.clientX - r.left) / r.width) * 64), Math.floor(((e.clientY - r.top) / r.height) * 64)];
        };
        const hover = $('#skinHover');
        ed.addEventListener('pointerdown', (e) => {
            const [x, y] = pos(e);
            if (K.tool === 'pick') {
                const c = hexAt(x, y);
                if (c) {
                    K.color = c;
                    K.tool = 'brush';
                    api.paintView();
                    toast(`取到 ${c}，换回了画笔`);
                }
                return;
            }
            if (K.tool === 'erase' && K.layer === 0) return toast('身体层不能透明（游戏里会显示成黑色）。换到「外套层」再擦');
            snap();
            if (K.tool === 'fill') {
                fill(x, y);
                preview();
                return;
            }
            down = true;
            last = [x, y];
            try { ed.setPointerCapture(e.pointerId); } catch {}
            put(x, y);
            preview();
        });
        ed.addEventListener('pointermove', (e) => {
            const [x, y] = pos(e);
            const L = locate(x, y, K.slim);
            if (hover) hover.textContent = L ? `${L.p.name} · ${L.layer ? L.p.on : '身体层'} · ${{ front: '正面', back: '背面', left: '左侧', right: '右侧', top: '顶面', bottom: '底面' }[L.f]} · ${x}, ${y}` : `${x}, ${y} · 没用到的格子`;
            if (!down) return;
            // 连线，快速拖动时不断笔
            let [x0, y0] = last;
            const dx = Math.abs(x - x0);
            const dy = -Math.abs(y - y0);
            const sx = x0 < x ? 1 : -1;
            const sy = y0 < y ? 1 : -1;
            let err = dx + dy;
            for (let n = 0; n < 200; n++) {
                put(x0, y0);
                if (x0 === x && y0 === y) break;
                const e2 = 2 * err;
                if (e2 >= dy) { err += dy; x0 += sx; }
                if (e2 <= dx) { err += dx; y0 += sy; }
            }
            last = [x, y];
            preview();
        });
        const end = () => { down = false; };
        ed.addEventListener('pointerup', end);
        ed.addEventListener('pointercancel', end);
        ed.addEventListener('pointerleave', () => { if (hover) hover.textContent = '指到格子上看是哪个部位'; });

        // 3D 预览：拖着转
        const st = $('#skinStage');
        let rot = null;
        st?.addEventListener('pointerdown', (e) => {
            rot = { x: e.clientX, y: e.clientY, ry: K.ry, rx: K.rx };
            try { st.setPointerCapture(e.pointerId); } catch {}
            st.classList.add('drag');
        });
        st?.addEventListener('pointermove', (e) => {
            if (!rot) return;
            K.ry = rot.ry + (e.clientX - rot.x) * 0.6;
            K.rx = Math.max(-40, Math.min(40, rot.rx - (e.clientY - rot.y) * 0.4));
            turn();
        });
        const stop = () => { rot = null; st?.classList.remove('drag'); };
        st?.addEventListener('pointerup', stop);
        st?.addEventListener('pointercancel', stop);
        $('#skinFile')?.addEventListener('change', (e) => importPng(e.target.files?.[0]));
        $('#skRefFile')?.addEventListener('change', (e) => addRefs(e.target.files || []));
        // 参考图直接拖进「AI 生成」那张卡片
        const card = $('.sk-ai');
        card?.addEventListener('dragover', (e) => {
            if (![...(e.dataTransfer?.items || [])].some((x) => x.kind === 'file')) return;
            e.preventDefault();
            card.classList.add('drop');
        });
        card?.addEventListener('dragleave', (e) => { if (!card.contains(e.relatedTarget)) card.classList.remove('drop'); });
        card?.addEventListener('drop', (e) => {
            e.preventDefault();
            card.classList.remove('drop');
            addRefs(e.dataTransfer?.files || []);
        });
    }
    const turn = () => $('#skinTurn')?.style.setProperty('transform', `rotateX(${K.rx}deg) rotateY(${K.ry}deg)`);

    function after() {
        if (S.page !== 'skin') return;
        ensure();
        if (REAL) loadProfile();
        bindEditor();
        paintMask();
        turn();
        const ed = $('#skinCv');
        if (ed) {
            const eg = ed.getContext('2d');
            eg.clearRect(0, 0, 64, 64);
            eg.drawImage(cv, 0, 0);
        }
        $('#skinStage')?.style.setProperty('--skin', `url(${url()})`);
    }

    function thumb(src = cv) {
        const t = document.createElement('canvas');
        t.width = 16;
        t.height = 32;
        const tg = t.getContext('2d');
        for (const p of parts(K.slim)) for (const layer of [0, 1]) {
            const r = faceOf(p, layer).front;
            tg.drawImage(src, r[0], r[1], r[2], r[3], p.x, p.y, r[2], r[3]);
        }
        return t.toDataURL();
    }
    function remember(label) {
        K.hist.unshift({ label, at: new Date().toTimeString().slice(0, 5), url: thumb(), data: g.getImageData(0, 0, 64, 64) });
        K.hist.splice(REAL ? 20 : 6);
        if (REAL) invoke('skin_history_save', { png: url(), label, slim: K.slim }).catch(() => {});
    }

    // ---------- 真程序：正版档案、历史版本存在本机 ----------
    const isMsa = () => REAL && api.acct()?.kind === 'msa';
    function loadPng(src) {
        return new Promise((res, rej) => {
            const im = new Image();
            im.onload = () => {
                const t = document.createElement('canvas');
                t.width = t.height = 64;
                const tg = t.getContext('2d', { willReadFrequently: true });
                tg.drawImage(im, 0, 0);
                res({ canvas: t, data: tg.getImageData(0, 0, 64, 64) });
            };
            im.onerror = () => rej(new Error('图片读不了'));
            im.src = src;
        });
    }
    if (REAL) {
        invoke('skin_history').then(async (list) => {
            for (const h of list) {
                try {
                    const { canvas, data } = await loadPng(h.png);
                    const d = new Date(h.at);
                    K.hist.push({ label: h.label, at: `${d.getMonth() + 1}-${d.getDate()} ${d.toTimeString().slice(0, 5)}`, url: thumb(canvas), data });
                } catch {}
            }
            if (S.page === 'skin') api.paintView();
        }).catch(() => {});
    }
    let profAcct = null;
    function loadProfile(force) {
        if (!isMsa()) {
            K.profile = null;
            profAcct = null;
            return;
        }
        const a = api.acct();
        if (!force && profAcct === a.id) return;
        profAcct = a.id;
        K.profile = { loading: true };
        invoke('skin_profile', { account: a.id }).then((p) => {
            K.profile = p;
        }, (e) => {
            K.profile = { error: String(e) };
        }).finally(() => S.page === 'skin' && api.paintView());
    }

    function importPng(file) {
        if (!file) return;
        const rd = new FileReader();
        rd.onload = () => {
            const im = new Image();
            im.onload = () => {
                if (im.width !== 64 || (im.height !== 64 && im.height !== 32)) return toast('要 64×64（或老式 64×32）的 PNG 皮肤');
                remember('导入前');
                snap();
                g.clearRect(0, 0, 64, 64);
                g.drawImage(im, 0, 0);
                if (im.height === 32) {
                    // 老式皮肤没有左手左腿：照右边镜像一份
                    for (let y = 16; y < 32; y++) for (let x = 0; x < 16; x++) {
                        const m = mirrorOf(x, y, false);
                        const c = hexAt(x, y);
                        if (m && c) { g.fillStyle = c; g.fillRect(m[0], m[1], 1, 1); }
                        const m2 = mirrorOf(x + 40, y, false);
                        const c2 = hexAt(x + 40, y);
                        if (m2 && c2) { g.fillStyle = c2; g.fillRect(m2[0], m2[1], 1, 1); }
                    }
                }
                preview();
                api.paintView();
                toast(`导入了 ${file.name}`);
            };
            im.src = rd.result;
        };
        rd.readAsDataURL(file);
    }

    // ---------- AI 生成的参考图（可以多张：同一个角色的正面、背面、细节） ----------
    const MAX_REFS = 6;
    function shrink(file) {
        return new Promise((res, rej) => {
            const rd = new FileReader();
            rd.onerror = () => rej(new Error('读不了'));
            rd.onload = () => {
                const im = new Image();
                im.onerror = () => rej(new Error('读不了'));
                im.onload = () => {
                    const k = Math.min(1, 1024 / Math.max(im.width, im.height));
                    const w = Math.max(1, Math.round(im.width * k));
                    const h = Math.max(1, Math.round(im.height * k));
                    const c = document.createElement('canvas');
                    c.width = w;
                    c.height = h;
                    const cg = c.getContext('2d');
                    // 小图（像素画、皮肤截图）留 PNG 不糊；大图转 JPG，透明的地方垫白
                    const png = file.type === 'image/png' && w * h <= 512 * 512;
                    cg.imageSmoothingEnabled = k < 1;
                    if (!png) {
                        cg.fillStyle = '#fff';
                        cg.fillRect(0, 0, w, h);
                    }
                    cg.drawImage(im, 0, 0, w, h);
                    res(c.toDataURL(png ? 'image/png' : 'image/jpeg', 0.88));
                };
                im.src = rd.result;
            };
            rd.readAsDataURL(file);
        });
    }
    async function addRefs(files) {
        const list = [...files].filter((f) => /^image\/(png|jpeg|webp|gif)$/.test(f.type));
        if (!list.length) return toast('参考图要 PNG、JPG 或 WebP');
        let n = 0;
        for (const f of list) {
            if (K.refs.length >= MAX_REFS) {
                toast(`参考图最多 ${MAX_REFS} 张`);
                break;
            }
            try {
                K.refs.push({ name: f.name || '粘贴的图', url: await shrink(f) });
                n++;
            } catch {
                toast(`「${f.name}」读不了`);
            }
        }
        if (n && S.page === 'skin') api.paintView();
    }
    // 截图、网页上复制的图，直接 Ctrl V
    document.addEventListener('paste', (e) => {
        if (S.page !== 'skin') return;
        const files = [...(e.clipboardData?.files || [])].filter((f) => f.type.startsWith('image/'));
        if (!files.length) return;
        e.preventDefault();
        addRefs(files);
    });

    async function generate() {
        const ta = $('#skinDesc');
        const text = ta?.value.trim();
        if (K.busy) return;
        if (K.route !== 'draw') {
            const img = api.aiFor('img');
            return toast(img ? `原型里只演示「描述 → 程序画」。正式版会用「${img.name}」画立绘再转成皮肤` : '还没配生图源（设置 → AI 源里加一个 ComfyUI 或云端生图）。原型里只演示「描述 → 程序画」');
        }
        const refs = K.refs.map((r) => r.url);
        if (!text && !refs.length) {
            ta?.focus();
            return toast('先写一句想要什么样的，或者加一张参考图');
        }
        const ai = REAL && api.aiReady();
        if (refs.length && !ai) {
            if (!text) return toast(REAL ? '看参考图要先在设置里加一个能看图的 AI 源' : '原型里不看参考图，写一句描述试试');
            toast(REAL ? '还没有 AI 源，参考图先不看，按文字拆' : '原型里不看参考图，只按文字拆');
        }
        K.busy = true;
        remember('AI 生成前');
        snap();
        let spec, said;
        if (ai) {
            K.gen = { text, said: [refs.length ? `正在看 ${refs.length} 张参考图…` : '正在问 AI…'], step: '' };
            api.paintView();
            try {
                ({ spec, said } = await invoke('skin_describe', { text, images: refs }));
            } catch (e) {
                if (!text) {
                    K.busy = false;
                    K.gen = null;
                    K.undo.pop();
                    api.paintView();
                    return toast(`AI 没看懂参考图：${e}`, { ms: 8000 });
                }
                toast(`AI 没拆出来（${e}），先用关键词拆`);
                ({ spec, said } = parseDesc(text));
            }
        } else {
            if (REAL && !refs.length) toast('还没有 AI 源，先用关键词拆；到设置里加一个 AI 源效果更好');
            ({ spec, said } = parseDesc(text));
        }
        K.gen = { text, said: [], step: '' };
        api.paintView();
        const wait = (ms) => new Promise((r) => setTimeout(r, ms));
        await wait(500);
        for (const x of said) {
            K.gen.said.push(x);
            paintGen();
            await wait(110);
        }
        for (const [name, f] of drawSteps(g, spec, K.slim)) {
            K.gen.step = name;
            paintGen();
            f();
            preview();
            await wait(260);
        }
        K.gen.step = '';
        K.busy = false;
        api.paintView();
        toast('画好了。可以接着在左边改，满意了点「换上」');
    }
    function paintGen() {
        const el = $('#skGen');
        if (!el || !K.gen) return;
        el.innerHTML = genHTML(K);
    }

    async function upload() {
        const a = api.acct();
        closeUp();
        if (REAL) {
            try {
                remember('换上前');
                K.profile = await invoke('skin_upload', { account: a.id, png: url(), slim: K.slim });
                applyFace(a);
                api.paintView();
                toast('换好了。进任何服务器，别人看到的都是这个皮肤');
            } catch (e) {
                toast(String(e), { ms: 8000 });
            }
            return;
        }
        const t = api.addTask(`换皮肤 · ${a.name}`, '上传到 Minecraft 正版档案');
        api.runTask(t, 1200, () => {
            applyFace(a);
            remember('换上的版本');
            if (S.page === 'skin') api.paintView();
            toast('换好了。进任何服务器，别人看到的都是这个皮肤');
        });
    }
    function applyFace(a) {
        K.uploaded += 1;
        const key = `skin-${a.id}-${K.uploaded}`;
        // 头像：脸 + 帽子层
        const rows = [];
        const pal = {};
        const chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+-=[]{};:,.<>?/|~';
        let n = 0;
        for (let y = 0; y < 8; y++) {
            let row = '';
            for (let x = 0; x < 8; x++) {
                const c = hexAt(40 + x, 8 + y) || hexAt(8 + x, 8 + y) || '#000000';
                let ch = Object.keys(pal).find((k) => pal[k] === c);
                if (!ch) {
                    ch = chars[n++ % chars.length];
                    pal[ch] = c;
                }
                row += ch;
            }
            rows.push(row);
        }
        pal.S = hexAt(44, 28) || '#E6C3A5';
        FACES[key] = { rows, pal };
        registerSkin(key, g.getImageData(0, 0, 64, 64), K.slim);
        a.face = key;
        api.paintAcct();
        api.paintScenes?.();
    }
    const closeUp = () => api.closePop(false);

    const api2 = {
        K, after,
        undo: () => {
            if (S.page !== 'skin' || !K.undo.length) return false;
            g.putImageData(K.undo.pop(), 0, 0);
            preview();
            return true;
        },
        actions: {
            skin: () => {
                api.closePop(false);
                api.goPage('skin');
            },
            'skin-back': () => api.goPage('home'),
            'sk-tool': (el) => {
                K.tool = el.dataset.v;
                api.paintView();
            },
            'sk-layer': (el) => {
                K.layer = Number(el.dataset.v);
                if (K.layer === 0 && K.tool === 'erase') K.tool = 'brush';
                api.paintView();
            },
            'sk-part': (el) => {
                K.part = el.dataset.v;
                api.paintView();
            },
            'sk-sw': (el) => {
                K.color = el.dataset.v;
                if (K.tool === 'erase' || K.tool === 'pick') K.tool = 'brush';
                api.paintView();
            },
            'sk-undo': () => {
                if (!K.undo.length) return toast('没有可以撤销的了');
                g.putImageData(K.undo.pop(), 0, 0);
                preview();
            },
            'sk-slim': (el) => {
                const slim = el.dataset.v === '1';
                if (slim === K.slim) return;
                K.slim = slim;
                api.paintView();
                toast(slim ? 'Alex 体型：手臂 3 像素宽。右边 1 列像素不再显示' : 'Steve 体型：手臂 4 像素宽');
            },
            'sk-pose': (el) => {
                K.pose = el.dataset.v;
                api.paintView();
            },
            'sk-face': (el) => {
                K.ry = el.dataset.v === 'back' ? 180 - 28 : -28;
                K.rx = -10;
                turn();
            },
            'sk-route': (el) => {
                K.route = el.dataset.v;
                api.paintView();
            },
            'sk-ex': (el) => {
                const ta = $('#skinDesc');
                if (ta) ta.value = el.dataset.v;
                generate();
            },
            'sk-gen': () => generate(),
            'sk-ref-del': (el) => {
                K.refs.splice(Number(el.dataset.k), 1);
                api.paintView();
            },
            'sk-hist': (el) => {
                const h = K.hist[Number(el.dataset.k)];
                if (!h) return;
                snap();
                g.putImageData(h.data, 0, 0);
                preview();
                toast(`退回到「${h.label}」（${h.at}）。按撤销能回来`);
            },
            'skin-import': async () => {
                if (REAL) {
                    try {
                        const r = await invoke('skin_fetch', { account: api.acct().id });
                        const { data } = await loadPng(r.png);
                        remember('导入前');
                        snap();
                        g.putImageData(data, 0, 0);
                        K.slim = !!r.slim;
                        preview();
                        api.paintView();
                        toast('拿到了正版档案上现在的皮肤');
                    } catch (e) {
                        toast(String(e));
                    }
                    return;
                }
                remember('导入前');
                snap();
                loadDefault();
                preview();
                toast('原型里用示例皮肤代替你正版档案上的皮肤');
            },
            'sk-cape': async (el) => {
                try {
                    K.profile = await invoke('skin_cape', { account: api.acct().id, cape: el.dataset.v || null });
                    api.paintView();
                    toast(el.dataset.v ? '换好了披风' : '收起了披风');
                } catch (e) {
                    toast(String(e));
                }
            },
            'skin-up': (el) => {
                const a = api.acct();
                if (REAL ? a.kind === 'msa' : a.id === 'msa') {
                    api.openPop(el, `<div class="pop-h">${lvChip('ext')} 换上这个皮肤？</div><p class="pop-note big">会上传到 ${a.name} 的正版档案。之后进任何服务器，别人看到的都是它。能随时换回去（「历史版本」里有原来的）。</p><div class="pop-act"><button type="button" class="btn sm pri" data-act="skin-up-yes">上传并换上</button><button type="button" class="btn sm ghost" data-act="pop-close">先不</button></div>`, 'below');
                } else {
                    api.openPop(el, `<div class="pop-h">离线账户的皮肤只有你看得到</div><p class="pop-note big">离线账户没有正版档案可以上传。想让别人也看到，可以：</p><ul class="pop-list"><li>用外置登录（比如 LittleSkin 这类皮肤站），把皮肤传到那边</li><li>或者大家都装 CustomSkinLoader 这类 Mod</li></ul><div class="pop-act"><button type="button" class="btn sm pri" data-act="skin-local">只在本机用这个皮肤</button></div>`, 'below');
                }
            },
            'skin-up-yes': () => upload(),
            'skin-local': () => {
                closeUp();
                applyFace(api.acct());
                remember('本机用的版本');
                api.paintView();
                toast('换好了：本机的头像和风景里的小人都是这个皮肤');
            },
        },
        change(t) {
            if (t.id === 'skColor') {
                K.color = t.value;
                if (K.tool !== 'fill') K.tool = 'brush';
                api.paintView();
                return true;
            }
            if (t.id === 'skMirror') {
                K.mirror = t.checked;
                return true;
            }
            if (t.id === 'skOuter') {
                K.outer = t.checked;
                $('#skinRig')?.classList.toggle('no-outer', !K.outer);
                return true;
            }
            return false;
        },
    };
    // 动作、撤销、输入框变化都先把皮肤画好（ensure 只画一次）
    for (const [k, f] of Object.entries(api2.actions)) api2.actions[k] = (...a) => (ensure(), f(...a));
    for (const k of ['undo', 'change']) {
        const f = api2[k];
        api2[k] = (...a) => (S.page === 'skin' && ensure(), f(...a));
    }
    return api2;
}

export function genHTML(K) {
    const g = K.gen;
    if (!g) return '';
    return `<div class="sk-said">${g.said.map((x) => `<span class="chip acc in">${x}</span>`).join('')}</div>
        ${g.step ? `<p class="note">${'正在画：' + g.step}…</p>` : g.said.length ? '<p class="note">模型拆出了这份清单，程序按皮肤的 UV 布局画出来。原型里用关键词模拟模型的拆解。</p>' : '<p class="note">正在拆…</p>'}`;
}
