// 实例头图：按实例画一幅像素风景（天空、远山、地形、树、矿、站在地上的玩家）。
// 种子固定，同一个实例每次画出来都一样；云单独一层，慢慢飘。
import { FACES } from './ui.js';

const BIOMES = {
    grass: { sky: ['#5FAEE6', '#C4E6F7'], sun: '#FFF4B8', far: '#93C2A6', top: '#69AA4B', dirt: '#8B5E3C', stone: '#7D7D7D', leaf: '#3E7F2E', trunk: '#6B4A2B', trees: 0.14 },
    brass: { sky: ['#E57E55', '#F9D38E'], sun: '#FFE6A0', far: '#C98F6A', top: '#B39A5C', dirt: '#7A5B3D', stone: '#6B665C', leaf: '#5C7A35', trunk: '#5A3F27', trees: 0.05, chimney: '#8E7A5A' },
    stone: { sky: ['#86ABD6', '#E3ECF5'], sun: '#FFFFFF', far: '#B7C6D6', top: '#F1F5F8', dirt: '#8C8C8C', stone: '#6F6F6F', leaf: '#2F5F3A', trunk: '#4E3A26', trees: 0.05, peaks: true },
    cobble: { sky: ['#3F4A8A', '#E89A70'], sun: '#FFD27A', far: '#6B5B86', top: '#5E8F43', dirt: '#6E4D33', stone: '#5E5E5E', leaf: '#2E5E28', trunk: '#4A3320', trees: 0.1 },
    nether: { sky: ['#260707', '#6E1C14'], far: '#4A1210', top: '#8E3434', dirt: '#6E2A2A', stone: '#4A1A1A', lava: '#F26B1D', trees: 0, embers: true },
    diamond: { sky: ['#0E1A33', '#2E4A78'], sun: '#E8EEF7', moon: true, stars: true, far: '#24375A', top: '#4E8F5E', dirt: '#4A3A2A', stone: '#4A4F5A', leaf: '#28533A', trunk: '#3C2C1E', trees: 0.08, ore: '#6FE0D6' },
    copper: { sky: ['#F4A66C', '#FDE8CF'], sun: '#FFF1C2', far: '#E2B48F', top: '#79B060', dirt: '#8E6040', stone: '#7A7A7A', leaf: '#4E8B3A', trunk: '#6A4A2C', trees: 0.12 },
};

// 衣服颜色跟着账户走：换账户，站在地上的人也换
const OUTFIT = {
    pingyi: { shirt: '#6D4ED6', pants: '#2C2B3F', shoe: '#1D1C26' },
    steve: { shirt: '#3BA7A1', pants: '#3B3F8F', shoe: '#5A5A5A' },
};

export function seedOf(str) {
    let h = 2166136261;
    for (const ch of String(str)) h = Math.imul(h ^ ch.charCodeAt(0), 16777619);
    return h >>> 0;
}

function rng(seed) {
    let s = seed || 1;
    return () => {
        s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
        return s / 4294967296;
    };
}

function setup(canvas) {
    const r = canvas.getBoundingClientRect();
    const W = Math.max(1, Math.round(r.width));
    const H = Math.max(1, Math.round(r.height));
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    const g = canvas.getContext('2d');
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.imageSmoothingEnabled = false;
    return { g, W, H };
}

// 一个方块：底色 + 几粒明暗像素，有点 Minecraft 材质的意思
function blockAt(g, x, y, s, color, rand) {
    g.fillStyle = color;
    g.fillRect(x, y, s, s);
    const p = Math.max(2, Math.round(s / 5));
    const n = Math.max(1, Math.floor(s / p));
    for (let i = 0; i < 4; i++) {
        g.fillStyle = rand() < 0.5 ? 'rgba(0,0,0,.11)' : 'rgba(255,255,255,.09)';
        g.fillRect(x + Math.floor(rand() * n) * p, y + Math.floor(rand() * n) * p, p, p);
    }
}

function figure(g, cx, ground, face, px) {
    const f = FACES[face] || FACES.steve;
    const o = OUTFIT[face] || OUTFIT.steve;
    const skin = f.pal.S;
    const left = Math.round(cx - 8 * px);
    const top = Math.round(ground - 32 * px);
    const rect = (x, y, w, h, c) => {
        g.fillStyle = c;
        g.fillRect(left + x * px, top + y * px, w * px, h * px);
    };
    f.rows.forEach((row, y) => [...row].forEach((ch, x) => rect(4 + x, y, 1, 1, f.pal[ch])));
    rect(4, 8, 8, 12, o.shirt);
    rect(0, 8, 4, 4, o.shirt);
    rect(12, 8, 4, 4, o.shirt);
    rect(0, 12, 4, 8, skin);
    rect(12, 12, 4, 8, skin);
    rect(4, 20, 8, 10, o.pants);
    rect(4, 30, 8, 2, o.shoe);
    g.fillStyle = 'rgba(0,0,0,.14)';
    g.fillRect(left + 8 * px, top + 8 * px, 4 * px, 24 * px);
    g.fillRect(left + 12 * px, top + 8 * px, 4 * px, 12 * px);
}

export function paintScene(canvas, kind, seedStr, { block = 12, face = null } = {}) {
    const B = BIOMES[kind] || BIOMES.grass;
    const { g, W, H } = setup(canvas);
    const rand = rng(seedOf(seedStr));
    const sky = g.createLinearGradient(0, 0, 0, H);
    sky.addColorStop(0, B.sky[0]);
    sky.addColorStop(1, B.sky[1]);
    g.fillStyle = sky;
    g.fillRect(0, 0, W, H);
    if (B.stars) {
        for (let i = 0; i < (W * H) / 700; i++) {
            g.fillStyle = `rgba(255,255,255,${0.35 + rand() * 0.6})`;
            const s = rand() < 0.15 ? 3 : 2;
            g.fillRect(Math.floor(rand() * W), Math.floor(rand() * H * 0.55), s, s);
        }
    }
    if (B.sun) {
        const s = Math.round(block * 2.6);
        const sx = Math.round(W * (0.1 + rand() * 0.18));
        const sy = Math.round(H * 0.16);
        g.globalAlpha = 0.22;
        g.fillStyle = B.sun;
        g.fillRect(sx - s * 0.45, sy - s * 0.45, s * 1.9, s * 1.9);
        g.globalAlpha = 1;
        g.fillRect(sx, sy, s, s);
        if (B.moon) {
            g.fillStyle = 'rgba(30,40,70,.18)';
            g.fillRect(sx + s * 0.2, sy + s * 0.25, s * 0.25, s * 0.25);
            g.fillRect(sx + s * 0.55, sy + s * 0.55, s * 0.2, s * 0.2);
        }
    }
    const cols = Math.ceil(W / block) + 1;
    const rows = Math.ceil(H / block);
    // 远山：一层淡一点的剪影
    const p1 = rand() * 10;
    const p2 = rand() * 10;
    g.fillStyle = B.far;
    for (let x = 0; x < cols; x++) {
        let h = rows * 0.42 + Math.sin(x * 0.11 + p1) * 2.2 + Math.sin(x * 0.037 + p2) * 3;
        if (B.peaks) h -= Math.abs(Math.sin(x * 0.21 + p2)) * 4;
        g.fillRect(x * block, Math.round(h) * block, block, H);
    }
    if (B.chimney) {
        for (let k = 0; k < 2; k++) {
            const x = Math.round(cols * (0.45 + k * 0.17));
            const top = Math.round(rows * (0.22 + rand() * 0.1));
            g.fillStyle = B.chimney;
            g.fillRect(x * block, top * block, block * 2, H);
            g.fillStyle = 'rgba(255,255,255,.35)';
            for (let i = 1; i < 4; i++) g.fillRect((x + (i % 2)) * block, (top - i * 1.4) * block, block * 1.4, block);
        }
    }
    // 近处地形：草（或者雪、地狱岩）+ 三层土 + 石头
    const q1 = rand() * 10;
    const q2 = rand() * 10;
    const heights = [];
    for (let x = 0; x < cols; x++) {
        const h = Math.round(rows * 0.66 + Math.sin(x * 0.16 + q1) * 1.6 + Math.sin(x * 0.05 + q2) * 2.4);
        heights.push(h);
        for (let y = h; y < rows; y++) {
            let c = y === h ? B.top : y < h + 3 ? B.dirt : B.stone;
            if (B.lava && y >= rows - 1 && rand() < 0.55) c = B.lava;
            if (B.ore && y > h + 3 && rand() < 0.05) c = B.ore;
            blockAt(g, x * block, y * block, block, c, rand);
        }
    }
    // 树：树干 + 一团树叶，彼此隔开
    for (let x = 3; x < cols - 3; x++) {
        if (rand() >= B.trees) continue;
        const h = heights[x];
        const th = 3 + Math.floor(rand() * 2);
        for (let k = 1; k <= th; k++) blockAt(g, x * block, (h - k) * block, block, B.trunk, rand);
        for (let dy = 0; dy < 3; dy++) {
            for (let dx = -2; dx <= 2; dx++) {
                if ((Math.abs(dx) === 2 && dy !== 1) || rand() < 0.08) continue;
                blockAt(g, (x + dx) * block, (h - th - 2 + dy) * block, block, B.leaf, rand);
            }
        }
        x += 5;
    }
    if (B.embers) {
        for (let i = 0; i < W / 14; i++) {
            g.fillStyle = `rgba(255,${120 + Math.floor(rand() * 80)},60,${0.5 + rand() * 0.5})`;
            g.fillRect(Math.floor(rand() * W), Math.floor(rand() * H * 0.7), 3, 3);
        }
    }
    if (face) {
        const fx = Math.min(cols - 3, Math.round(cols * 0.8));
        const px = Math.max(2, Math.round(block / 3.5));
        figure(g, fx * block + block / 2, heights[fx] * block, face, px);
    }
    // 底部压暗一点，叠在上面的字更清楚
    const shade = g.createLinearGradient(0, H * 0.35, 0, H);
    shade.addColorStop(0, 'rgba(0,0,0,0)');
    shade.addColorStop(1, 'rgba(0,0,0,.5)');
    g.fillStyle = shade;
    g.fillRect(0, 0, W, H);
}

// 云：画两遍首尾相接，CSS 平移一半宽度就能无缝循环
export function paintClouds(canvas, kind, seedStr, block = 12) {
    const { g, W, H } = setup(canvas);
    if (kind === 'nether') return;
    const rand = rng(seedOf(seedStr) ^ 0x9e3779b9);
    const half = W / 2;
    const alpha = kind === 'diamond' ? 0.22 : kind === 'cobble' ? 0.55 : 0.85;
    const clouds = [];
    for (let i = 0; i < Math.max(3, Math.round(half / 220)); i++) {
        clouds.push({ x: rand() * half, y: H * (0.08 + rand() * 0.3), w: 4 + Math.floor(rand() * 6), h: 1 + Math.floor(rand() * 2) });
    }
    g.fillStyle = `rgba(255,255,255,${alpha})`;
    for (const off of [0, half]) {
        for (const c of clouds) {
            const x = Math.round(c.x + off);
            const y = Math.round(c.y / block) * block;
            g.fillRect(x, y, c.w * block, c.h * block);
            g.fillRect(x + block, y - block, (c.w - 2) * block, block);
        }
    }
}
