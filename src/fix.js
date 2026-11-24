// 二分查冲突：不用 AI。先建快照，每轮禁用一半启动到主菜单，看还崩不崩；有前置关系的一起进退
import { $, toast } from './ui.js';
import { conflictCrash } from './data.js';
import { tileState, bisectHead, bisectLog, bisectFoot, bisectFound } from './views/fix.js';

export function createFix(api) {
    const { S } = api;
    let timer = null;
    const here = (i) => S.page === 'home' && S.inst === i.id && S.tab === 'overview';
    const key = (m) => m.unit || m.id;

    function start(i) {
        if (!i || i.bisect) return;
        const mods = i.mods.filter((m) => m.on && m.name !== 'Fabric API');
        const units = [...new Set(mods.map(key))];
        i.bisect = { mods, suspects: units, testing: null, log: [], found: null, phase: 'run', snap: i.mods.map((m) => [m, m.on]) };
        if (here(i)) api.paintBody();
        else api.setTab('overview');
        toast('给 mods 文件夹建了快照「二分前」，开始一轮一轮地试');
        timer = setTimeout(() => round(i), 600);
    }

    function round(i) {
        const b = i.bisect;
        if (!b) return;
        if (b.suspects.length === 1) return verify(i);
        const half = b.suspects.slice(0, Math.ceil(b.suspects.length / 2));
        b.testing = half;
        const e = { n: b.mods.filter((m) => half.includes(key(m))).length, state: 'run' };
        b.log.push(e);
        paint(i);
        // 模拟：禁用这一半启动到主菜单
        timer = setTimeout(() => {
            const crashed = conflictCrash({ ...i, mods: i.mods.map((m) => ({ ...m, on: m.on && !half.includes(key(m)) })) });
            e.state = crashed ? 'crash' : 'ok';
            b.suspects = crashed ? b.suspects.filter((k) => !half.includes(k)) : half;
            b.testing = null;
            paint(i);
            timer = setTimeout(() => round(i), 700);
        }, 1300);
    }

    function verify(i) {
        const b = i.bisect;
        const k = b.suspects[0];
        const m = b.mods.find((x) => key(x) === k);
        b.testing = [k];
        const e = { verify: true, name: m.name, state: 'run' };
        b.log.push(e);
        paint(i);
        timer = setTimeout(() => {
            e.state = 'ok';
            b.testing = null;
            b.found = k;
            b.phase = 'done';
            paint(i);
        }, 1100);
    }

    // 方块墙原地换状态（有过渡），整张卡不重画
    function paint(i) {
        if (!here(i)) return;
        const card = $('#bisectCard');
        if (!card) return api.paintBody();
        const b = i.bisect;
        card.querySelectorAll('.bt[data-m]').forEach((el) => {
            const m = b.mods.find((x) => x.id === el.dataset.m);
            if (m) el.className = `bt ${tileState(b, m)}`;
        });
        $('#bHead').innerHTML = bisectHead(b);
        $('#bLog').innerHTML = bisectLog(b);
        $('#bFoot').innerHTML = bisectFoot(b, i);
        const f = $('#bFound');
        f.innerHTML = bisectFound(b, i);
        f.hidden = b.phase !== 'done';
    }

    function restore(i) {
        clearTimeout(timer);
        i.bisect.snap.forEach(([m, on]) => { m.on = on; });
        i.bisect = null;
    }

    function apply(i) {
        const b = i.bisect;
        if (!b?.found) return;
        const k = b.found;
        restore(i);
        const off = i.mods.filter((m) => key(m) === k);
        off.forEach((m) => { m.on = false; });
        api.RUN.delete(i.id);
        api.refresh(i.id, true);
        toast(`只禁用了 ${off.map((m) => m.name).join('、')}，其他都恢复了`, {
            undo: () => {
                off.forEach((m) => { m.on = true; });
                if (api.cur() === i) api.paintView();
            },
        });
    }

    function cancel(i) {
        if (!i?.bisect) return;
        restore(i);
        api.refresh(i.id, true);
        toast('停下了，Mod 全部恢复成二分前的样子');
    }

    return {
        start,
        stop: (i) => i?.bisect && restore(i),
        actions: {
            'bisect-start': () => start(api.cur()),
            'bisect-apply': () => apply(api.cur()),
            'bisect-cancel': () => cancel(api.cur()),
        },
    };
}
