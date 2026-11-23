// 出错就修好：规则认出来的直接给修复按钮；认不出来的给「让 AI 查」和「二分查冲突」（不用 AI）
import { esc } from '../ui.js';
import { icon } from '../icons.js';

export function crashBanner(c) {
    const i = c.inst;
    if (i.bisect) return bisectCard(i);
    if (c.RUN.get(i.id)?.phase !== 'crashed') return '';
    if (i.diag) {
        return `<div class="banner" role="alert">${icon('alert', 20)}<div class="tx"><b>${esc(i.diag.title)}<span class="rule-tag">规则认出来的 · 不用 AI</span></b><p>${esc(i.diag.detail)}</p></div>
            <div class="actions"><button type="button" class="btn pri" id="crashFix" data-act="crash-fix">${icon('wrench')}${esc(i.diag.fix)}</button><button type="button" class="btn ghost" data-act="crash-dismiss">先不管</button></div></div>`;
    }
    if (i.crashDemo === 'conflict') {
        return `<div class="banner" role="alert">${icon('alert', 20)}<div class="tx"><b>游戏崩溃了，规则没认出原因</b><p>渲染时出了空指针错误，堆栈里同时有 Glimmer Render 和 Sodium。现有规则对不上，没法直接给修复按钮。</p></div>
            <div class="actions">
                <button type="button" class="btn pri" id="crashFix" data-act="ai-crash">${icon('sparkles')}让 AI 查${c.aiReady() ? '' : '（要先填 Key）'}</button>
                <button type="button" class="btn" data-act="bisect-start">${icon('split')}二分查冲突 Mod（不用 AI）</button>
                <button type="button" class="btn ghost" data-act="crash-dismiss">先不管</button>
            </div></div>`;
    }
    return '';
}

// 方块墙：每个 Mod 一格，二分时一轮一轮暗下去，最后只剩捣乱的那个
export function tileState(b, m) {
    const u = m.unit || m.id;
    if (b.found === u) return 'culprit';
    if (b.testing?.includes(u)) return 'off';
    if (b.suspects.includes(u)) return 'sus';
    return 'clear';
}

export function bisectHead(b) {
    if (b.found && b.phase === 'done') return '<span class="chip ok">找到了</span>';
    return `<span class="chip acc"><i class="dot busy"></i>第 ${Math.max(1, b.log.length)} 轮 · 还剩 ${b.suspects.length} 组怀疑</span>`;
}

export function bisectLog(b) {
    return b.log.map((e, k) => {
        if (e.verify) {
            return `<li class="${e.state}"><b>确认</b>只禁用 ${esc(e.name)}，启动到主菜单${e.state === 'run' ? '……' : ' → 没崩，就是它'}</li>`;
        }
        const res = e.state === 'run' ? '启动到主菜单……' : e.state === 'ok' ? '没崩 → 问题在禁用的这一半' : '还是崩 → 问题在没禁用的那一半';
        return `<li class="${e.state}"><b>第 ${k + 1} 轮</b>禁用 ${e.n} 个 → ${res}</li>`;
    }).join('');
}

export function bisectFoot(b, i) {
    if (b.phase !== 'done') return '<button type="button" class="btn ghost" data-act="bisect-cancel">停下，全部恢复</button>';
    const m = i.mods.find((x) => (x.unit || x.id) === b.found);
    return `<button type="button" class="btn pri" data-act="bisect-apply">${icon('check')}恢复快照，只禁用 ${esc(m?.name || '它')}</button>
        <button type="button" class="btn" data-act="ai-crash">${icon('sparkles')}问 AI 有没有不用禁用的办法</button>
        <button type="button" class="btn ghost" data-act="bisect-cancel">全部恢复，先不管</button>`;
}

export function bisectFound(b, i) {
    if (b.phase !== 'done') return '';
    const m = i.mods.find((x) => (x.unit || x.id) === b.found);
    return `${icon('check', 15)}<span><b>${esc(m?.name || '')}</b> 在场就崩，拿掉就好。${b.log.length - 1} 轮就从 ${b.mods.length} 个 Mod 里找出来了。</span>`;
}

function bisectCard(i) {
    const b = i.bisect;
    const tiles = b.mods.map((m) => `<span class="bt ${tileState(b, m)}" data-m="${m.id}" style="--h:${m.h}" title="${esc(m.name)}">${esc(m.name[0])}</span>`).join('');
    return `<section class="card bisect" id="bisectCard">
        <h3><span class="ico violet">${icon('split', 16)}</span>二分查冲突 Mod<span id="bHead">${bisectHead(b)}</span></h3>
        <p class="note">不用 AI：先给 mods 文件夹建快照，每轮禁用一半启动到主菜单，看还崩不崩。有前置关系的一起进退；Fabric API 是大家的前置，一直开着。</p>
        <div class="btiles" id="bTiles">${tiles}</div>
        <div class="blegend"><span><i class="bt sus"></i>还在怀疑</span><span><i class="bt off"></i>这一轮禁用</span><span><i class="bt clear"></i>排除了</span><span><i class="bt culprit"></i>就是它</span></div>
        <ol class="brounds" id="bLog">${bisectLog(b)}</ol>
        <p class="bfound" id="bFound"${b.phase === 'done' ? '' : ' hidden'}>${bisectFound(b, i)}</p>
        <div class="actions" id="bFoot">${bisectFoot(b, i)}</div>
    </section>`;
}
