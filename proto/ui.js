// 原型的小工具：DOM、偏好存储、时间格式、提示与撤销、像素画
export const $ = (s, el = document) => el.querySelector(s);
export const $$ = (s, el = document) => [...el.querySelectorAll(s)];
export const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

// 每个浏览器自己的偏好（上次的实例、页面、主题）；读不到就用默认值
export const store = {
    get(k, d) {
        try {
            const v = localStorage.getItem('ls-proto:' + k);
            return v == null ? d : JSON.parse(v);
        } catch {
            return d;
        }
    },
    set(k, v) {
        try {
            localStorage.setItem('ls-proto:' + k, JSON.stringify(v));
        } catch {}
    },
};

const pad = (n) => String(n).padStart(2, '0');
const dayStart = (t) => new Date(t.getFullYear(), t.getMonth(), t.getDate()).getTime();

export function ago(ts) {
    const d = Date.now() - ts;
    if (d < 60e3) return '刚刚';
    if (d < 3600e3) return `${Math.floor(d / 60e3)} 分钟前`;
    const t = new Date(ts);
    const days = Math.round((dayStart(new Date()) - dayStart(t)) / 864e5);
    const hm = `${pad(t.getHours())}:${pad(t.getMinutes())}`;
    if (days === 0) return `今天 ${hm}`;
    if (days === 1) return `昨天 ${hm}`;
    if (days < 7) return `${days} 天前`;
    return `${t.getMonth() + 1}月${t.getDate()}日`;
}

export function hours(min) {
    if (min < 60) return `${min} 分钟`;
    const h = Math.floor(min / 60);
    const m = min % 60;
    return m && h < 10 ? `${h} 小时 ${m} 分` : `${h} 小时`;
}

export function clock(sec) {
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    return h ? `${h}:${pad(m)}:${pad(sec % 60)}` : `${pad(m)}:${pad(sec % 60)}`;
}

export const stamp = () => {
    const t = new Date();
    return `[${pad(t.getHours())}:${pad(t.getMinutes())}:${pad(t.getSeconds())}]`;
};

// 提示放在主区左下角（右下角是启动按钮）。带撤销的提示可以按 Ctrl+Z
let undoStack = [];
export function toast(msg, { undo, ms = 3600 } = {}) {
    const box = $('#toasts');
    const el = document.createElement('div');
    el.className = 'toast';
    el.innerHTML = `<span>${esc(msg)}</span>${undo ? '<button type="button" class="link">撤销 <kbd>Ctrl Z</kbd></button>' : ''}`;
    box.append(el);
    const entry = { el, undo };
    let timer;
    entry.close = () => {
        clearTimeout(timer);
        el.remove();
        undoStack = undoStack.filter((x) => x !== entry);
    };
    if (undo) {
        undoStack.push(entry);
        el.querySelector('button').addEventListener('click', () => {
            undo();
            entry.close();
        });
    }
    timer = setTimeout(entry.close, undo ? 8000 : ms);
    // 最多同时留三条
    const all = [...box.children];
    all.slice(0, Math.max(0, all.length - 3)).forEach((old) => {
        const e = undoStack.find((x) => x.el === old);
        if (e) e.close();
        else old.remove();
    });
    return entry;
}

export function undoLast() {
    const e = undoStack.at(-1);
    if (!e) return false;
    e.undo();
    e.close();
    return true;
}

// 方块图标：顶面一条 + 侧面 + 像素格，只用 CSS
export const block = (kind, size = 28) => `<i class="blk blk-${kind}" style="--s:${size}px" aria-hidden="true"></i>`;

// 玩家头像：8×8 的脸画进 canvas（示例皮肤，不是谁的真皮肤）
const FACES = {
    steve: {
        rows: ['HHHHHHHH', 'HHHHHHHH', 'HSSSSSSH', 'SSSSSSSS', 'SWBSSBWS', 'SSSNNSSS', 'SSMSSMSS', 'SSMMMMSS'],
        pal: { H: '#3B2716', S: '#B98A6D', W: '#FFFFFF', B: '#4A3A8F', N: '#8A5A44', M: '#6B3B2A' },
    },
    pingyi: {
        rows: ['HHHHHHHH', 'HHHHHHHH', 'HHSSSSHH', 'HSSSSSSH', 'SWESSEWS', 'SSSNNSSS', 'SSSMMSSS', 'SSSSSSSS'],
        pal: { H: '#1F1D26', S: '#E6C3A5', W: '#FFFFFF', E: '#2E6B5E', N: '#CFA185', M: '#A8705E' },
    },
};
const faceCache = {};
export function head(face, size = 32) {
    if (!(face in faceCache)) {
        try {
            const f = FACES[face] || FACES.steve;
            const c = document.createElement('canvas');
            c.width = c.height = 8;
            const g = c.getContext('2d');
            f.rows.forEach((row, y) => [...row].forEach((ch, x) => {
                g.fillStyle = f.pal[ch];
                g.fillRect(x, y, 1, 1);
            }));
            faceCache[face] = c.toDataURL();
        } catch {
            faceCache[face] = '';
        }
    }
    return `<img class="head" src="${faceCache[face]}" width="${size}" height="${size}" alt="">`;
}
