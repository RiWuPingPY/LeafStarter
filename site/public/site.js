// 官网脚本：图标、像素风景（和启动器同一个画法）、背景里的风和叶子、弹簧动效（进场、跟手、倾斜）、几处小演示、下载状态
// 动效不看 prefers-reduced-motion：开发机和很多 Windows 都关了系统动画，跟着系统走就一点不动
window.LS_READY = true;
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const coarse = matchMedia('(pointer: coarse)').matches;
const darkQ = matchMedia('(prefers-color-scheme: dark)');

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
    check: '<path d="M20 6 9 17l-5-5"/>',
};
const svg = (k) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${I[k] || ''}</svg>`;
$$('[data-i]').forEach((el) => { el.innerHTML = svg(el.dataset.i); });

// ---------- 像素风景：同一个种子，电脑和手机上是同一个世界 ----------
// scene.js 按画布当时的外框量尺寸，进场时还缩着一点，所以进场结束后再补画一次
let paint = () => {};
import('/demo/scene.js').then(({ paintScene }) => {
    paint = (root = document) => {
        $$('canvas[data-scene]', root).forEach((c) => {
            if (!c.getBoundingClientRect().width) return;
            const block = c.classList.contains('mini') ? 4 : c.closest('.sync-phone') ? 6 : c.closest('.mini-win') ? 7 : c.closest('.sync-pc') ? 9 : 12;
            paintScene(c, c.dataset.scene, c.dataset.seed, { block });
        });
    };
    paint();
    setTimeout(() => paint(), 1400);
}).catch(() => {});
let rt;
addEventListener('resize', () => { clearTimeout(rt); rt = setTimeout(() => paint(), 150); });

// ---------- 弹簧：所有跟手的东西都走它，松手时才有那一下回弹 ----------
const live = new Set();
let rafOn = false, lastT = 0;
function tick(now) {
    const dt = Math.min(0.032, (now - lastT) / 1000 || 0.016);
    lastT = now;
    live.forEach((s) => { if (s.step(dt)) live.delete(s); });
    if (live.size) requestAnimationFrame(tick); else rafOn = false;
}
function wake() { if (!rafOn) { rafOn = true; lastT = performance.now(); requestAnimationFrame(tick); } }
class Spring {
    constructor(v, k, c, on) { this.v = v; this.t = v; this.u = 0; this.k = k; this.c = c; this.on = on; }
    to(t) { this.t = t; live.add(this); wake(); }
    step(dt) {
        this.u += (-this.k * (this.v - this.t) - this.c * this.u) * dt;
        this.v += this.u * dt;
        const done = Math.abs(this.u) < 0.002 && Math.abs(this.v - this.t) < 0.002;
        if (done) this.v = this.t;
        this.on(this.v);
        return done;
    }
}

// ---------- 进场：同一组里错开一点点弹上来 ----------
const io = new IntersectionObserver((es) => {
    for (const e of es) {
        if (!e.isIntersecting) continue;
        e.target.classList.add('in');
        io.unobserve(e.target);
        if (e.target.querySelector('canvas[data-scene]')) setTimeout(() => paint(e.target), 1100);
    }
}, { threshold: 0.12, rootMargin: '0px 0px -6% 0px' });
$$('.rv').forEach((el) => {
    if (!el.style.getPropertyValue('--i')) {
        const sib = Array.from(el.parentElement.children).filter((x) => x.classList.contains('rv'));
        el.style.setProperty('--i', Math.min(sib.indexOf(el), 6));
    }
    io.observe(el);
});

// ---------- 背景：几条长长的风和几片叶子，鼠标划过、页面滚动都会起一阵风 ----------
const cv = $('#wind');
if (cv?.getContext) {
    const g = cv.getContext('2d');
    let W = 0, H = 0, gust = 0, px = -1, lastY = scrollY, dark = darkQ.matches;
    darkQ.addEventListener?.('change', (e) => { dark = e.matches; });
    const fit = () => { const dpr = Math.min(1.5, devicePixelRatio || 1); W = innerWidth; H = innerHeight; cv.width = W * dpr; cv.height = H * dpr; g.setTransform(dpr, 0, 0, dpr, 0, 0); };
    fit();
    addEventListener('resize', fit);
    addEventListener('pointermove', (e) => { if (px >= 0) gust = Math.min(3.4, gust + Math.abs(e.clientX - px) * 0.012); px = e.clientX; }, { passive: true });
    addEventListener('scroll', () => { gust = Math.min(3.4, gust + Math.abs(scrollY - lastY) * 0.006); lastY = scrollY; }, { passive: true });
    const R = (a, b) => a + Math.random() * (b - a);
    const lines = Array.from({ length: coarse ? 6 : 11 }, () => ({ y: R(0, 1), a: R(14, 46), f: R(0.0016, 0.0042), p: R(0, 6.28), s: R(0.14, 0.42), l: R(0.22, 0.5), x: R(0, 1.6), o: R(0.1, 0.26), sky: Math.random() < 0.35 }));
    // 叶子和几粒方块小像素（一点 Minecraft 的意思）
    const leaves = Array.from({ length: coarse ? 7 : 14 }, (_, k) => ({ x: R(0, 1), y: R(0, 1), s: R(5, 11), r: R(0, 6.28), v: R(0.012, 0.03), w: R(0.5, 1.6), p: R(0, 6.28), h: k % 3, px: k % 4 === 3 }));
    const LEAF = [['#5CCB5F', '#2FAF55', '#9BD8A8'], ['#3E9F5A', '#2F8A4C', '#4F8A69']];
    const PIX = [['#7FC8F0', '#5CCB5F'], ['#3D7FA8', '#3E9F5A']];
    let t0 = performance.now(), hidden = false;
    document.addEventListener('visibilitychange', () => { hidden = document.hidden; if (!hidden) { t0 = performance.now(); requestAnimationFrame(frame); } });
    function frame(now) {
        if (hidden) return;
        const dt = Math.min(0.05, (now - t0) / 1000);
        t0 = now;
        gust *= Math.pow(0.16, dt);
        const speed = 1 + gust;
        g.clearRect(0, 0, W, H);
        g.lineCap = 'round';
        for (const l of lines) {
            l.x += l.s * dt * speed * 0.42;
            if (l.x > 1.6) { l.x = -0.1; l.y = R(0, 1); }
            const x1 = l.x * W, x0 = x1 - l.l * W, y0 = l.y * H;
            const rgb = l.sky ? (dark ? '130,200,244' : '43,134,197') : (dark ? '95,214,140' : '47,160,85');
            const o = dark ? l.o * 0.55 : l.o;
            const grad = g.createLinearGradient(x0, 0, x1, 0);
            grad.addColorStop(0, `rgba(${rgb},0)`); grad.addColorStop(0.7, `rgba(${rgb},${o})`); grad.addColorStop(1, `rgba(${rgb},0)`);
            g.strokeStyle = grad;
            g.lineWidth = 1.2;
            g.beginPath();
            for (let x = x0; x <= x1; x += 22) { const y = y0 + Math.sin(x * l.f + l.p + now * 0.00035) * l.a; if (x === x0) g.moveTo(x, y); else g.lineTo(x, y); }
            g.stroke();
        }
        for (const f of leaves) {
            f.x += f.v * dt * speed * 1.5;
            f.y += Math.sin(now * 0.0005 * f.w + f.p) * 0.00034;
            f.r += dt * f.w * (0.5 + gust * 0.6);
            if (f.x > 1.05) { f.x = -0.05; f.y = R(0, 1); }
            g.save();
            g.translate(f.x * W, f.y * H);
            g.rotate(f.r);
            if (f.px) {
                g.globalAlpha = dark ? 0.35 : 0.42;
                g.fillStyle = PIX[dark ? 1 : 0][f.h % 2];
                const s = f.s * 0.55;
                g.fillRect(-s / 2, -s / 2, s, s);
            } else {
                g.scale(1, 0.46 + 0.4 * Math.sin(now * 0.0012 * f.w + f.p));
                g.globalAlpha = dark ? 0.4 : 0.5;
                g.fillStyle = LEAF[dark ? 1 : 0][f.h];
                g.beginPath(); g.moveTo(-f.s, 0); g.quadraticCurveTo(0, -f.s * 0.62, f.s, 0); g.quadraticCurveTo(0, f.s * 0.62, -f.s, 0); g.fill();
            }
            g.restore();
        }
        requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
}

// ---------- 顶栏：滚动后实一点；滚到哪一节，那一节的链接底下垫一块 ----------
const nav = $('#nav'), links = $$('#links a'), th = $('.links-th');
let curLink = null;
function spy() {
    nav?.classList.toggle('scrolled', scrollY > 8);
    if (!th || !links.length || !th.offsetParent) return;
    const y = innerHeight * 0.38;
    let cur = null;
    for (const a of links) {
        const s = document.getElementById(a.hash.slice(1));
        if (!s) continue;
        const r = s.getBoundingClientRect();
        if (r.top <= y && r.bottom > y) cur = a;
    }
    if (cur === curLink) return;
    curLink?.classList.remove('on');
    curLink = cur;
    if (!cur) { th.classList.remove('on'); return; }
    cur.classList.add('on');
    th.style.width = `${cur.offsetWidth}px`;
    th.style.transform = `translateX(${cur.offsetLeft}px)`;
    th.classList.add('on');
}
addEventListener('scroll', spy, { passive: true });
addEventListener('resize', () => { curLink?.classList.remove('on'); curLink = null; spy(); });
spy();

// ---------- 跟手：按钮被鼠标吸过去一点，卡片跟着鼠标微微转，鼠标那里亮一点 ----------
if (!coarse) {
    $$('.mag').forEach((b) => {
        let x = 0, y = 0;
        const ap = () => { b.style.translate = `${x.toFixed(1)}px ${y.toFixed(1)}px`; };
        const sx = new Spring(0, 200, 14, (v) => { x = v; ap(); }), sy = new Spring(0, 200, 14, (v) => { y = v; ap(); });
        b.addEventListener('pointermove', (e) => { const r = b.getBoundingClientRect(); sx.to((e.clientX - r.left - r.width / 2) * 0.2); sy.to((e.clientY - r.top - r.height / 2) * 0.3); });
        b.addEventListener('pointerleave', () => { sx.to(0); sy.to(0); });
    });
    $$('.tilt').forEach((c) => {
        let rX = 0, rY = 0, lift = 0;
        const ap = () => { c.style.transform = `perspective(1000px) rotateX(${rX.toFixed(2)}deg) rotateY(${rY.toFixed(2)}deg) translateY(${(-lift).toFixed(1)}px)`; };
        const a = new Spring(0, 170, 14, (v) => { rX = v; ap(); }), b = new Spring(0, 170, 14, (v) => { rY = v; ap(); }), l = new Spring(0, 170, 14, (v) => { lift = v; ap(); });
        c.addEventListener('pointermove', (e) => {
            const r = c.getBoundingClientRect(), fx = (e.clientX - r.left) / r.width, fy = (e.clientY - r.top) / r.height;
            b.to((fx - 0.5) * 5); a.to(-(fy - 0.5) * 5); l.to(5);
            c.style.setProperty('--mx', `${(fx * 100).toFixed(1)}%`);
            c.style.setProperty('--my', `${(fy * 100).toFixed(1)}%`);
        });
        c.addEventListener('pointerleave', () => { a.to(0); b.to(0); l.to(0); });
    });
    // 首屏的插件页：整块跟着鼠标转
    const stage = $('#stage'), pc = $('#pc');
    if (stage && pc) {
        let rx = 0, ry = 0;
        const ap = () => { pc.style.transform = `rotateX(${rx.toFixed(2)}deg) rotateY(${ry.toFixed(2)}deg)`; };
        const sRx = new Spring(0, 120, 12, (v) => { rx = v; ap(); }), sRy = new Spring(0, 120, 12, (v) => { ry = v; ap(); });
        stage.addEventListener('pointermove', (e) => { const r = stage.getBoundingClientRect(); sRy.to(((e.clientX - r.left) / r.width - 0.5) * 9); sRx.to(-((e.clientY - r.top) / r.height - 0.5) * 7); });
        stage.addEventListener('pointerleave', () => { sRx.to(0); sRy.to(0); });
    }
}

// ---------- 首屏的开关：真能点；没人点的时候自己演示一下 ----------
const plugs = $('#plugs');
if (plugs) {
    const onCount = $('#onCount'), toast = $('#pcToast'), tip = $('#tip');
    let toastT = 0, touched = false;
    const flip = (sw, quiet) => {
        const on = sw.getAttribute('aria-checked') !== 'true';
        sw.setAttribute('aria-checked', String(on));
        sw.closest('.pl').classList.toggle('off', !on);
        onCount.textContent = $$('.sw[aria-checked="true"]', plugs).length;
        onCount.classList.remove('tick'); void onCount.offsetWidth; onCount.classList.add('tick');
        if (quiet) return;
        toast.textContent = `${on ? '已打开' : '已关闭'}「${sw.getAttribute('aria-label')}」`;
        toast.classList.add('on');
        clearTimeout(toastT);
        toastT = setTimeout(() => toast.classList.remove('on'), 1800);
    };
    $$('.sw', plugs).forEach((sw) => sw.addEventListener('click', () => { touched = true; tip?.classList.add('off'); flip(sw); }));
    // 轮流把关着的两个打开再关上，有人点过就停
    const idle = [$$('.sw', plugs)[3], $$('.sw', plugs)[5]];
    // 提示气泡放在第一个关着的开关左上方，右下角的小尖角指过去（不压上一行的开关）
    const placeTip = () => {
        const stage = $('#stage');
        if (!tip || !stage) return;
        const s = stage.getBoundingClientRect(), b = idle[0].getBoundingClientRect();
        tip.style.right = 'auto';
        tip.style.left = `${Math.round(b.left - s.left - tip.offsetWidth + 6)}px`;
        tip.style.top = `${Math.round(b.top - s.top - tip.offsetHeight + 2)}px`;
    };
    placeTip();
    setTimeout(placeTip, 1300);
    addEventListener('resize', placeTip);
    document.fonts?.ready.then(placeTip);
    let k = 0;
    const demo = () => {
        if (touched) return;
        const sw = idle[k % 2];
        flip(sw, true);
        setTimeout(() => { if (!touched) flip(sw, true); k++; }, 1600);
    };
    setTimeout(() => { demo(); setInterval(demo, 4200); }, 2600);
}

// ---------- 同一个世界：底下的小字轮流说同步了什么 ----------
const syncText = $('#syncText');
if (syncText) {
    const says = ['版本、Mod、配置、存档一起过去', '同一个 Wi-Fi 下直接传', '同步前先留一份历史版本'];
    let k = 0;
    setInterval(() => {
        syncText.style.opacity = 0;
        setTimeout(() => { k = (k + 1) % says.length; syncText.textContent = says[k]; syncText.style.opacity = 1; }, 350);
    }, 3400);
}

// ---------- 右下角小窗：双击图标，3、2、1，启动 ----------
const win = $('#miniWin');
if (win) {
    const icon = $('#dIcon'), mt = $('#mwTitle'), ms = $('#mwSub'), mb = $('#mwBar');
    const steps = [
        ['3 秒后启动「和同学的生存」', '按住 Shift 打开界面', 0],
        ['2 秒后启动「和同学的生存」', '按住 Shift 打开界面', 0],
        ['1 秒后启动「和同学的生存」', '按住 Shift 打开界面', 0],
        ['正在检查文件', '1.21.1 · Fabric · 46 个 Mod', 35],
        ['正在启动 Java 21', '等游戏窗口', 78],
        ['游戏窗口出来了', '这个小窗自己关掉', 100],
    ];
    const show = ([t, s, p]) => { mt.textContent = t; ms.textContent = s; mb.style.width = `${p}%`; };
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    let first = true;
    (async () => {
        for (;;) {
            show(steps[0]);
            icon.classList.add('sel', 'clk');
            await wait(650);
            icon.classList.remove('clk');
            win.classList.add('on');
            if (first) { first = false; setTimeout(() => paint(win), 950); }
            for (const st of steps) { show(st); await wait(st[2] === 100 ? 1500 : 1050); }
            win.classList.remove('on');
            icon.classList.remove('sel');
            await wait(1600);
        }
    })();
}

// ---------- 邀请码：一个字一个字定下来，然后「已复制」 ----------
const code = $('#inviteCode'), copied = $('#copied');
if (code) {
    const A = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
    const pick = () => A[Math.floor(Math.random() * A.length)];
    setInterval(() => {
        const t = Array.from({ length: 18 }, pick).join('');
        let k = 0;
        copied?.classList.remove('on');
        const iv = setInterval(() => {
            k++;
            let s = 'LS1.';
            for (let i = 0; i < 18; i++) s += i < k ? t[i] : pick();
            code.textContent = `${s}…`;
            if (k >= 18) { clearInterval(iv); copied?.classList.add('on'); }
        }, 34);
    }, 3600);
}

// ---------- 二分找 Mod：一格一个，每轮排除一半 ----------
const mods = $('#mods');
if (mods) {
    const N = 48, round = $('#bisRound');
    mods.innerHTML = '<i></i>'.repeat(N);
    const cells = Array.from(mods.children);
    const keep = [[0, 48], [0, 24], [12, 24], [12, 18]];
    const set = (r) => {
        const [a, b] = keep[r];
        cells.forEach((c, i) => { c.classList.toggle('out', i < a || i >= b); c.classList.toggle('hot', r === 3 && i >= a && i < b); });
        if (round) round.textContent = Math.min(3, r + 1);
    };
    let r = 0;
    set(0);
    setInterval(() => { r = (r + 1) % 5; set(Math.min(r, 3)); }, 1300);
}

// ---------- 插件四层：滚到中间时摊开；指到哪一层，那一层亮起来 ----------
const stack = $('#stack');
if (stack) {
    const sOpen = new Spring(0.3, 60, 9, (v) => stack.style.setProperty('--open', v.toFixed(3)));
    let hover = false;
    const plys = () => {
        if (hover) return;
        const r = stack.getBoundingClientRect(), vh = innerHeight;
        if (r.bottom < 0 || r.top > vh) return;
        sOpen.to(Math.max(0.15, Math.min(1, 1.25 - Math.abs((r.top + r.height / 2) / vh - 0.5) * 2.2)));
    };
    addEventListener('scroll', plys, { passive: true });
    plys();
    stack.addEventListener('pointerenter', () => { hover = true; sOpen.to(1.3); });
    stack.addEventListener('pointerleave', () => { hover = false; plys(); });
    const lys = $$('.ly'), planes = $$('.ply', stack);
    let held = false, auto = 0;
    const hi = (n) => {
        stack.classList.toggle('dim', !!n);
        planes.forEach((p) => p.classList.toggle('hi', p.dataset.ly === n));
        lys.forEach((l) => l.classList.toggle('hi', l.dataset.ly === n));
    };
    lys.forEach((l) => {
        l.addEventListener('pointerenter', () => { held = true; hi(l.dataset.ly); });
        l.addEventListener('pointerleave', () => { held = false; hi(null); });
    });
    // 没人指的时候，从外观到原生轮流亮一遍
    setInterval(() => {
        if (held) return;
        const r = stack.getBoundingClientRect();
        if (r.bottom < 0 || r.top > innerHeight) return;
        auto = (auto % 5) + 1;
        hi(auto <= 4 ? String(auto) : null);
    }, 1500);
}

// ---------- 在线演示：点了才加载（演示里的界面一打开会抢焦点，提前加载会把整页滚过去） ----------
$('#demoGo')?.addEventListener('click', () => {
    const ph = $('#demoPh');
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
        $$('[data-dl]').forEach((el) => {
            const f = L.files?.[el.dataset.dl];
            if (!f) return;
            const a = document.createElement('a');
            a.className = el.className.replace(' off', '');
            a.href = `/download/${el.dataset.dl}`;
            a.innerHTML = `<span data-i="download">${svg('download')}</span>下载 · ${fmt(f.size)}`;
            el.replaceWith(a);
        });
        const t = $('#ctaText');
        const s = $('#ctaSub');
        if (t) t.textContent = `下载 LeafStarter ${L.version}`;
        if (s) s.textContent = `Windows 10 / 11 · ${L.date}`;
        const e = $('#endText');
        if (e) e.textContent = `下载 LeafStarter ${L.version}`;
        const h = $('#download h2');
        if (h) h.innerHTML = `下载 <span class="grad">${L.version}</span>`;
        const n = $('#dlNote');
        if (n) n.textContent = L.notes || '安装版会自动更新；便携版的数据放在旁边的 data 文件夹。';
    })
    .catch(() => {});
