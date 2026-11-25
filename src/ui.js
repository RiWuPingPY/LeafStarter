// 小工具：DOM、偏好存储、时间格式、提示与撤销、像素画
export const $ = (s, el = document) => el.querySelector(s);
export const $$ = (s, el = document) => [...el.querySelectorAll(s)];
export const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

// 每个浏览器自己的偏好（上次的实例、页面、主题）；读不到就用默认值
export const store = {
    get(k, d) {
        try {
            const v = localStorage.getItem('ls:' + k);
            return v == null ? d : JSON.parse(v);
        } catch {
            return d;
        }
    },
    set(k, v) {
        try {
            localStorage.setItem('ls:' + k, JSON.stringify(v));
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
export function toast(msg, { undo, ms = 3600, action } = {}) {
    const box = $('#toasts');
    const el = document.createElement('div');
    el.className = 'toast';
    el.innerHTML = `<span>${esc(msg)}</span>${undo ? '<button type="button" class="link">撤销 <kbd>Ctrl Z</kbd></button>' : action ? `<button type="button" class="link">${esc(action.label)}</button>` : ''}`;
    box.append(el);
    const entry = { el, undo };
    let timer;
    entry.close = () => {
        clearTimeout(timer);
        undoStack = undoStack.filter((x) => x !== entry);
        el.classList.add('out');
        setTimeout(() => el.remove(), 160);
    };
    if (undo) {
        undoStack.push(entry);
        el.querySelector('button').addEventListener('click', () => {
            undo();
            entry.close();
        });
    }
    if (action) {
        el.querySelector('button').addEventListener('click', () => {
            action.run();
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
export const FACES = {
    steve: {
        rows: ['HHHHHHHH', 'HHHHHHHH', 'HSSSSSSH', 'SSSSSSSS', 'SWBSSBWS', 'SSSNNSSS', 'SSMSSMSS', 'SSMMMMSS'],
        pal: { H: '#3B2716', S: '#B98A6D', W: '#FFFFFF', B: '#4A3A8F', N: '#8A5A44', M: '#6B3B2A' },
    },
    pingyi: {
        rows: ['HHHHHHHH', 'HHHHHHHH', 'HHSSSSHH', 'HSSSSSSH', 'SWESSEWS', 'SSSNNSSS', 'SSSMMSSS', 'SSSSSSSS'],
        pal: { H: '#1F1D26', S: '#E6C3A5', W: '#FFFFFF', E: '#2E6B5E', N: '#CFA185', M: '#A8705E' },
    },
    // 联机演示里的两个朋友
    xiaoming: {
        rows: ['HHHHHHHH', 'HHHHHHHH', 'HSSSSSSH', 'SSSSSSSS', 'SWBSSBWS', 'SSSNNSSS', 'SSSMMSSS', 'SSSSSSSS'],
        pal: { H: '#5B3A1E', S: '#F0C8A0', W: '#FFFFFF', B: '#2B4C8C', N: '#D9A27C', M: '#B5655A' },
    },
    azhe: {
        rows: ['HHHHHHHH', 'HGGGGGGH', 'HSSSSSSH', 'SSSSSSSS', 'SWKSSKWS', 'SSSNNSSS', 'SSMMMMSS', 'SSSSSSSS'],
        pal: { H: '#101418', G: '#2F7D5B', S: '#C99A76', W: '#FFFFFF', K: '#22303A', N: '#A8775A', M: '#7A4A3A' },
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

// ---------- 启动器自己的确认框和输入框 ----------
// Tauri 的对话框插件把 window.confirm() 换成了异步的（返回 Promise，永远算「真」），
// 直接 if (!confirm(...)) 会不等回答就往下走。所以一律用这两个，await 拿到真正的回答：
//   ask(文字) → true / false        askText(文字, 默认值) → 字符串 / null（取消）
export function ask(message, { ok = '确定', cancel = '取消', danger = false } = {}) {
    return modal(message, { ok, cancel, danger, input: null });
}

export function askText(message, value = '', { ok = '确定', cancel = '取消' } = {}) {
    return modal(message, { ok, cancel, danger: false, input: String(value ?? '') });
}

function modal(message, { ok, cancel, danger, input }) {
    return new Promise((resolve) => {
        const prev = document.activeElement;
        const wrap = document.createElement('div');
        wrap.className = 'ask-scrim';
        wrap.innerHTML = `<div class="ask" role="alertdialog" aria-modal="true"><p class="ask-msg"></p>${input != null ? '<input class="ask-in" type="text" autocomplete="off">' : ''}<div class="ask-act"><button type="button" class="btn" data-a="no"></button><button type="button" class="btn ${danger ? 'danger-pri' : 'pri'}" data-a="yes"></button></div></div>`;
        wrap.querySelector('.ask-msg').textContent = message;
        wrap.querySelector('[data-a="no"]').textContent = cancel;
        wrap.querySelector('[data-a="yes"]').textContent = ok;
        const inp = wrap.querySelector('.ask-in');
        if (inp) inp.value = input;
        const done = (yes) => {
            wrap.remove();
            document.removeEventListener('keydown', key, true);
            prev?.focus?.();
            resolve(input != null ? (yes ? inp.value : null) : yes);
        };
        // 框开着时键盘只归它：Esc 取消、回车确定，别的快捷键不往下传
        const key = (e) => {
            e.stopPropagation();
            if (e.key === 'Escape') {
                e.preventDefault();
                done(false);
            } else if (e.key === 'Enter') {
                e.preventDefault();
                done(true);
            }
        };
        wrap.addEventListener('click', (e) => {
            e.stopPropagation();
            const a = e.target.closest('[data-a]')?.dataset.a;
            if (a) done(a === 'yes');
            else if (e.target === wrap) done(false);
        });
        document.addEventListener('keydown', key, true);
        document.body.appendChild(wrap);
        (inp || wrap.querySelector('[data-a="yes"]')).focus();
        inp?.select();
    });
}
