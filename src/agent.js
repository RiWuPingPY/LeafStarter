// AI 助手：按脚本演示几个场景。正式版是本机 Rust 后端里的循环 + 工具注册表 + 权限分级，
// 这里只把「读东西直接读、动手前先问、能撤回的先建快照、发出去之前打码」的手感做出来
import { $, toast } from './ui.js';
import { conflictCrash } from './data.js';
import { agentPanel, agItem, agMeta } from './views/agentView.js';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const PREVIEW_CRASH = [
    '—— crash-reports/crash-2026-11-22_21.40.12-client.txt（节选）——',
    'Description: Rendering chunk',
    'java.lang.NullPointerException: Cannot invoke "class_4588.method_22918(...)" because "buffer" is null',
    '    at glimmer.render.FoliageHooks.emit(FoliageHooks.java:88)',
    '    at me.jellysquid.mods.sodium.client.render.chunk.compile.tasks.ChunkBuilderMeshingTask.execute(…:141)',
    '-- System Details --',
    'Java: 17.0.13 · 内存上限 4096 MB',
    '游戏目录: C:\\Users\\«用户名»\\Games\\.minecraft\\versions\\空岛生存',
    '启动参数: --username PingYi --uuid «已隐藏» --accessToken «已隐藏»',
].join('\n');

export function createAgent(api) {
    const { S } = api;
    const A = { open: false, items: [], busy: false, allowRev: false, tokens: 0, pending: null, seq: 0 };
    const motion = () => (document.documentElement.dataset.motion === 'reduced' ? 'auto' : 'smooth');

    // ---------- 面板 ----------
    function render() {
        $('#agent').innerHTML = agentPanel(api.ctx(), A);
        const b = $('#agBody');
        if (b) b.scrollTop = b.scrollHeight;
    }
    function markBtn() {
        const b = $('#aiBtn');
        if (!b) return;
        b.classList.toggle('on', A.open);
        b.setAttribute('aria-expanded', A.open);
    }
    function open(scn, arg) {
        api.closeDrawer();
        api.closePop(false);
        if (!A.open) {
            A.open = true;
            render();
            markBtn();
        }
        if (scn) start(scn, arg);
        else setTimeout(() => $('#agInput')?.focus(), 60);
    }
    function close() {
        if (!A.open) return false;
        A.open = false;
        markBtn();
        const el = $('#agent .drawer');
        if (!el) return true;
        el.classList.add('out');
        setTimeout(() => {
            if (!A.open) $('#agent').innerHTML = '';
        }, 170);
        return true;
    }
    function foot() {
        const m = $('#agMeta');
        if (m) m.innerHTML = agMeta(api.ctx(), A);
        const s = $('#agSend');
        if (s) s.disabled = A.busy || !api.aiReady();
    }
    function scrollEnd() {
        const b = $('#agBody');
        if (b) b.scrollTo({ top: b.scrollHeight, behavior: motion() });
    }

    // ---------- 时间线 ----------
    function add(it) {
        it.id = 'ag' + ++A.seq;
        A.items.push(it);
        const list = $('#agList');
        if (list) {
            if (A.items.length === 1) list.innerHTML = '';
            list.insertAdjacentHTML('beforeend', agItem(it));
            list.lastElementChild?.classList.add('in');
            scrollEnd();
        }
        return it;
    }
    function drop(it) {
        A.items = A.items.filter((x) => x !== it);
        $('#' + it.id)?.remove();
    }
    function update(it) {
        const el = $('#' + it.id);
        if (el) el.outerHTML = agItem(it);
    }
    const spend = (n) => {
        A.tokens += n;
        foot();
    };
    const user = (text) => add({ kind: 'user', text });
    const say = (html) => {
        spend(600 + html.length * 3);
        return add({ kind: 'say', html });
    };
    async function think(ms) {
        const t = add({ kind: 'think' });
        await sleep(ms);
        drop(t);
    }
    async function tool(level, title, detail, { ms = 650, preview, fn } = {}) {
        const it = add({ kind: 'tool', level, title, detail, state: 'run', preview: S.aiPreview ? preview : null });
        await sleep(ms);
        fn?.();
        it.state = 'done';
        update(it);
        spend(preview ? 1800 : 500);
        return it;
    }
    // 动手之前问一句。「可撤回」那一级可以「这次会话都允许」，后面同级的就不再问
    function confirm(o) {
        if (o.level === 'rev' && A.allowRev) {
            add({ kind: 'note', text: `这次会话里「可撤回」的都允许了：${o.title}` });
            return Promise.resolve(true);
        }
        const it = add({ kind: 'confirm', state: 'wait', ...o });
        scrollEnd();
        return new Promise((res) => { it.resolve = res; });
    }
    function answer(id, ok, all) {
        const it = A.items.find((x) => x.id === id);
        if (!it || it.state !== 'wait') return;
        if (all) A.allowRev = true;
        it.state = ok ? (all ? 'all' : 'yes') : 'no';
        update(it);
        it.resolve(ok);
    }
    async function waitRun(id) {
        for (let k = 0; k < 60; k++) {
            await sleep(300);
            const r = api.RUN.get(id);
            if (r?.phase === 'running') return true;
            if (r?.phase === 'crashed') return false;
        }
        return false;
    }

    // ---------- 场景 ----------
    const SCN = {
        async crash(arg = {}) {
            const i = api.byId(arg.id) || api.byId('sky');
            user(arg.text || `「${i.name}」刚才崩了，帮我查查`);
            await think(600);
            await tool('read', '读取崩溃报告', 'crash-reports/crash-2026-11-22_21.40.12-client.txt', { preview: PREVIEW_CRASH });
            await tool('read', '读取日志', 'logs/latest.log 最后 400 行');
            await tool('read', '列出 Mod', `${i.mods.length} 个 Mod 的版本和前置关系`);
            await tool('read', '查 Modrinth', 'Glimmer Render 的版本记录和更新说明', { ms: 800 });
            await think(900);
            if (!conflictCrash(i)) {
                say('<p>上次那次崩溃是 <b>Glimmer Render 0.4.2</b> 和 <b>Sodium</b> 冲突。现在 Glimmer Render 已经更新或者禁用了，照现在的组合启动应该不会再崩。</p>');
                return;
            }
            say('<p><b>原因找到了。</b>Glimmer Render 0.4.2 和 Sodium 0.5.11 都在改区块渲染：Sodium 换掉了原版的顶点缓冲，Glimmer Render 还在往旧的缓冲里写，拿到的是空的，所以崩在 <span class="mono">FoliageHooks.java:88</span>。</p><p>Glimmer Render 0.4.3 的更新说明里写着「修复与 Sodium 0.5.10 及以上的兼容问题」。比禁用它好：功能还在。</p>');
            const ok = await confirm({ level: 'rev', title: '把 Glimmer Render 更新到 0.4.3', detail: '改之前先给 mods 文件夹建快照，随时能一键撤回。' });
            if (!ok) return say('<p>好，先不动。想自己处理，就在「Mod」里更新或者禁用 Glimmer Render；也可以用「二分查冲突」再确认一遍。</p>');
            await tool('rev', '建快照', '「AI 修复前」· mods 文件夹 24 个文件', { ms: 450 });
            await tool('rev', '更新 Mod', 'Glimmer Render 0.4.2 → 0.4.3（从 Modrinth 下载，校验哈希）', { ms: 800, fn: () => api.fixGlimmer(i) });
            const go = await confirm({ level: 'ext', title: `启动「${i.name}」验证`, detail: '进到主菜单、世界加载完不崩，就算修好了。' });
            if (!go) return say('<p>好。你方便的时候自己启动试试，还崩就再叫我。</p>');
            const t = add({ kind: 'tool', level: 'ext', title: '启动游戏', detail: '等它进主菜单……', state: 'run' });
            api.launch(i.id);
            const okRun = await waitRun(i.id);
            t.state = okRun ? 'done' : 'fail';
            t.detail = okRun ? '进到主菜单了，世界也加载完了' : '又崩了';
            update(t);
            say(okRun
                ? '<p><b>修好了</b>，没再崩。快照「AI 修复前」留着，想退回去，在提示里点撤销，或者到「Mod」页把 Glimmer Render 改回去。</p>'
                : '<p>还是崩了。我把两次的日志整理成一份报告，你可以发给 Glimmer Render 的作者；先禁用它能玩。</p>');
        },

        async server(arg = {}) {
            const i = api.byId(arg.id) || api.mp.failedSetup();
            user(arg.text || '开服卡在一步上了，帮我看看');
            await think(600);
            const s = i?.srv;
            const x = s?.steps[s.k];
            if (!x || x.state !== 'fail') {
                say('<p>现在没有卡住的开服剧本。开服时哪一步失败了，点那一步旁边的「交给 AI 看看」就行。</p>');
                return;
            }
            await tool('read', '读取剧本输出', `第 ${s.k + 1} 步「${x.t}」· 最后 30 行`, {
                preview: [`—— 剧本第 ${s.k + 1} 步：${x.t}（最后 3 行）——`, ...x.failOut, `服务器：root@«${s.ip.replace(/\.\d+$/, '.x')}»    登录：密钥（«在保管库里»）`].join('\n'),
            });
            await tool('read', '看前几步的结果', '登录、开端口、识别系统都正常');
            await think(800);
            say('<p><b>是软件源的问题，不是服务器坏了。</b>这台机器默认的软件源 <span class="mono">mirrors.cloud.example</span> 连续超时 3 次；前面几步都正常，说明机器本身是好的。</p><p>换成清华大学的镜像源，再装一次 Java 就行。</p>');
            const cmd = [
                'cp /etc/apt/sources.list /etc/apt/sources.list.bak',
                "sed -i 's#http://mirrors.cloud.example#https://mirrors.tuna.tsinghua.edu.cn#g' /etc/apt/sources.list",
                `apt-get update && apt-get install -y openjdk-${i.javaNeed}-jre-headless`,
            ];
            const ok = await confirm({ level: 'ext', title: `在服务器 ${s.ip} 上执行 3 条命令`, detail: '会改软件源配置；第一条先把原来的备份成 sources.list.bak。', cmd });
            if (!ok) return say('<p>好。你也可以照剧本里「怎么手动处理」自己换源，然后点「重试这一步」。</p>');
            await tool('ext', '在服务器上执行', `换软件源、装 Java ${i.javaNeed}`, { ms: 1500 });
            say(`<p>Java ${i.javaNeed} 装好了（${i.javaNeed}.0.13）。剧本从下一步接着走，你可以在「联机」里看着。</p>`);
            api.mp.resumeSetup(i);
        },

        async pack(arg = {}) {
            user(arg.text || '配一个整合包：1.20.1 Fabric，科技向，轻薄本也能跑');
            await think(700);
            await tool('read', '搜 Modrinth', '科技 · Fabric · 1.20.1 · 按下载量');
            await tool('read', '查兼容和前置', '12 个候选，排除 3 个和 1.20.1 不兼容的');
            await think(800);
            say(`<p>按「科技向、轻薄本能跑」挑了 5 个，再加上流畅优先那 6 个：</p><ul class="ag-ul">
                <li><b>Create Fabric</b>：机械动力，科技向的主菜</li>
                <li><b>Tech Reborn</b>：电力和机器</li>
                <li><b>Applied Energistics 2</b>：物流和存储</li>
                <li><b>EMI</b>、<b>Jade</b>：查配方、看方块信息</li>
                <li><b>Sodium</b>、<b>Lithium</b>、<b>FerriteCore</b> 等 6 个：流畅优先那一组，轻薄本也能跑</li></ul>
                <p>前置（Fabric API、Reborn Core）会自动带上。</p>`);
            const ok = await confirm({ level: 'rev', title: '新建实例「科技向 1.20.1」并装上这 11 个 Mod', detail: '不满意直接删掉这个实例就行（进回收站）。' });
            if (!ok) return say('<p>好。想换几个 Mod 就告诉我，比如「不要 AE2，换成精致存储」。</p>');
            await tool('rev', '新建实例', '科技向 1.20.1 · Fabric 0.16.9', { ms: 500, fn: () => api.createPack() });
            say('<p>开始装了，左边能看到进度。装好按 <kbd>Enter</kbd> 就能玩。</p>');
        },

        async perf(arg = {}) {
            const i = api.byId('survival') || api.cur();
            user(arg.text || `让「${i.name}」更流畅一点`);
            await think(600);
            await tool('read', '看这台电脑', '16 GB 内存 · 核显 Intel Iris Xe · 笔记本，插着电');
            await tool('read', '看实例设置', `流畅优先${i.perf ? '已开' : '没开'} · 内存自动（4 GB）· 视距 12`);
            await think(700);
            say(`<p>${i.perf ? '流畅优先那组 Mod 已经装了。' : ''}剩下能调的：</p><ul class="ag-ul">
                <li>视距 12 对核显偏高，<b>改成 10</b> 帧数会稳很多，远处看着差别不大</li>
                <li>内存给 4 GB 够用，再加反而让垃圾回收变慢，不用动</li></ul>`);
            const ok = await confirm({ level: 'rev', title: `把「${i.name}」的视距从 12 改成 10`, detail: '改的是 options.txt，改之前先备份一份；不满意随时改回去。' });
            if (!ok) return say('<p>好，不动。</p>');
            await tool('rev', '改 options.txt', 'renderDistance: 12 → 10（备份在 options.txt.bak）', { ms: 450 });
            say('<p>改好了，下次启动生效。</p>');
        },
    };

    function start(scn, arg) {
        if (!api.aiReady()) {
            A.pending = [scn, arg];
            if (!A.open) open();
            else render();
            return;
        }
        if (A.busy) return toast('AI 还在处理上一件事');
        A.busy = true;
        foot();
        SCN[scn](arg).finally(() => {
            A.busy = false;
            foot();
        });
    }

    function send() {
        const ta = $('#agInput');
        const text = ta?.value.trim();
        if (!text || A.busy) return;
        ta.value = '';
        const cur = api.cur();
        if (/崩|闪退|报错|crash/i.test(text)) return start('crash', { text, id: cur?.crashDemo === 'conflict' ? cur.id : 'sky' });
        if (/整合包|配一/.test(text)) return start('pack', { text });
        if (/流畅|卡顿|帧|fps/i.test(text)) return start('perf', { text });
        if (/开服|服务器|软件源/.test(text) && api.mp.failedSetup()) return start('server', { text });
        A.busy = true;
        foot();
        (async () => {
            user(text);
            await think(700);
            say('<p>原型里我只会演示几个固定场景：查崩溃、开服卡住、配整合包、调流畅。正式版这里可以随便问，比如「为什么进服务器卡」「这两个 Mod 能一起装吗」。</p>');
        })().finally(() => {
            A.busy = false;
            foot();
        });
    }

    function demo() {
        api.aiDemo();
        api.save();
        render();
        if (S.page === 'settings') api.paintView();
        else api.paintBody();
        toast('填了一个示例 Key（原型里不真的调模型）');
        if (A.pending) {
            const [s, a] = A.pending;
            A.pending = null;
            start(s, a);
        }
    }

    return {
        A,
        open,
        close,
        rerender: () => A.open && render(),
        send,
        actions: {
            'ai-open': () => (A.open ? close() : open()),
            'ai-close': () => close(),
            'ai-crash': () => open('crash', { id: S.inst }),
            'ai-demo': () => demo(),
            'ai-settings': () => {
                close();
                api.goPage('settings');
                requestAnimationFrame(() => $('#aiPanel')?.scrollIntoView({ block: 'start', behavior: motion() }));
            },
            'ag-sug': (el) => start(el.dataset.v, {}),
            'ag-send': () => send(),
            'ag-yes': (el) => answer(el.dataset.id, true),
            'ag-all': (el) => answer(el.dataset.id, true, true),
            'ag-no': (el) => answer(el.dataset.id, false),
        },
    };
}
