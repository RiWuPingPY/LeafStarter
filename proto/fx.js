// 游戏真的启动成功的那一下：一把经验球从启动按钮里蹦出来。一天就几次，值得热闹一点
const reduced = () => {
    const m = document.documentElement.dataset.motion;
    return m === 'reduced' || (m === 'system' && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
};

export function burst(from, host) {
    if (!from || !host || reduced()) return;
    const hr = host.getBoundingClientRect();
    const r = from.getBoundingClientRect();
    const c = document.createElement('canvas');
    c.className = 'fx';
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    c.width = hr.width * dpr;
    c.height = hr.height * dpr;
    host.append(c);
    const g = c.getContext('2d');
    g.scale(dpr, dpr);
    const ox = r.left - hr.left;
    const oy = r.top - hr.top + r.height * 0.4;
    const parts = Array.from({ length: 40 }, () => ({
        x: ox + 20 + Math.random() * (r.width - 40),
        y: oy,
        vx: (Math.random() - 0.5) * 6,
        vy: -4 - Math.random() * 6,
        s: 6 + Math.round(Math.random() * 4),
        life: 0,
        max: 46 + Math.random() * 26,
        hue: 72 + Math.random() * 38,
    }));
    const done = () => c.remove();
    const step = () => {
        g.clearRect(0, 0, hr.width, hr.height);
        let alive = 0;
        for (const p of parts) {
            p.life += 1;
            if (p.life > p.max) continue;
            alive += 1;
            p.vy += 0.2;
            p.x += p.vx;
            p.y += p.vy;
            g.globalAlpha = 1 - p.life / p.max;
            g.fillStyle = `hsl(${p.hue} 85% 52%)`;
            g.fillRect(p.x, p.y, p.s, p.s);
            g.fillStyle = `hsl(${p.hue} 100% 86%)`;
            g.fillRect(p.x + 1, p.y + 1, Math.ceil(p.s / 2), Math.ceil(p.s / 2));
        }
        g.globalAlpha = 1;
        if (alive) requestAnimationFrame(step);
        else done();
    };
    requestAnimationFrame(step);
    setTimeout(done, 2500); // 页面在后台时 rAF 不跑，别把画布留在那儿
}
