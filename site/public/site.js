// 官网脚本：图标、像素风景（和启动器同一个画法）、进场、右下角小窗倒数、邀请码、下载状态
import { paintScene, paintClouds } from '/demo/scene.js';

const I = {
    github: '<path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"/><path d="M9 18c-4.51 2-5-2-7-2"/>',
    download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
    mouse: '<path d="M12 2v4"/><rect x="5" y="6" width="14" height="16" rx="7"/><path d="M12 10v2"/>',
    refresh: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
    play: '<polygon points="6 3 20 12 6 21 6 3"/>',
    lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
    code: '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>',
    package: '<path d="M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z"/><path d="M12 22V12"/><path d="m3.3 7 7.703 4.734a2 2 0 0 0 1.994 0L20.7 7"/><path d="m7.5 4.27 9 5.15"/>',
    wifi: '<path d="M12 20h.01"/><path d="M2 8.82a15 15 0 0 1 20 0"/><path d="M5 12.859a10 10 0 0 1 14 0"/><path d="M8.5 16.429a5 5 0 0 1 7 0"/>',
    history: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>',
    info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
    link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
    server: '<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/><rect width="20" height="8" x="2" y="14" rx="2" ry="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/>',
    alert: '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
    search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
    zap: '<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>',
    languages: '<path d="m5 8 6 6"/><path d="m4 14 6-6 2-3"/><path d="M2 5h12"/><path d="M7 2h1"/><path d="m22 22-5-10-5 10"/><path d="M14 18h6"/>',
    brush: '<path d="m9.06 11.9 8.07-8.06a2.85 2.85 0 1 1 4.03 4.03l-8.06 8.08"/><path d="M7.07 14.94c-1.66 0-3 1.35-3 3.02 0 1.33-2.5 1.52-2 2.02 1.08 1.1 2.49 2.02 4 2.02 2.2 0 4-1.8 4-4.04a3.01 3.01 0 0 0-3-3.02z"/>',
    map: '<path d="M14.106 5.553a2 2 0 0 0 1.788 0l3.659-1.83A1 1 0 0 1 21 4.619v12.764a1 1 0 0 1-.553.894l-4.553 2.277a2 2 0 0 1-1.788 0l-4.212-2.106a2 2 0 0 0-1.788 0l-3.659 1.83A1 1 0 0 1 3 19.381V6.618a1 1 0 0 1 .553-.894l4.553-2.277a2 2 0 0 1 1.788 0z"/><path d="M15 5.764v15"/><path d="M9 3.236v15"/>',
    cpu: '<rect width="16" height="16" x="4" y="4" rx="2"/><rect width="6" height="6" x="9" y="9" rx="1"/><path d="M15 2v2"/><path d="M15 20v2"/><path d="M2 15h2"/><path d="M2 9h2"/><path d="M20 15h2"/><path d="M20 9h2"/><path d="M9 2v2"/><path d="M9 20v2"/>',
    user: '<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>',
    monitor: '<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>',
    file: '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/>',
    usb: '<circle cx="10" cy="7" r="1"/><circle cx="4" cy="20" r="1"/><path d="M4.7 19.3 19 5"/><path d="m21 3-3 1 2 2Z"/><path d="M9.26 7.68 5 12l2 5"/><path d="m10 14 5 2 3.5-3.5"/><path d="m18 12 1-1 1 1-1 1Z"/>',
    phone: '<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>',
    shield: '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>',
};
document.querySelectorAll('[data-i]').forEach((el) => {
    el.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${I[el.dataset.i] || ''}</svg>`;
});

// ---------- 像素风景：同一个种子，电脑和手机上是同一个世界 ----------
function paintAll() {
    document.querySelectorAll('canvas[data-scene]').forEach((c) => {
        if (!c.getBoundingClientRect().width) return;
        const small = c.classList.contains('mini');
        const block = small ? 4 : c.closest('.phone') ? 9 : c.closest('.mini-win') ? 7 : 12;
        paintScene(c, c.dataset.scene, c.dataset.seed, { block, face: c.dataset.face ? 'steve' : null });
    });
}
paintAll();
let rt;
addEventListener('resize', () => {
    clearTimeout(rt);
    rt = setTimeout(paintAll, 150);
});

// ---------- 顶栏滚动后加一条分隔线 ----------
const nav = document.getElementById('nav');
const onScroll = () => nav.classList.toggle('scrolled', scrollY > 8);
addEventListener('scroll', onScroll, { passive: true });
onScroll();

// ---------- 进场：淡入上浮（自己开着，不跟系统的「减少动态效果」走，见站内样式说明） ----------
const io = new IntersectionObserver((es) => {
    for (const e of es) {
        if (e.isIntersecting) {
            e.target.classList.add('in');
            io.unobserve(e.target);
        }
    }
}, { rootMargin: '0px 0px -8% 0px' });
document.querySelectorAll('.reveal').forEach((el, i) => {
    el.style.transitionDelay = `${Math.min(i % 4, 3) * 60}ms`;
    io.observe(el);
});

// ---------- 右下角小窗：3、2、1，启动 ----------
const mw = { t: document.getElementById('mwTitle'), s: document.getElementById('mwSub'), b: document.getElementById('mwBar') };
if (mw.t) {
    const steps = [
        ['3 秒后启动「和同学的生存」', '按住 Shift 打开界面', 0],
        ['2 秒后启动「和同学的生存」', '按住 Shift 打开界面', 0],
        ['1 秒后启动「和同学的生存」', '按住 Shift 打开界面', 0],
        ['正在检查文件', '1.21.1 · Fabric · 46 个 Mod', 35],
        ['正在启动 Java 21', '等游戏窗口', 78],
        ['游戏窗口出来了', '这个小窗自己关掉', 100],
    ];
    let k = 0;
    setInterval(() => {
        const [t, s, p] = steps[k % steps.length];
        mw.t.textContent = t;
        mw.s.textContent = s;
        mw.b.style.width = `${p}%`;
        k++;
    }, 1100);
}

// ---------- 邀请码：像刚复制出来那样滚动 ----------
const code = document.getElementById('inviteCode');
if (code) {
    const A = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
    setInterval(() => {
        let s = 'LS1.';
        for (let i = 0; i < 18; i++) s += A[Math.floor(Math.random() * A.length)];
        code.textContent = s + '…';
    }, 2600);
}

// ---------- 在线演示：点了才加载（演示里的界面一打开会抢焦点，提前加载会把整页滚过去） ----------
document.getElementById('demoGo')?.addEventListener('click', () => {
    const ph = document.getElementById('demoPh');
    const f = document.createElement('iframe');
    f.src = '/demo/';
    f.title = 'LeafStarter 界面演示';
    ph.replaceWith(f);
});

// ---------- 下载状态：开放了就把按钮点亮 ----------
const fmt = (b) => (b > 1048576 ? `${(b / 1048576).toFixed(1)} MB` : `${Math.round(b / 1024)} KB`);
fetch('/api/status')
    .then((r) => (r.ok ? r.json() : null))
    .then((st) => {
        if (!st?.downloadsOpen || !st.latest) return;
        const L = st.latest;
        document.querySelectorAll('[data-dl]').forEach((el) => {
            const f = L.files?.[el.dataset.dl];
            if (!f) return;
            const a = document.createElement('a');
            a.className = el.className.replace(' off', '');
            a.href = `/download/${el.dataset.dl}`;
            a.innerHTML = `<span data-i="download">${I.download ? `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${I.download}</svg>` : ''}</span>下载 · ${fmt(f.size)}`;
            el.replaceWith(a);
        });
        const t = document.getElementById('ctaText');
        const s = document.getElementById('ctaSub');
        if (t) t.textContent = `下载 LeafStarter ${L.version}`;
        if (s) s.textContent = `Windows 10 / 11 · ${L.date}`;
        const h = document.querySelector('#download h2');
        if (h) h.innerHTML = `下载 <span class="grad">${L.version}</span>`;
        const n = document.getElementById('dlNote');
        if (n) n.textContent = L.notes || '安装版会自动更新；便携版的数据放在旁边的 data 文件夹。';
    })
    .catch(() => {});
