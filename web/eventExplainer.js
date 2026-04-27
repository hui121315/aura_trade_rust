/* ===========================================================
 * eventExplainer.js  —  事件点击解释器
 *
 * 为右侧面板（最近事件 / 葛南维信号 / 均线交叉 / K 线形态）提供
 * 统一的"点击 → 图表定位 + 弹出解释"能力。
 *
 * 对外 API（挂在 window.AuraExplainer）:
 *   explain(type, payload)   触发解释；type 取值:
 *     - 'granville'  payload: { rule, timeMs, price, bar }
 *     - 'cross'      payload: { kind('Golden'|'Death'), fast, slow, timeMs, price, bar }
 *     - 'trap'       payload: { kind('Bull'|'Bear'), timeMs, price, bar, breakoutBar, reversalBar }
 *     - 'advMa'      payload: { kind('Guillotine'|'PoissonSpider'|...), timeMs, price, bar }
 *     - 'confluence' payload: { centerPrice, uniqueKinds, strengthMultiplier, timeMs }
 *     - 'pattern'    payload: { label, direction, strength, timeMs, price, bar }
 *   close()                  关闭浮层
 *
 * 依赖（可选）:
 *   window.__AuraTradeApi.scrollToTime(timeSec)
 *   window.__AuraTradeApi.highlightBar(timeSec, label, direction)
 *
 * 知识字典设计：保持精简（summary ≤ 150 字），深度解读跳到 /knowledge.html。
 * ========================================================= */

(() => {
  'use strict';

  // ---------- 知识字典 ----------
  const KB = {
    // ---- 葛南维 8 法则 ----
    granville: {
      B1BreakoutBuy: {
        title: 'B1 突破买入（第一买点）',
        direction: 1,
        summary: '均线从下降转为<b>走平或翻多</b>，价格从下方<b>突破均线</b>。这是趋势反转的第一个买入信号 —— 熊转牛的起点。',
        how: '均线斜率由负转正（或接近 0）+ 价格上穿均线 + 收盘站稳。通常配合放量更可靠。',
        tip: '仓位 30-50%，止损设均线下方 2×ATR。突破当根 <b>放量 ≥ 1.5× 均量</b> 是最佳买点。',
        mistakes: '只看价格穿均线、不看均线斜率 —— 在下跌中继会被反复打脸。',
        kbId: 'ma-l1',
      },
      B2PullbackBuy: {
        title: 'B2 回踩买入（第二买点）',
        direction: 1,
        summary: '上升趋势中价格<b>回调到均线附近</b>后反弹 —— 是最稳定可复制的买点。"趋势中的回调 = 主力给你上车的机会"。',
        how: '均线向上 + 价格回到均线 ±1% 范围 + 出现阳线反弹 + 放量配合。',
        tip: '仓位 50-70%（牛市加仓机会）。止损设均线下方 1.5×ATR。止盈瞄准前期 Swing High。',
        mistakes: '均线本身已转平 / 转空时仍视作 B2 —— 那是 S1 下跌的开始。',
        kbId: 'ma-l2',
      },
      B3FalseBreakBuy: {
        title: 'B3 假跌破买入（回踩不破）',
        direction: 1,
        summary: '上升趋势中价格<b>短暂跌破均线</b>但立即收回 —— 主力"洗盘"的经典信号。是强势趋势中的加仓机会。',
        how: '均线仍向上 + 价格盘中跌破但收盘拉回 + 跌破幅度 ≤ 3% + 快速反弹。',
        tip: '仓位 40-60%。止损设假跌破当根最低点。R:R 通常能到 3:1。',
        mistakes: '把 B3 和真跌破（S1）混淆。关键是"收盘是否收回均线上方"。',
        kbId: 'ma-l3',
      },
      B4DivergenceBuy: {
        title: 'B4 乖离买入（超跌反弹）',
        direction: 1,
        summary: '下跌过程中价格<b>极度偏离均线</b>（BIAS 达 -8% 到 -12%+），短期必然向均线回归 —— 抢反弹机会。',
        how: 'BIAS = (价格 - MA) / MA × 100%。BIAS < -8% 视为超跌。加密可到 -12%~-20%。',
        tip: '短线机会，仓位 20-40%。止损 2% 硬止损。止盈回到均线附近即可。',
        mistakes: '在下跌趋势中把 B4 当成反转买点长期持有。它只是短线反弹。',
        kbId: 'ma-l4',
      },
      S1BreakdownSell: {
        title: 'S1 跌破卖出（第一卖点）',
        direction: -1,
        summary: '均线从上升转为<b>走平或翻空</b>，价格从上方<b>跌破均线</b>。是顶部反转的第一个卖出信号 —— 牛转熊的起点。',
        how: '均线斜率由正转负 + 价格下穿均线 + 收盘确认。通常也配合放量。',
        tip: '立即减仓 50% 以上。止损设均线上方 2×ATR。若趋势确认，清仓观望。',
        mistakes: '期望"只是洗盘" —— 把 S1 当 B3 会让你错过最佳离场窗口。',
        kbId: 'ma-l5',
      },
      S2ReboundSell: {
        title: 'S2 反弹卖出（第二卖点）',
        direction: -1,
        summary: '下降趋势中价格<b>反弹到均线附近</b>后受压回落。是逃顶 / 做空的机会。',
        how: '均线向下 + 价格反弹到均线 ±1% + 出现阴线 + 放量配合。',
        tip: '清仓或开空，仓位 50-70%。止损设均线上方 1.5×ATR。',
        mistakes: '把 S2 当作"反弹继续"而追涨，结果被均线压制。',
        kbId: 'ma-l6',
      },
      S3FalseBreakSell: {
        title: 'S3 假突破卖出（反弹不过）',
        direction: -1,
        summary: '下降趋势中价格<b>短暂突破均线</b>但立即回落 —— 是下跌中继的"反抽出货"陷阱。',
        how: '均线仍向下 + 价格盘中突破但收盘拉回 + 突破幅度 ≤ 3% + 快速回落。',
        tip: '开空机会，止损设假突破当根最高点。R:R 可达 3:1。',
        mistakes: '把 S3 和真突破（B1）混淆。关键：看均线斜率。',
        kbId: 'ma-l7',
      },
      S4DivergenceSell: {
        title: 'S4 乖离卖出（超涨回落）',
        direction: -1,
        summary: '上涨过程中价格<b>极度偏离均线</b>（BIAS 达 +8% 到 +12%+），短期必然向均线回归 —— 止盈/做空机会。',
        how: 'BIAS > +8% 视为超涨。加密市场可到 +12%~+20%。配合顶背离更可靠。',
        tip: '减仓 / 止盈机会，仓位 20-40% 做空。止损 2% 硬止损。',
        mistakes: '在强势趋势中 BIAS 可以长期维持高位（"超买钝化"），单纯靠 S4 做空容易被止损。',
        kbId: 'ma-l8',
      },
    },

    // ---- 均线交叉 ----
    cross: {
      Golden: {
        title: '金叉',
        direction: 1,
        summary: '<b>快速均线上穿慢速均线</b>，表示短期动能超过长期。是经典的买入信号。<br>MA5×MA10 快线金叉用于短线；MA50×MA200 黄金交叉用于大趋势。',
        how: '快线从下方向上穿越慢线 + 价格站稳快线上方。',
        tip: '金叉后若配合放量 + 多头排列则最强。短线金叉仓位 30%，长线黄金交叉仓位 50%+。',
        mistakes: '金叉后 1-2 根再反死叉（假金叉）—— 震荡市常发生。建议等回踩确认。',
        kbId: 'ma-cross',
      },
      Death: {
        title: '死叉',
        direction: -1,
        summary: '<b>快速均线下穿慢速均线</b>，表示短期动能转弱。是经典的卖出信号。<br>MA50×MA200 死亡交叉是熊市的最强确认。',
        how: '快线从上方向下穿越慢线 + 价格跌破快线。',
        tip: '死叉后减仓 30-70%。若是慢周期（MA50×MA200）死叉则清仓观望。',
        mistakes: '死叉后快速反金叉（假死叉）—— 同样等确认。',
        kbId: 'ma-cross',
      },
    },

    // ---- 陷阱 ----
    trap: {
      Bull: {
        title: '多头陷阱（Bull Trap）',
        direction: -1,
        summary: '价格向上<b>假突破</b>阻力位，吸引多头追涨，然后迅速回落 —— 是主力"诱多出货"的经典手法。',
        how: '价格突破近期高点 / 阻力 → 1-3 根内快速回落 → 收盘回到突破前水平。',
        tip: '确认后<b>反向做空</b>或清仓。止损设在假突破的最高点。',
        mistakes: '追涨假突破 —— 新手最常见的死法。突破必须等回踩确认才可信。',
        kbId: 'sig-traps',
      },
      Bear: {
        title: '空头陷阱（Bear Trap）',
        direction: 1,
        summary: '价格向下<b>假跌破</b>支撑位，吸引空头追空，然后迅速反弹 —— 是主力"诱空吸筹"的典型操作。',
        how: '价格跌破近期低点 / 支撑 → 1-3 根内快速反弹 → 收盘回到跌破前水平。',
        tip: '确认后<b>反向做多</b>。止损设在假跌破的最低点。这是威科夫的"Spring"信号。',
        mistakes: '看到跌破就做空 —— 被扫损是常态。耐心等收盘确认。',
        kbId: 'sig-traps',
      },
    },

    // ---- 高级均线事件 ----
    advMa: {
      Guillotine: {
        title: '断头铡刀（最强空头形态）',
        direction: -1,
        summary: '一根<b>长阴线同时跌破 MA5 / MA10 / MA20 / MA60</b> 多条均线。是均线技术分析中<b>最强的空头信号</b>（胜率 > 75%）。',
        how: '单根 K 线实体覆盖 4 条以上均线 + 高位出现 + 放量。',
        tip: '立即清仓，不抄底。止损设断头当根最高点。通常预示 15-30% 的后续跌幅。',
        mistakes: '把断头当作"超跌反弹机会" —— 这是错读形态，断头后往往还有大跌。',
        kbId: 'ma-guillotine',
      },
      PoissonSpider: {
        title: '毒蜘蛛（放量死叉）',
        direction: -1,
        summary: 'MA5 / MA10 / MA20 <b>在高位死叉同时放量</b>，像蜘蛛网一样笼罩住价格。是顶部反转的强信号。',
        how: '3 条短期均线在高位纠缠后死叉 + 死叉当根放量 ≥ 1.5× 均量。',
        tip: '减仓 70% 以上。下一波下跌通常 10-20%。',
        mistakes: '高位震荡时也会出现短暂死叉但未放量 —— 关键看量能。',
        kbId: 'ma-poisson-spider',
      },
      HangingScallions: {
        title: '旱地拔葱（强势突破）',
        direction: 1,
        summary: '价格从均线密集区<b>单根长阳突破</b>多条均线 —— 是强势启动的信号，通常预示一波中级行情。',
        how: '均线粘合 + 长阳线实体 ≥ 1.5×ATR + 收盘突破最上方均线 + 放量。',
        tip: '追涨或回踩加仓。仓位 50-70%。止损设拔葱当根最低点。',
        mistakes: '没放量的拔葱容易假突破。必须配合量能。',
        kbId: 'ma-hanging-scallions',
      },
      BondUpwardDiverge: {
        title: '再次粘合向上发散',
        direction: 1,
        summary: '均线粘合后第二次向上发散 —— 比第一次发散更可靠，主升浪的标志。',
        how: '之前已有一次粘合 + 发散 → 回调粘合 → 再次向上发散。',
        tip: '加仓或建仓机会，仓位 50-80%。第二次发散的行情通常更持久。',
        mistakes: '把第一次发散当作第二次 —— 要看历史图，必须是"第二次"。',
        kbId: 'ma-bond-pattern',
      },
    },

    // ---- 合流 ----
    confluence: {
      default: {
        title: '多合一共振（Confluence）',
        direction: 0,
        summary: '<b>多种独立信号（MA / 趋势线 / Fibonacci / 形态等）在同一价位附近重合</b>，形成高可靠度的关键点位。',
        how: '≥ 3 类组件 + 价格容差 < 0.5% + 强度倍数 > 1.5×。',
        tip: '这是<b>最高级别</b>的入场 / 出场点。按合流方向操作，仓位可加大 20-30%。',
        mistakes: '盲目追随合流位而忽略大趋势方向 —— 合流也要顺势。',
        kbId: 'sig-confluence',
      },
    },

    // ---- K 线形态 ----
    pattern: {
      '锤头': {
        title: '锤头线（Hammer）', direction: 1,
        summary: '下影长 ≥ 2× 实体，上影极短 —— 是<b>底部反弹</b>的经典信号。尤其在下跌末期出现时最有意义。',
        how: '下影至少是实体的 2 倍 + 上影几乎没有 + 实体较小（颜色次要）。',
        tip: '下跌末期 + 锤头 + 次日阳线确认 = 反弹买点。仓位 30%，止损锤头最低点。',
        mistakes: '上升中途出现的锤头意义不大 —— 要在明显下跌后才有反转含义。',
        kbId: 'ck-single',
      },
      '流星': {
        title: '流星线（Shooting Star）', direction: -1,
        summary: '上影长 ≥ 2× 实体，下影极短 —— 是<b>顶部见顶</b>的经典警示。高位出现尤其重要。',
        how: '上影至少是实体的 2 倍 + 下影极短 + 实体较小。',
        tip: '高位 + 流星 + 次日阴线确认 = 减仓信号。至少减 50%。',
        mistakes: '把流星和倒锤头混淆 —— 倒锤头在下跌末期、流星在上涨末期。',
        kbId: 'ck-single',
      },
      '十字星': {
        title: '十字星（Doji）', direction: 0,
        summary: '开盘价 ≈ 收盘价，实体极小。代表<b>市场犹豫</b>。在关键位置是变盘信号。',
        how: '实体 ≤ ATR 的 5%，上下影正常或较长。',
        tip: '十字星本身方向中性，关键看<b>下一根 K 线确认方向</b>。',
        mistakes: '过早下结论。等确认 K 线。',
        kbId: 'ck-single',
      },
      '大阳线': {
        title: '大阳线', direction: 1,
        summary: '长阳实体（≥ 1.5× ATR）+ 小影线。是强势推动的体现 —— 多头掌控。',
        how: '实体 ≥ 1.5× 平均实体 + 上下影 ≤ 实体的 10%。',
        tip: '强势买方主导。顺势持多，止损设大阳开盘价。',
        mistakes: '追高买入 —— 大阳后常有短期回调，等回踩再进。',
        kbId: 'ck-single',
      },
      '大阴线': {
        title: '大阴线', direction: -1,
        summary: '长阴实体 + 小影线。是强势抛压的体现 —— 空头掌控。',
        how: '实体 ≥ 1.5× 平均实体 + 上下影 ≤ 实体的 10%。',
        tip: '强势卖方主导。清仓或反手做空，止损设大阴开盘价。',
        mistakes: '想抄底 —— 大阴后常有延续下跌。',
        kbId: 'ck-single',
      },
      '上吊线': {
        title: '上吊线（Hanging Man）', direction: -1,
        summary: '形状像锤头但出现在<b>高位</b> —— 是见顶警示（锤头在底部是看涨，在顶部是看跌）。',
        how: '下影 ≥ 2× 实体 + 上影极短 + 在明显上升趋势的高位。',
        tip: '等下一根阴线确认，然后减仓。',
        mistakes: '只看形状不看位置 —— 位置决定方向。',
        kbId: 'ck-single',
      },
      '倒锤头': {
        title: '倒锤头（Inverted Hammer）', direction: 1,
        summary: '形状像流星但出现在<b>下跌末期</b> —— 反转尝试信号。比锤头稍弱。',
        how: '上影 ≥ 2× 实体 + 下影极短 + 在明显下跌趋势的低位。',
        tip: '需次日阳线确认才建仓。仓位 20-30%。',
        mistakes: '没等确认就买 —— 倒锤头假信号较多。',
        kbId: 'ck-single',
      },
      '看涨吞没': {
        title: '看涨吞没（Bullish Engulfing）', direction: 1,
        summary: '阳线实体<b>完全吞没</b>前一根阴线实体 —— 多方决定性反扑。底部最可靠的反转信号之一。',
        how: '前阴后阳 + 阳线实体 > 阴线实体 + 阳线开盘 < 阴线收盘 + 阳线收盘 > 阴线开盘。',
        tip: '底部吞没 = 强买入信号。仓位 50%，止损吞没最低点。',
        mistakes: '上升中途出现的吞没意义较弱 —— 底部才是最佳位置。',
        kbId: 'ck-double-engulf',
      },
      '看跌吞没': {
        title: '看跌吞没（Bearish Engulfing）', direction: -1,
        summary: '阴线实体<b>完全吞没</b>前一根阳线实体 —— 空方决定性反扑。顶部最可靠的反转信号之一。',
        how: '前阳后阴 + 阴线实体 > 阳线实体 + 阴线开盘 > 阳线收盘 + 阴线收盘 < 阳线开盘。',
        tip: '顶部吞没 = 强卖出信号。减仓 70%+ 或反手做空。',
        mistakes: '下跌中途的吞没 —— 是下跌中继，不是反转。',
        kbId: 'ck-double-engulf',
      },
      '早晨之星': {
        title: '早晨之星（Morning Star）', direction: 1,
        summary: '三根 K 线组合：<b>大阴 + 小星 + 大阳</b>。底部最强反转信号之一，三 K 线像黎明前的启明星。',
        how: '第 1 根大阴 + 第 2 根小实体（跳空低开）+ 第 3 根大阳（实体回到第 1 根中点以上）。',
        tip: '底部早晨之星 = 顶级买点，仓位 50-70%。止损第 2 根最低点。',
        mistakes: '把普通的"阴-小-阳"当早晨之星 —— 必须第 3 根大阳回到第 1 根中点以上。',
        kbId: 'ck-triple-star',
      },
      '黄昏之星': {
        title: '黄昏之星（Evening Star）', direction: -1,
        summary: '三根 K 线组合：<b>大阳 + 小星 + 大阴</b>。顶部最强反转信号之一，像落日黄昏前的最后光辉。',
        how: '第 1 根大阳 + 第 2 根小实体（跳空高开）+ 第 3 根大阴（实体回到第 1 根中点以下）。',
        tip: '顶部黄昏之星 = 顶级卖出信号，清仓或反手做空。止损第 2 根最高点。',
        mistakes: '忽视第 2 根的"跳空"条件 —— 没有跳空只是普通三 K 组合。',
        kbId: 'ck-triple-star',
      },
      '三白兵': {
        title: '三白兵（Three White Soldiers）', direction: 1,
        summary: '<b>连续三根大阳</b>，每根收盘创新高。是强势启动或反转的标志 —— 多头势如破竹。',
        how: '三根连续大阳 + 每根收盘 > 前一根 + 上影较短 + 开盘在前一根实体内。',
        tip: '顺势持多。若在底部出现则反转信号极强，仓位 70%+。',
        mistakes: '追在三白兵末端 —— 此时短期超买，等回调。',
        kbId: 'ck-triple',
      },
      '三乌鸦': {
        title: '三乌鸦（Three Black Crows）', direction: -1,
        summary: '<b>连续三根大阴</b>，每根收盘创新低。是强势反转或延续的标志 —— 空头全面胜利。',
        how: '三根连续大阴 + 每根收盘 < 前一根 + 下影较短 + 开盘在前一根实体内。',
        tip: '清仓或做空。高位三乌鸦预示大跌。',
        mistakes: '抄底 —— 三乌鸦通常是下跌中继，而非末端。',
        kbId: 'ck-triple',
      },
      '刺透形态': {
        title: '刺透形态（Piercing Pattern）', direction: 1,
        summary: '阴线后出现阳线，<b>阳线实体超过前阴线一半以上</b>。是底部温和反转信号（比吞没稍弱）。',
        how: '前阴后阳 + 阳线开盘 < 阴线最低 + 阳线收盘 > 阴线实体中点。',
        tip: '底部刺透 + 次日确认 = 买入机会。仓位 30-50%。',
        mistakes: '实体不足一半的"伪刺透" —— 信号不成立。',
        kbId: 'ck-double-piercing',
      },
      '乌云盖顶': {
        title: '乌云盖顶（Dark Cloud Cover）', direction: -1,
        summary: '阳线后出现阴线，<b>阴线实体跌破前阳线一半以上</b>。是顶部温和反转信号。',
        how: '前阳后阴 + 阴线开盘 > 阳线最高 + 阴线收盘 < 阳线实体中点。',
        tip: '顶部乌云 = 减仓信号，减 50%+。',
        mistakes: '阴线不到一半的"伪乌云" —— 信号不成立。',
        kbId: 'ck-double-piercing',
      },
      '十字孕线': {
        title: '十字孕线（Harami Cross）', direction: 0,
        summary: '大实体 K 线后紧跟一根<b>十字星</b>，且十字星被大实体完全包含 —— 是强烈的犹豫 / 变盘信号。',
        how: '前 K 大实体 + 后 K 十字星且被前实体完全包住。',
        tip: '变盘警示。等下一根 K 线确认方向。',
        mistakes: '过早下结论。十字孕线仅是警示，不是方向。',
        kbId: 'ck-double-harami',
      },
      '孕线': {
        title: '孕线（Harami）', direction: 0,
        summary: '大实体 K 线后紧跟一根<b>小实体 K 线</b>，且小实体被大实体包含 —— 是动能衰竭 / 变盘信号。',
        how: '前 K 大实体 + 后 K 小实体 + 后 K 完全被前 K 实体包含。',
        tip: '动能减弱警示。等下一根确认。',
        mistakes: '把孕线当反转信号立即操作 —— 它只是"可能反转"。',
        kbId: 'ck-double-harami',
      },
      '吊颈线': {
        title: '吊颈线', direction: -1,
        summary: '高位出现的锤头形状 —— 与"上吊线"同义。是见顶警示。',
        how: '同上吊线。',
        tip: '等阴线确认再减仓。',
        kbId: 'ck-single',
      },
      '长下影': {
        title: '长下影线', direction: 1,
        summary: '下影线长度 ≥ 2× 实体 —— 下方有强买盘承接，底部支撑信号。',
        how: '下影 ≥ 2× 实体。可在任何位置出现。',
        tip: '低位 + 长下影 + 次日阳线 = 买入机会。',
        kbId: 'ck-single',
      },
      '长上影': {
        title: '长上影线', direction: -1,
        summary: '上影线长度 ≥ 2× 实体 —— 上方有强卖压，顶部阻力信号。',
        how: '上影 ≥ 2× 实体。可在任何位置出现。',
        tip: '高位 + 长上影 + 次日阴线 = 减仓信号。',
        kbId: 'ck-single',
      },
      'T字线': {
        title: 'T 字线', direction: 1,
        summary: '开盘 = 收盘 = 最高价，只有下影。是<b>极端</b>底部反转信号 —— 下方买盘强势。',
        how: '开盘 = 收盘 = 高点 + 下影显著。',
        tip: '下跌末期 + T 字 = 强买入信号。',
        kbId: 'ck-single',
      },
      '倒T字线': {
        title: '倒 T 字线', direction: -1,
        summary: '开盘 = 收盘 = 最低价，只有上影。是<b>极端</b>顶部反转信号 —— 上方卖压强势。',
        how: '开盘 = 收盘 = 低点 + 上影显著。',
        tip: '上涨末期 + 倒 T = 强卖出信号。',
        kbId: 'ck-single',
      },
      '启明星': {
        title: '启明星', direction: 1,
        summary: '早晨之星的别称。底部大阴 + 小星 + 大阳的三 K 组合。',
        how: '同早晨之星。',
        tip: '顶级买入信号，仓位 50-70%。',
        kbId: 'ck-triple-star',
      },
      '弃婴底': {
        title: '弃婴底（Bullish Abandoned Baby）', direction: 1,
        summary: '极其罕见的顶级底部反转：大阴 + 跳空十字星 + 大阳，<b>两侧都是跳空</b>。成功率极高但极少见。',
        how: '三根 K 线，中间十字星两侧都跳空（上下影都不重叠）。',
        tip: '见到即买入，仓位 70%+。止损跳空缺口下沿。',
        mistakes: '把普通的早晨之星当弃婴底 —— 关键是两侧跳空。',
        kbId: 'ck-triple-abandoned',
      },
      '弃婴顶': {
        title: '弃婴顶（Bearish Abandoned Baby）', direction: -1,
        summary: '极其罕见的顶级顶部反转：大阳 + 跳空十字星 + 大阴，<b>两侧都是跳空</b>。',
        how: '三根 K 线，中间十字星两侧都跳空。',
        tip: '见到即清仓或做空，仓位 70%+。',
        kbId: 'ck-triple-abandoned',
      },
    },
  };

  // ---------- 内部工具 ----------
  function $(id) { return document.getElementById(id); }

  function fmtPrice(v) {
    if (v == null || !isFinite(v)) return '—';
    if (v > 1000) return v.toFixed(2);
    if (v > 10) return v.toFixed(3);
    if (v > 0.1) return v.toFixed(4);
    return v.toFixed(6);
  }

  function fmtTime(ms) {
    if (!ms) return '—';
    try { return new Date(ms).toLocaleString('zh-CN', { hour12: false }); }
    catch (_) { return String(ms); }
  }

  function escape(s) {
    return String(s || '').replace(/[&<>"']/g, (c) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
    }[c]));
  }

  // ---------- 浮层渲染 ----------
  function close() {
    const existing = $('ee-panel');
    if (existing) existing.remove();
    // 清除图表高亮
    try { window.__AuraTradeApi?.clearHighlight?.(); } catch (_) { /* noop */ }
  }

  function renderPanel(data, meta) {
    close();

    const dirClass = data.direction > 0 ? 'bull' : data.direction < 0 ? 'bear' : 'neutral';
    const dirIcon = data.direction > 0 ? '▲' : data.direction < 0 ? '▼' : '●';
    const dirLabel = data.direction > 0 ? '看涨' : data.direction < 0 ? '看跌' : '中性';

    const panel = document.createElement('div');
    panel.id = 'ee-panel';
    panel.className = `ee-panel ${dirClass}`;

    const metaParts = [];
    if (meta.time) metaParts.push(`<span class="ee-meta-item">📅 ${escape(meta.time)}</span>`);
    if (meta.price) metaParts.push(`<span class="ee-meta-item">💰 ${escape(meta.price)}</span>`);
    if (meta.bar != null) metaParts.push(`<span class="ee-meta-item">🕯 Bar #${meta.bar}</span>`);

    panel.innerHTML = `
      <div class="ee-head">
        <div class="ee-title-wrap">
          <span class="ee-dir ${dirClass}">${dirIcon}</span>
          <span class="ee-title">${escape(data.title)}</span>
          <span class="ee-dir-label ${dirClass}">${dirLabel}</span>
        </div>
        <button class="ee-close" type="button" aria-label="关闭">×</button>
      </div>
      ${metaParts.length ? `<div class="ee-meta">${metaParts.join('')}</div>` : ''}
      <div class="ee-body">
        <div class="ee-sec">
          <div class="ee-sec-title">📖 这是什么？</div>
          <div class="ee-sec-body">${data.summary}</div>
        </div>
        ${data.how ? `<div class="ee-sec">
          <div class="ee-sec-title">👁️ 如何识别</div>
          <div class="ee-sec-body">${data.how}</div>
        </div>` : ''}
        ${data.tip ? `<div class="ee-sec good">
          <div class="ee-sec-title">✅ 操作建议</div>
          <div class="ee-sec-body">${data.tip}</div>
        </div>` : ''}
        ${data.mistakes ? `<div class="ee-sec warn">
          <div class="ee-sec-title">⚠️ 常见错误</div>
          <div class="ee-sec-body">${data.mistakes}</div>
        </div>` : ''}
      </div>
      <div class="ee-foot">
        ${data.kbId ? `<a class="ee-kb-link" href="/knowledge.html#${escape(data.kbId)}" target="_blank" rel="noopener">📚 查看完整知识库讲解 →</a>` : '<span class="ee-kb-hint">已定位到图表</span>'}
      </div>
    `;
    document.body.appendChild(panel);

    panel.querySelector('.ee-close').addEventListener('click', close);
  }

  // ---------- 主入口 ----------
  function explain(type, payload) {
    if (!payload) return;
    const meta = {
      time: payload.timeMs ? fmtTime(payload.timeMs) : null,
      price: payload.price != null ? fmtPrice(payload.price) : null,
      bar: payload.bar != null ? payload.bar : null,
    };

    let data;
    let label;

    if (type === 'granville') {
      data = KB.granville[payload.rule];
      if (!data) { console.warn('[Explainer] 未知 Granville rule:', payload.rule); return; }
      label = data.title.split('（')[0];
    } else if (type === 'cross') {
      const kind = payload.kind === 'Golden' ? 'Golden' : 'Death';
      const base = KB.cross[kind];
      if (!base) return;
      const suffix = (payload.fast && payload.slow) ? ` MA${payload.fast}×MA${payload.slow}` : '';
      data = { ...base, title: base.title + suffix };
      label = data.title;
    } else if (type === 'trap') {
      const kind = payload.kind === 'Bull' ? 'Bull' : 'Bear';
      data = KB.trap[kind];
      if (!data) return;
      label = data.title.split('（')[0];
    } else if (type === 'advMa') {
      data = KB.advMa[payload.kind];
      if (!data) { console.warn('[Explainer] 未知 advMa kind:', payload.kind); return; }
      label = data.title.split('（')[0];
    } else if (type === 'confluence') {
      data = { ...KB.confluence.default };
      if (payload.uniqueKinds || payload.strengthMultiplier) {
        data.summary = `此合流涉及 <b>${payload.uniqueKinds || '?'} 类独立组件</b>，强度倍数 <b>×${(payload.strengthMultiplier || 1).toFixed(1)}</b>。<br><br>` + data.summary;
      }
      label = data.title;
    } else if (type === 'pattern') {
      const base = KB.pattern[payload.label];
      if (base) {
        data = base;
      } else {
        // 通用兜底
        data = {
          title: payload.label || 'K 线形态',
          direction: payload.direction || 0,
          summary: `"<b>${escape(payload.label || '未知')}</b>" K 线形态。具体解读请查看知识库。`,
          kbId: 'ck-single',
        };
      }
      label = data.title.split('（')[0];
    } else {
      console.warn('[Explainer] 未知类型:', type);
      return;
    }

    // 渲染解释浮层
    renderPanel(data, meta);

    // 图表定位 + 高亮
    try {
      const api = window.__AuraTradeApi;
      if (api && payload.timeMs) {
        const ts = Math.floor(payload.timeMs / 1000);
        api.scrollToTime?.(ts);
        api.highlightBar?.(ts, label, data.direction);
      }
    } catch (e) {
      console.warn('[Explainer] 图表操作失败:', e);
    }
  }

  // Esc 关闭浮层
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') close();
  });

  // 暴露 API
  window.AuraExplainer = { explain, close, KB };
})();
