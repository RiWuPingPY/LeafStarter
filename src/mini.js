// 直接启动的小窗：倒数，然后跟着后端的启动进度走；游戏窗口出来后端就把小窗关了。
import { block, esc } from './ui.js';
import { blockOf } from './real.js';

const T = window.__TAURI__;
const q = new URLSearchParams(location.search);
const name = q.get('n') || '';
const kind = blockOf(q.get('l') || '');
const until = Date.now() + (Number(q.get('d')) || 3000);
const PHASE = { account: '检查账户', files: '补全游戏文件', java: '准备 Java', starting: '等游戏窗口', running: '游戏窗口出来了' };

// 主题和动效由后端放在网址里（读 localStorage 冷启动要等将近 1 秒）
const theme = q.get('t') || 'system';
if (theme !== 'system') document.documentElement.dataset.theme = theme;
document.documentElement.dataset.motion = q.get('m') || 'full';

const mw = document.getElementById('mw');
let launching = null; // 后端推来的最新状态

function draw() {
    const left = Math.max(0, Math.ceil((until - Date.now()) / 1000));
    let title, sub, bar = '<i class="bar" id="bar"></i>';
    if (!launching) {
        title = `${left} 秒后启动「${esc(name)}」`;
        sub = '按住 Shift 再打开 LeafStarter 就是正常界面';
    } else {
        const p = launching;
        title = `正在启动「${esc(name)}」`;
        sub = esc(p.phase === 'files' && p.pct ? `${PHASE.files} ${p.pct}%` : p.detail || PHASE[p.phase] || '');
        bar = p.phase === 'files' ? `<i class="bar" style="transform: scaleX(${Math.max(2, p.pct) / 100})"></i>` : '<i class="bar busy"></i>';
    }
    const cancel = launching ? '' : '<button type="button" class="btn" data-act="cancel">取消 <kbd>Esc</kbd></button>';
    mw.innerHTML = `${block(kind, 36)}<span class="tx" data-tauri-drag-region><b>${title}</b><small>${sub}</small></span>
        <span class="acts">${cancel}<button type="button" class="btn" data-act="open">打开启动器</button></span>
        ${bar}`;
    if (!launching) countdownBar();
}

// 倒数的那条线：从满到空
function countdownBar() {
    const el = document.getElementById('bar');
    const ms = Math.max(0, until - Date.now());
    el.style.transform = `scaleX(${ms / 3000})`;
    requestAnimationFrame(() => {
        el.style.transition = `transform ${ms}ms linear`;
        el.style.transform = 'scaleX(0)';
    });
}

const cancel = () => T.core.invoke('direct_cancel');
mw.addEventListener('click', (e) => {
    const b = e.target.closest('[data-act]');
    if (!b) return;
    if (b.dataset.act === 'cancel') cancel();
    if (b.dataset.act === 'open') T.core.invoke('direct_open');
});
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') launching ? T.core.invoke('direct_open') : cancel();
});

// 只在秒数变的时候重画，倒数线靠 CSS 过渡自己走
let shown = -1;
const tick = setInterval(() => {
    if (launching) return clearInterval(tick);
    const left = Math.ceil((until - Date.now()) / 1000);
    if (left !== shown && left >= 0) {
        shown = left;
        const b = mw.querySelector('b');
        if (b) b.textContent = `${Math.max(0, left)} 秒后启动「${name}」`;
    }
}, 100);

T.event.listen('launch-state', (e) => {
    const p = e.payload;
    if (!p.key.endsWith('/' + name)) return;
    launching = p;
    draw();
});

draw();
