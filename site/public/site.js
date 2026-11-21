// 官网脚本：图标、最新版本信息、下载次数、演示窗口缩放、滚动入场
(() => {
    const I = {
        monitor: '<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>',
        phone: '<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>',
        layers: '<path d="m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>',
        hammer: '<path d="m15 12-8.373 8.373a1 1 0 1 1-3-3L12 9"/><path d="m18 15 4-4"/><path d="m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172V7l-2.26-2.26a6 6 0 0 0-4.202-1.756L9 2.96l.92.82A6.18 6.18 0 0 1 12 8.4V10l2 2h1.172a2 2 0 0 1 1.414.586L18.5 14.5"/>',
        zap: '<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>',
        download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
        rocket: '<path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z"/><path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z"/><path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0"/><path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5"/>',
        shield: '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>',
    };
    document.querySelectorAll('[data-i]').forEach((el) => {
        el.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${I[el.dataset.i] || ''}</svg>`;
    });

    const size = (b) => {
        if (!b) return '—';
        const u = ['B', 'KB', 'MB', 'GB'];
        let i = 0;
        while (b >= 1024 && i < u.length - 1) { b /= 1024; i++; }
        return (i ? b.toFixed(1) : b) + ' ' + u[i];
    };

    // 按访客设备把对应平台的下载放到主按钮
    const isAndroid = /android/i.test(navigator.userAgent);
    const isIOS = /iphone|ipad|ipod/i.test(navigator.userAgent);
    if (isAndroid) {
        const main = document.getElementById('ctaMain');
        const alt = document.getElementById('ctaAlt');
        main.parentNode.insertBefore(alt, main);
        alt.classList.add('primary');
        main.classList.remove('primary');
    }
    document.querySelector(`.dl[data-plat="${isAndroid ? 'android' : 'windows'}"]`)?.classList.add('hot');

    // 微信 / QQ 内置浏览器会拦截安装包下载：提示去系统浏览器打开
    if (/MicroMessenger|QQ\//i.test(navigator.userAgent)) {
        const tip = document.createElement('div');
        tip.className = 'wx-tip';
        tip.innerHTML = '<b>微信里没法直接下载安装包</b><span>点右上角「···」→「在浏览器打开」再下载</span><i>↗</i>';
        document.body.appendChild(tip);
        document.querySelectorAll('[data-dl]').forEach((a) =>
            a.addEventListener('click', (e) => {
                e.preventDefault();
                tip.classList.remove('shake');
                void tip.offsetWidth;
                tip.classList.add('shake');
            }),
        );
    }

    fetch('/api/latest')
        .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
        .then((rel) => {
            const date = (rel.date || '').slice(0, 10);
            document.getElementById('heroMeta').textContent = `v${rel.version} · ${date} 更新${isIOS ? ' · 暂无 iOS 版' : ''}`;
            document.getElementById('footVer').textContent = `当前版本 v${rel.version}`;
            if (rel.notes?.length) document.getElementById('relNotes').textContent = `v${rel.version} 更新：${rel.notes.join('；')}`;
            for (const [plat, f] of Object.entries(rel.files || {})) {
                const card = document.querySelector(`.dl[data-plat="${plat}"]`);
                if (!card) continue;
                card.querySelector('[data-f=version]').textContent = 'v' + rel.version;
                card.querySelector('[data-f=size]').textContent = size(f.size);
                card.querySelector('[data-f=date]').textContent = date.slice(5) || '—';
                if (f.sha256) card.querySelector('[data-f=sha256]').textContent = 'SHA-256 ' + f.sha256;
            }
            const w = rel.files?.windows;
            const a = rel.files?.android;
            if (w) document.getElementById('ctaMainMeta').textContent = `x64 安装包 · ${size(w.size)}`;
            if (a) document.getElementById('ctaAltMeta').textContent = `arm64 APK · ${size(a.size)}`;
        })
        .catch(() => {
            document.getElementById('heroMeta').textContent = '安装包即将上传';
        });

    fetch('/api/stats')
        .then((r) => r.json())
        .then((s) => {
            const n = (s.windows || 0) + (s.android || 0);
            document.getElementById('dlCount').textContent = n.toLocaleString('zh-CN');
        })
        .catch(() => {});

    // 演示窗口：窄屏用手机尺寸，iframe 按容器宽度整体缩放
    const win = document.getElementById('demoWin');
    const body = document.getElementById('demoBody');
    const frame = document.getElementById('demo');
    const fit = () => {
        const phone = window.innerWidth < 640;
        win.classList.toggle('phone', phone);
        const w = phone ? 390 : 1200;
        frame.style.transform = `scale(${body.clientWidth / w})`;
    };
    fit();
    window.addEventListener('resize', fit);

    // 滚动进入
    const io = new IntersectionObserver(
        (entries) => entries.forEach((e) => { if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target); } }),
        { rootMargin: '0px 0px -8% 0px' },
    );
    document.querySelectorAll('main section:not(.hero) .glass, .sec-head').forEach((el, i) => {
        el.classList.add('reveal');
        el.style.transitionDelay = `${(i % 3) * 60}ms`;
        io.observe(el);
    });
})();
