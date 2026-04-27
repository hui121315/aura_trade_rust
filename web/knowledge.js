/* ===========================================================
 * knowledge.js — 知识库页面（数据驱动）
 *
 * 所有知识点定义在 SECTIONS 数组中，由 renderAll() 渲染到 #kb-body。
 * 支持：
 *   - 实时搜索（标题/简介/引用/标签/代码文件名）
 *   - 锚点跳转 + 左侧导航高亮（IntersectionObserver）
 *   - 关键词高亮
 * ========================================================= */

(() => {
  'use strict';

  const $ = (id) => document.getElementById(id);

  // ---------- 分组元数据 ----------
  const GROUPS = [
    { key: 'core',   title: '核心理念', emoji: '🌟',
      intro: '8 条贯穿三书的顶层原则。所有信号、指标、策略都围绕这些理念展开。' },
    { key: 'ma',     title: '均线分析', emoji: '📈',
      intro: '《均线技术分析》381 页核心：排列 / 交叉 / 粘合 / 8 大特殊形态 + 双线组合。' },
    { key: 'trend',  title: '趋势分析', emoji: '📉',
      intro: '《趋势技术分析》100% 完成：支撑阻力、趋势线、通道、道氏理论、Fibonacci。' },
    { key: 'candle', title: 'K 线形态', emoji: '🕯️',
      intro: '《K 线技术分析》808 页：单/双/三 K 线组合 + 11 种图表形态（头肩/双顶/旗形…）。' },
    { key: 'signal', title: '信号分级', emoji: '🎯',
      intro: 'L1-L8 分级体系 + Confluence 多合一 + 陷阱/潜伏识别 + Priority 路由。' },
    { key: 'risk',   title: '风险管理', emoji: '🛡️',
      intro: '仓位校验、ATR 止损、R:R 盈亏比、分级减仓路径 —— 资金安全永远第一。' },
    { key: 'psych',  title: '交易心理', emoji: '🧠',
      intro: '贪婪 / 恐惧 / 损失厌恶 / 后悔 / 过度自信 —— 技术分析解决 30% 的问题，心理控制解决 70%。' },
    { key: 'practice', title: '实战策略', emoji: '🎯',
      intro: '组合策略模板：回踩买入 / 突破追击 / 底背离建仓 / 分批抄底 等可直接执行的剧本。' },
    { key: 'api',    title: 'API 索引', emoji: '⚙️',
      intro: '全部 HTTP / WebSocket 端点，按功能分组。可直接在浏览器访问测试。' },
  ];

  // ---------- 内容数据 ----------
  // 每个 section：{ id, group, title, emoji, badges, desc, quotes[], meta{}, tags[] }
  const SECTIONS = [
    // ==================== 核心理念 ====================
    {
      id: 'sec-overview',
      group: 'core',
      title: '三书总纲（兵书 / 战阵 / 兵道）',
      badges: [{ text: '顶层原则', kind: 'iron' }],
      desc: '三书封底均以此收尾，是 AURA 项目的最高指导原则。AURA 按兵书/战阵/兵道的架构设计：<b>engine/</b> = 兵书（趋势分析技术），<b>server/ + web/</b> = 战阵（交易系统），<b>PRD 手册</b> = 兵道（交易习惯）。',
      quotes: [
        { text: '如果说股市如战场，那么趋势分析技术就是兵书，交易系统就是战阵，交易习惯就是兵道。', source: '三书共同封底' },
      ],
      meta: {
        '手册': '<code class="kb-file">AURA_BOOK_HANDBOOK.md</code>',
        'PRD': '<code class="kb-file">PRD_REVISION_DRAFT_v4.md</code>',
      },
      tags: ['总纲', '架构', 'handbook', '兵书'],
    },
    {
      id: 'core-3pct',
      group: 'core',
      title: '3% 有效突破阈值',
      badges: [{ text: '跨全书铁证', kind: 'iron' }],
      desc: '跨越三本书的统一阈值。<b>突破趋势线 / 支撑 / 阻力 / 颈线</b>必须伴随 ≥ 3% 的价格幅度才视为有效，否则视为假突破。低于 3% 的突破会被陷阱检测模块识别。',
      whatIs: `<p>假设你看到价格跌破了一条上升趋势线，你会马上做空吗？错了。<b>很多时候价格只是"碰一下"就反弹</b>，只有真正跌 3% 以上且收盘确认，才是"真突破"。</p>
<p>为什么是 3%？这是三本书通过大量历史验证总结出来的经验值：低于 3% 的穿越有 60-70% 概率是<b>假突破</b>（即主力诱导），大于 3% 后真突破的概率上升到 85%+。</p>
<p>AURA 把这个阈值作为全系统的统一规则，应用于：趋势线跌破、支撑阻力角色翻转、头肩颈线确认、旗形突破验证等所有"突破"相关判断。</p>`,
      howTo: [
        '<b>确定参考位</b>：先画出趋势线 / 支撑线 / 阻力线 / 颈线。',
        '<b>计算阈值</b>：突破所需价格幅度 = 参考位价格 × 0.03。',
        '<b>检查收盘价</b>：必须用收盘价而非最高/最低价。影线穿越不算。',
        '<b>等待次根 K 线</b>：突破的这根 K 线收盘符合 3% 后，再观察下一根是否回抽但不回到参考位之内（确认有效）。',
        '<b>放量配合</b>：真突破通常伴随 ≥ 1.5 × 均量的放量。',
      ],
      params: {
        '标准阈值': '<code>3.0%</code>（EFFECTIVE_BREAK_PCT）',
        '对数坐标': '相对百分比而非绝对价格',
        '确认方式': '收盘价 +  次根不回抽',
      },
      strategy: `<ul>
<li><b>突破 &lt; 3%</b>：视为<b>陷阱</b>，不可追涨追跌。反向交易（如反弹做多 / 反抽做空）。</li>
<li><b>突破 ≥ 3% 且放量</b>：有效突破，顺势交易，目标位按量度法则计算。</li>
<li><b>突破 3-5% 但量能不足</b>：半仓建立头寸，观察后续 2-3 根确认。</li>
</ul>`,
      mistakes: [
        '<b>看到影线穿越就做空 / 做多</b>：必须等收盘价！下影下穿支撑但阳线收回，是典型假突破（诱空）。',
        '<b>盯盘做交易</b>：没等日线 / 小时线收盘就操作，被下影 / 上影诱导。',
        '<b>小周期的 3%</b>：1 分钟 K 线波动 3% 太常见，3% 阈值主要适用于 4h 以上周期。',
        '<b>忽略上下文</b>：在长期阻力上方的 3% 突破远强于在下降通道中的 3% 跳空。',
      ],
      example: `<b>典型案例</b>：BTC 2021 年 5 月跌破 52000 支撑 —— 当日跌至 50100（下跌 3.6%），收盘 50200（有效突破）。3 天后继续跌到 40000（-23%）。若仅看影线最低 48000 就过早做空，可能在反弹到 55000 时被止损。`,
      quotes: [
        { text: '趋势线有效突破必须满足 ≥ 3% 幅度。', source: 'trend p.203' },
        { text: '多级趋势矩阵决策阈值统一为 3%。', source: 'trend p.216' },
        { text: '旗形突破有效性：第 3 条铁证。', source: 'candle p.770' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs:111</code><br><code class="kb-file">src/engine/trend/sr.rs:86</code>',
        '常量': '<code>EFFECTIVE_BREAK_PCT = 0.03</code>',
        '应用': 'TrendLine.check_effective_break / SrLevel.detect_role_flips / 对数坐标 / 角色翻转',
      },
      tags: ['3%', '突破', '铁证', 'break', '趋势线', '假突破', '陷阱'],
    },
    {
      id: 'core-60ma',
      group: 'core',
      title: '60 日均线（定性线）',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '长期趋势', kind: 'bull' }],
      desc: 'MA60 是 <b>长期趋势的分水岭</b>，也是断头铡刀的前提条件。一旦价格跌破 MA60 且 MA60 向下，即为长期趋势恶化。双线组合中扮演"定性线"角色，MA20 为"节奏线"。',
      whatIs: `<p>为什么是"60 日"而不是 50 或 100？因为 60 代表约 <b>3 个月</b>的交易时间（60 交易日 ≈ 12 周 ≈ 3 月），这是大部分主力资金调整仓位的周期。</p>
<p>60 日均线也叫 <b>定性线</b>：意思是它决定"这只币目前到底是牛市还是熊市"这个定性的问题。只要价格站在 MA60 上方且 MA60 向上，那就是"牛市性质"；反之就是"熊市性质"。不管中间多么波动，大方向不变。</p>
<p>短周期均线（MA5 / MA10 / MA20）负责"节奏"——决定何时买卖。60 日负责"定性"——决定要不要参与这只币。AURA 的 <code>QUALITATIVE_PERIOD = 60</code> 把这个铁证编码为全系统约束。</p>`,
      howTo: [
        '<b>画出 MA60</b>：在图表上绘制 60 周期简单均线（4h K 线用 60 × 4h 即 10 天均线；日线用 60 天）。',
        '<b>判断价格位置</b>：当前收盘价 > MA60 = 牛市性质；收盘价 < MA60 = 熊市性质。',
        '<b>判断 MA60 方向</b>：近 10 根 MA60 整体向上 = 真牛；向下 = 真熊；平坦 = 震荡期。',
        '<b>关注交叉时刻</b>：价格第一次跌破 MA60 是重要预警（L6 级别）；MA60 本身跌头更严重（L7 级别）。',
      ],
      params: {
        '定性周期': '<code>60</code>（日线即 60 天，4h 即 10 天）',
        '牛/熊判定': '收盘 vs MA60 位置',
        '方向判定': '近 10 根 MA60 斜率符号',
      },
      strategy: `<ul>
<li><b>价格 > MA60 且 MA60 向上</b>：只做多不做空。短周期死叉视为回调买入机会而非反转。</li>
<li><b>价格 < MA60 且 MA60 向下</b>：只做空或观望。任何短周期金叉视为反弹诱多。</li>
<li><b>价格接近 MA60（±3% 内）</b>：高度警惕，这是主力决定方向的关键位。</li>
<li><b>MA60 翻头（由上转下）</b>：即便价格未跌破，也应减仓 50%（葛南维 L4 预警）。</li>
</ul>`,
      mistakes: [
        '<b>只用短周期 MA 做交易</b>：MA5 / MA10 金叉死叉会让你反复买卖，忽略 MA60 会跟不上大趋势。',
        '<b>在 MA60 下方抄底</b>：大部分"便宜了"的感觉都是 MA60 下的陷阱。等价格重新站回 MA60 之上再买。',
        '<b>盯着 MA60 的单根交叉</b>：交叉瞬间不重要，重要的是交叉后能否稳定站稳（连续 3 根以上）。',
      ],
      example: `<b>BTC 2022 牛熊转换</b>：BTC 在 2022/1 跌破 MA60（日线）@ 44000，MA60 次月翻头向下。随后全年走熊，直到 2023/1 重新站上 MA60 @ 21000 才重启牛市。所有在此期间"抄底"的交易者多数被套 50%+。`,
      quotes: [
        { text: '60 日均线是定性线，是长期趋势的分水岭。', source: 'ma 全书反复' },
        { text: '断头铡刀之前：60 日均线一直下行（必要前提）。', source: 'ma p.310' },
        { text: '双线组合中，60 日定性、20 日节奏，缺一不可。', source: 'ma p.200' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs</code>',
        '常量': '<code>QUALITATIVE_PERIOD = 60</code>',
        '应用': 'Granville L4 警告 / 断头铡刀 / 双线组合 6 条',
      },
      tags: ['60日', 'MA60', '定性线', '均线', '分水岭', '长期', '牛熊'],
    },
    {
      id: 'core-fatigue',
      group: 'core',
      title: '信号衰减原则',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '反直觉', kind: 'warn' }],
      desc: '下跌趋势中反复出现的均线死叉 / 特殊形态，<b>只有前一两次有实战意义</b>。越往后越接近底部，信号不再可靠。AURA 的 <code>SignalFatigue</code> 模块会计数同类信号连续出现次数，超过阈值后自动降权。',
      whatIs: `<p>很多新手有一个误区：<b>"信号出现越多次越准"</b>——实际上完全相反。</p>
<p>想象一条下跌趋势的图：下跌开始时出现一次死叉是<b>真反转</b>（最强信号）；中间又出现一次死叉可能还在跌（弱信号）；后期再出现第 5 次死叉，此时价格可能已经接近底部，再做空反而会被反弹套牢。</p>
<p>这就是<b>信号衰减</b>：同一类信号连续出现，价值递减。原因是市场已经充分消化了这类信号的信息，剩下的空头力量越来越少。</p>`,
      howTo: [
        '<b>计数同类信号</b>：在某个趋势阶段内（未出现反转前），记录死叉 / 断头铡刀等信号出现的次数。',
        '<b>应用衰减系数</b>：第 1 次权重 100%，第 2 次 60%，第 3 次 30%，第 4 次及以后 10%。',
        '<b>重置条件</b>：只要出现明确的趋势反转（如金叉 + MA60 翻头），计数清零重新开始。',
      ],
      params: {
        '衰减曲线': '<code>[1.0, 0.6, 0.3, 0.1, ...]</code>',
        '重置条件': '反向趋势确立（金叉 + MA 翻头）',
      },
      strategy: `<ul>
<li><b>第一次死叉</b>：最有效，全仓做空或清仓。</li>
<li><b>第二次死叉</b>：半仓或不加仓，已经错过最佳时机。</li>
<li><b>第三次以上</b>：反而应警惕反弹机会，不追空。</li>
<li><b>做多同理</b>：反弹中第一次金叉最有价值，后续金叉逐渐失效。</li>
</ul>`,
      mistakes: [
        '<b>看到信号就机械执行</b>：忽略次数会让你在底部区域不断做空。',
        '<b>认为"信号越多越准"</b>：完全反了，第 5 次信号可靠度可能不到 10%。',
        '<b>不重置计数</b>：趋势反转后忘记清零，会把新趋势的第一次信号当成旧的第 N 次。',
      ],
      example: `<b>BTC 2022 年案例</b>：全年下跌中出现 <b>6 次</b> ma 断头铡刀。第 1 次（2022/1 @ 44000）后跌至 33000（-25%）；第 2 次（2022/5 @ 38000）后跌至 28000（-26%）；第 3 次（2022/6 @ 30000）后只跌至 19000（-37% 但横盘半年）；第 4-6 次后基本横盘或小反弹。若严格按信号做空，前 2-3 次盈利能覆盖后面的亏损；若第 5 次还跟着做空会被 2023 年初反弹吃掉。`,
      quotes: [
        { text: '在长期下降趋势中，经常多次出现均线复合死亡走势。对交易而言，最具有实战意义的只有前期的一两次，越靠后的均线复合死亡离底部越近，技术信号就越不可靠。', source: 'ma p.360' },
        { text: '如果严格执行纪律，应当在第一次或第二次发出卖出信号时就已空仓，即使后面发出十次、二十次卖出信号，其实都没有太大的意义。', source: 'ma p.360' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/fatigue.rs</code>',
        '测试': '8 个测试用例',
        '应用': '特殊形态权重衰减 / 重复信号过滤',
      },
      tags: ['衰减', 'fatigue', '死叉', '信号', '第一次最有效'],
    },
    {
      id: 'core-position',
      group: 'core',
      title: '分级减仓哲学',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '风险控制', kind: 'bear' }],
      desc: '不同信号对应不同的最大仓位上限。<b>葛南维 L4 轻仓 30%</b>、<b>倒置 V 三次减仓（30% / 50% / 100%）</b>、<b>岛形反转时间映射</b>、<b>镊子顶短线清仓</b>。AURA 的 <code>PositionLimit</code> 常量将这些铁证编码为强制约束。',
      whatIs: `<p>很多新手信奉"全仓出击、不赚钱不罢休"，这是散户亏钱的首要原因。<b>专业交易员的共识是：仓位管理比选股票更重要</b>。</p>
<p>分级减仓意味着：<b>不同的警报级别对应不同的减仓比例</b>。L1 警报减 10%，L4 警报减 30%，L7 警报减 50%，L8 紧急警报清仓。这样即便判断错误，也不会因单次失误而巨亏。</p>
<p>AURA 的 <code>PositionLimit</code> 实际上是<b>硬约束</b>：如果当前信号级别为 L4，那么最大允许仓位就是 30%——即便系统发现了看起来很好的买点，也不允许加到 50%。这是经验沉淀，避免情绪化操作。</p>`,
      howTo: [
        '<b>判断信号级别</b>：L1 / L3 / L4 / L6 / L8（详见信号分级章）。',
        '<b>查询对应上限</b>：L1 ≤ 20%，L3 ≤ 50%，L4 ≤ 70%（牛市）/ 30%（警报），L8 = 清仓 / 满仓。',
        '<b>检查当前仓位</b>：如果当前仓位 > 上限，必须减到上限。',
        '<b>执行减仓</b>：优先减最弱 / 最亏损的头寸；均值持仓者按比例减。',
      ],
      params: {
        'L4_MAX': '<code>0.30</code>（葛南维 L4 警报）',
        'BULL_MAX': '<code>1.00</code>（明确牛市）',
        'SELL_MAX': '<code>0.00</code>（强卖出信号）',
        '紧急阶梯': '30% / 50% / 100%（倒置 V 三段）',
      },
      strategy: `<ul>
<li><b>L1 预警</b>：观察，不操作。</li>
<li><b>L4 警报</b>：减仓至 30% 以下（经典"轻仓过渡"）。</li>
<li><b>L6 急警</b>：减仓至 50%，止损调到盈亏平衡点。</li>
<li><b>L8 紧急</b>：立即清仓，不等反弹。</li>
<li><b>顺序减仓</b>：先减没到目标位的盈利单 → 再减亏损单。</li>
</ul>`,
      mistakes: [
        '<b>"再等等"心态</b>：L4 警报后想等反弹再减，结果被套得更深。',
        '<b>全仓 All-in</b>：即便是 L8 级别买入信号，满仓也违反风控纪律，应留 10-20% 观察资金。',
        '<b>不均减</b>：只减盈利单保留亏损单，这是心理诱因，不是策略。',
      ],
      example: `<b>ETH 2021/11/10 高点案例</b>：ETH 达到 4878 后出现 L4 级别顶部背离警报。若持有 100% 仓位的交易员遵守 L4 ≤ 30%，应减仓 70%。2022/6 ETH 跌至 880，未减仓的损失 82%，遵守分级减仓的损失 ≤ 25%（因为 70% 仓位在 4500 附近已退出）。`,
      quotes: [
        { text: '短线清仓 / 中长线减仓。', source: 'candle p.180 镊子顶' },
        { text: '30% / 50% / 100% 三段减仓。', source: 'candle p.605 倒置 V' },
        { text: '时间→级别映射。', source: 'candle p.660 岛形反转' },
        { text: '仓位 ≤ 30%。', source: 'ma p.100 葛南维 L4' },
        { text: '跌破长期上升 = 清仓（即便未逆转）。', source: 'trend p.221 SELL-1' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/strategy.rs:238</code>',
        '常量': 'L4_MAX=0.30 / BULL_MAX=1.00 / SELL_MAX=0.00',
        '应用': 'PlaybookRunner 仓位强制 / 分级减仓路径',
      },
      tags: ['仓位', '减仓', 'PositionLimit', 'L4', '30%', '风控'],
    },
    {
      id: 'core-confluence',
      group: 'core',
      title: '多形态共振（Confluence）',
      badges: [{ text: '信号倍增', kind: 'iron' }, { text: 'R-P1-16', kind: '' }],
      desc: '多个独立信号在同一价位 ±3% 范围内出现时，信号强度倍增。常见组合：<b>均线 + 趋势线 + 水平支撑</b>、<b>断头铡刀 + 倾盆大雨 + S6 卖出 + 死亡谷</b>。AURA 的 <code>Confluence</code> 模块会识别这些多合一现象并提升信号分级。',
      whatIs: `<p>单独一个信号容易出错（假突破、假交叉常见），但如果<b>好几个独立信号同时指向同一方向</b>，那置信度就会大大提升——就像法庭上多个证人都证明同一件事。</p>
<p>AURA 定义的共振要求：<b>3 个以上不同类别的信号 + 都落在 ±3% 价格带内 + 方向一致</b>。满足这三条的时刻非常稀少，但一旦出现，胜率往往 70%+。</p>
<p>共振信号的等级会自动"晋升"：单一信号 L3 → 三合一 L6 → 四合一 L8（顶级）。L8 级别的共振几乎不会错，是最佳建仓 / 清仓时机。</p>`,
      howTo: [
        '<b>找到关键价位</b>：当前重要支撑或阻力价格（可从 SR / MA / Fib / 整数关口产生）。',
        '<b>列出指向该价位的信号</b>：MA60 / 前高 / 0.618 回撤 / 趋势线 / K 线形态 …',
        '<b>统计独立类别</b>：MA 算 1 类，SR 算 1 类，K 线形态算 1 类。',
        '<b>判断级别</b>：3 类 = L6 共振，4 类 = L8 顶级共振。',
        '<b>检查方向一致性</b>：所有信号必须一致指向看涨或看跌，否则抵消不是共振。',
      ],
      params: {
        '价格带': '<code>±3%</code>',
        '类别数': '<code>≥ 3</code> 类独立（MA / SR / Fib / K 线 / 心理位）',
        'L6 阈值': '3 类',
        'L8 阈值': '4 类 + 放量配合',
      },
      strategy: `<ul>
<li><b>L6 共振</b>：建仓 50-70%，置信度较高。</li>
<li><b>L8 顶级共振</b>：建仓 80-100%，这是少见的绝佳机会。</li>
<li><b>方向分歧</b>：如 MA 看涨但 K 线看跌，应观望而非强行选边。</li>
<li><b>提前布局</b>：如果共振价位在下方 3-5%，可挂限价单分批进场。</li>
</ul>`,
      mistakes: [
        '<b>只数相同类别</b>：MA5 金叉 + MA10 金叉只算 1 类信号（都是 MA），不是共振。',
        '<b>距离太远的共振</b>：信号价位距离 > 5% 的"共振"其实是独立信号，不能合并。',
        '<b>忽略放量</b>：真正的 L8 顶级共振一定伴随放量；无量共振可能是诱导。',
      ],
      example: `<b>BTC 2020/10 底部 L8 共振</b>：价位 10500 附近同时出现：① MA200 支撑 ② 2019 年阻力转支撑（R-P1-36 互通） ③ 0.618 回撤位 ④ 月线锤头 + 放量。4 类独立信号 = L8 级。AURA 识别后发出重仓买入。随后 BTC 从 10500 涨到 60000，半年 +470%。`,
      quotes: [
        { text: '均线 + 趋势线 + 支撑位 ±3% 合流 = 强支撑。', source: 'R-P1-16 多合一' },
        { text: '断头铡刀 + 倾盆大雨 + S6 卖出 + 死亡谷（四大共振）。', source: 'ma p.310' },
        { text: '底部三形态互通：V / 淡友 / 岛形。', source: 'R-P1-36' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/confluence.rs</code>',
        'API': '<span class="kb-chip api">/api/signals</span> <span class="kb-chip api">/api/resonance</span>',
        '字段': 'confluences: Vec&lt;Confluence&gt;',
      },
      tags: ['共振', 'confluence', '多合一', '倍增', 'L8', 'L6'],
    },
    {
      id: 'core-mm',
      group: 'core',
      title: '主力行为学视角',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '思维转变', kind: 'warn' }],
      desc: '每种图表形态都对应主力的某种行为：<b>扩散三角形 = 主力过顶吸筹洗盘</b>、<b>矩形 = 高抛低吸囤积</b>、<b>潜伏突破 = 小阳线缩量吸筹</b>、<b>倒三阳 = 主力出货</b>。AURA 的 <code>MarketMakerBehavior</code> 将此维度编入形态识别。',
      whatIs: `<p>很多散户看图只看"价格涨跌"，职业交易员看图会问：<b>"主力现在在做什么？"</b> 这就是主力行为学视角。</p>
<p>加密市场比传统市场更适用：大资金（鲸鱼 / 做市商）的行为主导短中期价格。他们的典型行为有 4 类：
<b>吸筹（Accumulation）</b>——低位买入，股价横盘；
<b>拉升（Markup）</b>——放量突破，明显上涨；
<b>出货（Distribution）</b>——高位震荡，散户接盘；
<b>打压（Markdown）</b>——放量跳水，带动恐慌盘出逃。</p>
<p>每种 K 线 / 图表形态都可归类到这 4 个行为。例如：矩形 = 吸筹或出货（看位置）；扩散三角 = 吸筹洗盘；倒三阳 = 出货；旗形 = 短暂休息（继续原方向）。</p>`,
      howTo: [
        '<b>先问位置</b>：当前价格处于牛市初期、中期、末期？还是熊市？（看 MA60 和大周期 Swing）',
        '<b>再看形态</b>：当前图表形态是什么？（矩形、三角、旗形、头肩...）',
        '<b>推断行为</b>：低位 + 矩形/横盘 = 吸筹；高位 + 矩形/震荡 = 出货；两者对策完全不同。',
        '<b>验证量能</b>：吸筹期量能萎缩然后突然放大；出货期量能持续较大但价格滞涨。',
      ],
      params: {
        '枚举': 'Accumulation / Markup / Distribution / Markdown / Washout',
        '位置': '牛初（低位）vs 牛末（高位）',
        '量价': '量缩价稳 = 吸筹；量稳价滞 = 出货',
      },
      strategy: `<ul>
<li><b>低位矩形 → 判定为吸筹</b>：突破矩形上沿时买入（主力拉升前夜）。</li>
<li><b>高位矩形 → 判定为出货</b>：跌破矩形下沿时清仓。</li>
<li><b>扩散三角形 → 主力过顶吸筹</b>：价格波动范围逐渐扩大但未突破前高，这是洗盘，耐心等待真正突破。</li>
<li><b>潜伏突破（小阳缩量破关键阻力）</b>：主力悄悄建仓，是极佳早期信号。</li>
</ul>`,
      mistakes: [
        '<b>不看位置只看形态</b>：同一个矩形在低位是吸筹，在高位就是出货，完全不同的操作。',
        '<b>"散户思维"</b>：认为价格跌就该买、涨就该卖。主力反向操作——他们低位买入后会拉升让你追高。',
        '<b>忽视量能</b>：没有量能配合的形态分析容易被误导。',
      ],
      example: `<b>BTC 2020/10-12 吸筹案例</b>：10 月后 BTC 在 10500-11500 横盘 8 周，量能持续萎缩（典型吸筹特征）。11 月下旬一根大阳线放量突破 11500，次日继续放量 —— 主力完成吸筹进入拉升阶段。后续 2 个月涨至 40000（+250%）。`,
      quotes: [
        { text: '扩散三角形 = 主力过顶吸筹洗盘。', source: 'candle p.720' },
        { text: '矩形 = 主力高抛低吸 / 囤积。', source: 'candle p.795' },
        { text: '倒三阳 = 主力出货。', source: 'candle p.400' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/market_maker.rs</code>',
        '枚举': 'Accumulation / Distribution / Washout',
        '应用': 'PatternAttribution 解读',
      },
      tags: ['主力', '吸筹', '出货', '洗盘', '鲸鱼', '做市商'],
    },
    {
      id: 'core-discipline',
      group: 'core',
      title: '谨慎买入，果断卖出',
      badges: [{ text: '哲学', kind: 'iron' }, { text: '顶级纪律', kind: 'warn' }],
      desc: '三书共同的交易纪律：买入前要确认 3-5 个信号共振；卖出只需一个强信号。踏空是保障资金安全必须付出的代价 —— AURA 的 Playbook 策略模板严格遵循此不对称哲学。',
      whatIs: `<p>这是三本书反复强调的"交易哲学的顶点"，也是大部分散户缺失的核心纪律。</p>
<p><b>谨慎买入</b>：买入是可选的，错过一次机会不损失什么。因此应<b>严苛</b>要求——必须 3-5 个信号共振才买入。</p>
<p><b>果断卖出</b>：卖出关乎资金安全，一旦判断错误可能导致巨亏。因此应<b>宽松</b>触发——只要出现一个明确的卖出信号（如断头铡刀 / 颈线跌破 / L4 警报），就应立即执行，不必等待 3 个信号都确认。</p>
<p>简单说：<b>买入要"求证"，卖出要"宁杀错"</b>。原书原话：<b>"踏空是保障资金安全必须付出的代价"</b> —— 宁可错过一次上涨，也不能错过一次卖出。</p>`,
      howTo: [
        '<b>买入前检查</b>：至少 3 个独立信号指向同一方向 + 放量 + 多头排列 + 价格 > MA60。</li>',
        '<b>卖出时判断</b>：只要出现一个强卖出信号（L6+），立即执行。不问理由、不问反弹。',
        '<b>不对称设置止损 / 止盈</b>：止损距离 × 2 ≤ 止盈距离（R:R ≥ 2），让收益跑赢亏损。',
      ],
      strategy: `<ul>
<li><b>买入触发</b>：Confluence ≥ 3 类 + 放量 + 趋势向上。</li>
<li><b>卖出触发</b>：任一 L6 信号 / 止损触发 / 仓位上限超出。</li>
<li><b>错过 vs 损失</b>：宁可错过 5 次上涨也不要承受 1 次 30% 的下跌。</li>
</ul>`,
      mistakes: [
        '<b>追涨（买得不谨慎）</b>：看到涨就追，没有等多信号共振。</li>',
        '<b>等等看（卖得不果断）</b>：明明看到断头铡刀了还不清仓，希望反弹后卖个高点。',
        '<b>损失厌恶</b>：不愿割肉，小亏变大亏。</li>',
        '<b>"反向持仓"</b>：看空的同时还持有现货（期货对冲除外）。</li>',
      ],
      example: `<b>ETH 2021/11 顶部案例</b>：ETH 在 4800 区域出现 L4 顶部背离，部分交易员坚持"再涨 200 USDT 就卖"，最终 ETH 跌到 880，亏损 82%。对比：果断在 4500-4700 区域清仓的人，损失锁定为 ≤ 20%。"踏空"后续 200 USDT 的利润，但保住了 80% 本金，这就是<b>果断卖出</b>的价值。`,
      quotes: [
        { text: '卖出信号一旦发出就应卖出，不必等待确认。', source: 'ma p.380' },
        { text: '踏空是保障资金安全必须付出的代价。', source: 'candle p.605 倒置 V' },
        { text: '黄昏之星 > 早晨之星（顶部卖出比底部买入更重要）。', source: 'candle p.320' },
        { text: 'SELL-1 清仓依然明智之举。', source: 'trend p.221' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/backtest/playbook.rs</code>',
        '体现': 'Playbook Buy/Sell 不对称阈值',
      },
      tags: ['纪律', '卖出', '果断', '哲学', '踏空', '不对称'],
    },

    // ==================== 均线分析 ====================
    {
      id: 'ma-alignment',
      group: 'ma',
      title: '均线排列（多头 / 空头 / Mixed）',
      badges: [{ text: '基础', kind: '' }, { text: '最重要信号', kind: 'bull' }],
      desc: '按周期从短到长排列均线，若均线值也从大到小 = <b>多头排列</b>；反之 = <b>空头排列</b>；不符合任一规则 = Mixed（混乱）。',
      whatIs: `<p>这是均线最基础也最重要的判断：把 MA5 / MA10 / MA20 / MA60 / MA120 这几条均线画在一起看。</p>
<p><b>多头排列</b>：MA5 > MA10 > MA20 > MA60 > MA120。短周期在上，长周期在下。像"扇形张开"向上，表示市场一致看多，资金持续买入。</p>
<p><b>空头排列</b>：完全反过来，MA5 < MA10 < MA20 < MA60 < MA120。表示市场一致看空，资金持续卖出。</p>
<p><b>Mixed</b>：均线相互缠绕、交叉、不按顺序。市场犹豫、没有明确方向，震荡整理中。</p>
<p>AURA 的 <code>classify</code> 函数带 0.5% 容差：均线距离 &lt; 0.5% 时视为"粘合"，不强求严格的大小关系，避免横盘抖动误判。</p>`,
      diagram: `<span class="d-mute">  多头排列（Bullish）</span>     <span class="d-mute">空头排列（Bearish）</span>
<span class="d-bull">    ═══  MA5</span>                 <span class="d-bear">══════  MA120</span>
<span class="d-bull">      ═══  MA10</span>                 <span class="d-bear">════  MA60</span>
<span class="d-bull">        ═══  MA20</span>               <span class="d-bear">════  MA20</span>
<span class="d-bull">          ═══  MA60</span>             <span class="d-bear">═══  MA10</span>
<span class="d-bull">            ═══  MA120</span>          <span class="d-bear">═══  MA5</span>
<span class="d-bull">        ↑ 扇形向上</span>                <span class="d-bear">↓ 扇形向下</span>`,
      howTo: [
        '<b>画 5 条均线</b>：MA5 / MA10 / MA20 / MA60 / MA120（AURA 默认配置）。',
        '<b>检查顺序</b>：从上到下是否按 5 → 10 → 20 → 60 → 120 排列（多头）？或反向（空头）？',
        '<b>检查斜率</b>：每条均线近 10 根是否同向？',
        '<b>检查间距</b>：距离是否均匀？均匀 = 健康趋势；距离突然扩大 = 加速拉升或加速下跌。',
      ],
      params: {
        '默认周期': '<code>[5, 10, 20, 60, 120]</code>',
        '容差': '<code>0.5%</code>（小于此视为粘合）',
        '结果': 'Bullish / Bearish / Mixed',
      },
      strategy: `<ul>
<li><b>多头排列</b>：趋势明确向上，所有回调买入。仓位上限 100%。</li>
<li><b>空头排列</b>：趋势明确向下，所有反弹做空或空仓。仓位上限 0%。</li>
<li><b>Mixed → 多头</b>：刚形成多头排列是大机会（L3 级买点）。</li>
<li><b>多头 → Mixed</b>：趋势瓦解预警，开始减仓。</li>
<li><b>Mixed 状态</b>：观望或小仓位操作，等待明确方向。</li>
</ul>`,
      mistakes: [
        '<b>周期选错</b>：太短周期（如 3/5/8）会频繁切换状态；太长周期（如 60/120/200）反应太慢。5/10/20/60/120 是平衡点。',
        '<b>看 2 条均线就下结论</b>：必须 3 条以上同时排列才可靠。',
        '<b>忽略斜率</b>：均线虽排列但所有均线都横盘（斜率 ≈ 0）是"假多头"。',
      ],
      example: `<b>BTC 2020/10 多头形成</b>：MA5 @ 11200 > MA10 @ 10800 > MA20 @ 10500 > MA60 @ 10200 > MA120 @ 10000，完美多头排列。AURA 标记为 Bullish，进入"只做多不做空"模式。后续 5 个月涨到 60000。`,
      quotes: [
        { text: '均线多头排列是牛市的基础。', source: 'ma Ch3 均线排列章' },
        { text: '多头排列的均线像一把张开的扇子，牛市的标志。', source: 'ma p.75' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/alignment.rs</code>',
        'API': '<span class="kb-chip api">/api/ma_state</span>',
        '字段': 'alignment / alignment_aliases',
      },
      tags: ['排列', 'alignment', '多头', '空头', 'bullish', 'bearish', 'mixed'],
    },
    {
      id: 'ma-granville',
      group: 'ma',
      title: '葛南维八大信号（L1-L8 总览）',
      badges: [{ text: '核心', kind: 'iron' }, { text: '最经典', kind: 'bull' }],
      desc: '均线领域最经典的交易信号体系，由美国技术分析大师 Joseph Granville 提出。按价格 vs 均线的相对位置 + 均线方向 + BIAS 程度，归纳 8 种买卖点（L1-L8）。',
      whatIs: `<p>想象一条 60 日均线像一条"价格中枢线"——价格应该围绕它波动。葛南维发现价格相对均线有 <b>8 种经典位置</b>，每一种都对应一个明确的交易动作。</p>
<p>L1-L4 是 <b>多头状态下的 4 种情况</b>（均线向上）：L1 回踩买、L2 跌破反弹再买、L3 反弹未上穿卖出、L4 远离均线超买卖出。</p>
<p>L5-L8 是 <b>空头状态下的 4 种情况</b>（均线向下）：L5 反弹至均线卖、L6 反弹未到再卖、L7 下跌未破均线买入、L8 远离均线超卖买入。</p>
<p>下方每个 L1-L8 各有详细卡片。AURA 实现完整 8 大信号并输出 GranvilleSignal 事件流。</p>`,
      diagram: `<span class="d-mute">     多头 4 点（均线向上）</span>       <span class="d-mute">空头 4 点（均线向下）</span>
<span class="d-mute">              ↗ MA60</span>                       ↘ MA60
<span class="d-bull">         ⓵</span> <span class="d-mute">回踩买</span>                      <span class="d-bear">⑤</span> <span class="d-mute">反弹卖</span>
<span class="d-bull">      ⓶</span> <span class="d-mute">跌破反买</span>                    <span class="d-bear">⑥</span> <span class="d-mute">反弹未到再卖</span>
<span class="d-bear">           ⓷</span> <span class="d-mute">反弹未破卖</span>              <span class="d-bull">⑦</span> <span class="d-mute">下跌未破买</span>
<span class="d-bear">              ⓸</span> <span class="d-mute">远离超买卖</span>           <span class="d-bull">⑧</span> <span class="d-mute">远离超卖买</span>`,
      strategy: `<ul>
<li><b>L1 + L8 是顶级买点</b>（趋势加持的买点）</li>
<li><b>L4 + L5 是顶级卖点</b>（警报级卖出）</li>
<li><b>L2 + L7 是中级机会</b>（趋势延续确认）</li>
<li><b>L3 + L6 是弱反弹点</b>（谨慎跟进）</li>
</ul>`,
      quotes: [
        { text: 'L4 = 价格远离均线 + BIAS > 8% + 仓位 ≤ 30%。', source: 'ma p.100' },
        { text: 'L1 买入：价格回落至 MA 附近 + MA 上升 + 不跌破。', source: 'ma p.96' },
        { text: '八大信号的核心是"价格 vs 均线 vs 斜率"的三元关系。', source: 'ma Ch2' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs</code>',
        'API': '<span class="kb-chip api">/api/ma_state</span>',
        '字段': 'granville: Vec&lt;GranvilleSignal&gt;',
        '测试': '12 个测试用例',
      },
      tags: ['葛南维', 'granville', 'L1', 'L4', 'L8', 'BIAS', '总览'],
    },
    {
      id: 'ma-granville-l1',
      group: 'ma',
      title: '葛南维 L1：均线上方回踩买点',
      badges: [{ text: 'L1 买入', kind: 'bull' }, { text: '顶级买点', kind: 'iron' }],
      desc: '牛市中价格从高位回落至 MA60 附近，但未跌破，随后反弹 —— 这是葛南维体系中最经典的买入信号。',
      whatIs: `<p>L1 是牛市中<b>最重要的加仓点</b>。市场刚刚经历一波上涨，现在正在"喘口气"，价格回到 MA60 附近。如果此时均线仍向上、价格不跌破均线，说明主力仍在掌控，只是洗掉部分浮筹。</p>
<p>这就像公司股价涨了一波后，机构投资者在等"回调"买入——L1 就是这个回调的底部。</p>`,
      howTo: [
        '<b>确认牛市</b>：MA60 向上（近 10 根斜率 > 0.3%）。',
        '<b>等待回踩</b>：价格从上方高点回落至 MA60 ±2% 范围内。',
        '<b>验证未破</b>：收盘价 ≥ MA60（即使下影穿过也不算跌破）。',
        '<b>等待反弹信号</b>：次日收阳线（或小时线转阳），确认买入。',
      ],
      params: {
        '均线': '通常 MA60（定性线）或 MA20（节奏线）',
        '距离': '<code>±2%</code>',
        '均线方向': '向上（斜率 > 0.3% / 10 根）',
      },
      strategy: `<ul>
<li><b>仓位</b>：L1 级别买入可加仓 30-50%（已持仓者）或全仓建立（空仓者）。</li>
<li><b>止损</b>：收盘跌破 MA60 - 3% 止损。</li>
<li><b>止盈</b>：到 L4 级别（远离均线 8% 以上）再考虑部分止盈。</li>
</ul>`,
      mistakes: [
        '<b>MA60 下方买 L1</b>：看起来像回踩但均线方向向下，其实是反弹诱多。',
        '<b>用短均线 L1</b>：MA5 / MA10 的 L1 信号太频繁，可靠度低。',
        '<b>等收盘前买</b>：价格可能在当日再跌破均线，等收盘确认后再买。',
      ],
      example: `<b>BTC 2020/11 L1 经典案例</b>：BTC 在 18000 附近高点后回落至 MA60 @ 15800（跌幅 -12%），未跌破。次日阳线确认反弹。L1 买入后 30 天涨至 29000（+83%）。`,
      quotes: [
        { text: 'L1 买入：价格回落至 MA 附近 + MA 上升 + 不跌破。', source: 'ma p.96' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l1</code>',
        'API': '<span class="kb-chip">granville[].kind=L1Buy</span>',
      },
      tags: ['葛南维', 'L1', '回踩', '买点', 'bull'],
    },
    {
      id: 'ma-granville-l2',
      group: 'ma',
      title: '葛南维 L2：跌破均线后迅速拉回的买点',
      badges: [{ text: 'L2 买入', kind: 'bull' }],
      desc: '牛市中价格短暂跌破 MA60 但 3 根内又回到均线上方 —— 这是"主力洗盘"的典型特征，是第二买点。',
      whatIs: `<p>L2 比 L1 更隐蔽也更凶险。价格短暂跌破 MA60，此时大部分散户以为"牛市结束了"而恐慌出逃，主力趁机低位吸筹，然后快速拉回均线上方。</p>
<p>这种"跌破后立即拉回"的走势，反而证明主力控盘力强 —— 他们不允许价格在 MA60 下方待太久。</p>`,
      howTo: [
        '<b>牛市基础</b>：MA60 必须向上。',
        '<b>观察跌破</b>：价格跌破 MA60（下影 + 收盘）。',
        '<b>等待回拉</b>：3 根 K 线内，收盘价重新站上 MA60。',
        '<b>确认量能</b>：回拉那根放量，跌破那根缩量（说明没有真正抛压）。',
      ],
      params: {
        '拉回窗口': '<code>3 根 K 线内</code>',
        '跌破深度': '通常 < 3%（超过 3% 则有效跌破）',
      },
      strategy: `<ul>
<li><b>仓位</b>：L2 级别可加仓 30-50%。</li>
<li><b>止损</b>：再次跌破 MA60 且连续 3 根不回 → 止损。</li>
</ul>`,
      mistakes: [
        '<b>第一次跌破就买</b>：必须等回拉确认，否则容易买在真正的断头铡刀（断头铡刀也是一根跌破，但不会拉回）。',
        '<b>跌破 > 3% 也当 L2</b>：超过 3% 属于有效跌破，不是 L2 而是趋势反转预警。',
      ],
      quotes: [
        { text: 'L2 买入：跌破后 3 根内拉回，多头控盘。', source: 'ma p.98' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l2</code>',
      },
      tags: ['葛南维', 'L2', '跌破拉回', '买点', '洗盘'],
    },
    {
      id: 'ma-granville-l3',
      group: 'ma',
      title: '葛南维 L3：均线上方反弹未创新高的卖点',
      badges: [{ text: 'L3 卖出', kind: 'bear' }],
      desc: '牛市末期价格反弹但未超过前高，且反弹乏力 —— 是牛市转弱的早期信号，应减仓观望。',
      whatIs: `<p>L3 是牛市即将结束的前兆：连续几波反弹都无法创新高，力度越来越弱。多头力量在衰竭。</p>
<p>此时虽然 MA60 可能还在向上，但价格的高点一次比一次低，呈现顶部震荡特征 —— 这是主力"逐步出货"的标志。</p>`,
      howTo: [
        '<b>找到前高</b>：近期 20-50 根内的最高价。',
        '<b>观察反弹</b>：价格反弹至 MA20 或 MA60 附近后回落。',
        '<b>验证未创新高</b>：本次反弹高点 < 前高（≥ 2% 差距）。',
        '<b>量能萎缩</b>：反弹量能 < 前次上涨量能。',
      ],
      strategy: `<ul>
<li><b>减仓至 50%</b>：L3 是预警级别，不必清仓但需降风险。</li>
<li><b>提高止损</b>：从原先的 MA60 - 3% 提高到前期 Swing Low。</li>
</ul>`,
      mistakes: [
        '<b>L3 就做空</b>：L3 是减仓信号不是做空信号，做空应等 L5 / L6。',
      ],
      quotes: [
        { text: 'L3 卖出：反弹未创新高 + 量能萎缩。', source: 'ma p.100' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l3</code>',
      },
      tags: ['葛南维', 'L3', '反弹未创高', '卖点'],
    },
    {
      id: 'ma-granville-l4',
      group: 'ma',
      title: '葛南维 L4：远离均线的超买卖点（BIAS > 8%）',
      badges: [{ text: 'L4 卖出', kind: 'bear' }, { text: '警报级', kind: 'warn' }],
      desc: '牛市中价格远远超过 MA60，乖离率 BIAS > 8%（或 +3σ），市场进入超买状态 —— 必须减仓至 30% 以下。',
      whatIs: `<p>L4 是"涨得太猛"的警报。<b>BIAS = (价格 - MA60) / MA60</b>，当 BIAS > 8%（不同市场阈值不同），意味着价格跑得比均线快得太多，迟早会回归。</p>
<p>L4 不一定意味着立即下跌，但继续加仓风险极高。原书明确：<b>"L4 信号出现时，仓位必须 ≤ 30%"</b>。这是硬规则。</p>`,
      howTo: [
        '<b>计算 BIAS</b>：(当前价 - MA60) / MA60 × 100%。',
        '<b>检查阈值</b>：BIAS > 8%（加密市场可用 12%）即触发 L4。',
        '<b>观察背离</b>：BIAS 创新高但价格未创新高 = 顶部背离，强化 L4 信号。',
      ],
      params: {
        'BIAS 阈值': '<code>8%</code>（股票）/ <code>12%</code>（加密）',
        '仓位上限': '<code>30%</code>（强制）',
      },
      strategy: `<ul>
<li><b>强制减仓至 30%</b>：PositionLimit.L4_MAX = 0.30。</li>
<li><b>设置止盈</b>：分批卖出剩余仓位。</li>
<li><b>不反向做空</b>：L4 只是警报，不是反转。</li>
</ul>`,
      mistakes: [
        '<b>全仓持有不减</b>：继续涨可能带来短期收益，但承担的回撤风险与持仓不对等。',
        '<b>L4 做空</b>：大忌。BIAS 超买可以延续很久（参考 DOGE 2021 年连涨 10 倍）。',
      ],
      example: `<b>ETH 2021/5/12 顶部 L4 警报</b>：ETH @ 4350，MA60 @ 3100，BIAS = +40%，早已超 8% 阈值。未减仓者随后经历 2022 年熊市，跌至 880（-80%）。减仓至 30% 者仅损失 24%。`,
      quotes: [
        { text: 'L4 = 价格远离均线 + BIAS > 8% + 仓位 ≤ 30%。', source: 'ma p.100' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l4</code>',
        '常量': 'L4_MAX = 0.30',
      },
      tags: ['葛南维', 'L4', 'BIAS', '超买', '警报', '减仓', '30%'],
    },
    {
      id: 'ma-granville-l5',
      group: 'ma',
      title: '葛南维 L5：均线下方反弹至均线的卖点',
      badges: [{ text: 'L5 卖出', kind: 'bear' }, { text: '顶级空点', kind: 'iron' }],
      desc: '空头排列中价格反弹至 MA60 附近，但未有效突破 —— 是最佳的顺势做空机会。',
      whatIs: `<p>L5 和 L1 是对称的：L1 是牛市中的回踩买，L5 是熊市中的反弹卖。价格反弹到均线附近，如果均线还向下，反弹就会在此受阻。</p>
<p>这是<b>做空最好的时机</b>之一，风险回报比最高（止损近、目标远）。</p>`,
      howTo: [
        '<b>确认熊市</b>：MA60 向下（近 10 根斜率 < -0.3%）。',
        '<b>等待反弹</b>：价格反弹至 MA60 ±2% 范围。',
        '<b>验证未破</b>：收盘价 < MA60。',
        '<b>等待下跌确认</b>：次日阴线，确认做空。',
      ],
      strategy: `<ul>
<li><b>做空时机</b>：L5 是最佳空点，止损设 MA60 + 3%。</li>
<li><b>目标位</b>：下一个 Swing Low 或 MA120。</li>
</ul>`,
      mistakes: [
        '<b>MA60 上方做"L5"</b>：均线方向向上时的反弹不是 L5，是 L1 / L2（买点）。方向判断不能错。',
      ],
      quotes: [
        { text: 'L5：空头排列中反弹至均线卖出，最佳空点。', source: 'ma p.104' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l5</code>',
      },
      tags: ['葛南维', 'L5', '反弹', '做空', 'bear'],
    },
    {
      id: 'ma-granville-l6',
      group: 'ma',
      title: '葛南维 L6：反弹未到均线的卖点',
      badges: [{ text: 'L6 卖出', kind: 'bear' }],
      desc: '空头排列中价格反弹力度弱，连 MA60 都摸不到就重新下跌 —— 空头力量极强，是加仓做空点。',
      whatIs: `<p>L6 比 L5 更弱，反弹幅度还没到均线就夭折。这说明<b>空头力量极强</b>，连反弹到均线都无法做到。</p>`,
      howTo: [
        '<b>反弹幅度</b>：未到 MA60（距离 > 3%）。',
        '<b>反弹时长</b>：通常 3-7 根 K 线内。',
        '<b>量能萎缩</b>：反弹期间量能萎缩。',
      ],
      strategy: '<p>L6 出现后，空头加仓；若已有空头，加仓 20-30%。</p>',
      quotes: [
        { text: 'L6：反弹无力，空头加仓点。', source: 'ma p.106' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l6</code>',
      },
      tags: ['葛南维', 'L6', '弱反弹', '做空'],
    },
    {
      id: 'ma-granville-l7',
      group: 'ma',
      title: '葛南维 L7：熊市中均线下方的短线反弹买点',
      badges: [{ text: 'L7 买入', kind: 'bull' }, { text: '短线', kind: '' }],
      desc: '空头排列中价格短暂下跌后反弹，但未触及 MA60 —— 是短线反弹买入机会（非主仓操作）。',
      whatIs: `<p>L7 是熊市中的<b>短线超跌反弹</b>机会。价格在均线下方大跌后，超卖反弹往往很快。但由于整体趋势向下，只适合短线操作。</p>`,
      strategy: `<ul>
<li><b>短线 3-7 天</b>：目标位 MA60 下方，到位即清仓。</li>
<li><b>不加大仓位</b>：L7 仅用 10-20% 仓位尝试。</li>
<li><b>反转信号</b>：若反弹突破 MA60 并站稳 3 根 → 升级为 L8。</li>
</ul>`,
      mistakes: [
        '<b>L7 当主买点</b>：熊市反弹幅度有限，风险回报低于顺势做空。',
      ],
      quotes: [
        { text: 'L7：熊市中超跌反弹，仅作短线。', source: 'ma p.108' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l7</code>',
      },
      tags: ['葛南维', 'L7', '超跌反弹', '短线'],
    },
    {
      id: 'ma-granville-l8',
      group: 'ma',
      title: '葛南维 L8：远离均线的超卖买点（BIAS < -8%）',
      badges: [{ text: 'L8 买入', kind: 'bull' }, { text: '顶级买点', kind: 'iron' }],
      desc: '熊市末期价格远远低于 MA60，BIAS < -8%（超卖）——市场恐慌已极致，反弹概率大增。',
      whatIs: `<p>L8 是"跌得太惨"的机会。当 BIAS 低于 -8%，市场的恐慌情绪已经充分释放，往往接近底部。</p>
<p>L8 是一切抄底信号中最纪律化的 —— 它不看主观判断，只看客观的 BIAS 数字。</p>`,
      params: {
        'BIAS 阈值': '<code>-8%</code>（股票）/ <code>-20%</code>（加密，波动更大）',
      },
      strategy: `<ul>
<li><b>分批买入</b>：BIAS 达到 -8% 买 1/3；-12% 再买 1/3；-20% 最后 1/3。</li>
<li><b>止损</b>：跌破前期低点 3%。</li>
<li><b>止盈</b>：回到 MA60 或 L4 级别再考虑减仓。</li>
</ul>`,
      mistakes: [
        '<b>L8 重仓一把抄底</b>：BIAS 超卖可以延续（尤其加密黑天鹅），必须分批。',
        '<b>L8 不止损</b>：极端行情下 BIAS 可以继续扩大（参考 LUNA 归零），止损是必要的。',
      ],
      example: `<b>BTC 2022/11/10 L8 底部</b>：BTC 跌至 15500，MA60 @ 19000，BIAS = -18%。AURA L8 触发。随后 2 个月反弹至 24000（+55%）。`,
      quotes: [
        { text: 'L8：远离均线超卖，反弹概率极高。', source: 'ma p.110' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/granville.rs::scan_l8</code>',
      },
      tags: ['葛南维', 'L8', '超卖', '抄底', 'BIAS'],
    },
    {
      id: 'ma-cross',
      group: 'ma',
      title: '金叉 / 死叉（带斜率确认）',
      badges: [{ text: 'E5 修复', kind: 'iron' }, { text: '最常用信号', kind: 'bull' }],
      desc: '均线交叉是技术分析最古老也最常用的信号之一。AURA 在原书基础上增加 <b>斜率同向确认</b>，避免横盘震荡产生的假金叉 / 假死叉。',
      whatIs: `<p><b>金叉</b>：短周期均线（如 MA5）从下向上穿过长周期均线（如 MA20），表示短期动能强于长期，是<b>买入信号</b>。</p>
<p><b>死叉</b>：短周期均线从上向下穿过长周期均线，表示短期动能弱于长期，是<b>卖出信号</b>。</p>
<p><b>但问题来了</b>：在横盘震荡市中，两条均线会频繁交叉但价格没有真正趋势。若仅凭"交叉"就买卖，会被反复"割韭菜"。因此原书强调：<b>真金叉必须伴随短均线本身也在向上</b>，而不只是比长均线高。AURA 通过 <b>5 根斜率回看</b> 实现这个铁证规则。</p>`,
      diagram: `<span class="d-mute">   价格 ↑</span>
<span class="d-mute">        │</span>           <span class="d-bull">MA5 (短)</span>
<span class="d-mute">        │         ╱─────</span>     <span class="d-bull">✓ 真金叉</span>
<span class="d-mute">        │       ╱─────</span>       <span class="d-bull">(两线同向向上)</span>
<span class="d-mute">        │     ╱╱</span>               MA20 (长)
<span class="d-mute">        │   ╱╱</span>
<span class="d-mute">        │  ╳  ← 交叉点 @ 斜率都 > 0</span>
<span class="d-mute">        │ ╱</span>
<span class="d-mute">        │╱</span>
<span class="d-mute">        └─────────────────→</span> 时间

<span class="d-mute">   价格 ↑</span>
<span class="d-mute">        │    ╱╲</span>                <span class="d-warn">✗ 假金叉</span>
<span class="d-mute">        │   ╱  ╲  ╱╲</span>           <span class="d-warn">(短均线反弹后掉头)</span>
<span class="d-mute">        │  ╳    ╳  ╲</span>
<span class="d-mute">        │ ╱      ╲  ╲</span>
<span class="d-mute">        │╱________╲__╲</span>          ← 横盘震荡
<span class="d-mute">        └─────────────────→</span>`,
      howTo: [
        '<b>观察两条均线</b>：选定快慢两个周期（常用 5/20 或 20/60）。',
        '<b>判断位置</b>：确认当前 MA 快线刚刚从下方穿到长线上方（或反之）。',
        '<b>验证斜率</b>：观察穿越点前 5 根 K 线的 MA 快线走势 —— 若明显向上为真金叉，若横盘或弯头为假金叉（AURA 降级为 PlainUp）。',
        '<b>对照价格</b>：真金叉时价格通常刚突破 MA20，而不是已大幅偏离。',
        '<b>检查成交量</b>：真金叉常伴随放量，假金叉缩量。',
      ],
      params: {
        '快线默认': '<code>MA5</code>',
        '慢线默认': '<code>MA20</code>（或 MA60 用于长期）',
        '斜率回看': '<code>5 根 K 线</code>',
        '同向阈值': '快线斜率 × 慢线斜率 > 0（不同号则降级）',
      },
      strategy: `<ul>
<li><b>真金叉 + 多头排列</b>：L3 级信号，可建仓 50-70%，止损设 MA20 下方 1 ATR。</li>
<li><b>真金叉 + 价格 > MA60</b>：L4+ 级（高于 60 日定性线），可重仓 80-100%。</li>
<li><b>真死叉 + 空头排列</b>：立即清仓或反手做空（若允许），参考 ma p.380 "卖出不等确认"。</li>
<li><b>PlainUp / PlainDown（降级）</b>：忽略或小仓观察，不作主交易依据。</li>
</ul>`,
      mistakes: [
        '<b>只看交叉不看斜率</b>：横盘期每 5-10 根就出现一次交叉，纯靠交叉交易是新手最大亏损点。',
        '<b>在 60 日均线下方买金叉</b>：价格低于长期定性线时，即便金叉也只是下跌中继反弹。',
        '<b>MA5 / MA10 短周期金叉高频</b>：适合日内交易者，对周线或月线交易者来说噪音太多。',
        '<b>忽视价量配合</b>：假金叉通常缩量，真金叉必放量（≥ 1.5× 均量）。',
        '<b>金叉后追高</b>：正确做法是等价格回踩 MA20 再进场，而不是交叉当根 K 线追涨。',
      ],
      example: `<b>BTCUSDT 4h 真金叉案例</b>：假设 MA5 在 62000 处穿越 MA20。过去 5 根 K 线 MA5 斜率为 +0.8%，MA20 斜率为 +0.3%，均为正且价格站稳 MA60（60000）之上 —— AURA 标记为 <b>CrossKind::Golden</b>，触发 L3 买入信号。订单：买入 60%，止损 60500（-2.4%），止盈 65000（+4.8%）。`,
      quotes: [
        { text: '金叉必须伴随短均线斜率向上，否则只是横盘穿越。', source: 'ma Ch3 均线交叉章' },
        { text: '横盘中的交叉信号不可信，应等待明确趋势。', source: 'ma p.85' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/alignment.rs:93-195</code>',
        '枚举': 'CrossKind::{Golden, Death, PlainUp, PlainDown}',
        'API': '<span class="kb-chip api">/api/ma_state</span> <span class="kb-chip">crosses[]</span>',
        '修复': 'E5 (Sprint 0 Patch 1) · 12 个测试',
      },
      tags: ['金叉', '死叉', 'cross', 'golden', 'death', '斜率', 'MA5', 'MA20'],
    },
    {
      id: 'ma-spread',
      group: 'ma',
      title: '粘合 / 发散 / 收敛',
      badges: [],
      desc: '多条均线两两价差 / 均值 < 阈值即为<b>粘合</b>（蓄势）；<b>发散</b>即价差快速扩大；<b>收敛</b>为均线逐渐靠拢。粘合后突破 = 旱地拔葱，粘合后下跌 = 毒蜘蛛。',
      quotes: [
        { text: '均线粘合是短暂平衡，突破方向决定新趋势。', source: 'ma Ch3' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs</code>',
        'API': '<span class="kb-chip api">/api/ma_state</span>',
        '字段': 'spread_state',
      },
      tags: ['粘合', '发散', '收敛', 'spread'],
    },
    {
      id: 'ma-guillotine',
      group: 'ma',
      title: '断头铡刀（ma 顶级卖出信号）',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '71% 胜率', kind: 'bear' }, { text: 'R-P1-53', kind: '' }],
      desc: '一根巨阴线从上方跌穿多条均线（尤其是 60 日定性线），带领短中长均线向下调头。<b>BTC/ETH 真实数据回测：胜率 71%，α = +1.37%</b>，是 ma 全书最强的死亡信号之一。',
      whatIs: `<p>想象一把巨大的<b>铡刀</b>从上方斩下 —— 一根长长的阴线（红色 K 线）一次性跌穿 MA5 / MA10 / MA20 / MA60 等多条均线，把"头"（价格）从"身体"（均线系统）上砍断。</p>
<p>为什么这么凶？因为它同时满足：<b>(1) 价格暴跌；(2) 一次性击穿多条均线的支撑；(3) 均线本身也开始跟随向下翻头</b>。这三点叠加意味着市场情绪 180° 转向，主力和散户共同出逃。</p>
<p>原书 ma p.310 强调："断头铡刀之前 60 日均线一直下行" —— 即如果长期趋势本已走弱，铡刀出现就是<b>确认</b>；如果 60 日均线本在向上，铡刀可能只是洗盘（需进一步观察）。</p>`,
      diagram: `<span class="d-mute">   价格 ↑</span>
<span class="d-bull">        │ ────╲</span>            ← 之前 MA 系统向上
<span class="d-bull">        │ ═══╲ ╲</span>            MA5/20/60 粘合
<span class="d-bull">        │ ═══╲ ╲═╲</span>
<span class="d-mute">        │</span> <span class="d-bear">████</span>               <span class="d-bear">💥 巨阴线（铡刀）</span>
<span class="d-mute">        │</span> <span class="d-bear">████</span>               <span class="d-bear">一根跌穿 MA5/20/60</span>
<span class="d-mute">        │</span> <span class="d-bear">████</span>               <span class="d-bear">实体 ≥ 3% ATR</span>
<span class="d-mute">        │</span> <span class="d-bear">████</span>_____          <span class="d-bear">量 ≥ 2× 均量</span>
<span class="d-mute">        │     ╲  ╲═══</span>       ← 均线集体向下
<span class="d-mute">        │      ╲   ╲═══</span>
<span class="d-mute">        └─────────────→</span>`,
      howTo: [
        '<b>找到一根特别长的阴线</b>：实体占 K 线总长 ≥ 70%，且实体 ≥ 3 × 近期 ATR（普通实体的 3 倍）。',
        '<b>确认穿越 ≥ 3 条均线</b>：这根阴线开盘在 MA5 之上，收盘跌破 MA20，中间穿越了 MA10 / MA20（含）。',
        '<b>验证 MA60 位置</b>：收盘价也跌破 60 日定性线（核心铁证），此为强版铡刀；未破则为弱版。',
        '<b>检查均线方向</b>：铡刀出现后的 3-5 根，至少 MA5 / MA10 / MA20 全部向下弯头。',
        '<b>放量确认</b>：成交量 ≥ 20 均量 × 2。缩量铡刀可能是诱空。',
      ],
      params: {
        '实体占比': '<code>≥ 70%</code>',
        '实体大小': '<code>≥ 3 × ATR(14)</code>',
        '穿越均线': '<code>≥ 3 条</code>（MA5/10/20 含一条 MA60）',
        '量能': '<code>≥ 2 × vol_ma20</code>',
        '60 日要求': '收盘跌破 MA60（强版）/ 接近 MA60（弱版）',
      },
      strategy: `<ul>
<li><b>持仓者</b>：<b>立即清仓 100%</b>（ma p.380 "卖出不等确认"）。不等反弹，不求精确卖在最高点。</li>
<li><b>空仓者</b>：反手做空（如允许），止损设在铡刀 K 线最高点上方 1 ATR。</li>
<li><b>目标位</b>：第一目标 = 铡刀长度 × 1 向下延伸；第二目标 = 前期支撑。</li>
<li><b>与其他信号共振时</b>：若同时出现倾盆大雨 / S6 卖出 / 死亡谷 = <b>L8 顶级共振</b>，信号权重翻倍。</li>
</ul>`,
      mistakes: [
        '<b>大阴线就当铡刀</b>：必须同时穿越多条均线且 MA60 确认，否则只是普通急跌。',
        '<b>缩量铡刀照做空</b>：缩量铡刀约 40% 概率是诱空（主力洗盘），应观察次日能否跟进放量收阴。',
        '<b>第三次第四次铡刀照做空</b>：信号衰减原则（ma p.360）—— 连续出现的铡刀，第一次最有效，第 N 次（N ≥ 3）已接近底部，反而是反弹机会。',
        '<b>不看大周期</b>：日线铡刀在周线上升趋势中出现，可能只是大趋势中的调整。',
        '<b>无止损做空</b>：铡刀次日可能出现反包阳（绝地反击），必须严格止损。',
      ],
      example: `<b>真实回测</b>：AURA 在 BTC / ETH 2018-2024 年日线上识别出 <b>14 次断头铡刀</b>，其中 10 次后续 20 根内跌幅 ≥ 5%（胜率 71.4%），平均超额收益 α = +1.37%（相对持仓不动）。最经典案例：<b>BTC 2022/5/12</b>（LUNA 崩盘日）与 <b>BTC 2022/11/8</b>（FTX 事件）均出现完美铡刀，AURA 均提前 1-2 日给出预警。`,
      quotes: [
        { text: '断头铡刀 + 倾盆大雨 + S6 卖出 + 死亡谷（四大共振），顶级卖出信号。', source: 'ma p.310' },
        { text: '断头铡刀之前，60 日均线一直下行，这是必要前提。', source: 'ma p.310' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/advanced.rs</code>',
        '测试': '8 个测试用例',
        'API': '<span class="kb-chip api">/api/signals</span> <span class="kb-chip">advanced_ma_events.kind=Guillotine</span>',
        '验证': '<code class="kb-file">examples/validate_new_patterns.rs</code>',
        '历史胜率': 'BTC/ETH 日线 71.4%（Sprint 8）',
      },
      tags: ['断头铡刀', 'guillotine', '71%', '死亡', '铁证', '卖出', 'L8'],
    },
    {
      id: 'ma-desert',
      group: 'ma',
      title: '旱地拔葱',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '看涨', kind: 'bull' }],
      desc: '长期粘合后一根大阳线放量向上突破，均线跟随。<b>放量是关键</b> —— 成交量 > 3×20 均量方有效。缩量拔葱容易假突破。',
      quotes: [
        { text: '长粘合后放量突破，主力启动行情。', source: 'ma Ch4' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/advanced.rs</code>',
        'API': '<span class="kb-chip api">/api/signals</span>',
        '字段': 'advanced_ma_events[].kind == "DesertBreakout"',
      },
      tags: ['旱地拔葱', 'desert', '粘合', '突破'],
    },
    {
      id: 'ma-spider',
      group: 'ma',
      title: '毒蜘蛛',
      badges: [{ text: '看跌', kind: 'bear' }],
      desc: '均线空头排列 + 短中长纠缠（相互穿越），如同蜘蛛网困住价格。一旦形成，通常伴随 <b>6-10 根震荡蓄势</b>，然后向下爆发。',
      quotes: [
        { text: '均线空头纠缠，价格如入蜘蛛网。', source: 'ma Ch4' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/advanced.rs</code>',
        'API': '<span class="kb-chip api">/api/signals</span>',
      },
      tags: ['毒蜘蛛', 'spider', '纠缠', '空头'],
    },
    {
      id: 'ma-repair',
      group: 'ma',
      title: '主动修复 vs 被动修复',
      badges: [],
      desc: '<b>主动修复</b>：价格横盘，均线自行追上价格（强势特征）。<b>被动修复</b>：价格下跌补齐乖离（弱势）。AURA 通过 BIAS 变化率 + 均线斜率的组合识别。',
      quotes: [
        { text: '主动修复是强势横盘，被动修复是补跌。', source: 'ma p.280' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/repair.rs</code>',
        '测试': '7 个测试用例',
      },
      tags: ['修复', 'repair', '主动', '被动', 'BIAS'],
    },
    {
      id: 'ma-qiguan',
      group: 'ma',
      title: '气贯长虹',
      badges: [{ text: '看涨', kind: 'bull' }],
      desc: '一根大阳线一举穿越多条长期均线（向上），并收在所有均线之上。极强势的转势信号，通常出现在长期盘整末尾。',
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/advanced.rs</code>',
        'API': '<span class="kb-chip api">/api/signals</span>',
      },
      tags: ['气贯长虹', '大阳线', '突破'],
    },
    {
      id: 'ma-dualline',
      group: 'ma',
      title: '双线组合（60 日定性 × 20 日节奏）',
      badges: [{ text: '铁证 6 条', kind: 'iron' }],
      desc: '原书 ma p.200 给出的 6 条铁证组合：<b>定性线上行 + 节奏线回踩</b> = 买，<b>定性线下行 + 节奏线反弹</b> = 卖。AURA 实现完整的 6 条规则。',
      quotes: [
        { text: '定性线 60 日决定长期趋势；节奏线 20 日决定短期进出。', source: 'ma p.200' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/dual_line.rs</code>',
        '测试': '8 个测试用例',
        '规则': '定性上行-节奏回踩 / 定性下行-节奏反弹 / 双线多头 / 双线空头 / 定性改变 / 节奏先行',
      },
      tags: ['双线', 'dual_line', '定性', '节奏', '60日', '20日'],
    },
    {
      id: 'ma-bias',
      group: 'ma',
      title: 'BIAS 乖离率',
      badges: [],
      desc: 'BIAS = (价格 - 均线) / 均线。正 BIAS 表示价格偏离均线上方，负则下方。<b>BIAS > 8%</b> 触发葛南维 L4 警告（超买）；<b>BIAS < -8%</b> 触发 L8（超卖）。',
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/compute.rs::bias</code>',
        'API': '<span class="kb-chip api">/api/ma_state</span>',
        '字段': 'bias_base / bias_base_period',
      },
      tags: ['BIAS', '乖离率', 'L4', 'L8'],
    },
    {
      id: 'ma-bull-trap',
      group: 'ma',
      title: '多头陷阱（Bull Trap）',
      badges: [{ text: '陷阱', kind: 'bear' }, { text: 'R-P1-17', kind: '' }],
      desc: '价格向上突破某关键阻力后未能站稳 3 根以上，随后快速跌回 —— 多头被"关在楼顶"的经典陷阱。',
      whatIs: `<p>市场的"狼来了"。价格突破前高或 MA60 或颈线，所有看涨信号都亮起，散户纷纷买入追涨。但突然间价格掉头向下，迅速跌回突破点之下 —— 追涨者被"套在山顶"。</p>
<p>这是主力在<b>诱多出货</b>：故意制造假突破把散户吸引进来，然后大量抛售。原书总结规律：<b>真突破通常一气呵成（3% 以上 + 连续站稳）；假突破常常"突而不破"</b>（刚过去又跌回）。</p>`,
      howTo: [
        '<b>识别突破</b>：价格突破关键阻力（前高、MA60、颈线）。',
        '<b>监控 3 根 K 线</b>：突破后 3 根内是否保持在突破价之上？',
        '<b>验证幅度</b>：突破幅度是否 ≥ 3%？',
        '<b>识破陷阱</b>：若 3 根内跌回突破价下方 → 多头陷阱确认。',
      ],
      params: {
        '确认窗口': '<code>3 根 K 线</code>',
        '有效阈值': '<code>3%</code>',
        '跌回条件': '收盘回到突破价下方',
      },
      strategy: `<ul>
<li><b>追涨者止损</b>：买入后价格回到突破价下方，立即止损。</li>
<li><b>反向做空</b>：确认陷阱后可顺势做空，止损设突破点上方。</li>
<li><b>避免追涨</b>：突破当根 K 线不追，等第 2 根站稳再确认。</li>
</ul>`,
      mistakes: [
        '<b>一突破就追</b>：90% 的追涨最终成为陷阱的受害者。',
        '<b>陷阱里不止损</b>：希望反弹到成本价再走，通常会越亏越深。',
      ],
      example: `<b>BTC 2021/11/8 多头陷阱</b>：BTC 突破 67000 历史新高到 69000（+3%），但次日即跌回 67000 下方。3 天后跌到 61000（-10%），追涨者集体被套。AURA 识别并标记为 BullTrap，建议做空。`,
      quotes: [
        { text: '假突破是主力诱多出货的常用手段。', source: 'ma p.200 主力行为' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/bull_trap.rs</code>',
        '测试': '7 个测试用例',
        'API': '<span class="kb-chip api">/api/signals</span> bull_traps',
      },
      tags: ['多头陷阱', 'bull_trap', '假突破', '诱多', '陷阱'],
    },
    {
      id: 'ma-bear-trap',
      group: 'ma',
      title: '空头陷阱（Bear Trap）',
      badges: [{ text: '陷阱', kind: 'bull' }],
      desc: '价格向下跌破关键支撑后迅速拉回 —— 空头被"关在楼底"，追空者被套。与多头陷阱完全对称。',
      whatIs: `<p>与多头陷阱对称。价格跌破重要支撑（前低、MA60、颈线），所有看跌信号亮起，散户恐慌割肉或追空。然后价格突然拉升，跌破者被套。</p>
<p>空头陷阱是<b>主力诱空吸筹</b>的典型手法：故意砸盘触发止损，在极低价位吸走廉价筹码，然后快速拉升。</p>`,
      howTo: [
        '<b>识别跌破</b>：价格跌破关键支撑。',
        '<b>3 根确认</b>：跌破后是否 3 根内拉回支撑上方？',
        '<b>成交量</b>：跌破缩量 + 拉回放量 = 典型陷阱。',
      ],
      strategy: `<p>确认陷阱后反向做多，止损设跌破最低点下方；追空者应立即平仓。</p>`,
      quotes: [
        { text: '恐慌砸盘后快速拉回是主力吸筹的信号。', source: 'ma Ch3' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/bull_trap.rs (对称实现)</code>',
      },
      tags: ['空头陷阱', 'bear_trap', '假跌破', '诱空'],
    },
    {
      id: 'ma-death-valley',
      group: 'ma',
      title: '死亡谷（Death Valley）',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '看跌', kind: 'bear' }],
      desc: '短、中、长三条均线先后向下交叉，形成一个向下的三角形 —— 最强的长期空头确认信号之一。',
      whatIs: `<p>想象三条均线（MA5 / MA20 / MA60）原本多头排列，现在开始"塌方"：</p>
<p><b>第 1 步</b>：MA5 先跌破 MA20（短中死叉）；</p>
<p><b>第 2 步</b>：MA20 接着跌破 MA60（中长死叉）；</p>
<p><b>第 3 步</b>：三条均线形成一个向下的"倒三角"图形，下方是深不可测的"死亡谷"。</p>
<p>这个过程通常持续 10-30 根 K 线，一旦形成，熊市基本确认。</p>`,
      diagram: `<span class="d-bull">      MA5  ╲</span>                <span class="d-mute">1. MA5 先跌破 MA20</span>
<span class="d-bull">    MA20 ══╲╲ ╲</span>          <span class="d-mute">2. MA20 后跌破 MA60</span>
<span class="d-bull">  MA60 ══════╲╲ ╲</span>         <span class="d-mute">3. 三角向下 = 死亡谷</span>
<span class="d-mute">    ╲╲ ╲ ╲</span>
<span class="d-mute">  ══╲╲ ╲ ╲</span>
<span class="d-bear">    ╲╲ ╲</span>
<span class="d-bear">      ╲╲</span>     <span class="d-bear">死亡谷</span>`,
      howTo: [
        '<b>观察 3 条均线</b>：MA5 / MA20 / MA60（或其他短中长组合）。',
        '<b>确认两次死叉</b>：短中死叉先发生，随后中长死叉。',
        '<b>时间间隔</b>：两次死叉间隔 < 30 根 K 线（越近越强）。',
        '<b>最终形态</b>：三条均线形成倒三角，价格在三角下方。',
      ],
      strategy: `<ul>
<li><b>清仓 + 反向做空</b>：死亡谷确认后牛市结束，应全仓退出。</li>
<li><b>目标位</b>：长期支撑（如 MA120）或前期重要低点。</li>
</ul>`,
      quotes: [
        { text: '死亡谷是三大均线向下交叉形成的倒三角。', source: 'ma p.280 均线特殊形态' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs</code>',
      },
      tags: ['死亡谷', 'death_valley', '倒三角', '熊市'],
    },
    {
      id: 'ma-golden-valley',
      group: 'ma',
      title: '金山谷 / 银山谷（Golden / Silver Valley）',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '看涨', kind: 'bull' }],
      desc: '死亡谷的反向形态：三条均线先后向上金叉形成向上三角形。金山谷（发生在长期底部）比银山谷（短期底部）更可靠。',
      whatIs: `<p>金山谷是死亡谷的镜像：空头排列中，短中长均线先后向上交叉：</p>
<p><b>第 1 步</b>：MA5 先上穿 MA20（短中金叉）；</p>
<p><b>第 2 步</b>：MA20 随后上穿 MA60（中长金叉）；</p>
<p><b>第 3 步</b>：三条均线形成向上三角，下方形成"山谷"。</p>
<p><b>金山谷 vs 银山谷</b>：出现在长期熊市底部 = 金山谷（高度可信）；出现在牛市中的短期回调后 = 银山谷（常见但可靠度中等）。</p>`,
      strategy: `<ul>
<li><b>金山谷 = 全面做多</b>：熊市反转信号，可重仓建仓。</li>
<li><b>银山谷 = 中型加仓</b>：牛市延续信号，加仓 30-50%。</li>
</ul>`,
      quotes: [
        { text: '金山谷在底部，银山谷在中继。', source: 'ma p.283' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs</code>',
      },
      tags: ['金山谷', '银山谷', 'golden_valley', '山谷', 'bull'],
    },
    {
      id: 'ma-bull-bear-boundary',
      group: 'ma',
      title: '牛熊分界（Bull-Bear Boundary）',
      badges: [{ text: 'MA60 核心', kind: 'iron' }],
      desc: '价格紧贴 MA60 附近波动 —— 多空胶着，未来方向取决于 MA60 的突破方向。',
      whatIs: `<p>牛熊分界是一种<b>关键决策时刻</b>。价格既不明显高于 MA60 也不明显低于，而是在 ±1% 范围内反复测试 MA60。</p>
<p>这种状态通常持续 5-20 根 K 线，是主力和散户博弈的焦点。一旦一方胜出：向上突破 → 回到牛市；向下跌破 → 开始熊市。</p>`,
      howTo: [
        '<b>价格贴近 MA60</b>：|价格 - MA60| / MA60 < 1%。',
        '<b>持续根数</b>：至少 5 根 K 线都在此范围内。',
        '<b>等待方向</b>：观察 MA60 的斜率，向上则看涨，向下则看跌。',
      ],
      strategy: `<p>牛熊分界期间<b>不做方向性交易</b>，等待明确突破。可挂区间单：上方 +1% 挂多单，下方 -1% 挂空单。</p>`,
      quotes: [
        { text: '牛熊分界是均线多空力量的焦点战场。', source: 'ma p.285' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs::BullBearBoundary</code>',
      },
      tags: ['牛熊分界', 'boundary', 'MA60', '胶着'],
    },
    {
      id: 'ma-fast-rise',
      group: 'ma',
      title: '快速上升 / 快速下降（价差扩大）',
      badges: [{ text: '趋势加速', kind: 'warn' }],
      desc: '短均线与长均线间距急剧扩大，价格动能加速 —— 既是趋势强化也是反转预警。',
      whatIs: `<p>当 MA5 与 MA60 之间的距离（spread = MA5/MA60 - 1）突然扩大到历史高位，说明价格正在<b>加速</b>：</p>
<p><b>快速上升</b>：spread > +10% 且仍在扩大 —— 牛市末期常见，需警惕 L4 超买。</p>
<p><b>快速下降</b>：spread < -10% 且仍在扩大 —— 熊市加速，L8 机会临近。</p>`,
      strategy: `<ul>
<li><b>快速上升</b>：减仓至 50%，准备接 L4 减仓信号。</li>
<li><b>快速下降</b>：分批准备 L8 抄底。</li>
</ul>`,
      mistakes: [
        '<b>加速时追涨</b>：spread 创新高往往是顶部，追进去就是 L4 警报位。',
      ],
      quotes: [
        { text: '均线扩大速度比绝对位置更重要。', source: 'ma p.290' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs::FastRise</code>',
      },
      tags: ['快速上升', '快速下降', 'spread', '加速'],
    },
    {
      id: 'ma-wave-up',
      group: 'ma',
      title: '逐浪上升 / 逐浪下降',
      badges: [{ text: '主流趋势', kind: 'bull' }],
      desc: '价格围绕 MA20 上下波动，但<b>波动中枢逐步抬升</b>（或下移）—— 经典的"走楼梯"式趋势。',
      whatIs: `<p>健康的牛市不会一路直线上涨，而是"涨 → 回调 → 再涨"，每次回调低点比上次高，每次高点也比上次高。把这些高低点连起来，会发现<b>中枢在逐步抬升</b>，就像走楼梯。</p>
<p>这种走势最稳健，也最容易操作：每次回调到 MA20 附近就是买点。</p>`,
      howTo: [
        '<b>识别至少 3 个 Swing 高点 + 3 个 Swing 低点</b>。',
        '<b>检查逐级抬升</b>：每个新高 > 上一个高点，每个新低 > 上一个低点。',
        '<b>中枢线</b>：连接相邻高点形成上升通道上沿，连接低点形成下沿。',
      ],
      strategy: `<ul>
<li><b>回调到通道下沿买入</b>：是最佳加仓点。</li>
<li><b>跌破通道下沿</b>：趋势结束，需警惕反转。</li>
</ul>`,
      quotes: [
        { text: '逐浪上升是健康牛市的标志。', source: 'ma p.295' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs::WaveUp</code>',
      },
      tags: ['逐浪上升', 'wave_up', '阶梯', '中枢抬升'],
    },
    {
      id: 'ma-waterfall',
      group: 'ma',
      title: '瀑布飞泻（Waterfall）',
      badges: [{ text: '极端下跌', kind: 'bear' }, { text: 'E1', kind: '' }],
      desc: '多条均线几乎成直线垂直下坠 —— 市场恐慌性抛售，跌停式走势。',
      whatIs: `<p>瀑布飞泻指的是图表上均线呈<b>陡峭的下降线</b>，几乎成 90° 垂直向下。价格一路跌破所有短中期支撑，没有任何像样的反弹。</p>
<p>这通常出现在重大利空事件（如 LUNA 崩盘 / FTX 事件）或系统性危机时期。一旦出现，不应尝试抄底，应等瀑布"流完"再观察。</p>`,
      strategy: `<ul>
<li><b>空仓观望</b>：瀑布期不抄底。</li>
<li><b>瀑布停止 = 等企稳信号</b>：等价格连续 3 根不创新低 + 放量后再考虑。</li>
</ul>`,
      mistakes: [
        '<b>瀑布中抄底</b>：接"飞刀"是新手最大错误。',
      ],
      example: `<b>LUNA 2022/5/9-5/12</b>：从 85 美元跌至 0.00001 美元，几乎归零。任何"抄底"都是灾难。`,
      quotes: [
        { text: '瀑布飞泻时，最好的操作是不操作。', source: 'ma p.298' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs::Waterfall</code>',
        '校准': 'E1 Patch 5 v2',
      },
      tags: ['瀑布', 'waterfall', '崩盘', '暴跌'],
    },
    {
      id: 'ma-mud-pit',
      group: 'ma',
      title: '烂泥潭（Mud Pit）',
      badges: [{ text: '横盘', kind: '' }, { text: 'E1', kind: '' }],
      desc: '均线完全缠绕、无方向，价格在一个狭窄区间反复波动 —— 最烂的交易环境，应回避。',
      whatIs: `<p>均线发散度极低（&lt; 1%），价格在狭窄区间（如 ±3%）内无规律波动 20 根以上 K 线 —— 这就是"烂泥潭"。</p>
<p>在烂泥潭里交易，进进出出都是假突破，手续费和滑点会慢慢吞噬本金。最好的策略是<b>休息，不操作</b>，等到趋势出现再回来。</p>`,
      strategy: `<p><b>唯一策略：不交易</b>。或者改用突破策略，挂区间上下沿单等真突破。</p>`,
      mistakes: [
        '<b>强行交易找手感</b>：烂泥潭里每次操作都是 50% 胜率的赌博。',
        '<b>不断修改策略</b>：参数调整解决不了问题，市场根本没有趋势可抓。',
      ],
      quotes: [
        { text: '烂泥潭时期，最大的勇气是休息。', source: 'ma p.300' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs::MudPit</code>',
      },
      tags: ['烂泥潭', 'mud_pit', '横盘', '震荡'],
    },

    // ==================== 趋势分析 ====================
    {
      id: 'tr-sr',
      group: 'trend',
      title: '支撑 / 阻力识别',
      badges: [],
      desc: '基于 Swing 高低点聚类，以 ±0.3% 为等价带合并。<b>触碰次数越多，强度越高</b>。当前价之上为阻力（红），之下为支撑（绿）；突破后角色翻转。',
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/sr.rs</code>',
        'API': '<span class="kb-chip api">/api/trend_state</span>',
        '字段': 'sr_levels[{price, kind, touches, role_history}]',
      },
      tags: ['支撑', '阻力', 'sr', 'level', 'swing'],
    },
    {
      id: 'tr-roleflip',
      group: 'trend',
      title: '角色翻转（突破后支撑变阻力）',
      badges: [{ text: 'E30', kind: 'iron' }],
      desc: '价格有效突破某 SR 位（幅度 ≥ 3%）后，<b>原阻力变支撑</b>，原支撑变阻力。AURA 在 <code>SrLevel.detect_role_flips</code> 中记录完整 role_history，前端也会根据当前价动态重新分类。',
      quotes: [
        { text: '突破后角色翻转，是技术分析三大假设之一（市场有记忆）。', source: 'trend p.40' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/sr.rs:86</code>',
        '修复': 'Sprint 2 E30',
        '前端': 'effectiveKind 动态分类',
      },
      tags: ['角色翻转', 'roleflip', '突破', '3%'],
    },
    {
      id: 'tr-lines',
      group: 'trend',
      title: '趋势线（多级矩阵）',
      badges: [{ text: 'R-P1-15', kind: 'iron' }],
      desc: '至少两个 Swing 点连线画趋势线。AURA 实现 <b>多级矩阵</b>：短 / 中 / 长期三级并存，短周期突破属正常波动，长周期跌破才是趋势逆转。',
      quotes: [
        { text: '短期突破是波动，长期突破是反转。', source: 'trend p.216' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs</code>',
        'API': '<span class="kb-chip api">/api/trend_state</span>',
        '字段': 'trend_lines[{p1, p2, slope, level}]',
      },
      tags: ['趋势线', 'trendline', '多级', '短中长'],
    },
    {
      id: 'tr-channel',
      group: 'trend',
      title: '通道（平行上下轨）',
      badges: [],
      desc: '趋势线 + 平行副线。通道内回归交易，突破上轨看延续，跌破下轨看反转。',
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs</code>',
        '字段': 'channel_upper / channel_lower',
      },
      tags: ['通道', 'channel', '平行', '上下轨'],
    },
    {
      id: 'tr-swings',
      group: 'trend',
      title: 'Swing 高低点（道氏基础）',
      badges: [],
      desc: 'ZigZag 算法的双重阈值版（价格 % + 时间 bar）。<b>Swing High</b> = 局部峰，<b>Swing Low</b> = 局部谷。所有趋势分析、S/R、Fib 都依赖 Swing。',
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/swing.rs</code>',
        'API': '<span class="kb-chip api">/api/trend_state</span>',
        '字段': 'swings[{index, price, kind}]',
      },
      tags: ['swing', '高低点', 'zigzag'],
    },
    {
      id: 'tr-dow',
      group: 'trend',
      title: '道氏理论 HH / HL / LH / LL',
      badges: [{ text: '理论', kind: 'iron' }],
      desc: '<b>Higher High + Higher Low</b> = 上升趋势；<b>Lower High + Lower Low</b> = 下降趋势。AURA 的 <code>state_machine</code> 模块根据 Swing 序列自动判定趋势阶段。',
      quotes: [
        { text: '更高点和更低点的连续出现，定义了趋势。', source: '道氏理论核心' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/state_machine.rs</code>',
        'API': '<span class="kb-chip api">/api/trend_state</span>',
        '字段': 'stage / trend_transitions',
      },
      tags: ['道氏', 'HH', 'HL', '趋势'],
    },
    {
      id: 'tr-fib',
      group: 'trend',
      title: 'Fibonacci 回撤（0.236 / 0.382 / 0.5 / 0.618 / 0.786）',
      badges: [],
      desc: '从最近显著 Swing High 到 Swing Low 绘制斐波那契回撤线。<b>0.618 是黄金回撤位</b>，最常被主力用作加仓点。前端通过"黄金"按钮切换显示。',
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/fib.rs</code>',
        'API': '<span class="kb-chip api">/api/trend_state</span>',
        '前端': '<code class="kb-file">app.js</code> show-fib 开关',
      },
      tags: ['Fibonacci', '黄金', '回撤', '0.618'],
    },
    {
      id: 'tr-reversal',
      group: 'trend',
      title: '趋势反转确认',
      badges: [],
      desc: '必须同时满足：<b>(1) 跌破最近长期上升趋势线</b>，<b>(2) 打破最低 Swing Low</b>，<b>(3) 放量</b>。AURA 在 <code>TransitionRecord</code> 中记录每次趋势阶段变化。',
      quotes: [
        { text: '三重确认才是真反转。', source: 'trend Ch3 惯性定律' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/state_machine.rs</code>',
        '字段': 'TransitionRecord.reason',
      },
      tags: ['反转', 'reversal', 'transition'],
    },
    {
      id: 'tr-dow-3rules',
      group: 'trend',
      title: '道氏理论三大假设（Dow Theory）',
      badges: [{ text: '理论基础', kind: 'iron' }],
      desc: '技术分析的基石理论，由 Charles Dow 在 19 世纪末提出。三大假设是所有图表分析的前提。',
      whatIs: `<p>道氏理论的三大假设是技术分析的"地基"：</p>
<ol>
<li><b>市场行为包容一切信息</b>：基本面、消息面、情绪面、资金面的一切因素都已反映在价格中。所以我们只需看图就够。</li>
<li><b>价格沿趋势运动</b>：趋势一旦形成就会延续，直到出现明确反转信号。这就是"顺势而为"的来源。</li>
<li><b>历史会重演</b>：因为人性不变（贪婪、恐惧），所以历史上成功的形态（头肩、双顶）今天仍然适用。</li>
</ol>
<p>如果你不同意这三条，技术分析对你没有意义。如果你同意，那么所有 K 线形态、趋势线、支撑阻力都是这三条的推论。</p>`,
      strategy: `<ul>
<li><b>第 1 条</b>：不必过度研究基本面，看图就够。</li>
<li><b>第 2 条</b>：顺势交易，永远不要逆势抄底或摸顶（除非你有 L8 级别证据）。</li>
<li><b>第 3 条</b>：相信历史形态的预测力，不要以为"这次不一样"。</li>
</ul>`,
      quotes: [
        { text: '道氏理论是一切技术分析之母。', source: 'trend Ch1 p.10' },
        { text: '市场总是对的，我们的观点不重要。', source: 'trend p.12' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/state_machine.rs</code>',
      },
      tags: ['道氏', 'dow', '三大假设', '理论基础'],
    },
    {
      id: 'tr-3levels',
      group: 'trend',
      title: '趋势三级别（主要 / 次要 / 短暂）',
      badges: [{ text: '分层思维', kind: 'iron' }],
      desc: '道氏理论将趋势分为三个级别：<b>主要趋势</b>（数月-数年）、<b>次要趋势</b>（数周-数月）、<b>短暂趋势</b>（数天）。不同时间框操作要匹配相应级别。',
      whatIs: `<p>想象海洋里有三种波动：</p>
<ul>
<li><b>潮汐</b>（主要趋势）：持续 1-3 年，如 BTC 2020-2021 牛市、2022 熊市。周线 / 月线判断。</li>
<li><b>波浪</b>（次要趋势）：持续数周-数月，是主要趋势中的回调或反弹。日线判断。</li>
<li><b>涟漪</b>（短暂趋势）：持续几天，是次要趋势中的波动。4h / 1h 判断。</li>
</ul>
<p><b>关键原则</b>：操作级别要匹配持有周期。日内交易者看 4h / 1h；波段交易者看日线；长期投资者看周线 / 月线。<b>不能混着看</b>，否则会被短期波动误导。</p>`,
      strategy: `<ul>
<li><b>先判断主要趋势</b>：看周线 MA60 方向。向上 = 牛市潮汐，只做多不做空。</li>
<li><b>再看次要趋势</b>：看日线 MA60。回调至日线支撑是买点。</li>
<li><b>最后看短暂趋势</b>：看 4h / 1h 决定具体入场时机。</li>
</ul>`,
      mistakes: [
        '<b>用 1h 图判断主要趋势</b>：1h 图只能看到涟漪，看不到潮汐。',
        '<b>短线交易用周线</b>：周线反转慢，短线用会错过时机。',
      ],
      quotes: [
        { text: '操作级别决定持有周期，混淆级别是亏损的根源。', source: 'trend p.30' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs (multi-level)</code>',
      },
      tags: ['趋势级别', '主要', '次要', '短暂', '潮汐', '波浪'],
    },
    {
      id: 'tr-inertia',
      group: 'trend',
      title: '惯性定律（Trend Inertia）',
      badges: [{ text: '核心原理', kind: 'iron' }],
      desc: '趋势一旦形成，会<b>倾向于继续</b>，直到出现明确反转信号。这就像牛顿第一定律，不是凭空的"想当然"。',
      whatIs: `<p>趋势为什么有惯性？三个原因：</p>
<ol>
<li><b>资金惯性</b>：大资金进场需要时间，出场也需要时间。主力建仓一次用 3-6 个月，不会在一天内改变方向。</li>
<li><b>情绪惯性</b>：牛市后期散户贪婪难改，熊市后期恐惧难消。情绪转变需要时间。</li>
<li><b>技术惯性</b>：趋势线、均线、支撑阻力都在延续方向上形成自我强化。</li>
</ol>
<p>所以<b>"顺势"永远比"逆势"容易赚钱</b>。即便逆势判断对了，也要付出更大的耐心和资金成本。</p>`,
      strategy: `<ul>
<li><b>顺势交易</b>：只做与主要趋势同向的交易。</li>
<li><b>不要猜顶摸底</b>：等明确的反转信号出现再操作。</li>
<li><b>持仓要有耐心</b>：趋势的惯性会给你耐心的回报。</li>
</ul>`,
      quotes: [
        { text: '惯性是趋势分析的第一定律。', source: 'trend Ch3 p.50' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/state_machine.rs</code>',
      },
      tags: ['惯性', 'inertia', '趋势', '顺势'],
    },
    {
      id: 'tr-break-effective',
      group: 'trend',
      title: '有效突破三要素',
      badges: [{ text: '铁证', kind: 'iron' }],
      desc: '不是所有突破都有效。原书明确要求三要素：<b>(1) 幅度 ≥ 3%</b>；<b>(2) 时间 ≥ 3 根</b>；<b>(3) 放量 ≥ 1.5×</b>。三者缺一不可。',
      whatIs: `<p>新手常犯的错：看到价格"突破"某条线就追。但实际上大部分"突破"都是假动作（诱多 / 诱空）。</p>
<p>原书给出 <b>"333 法则"</b>：</p>
<ul>
<li><b>幅度 3%</b>：突破价格 - 支撑/阻力位，差距至少 3%。</li>
<li><b>时间 3 根</b>：突破后至少 3 根 K 线收在突破位之外（不回穿）。</li>
<li><b>放量 3 倍</b>（保守：1.5 倍）：突破当根量能至少 1.5 倍 20 均量。</li>
</ul>
<p>只有三者同时满足才算真突破。缺一 = 可疑，缺两 = 大概率假，全缺 = 必是陷阱。</p>`,
      howTo: [
        '<b>检查幅度</b>：|突破价 - 参考位| / 参考位 ≥ 3%。',
        '<b>检查时间</b>：突破后的连续 3 根 K 线收盘都在突破位正确一侧。',
        '<b>检查量能</b>：突破根量能 ≥ 1.5 × vol_ma20。',
      ],
      params: {
        '幅度阈值': '<code>≥ 3%</code>',
        '时间确认': '<code>3 根 K 线</code>',
        '量能倍数': '<code>≥ 1.5 ×</code>',
      },
      strategy: '<p>三要素全满足 → 顺突破方向交易；缺任何一要素 → 视为陷阱，反向或观望。</p>',
      quotes: [
        { text: '有效突破必须同时满足三要素，缺一即为假。', source: 'trend p.204' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs::check_effective_break</code>',
      },
      tags: ['有效突破', '333法则', '3%', '放量', '时间'],
    },
    {
      id: 'tr-channel-up',
      group: 'trend',
      title: '上升通道 / 下降通道',
      badges: [{ text: '趋势结构', kind: 'bull' }],
      desc: '趋势线 + 平行副线形成的平行四边形区间，价格在其中有规律地波动 —— 趋势健康的标志。',
      whatIs: `<p>通道的构成：</p>
<ol>
<li><b>主趋势线</b>：连接至少 2 个 Swing 低点（上升）或 Swing 高点（下降）。</li>
<li><b>副线</b>：与主趋势线平行，经过相对一侧的 Swing。</li>
<li><b>通道内部</b>：价格往复运动的区域。</li>
</ol>
<p>健康的牛市 / 熊市都在通道内运行。价格从通道一侧到另一侧，再到一侧 —— 这是最赚钱的形态，因为<b>入场点和出场点都清晰</b>。</p>`,
      diagram: `<span class="d-bull">      ──────</span>         <span class="d-mute">上轨（阻力）</span>
<span class="d-bull">   ──────╱</span>
<span class="d-mute">  ╱       ╲</span>
<span class="d-bull">──────╱     ╲</span>
<span class="d-mute">      ╲     ╲</span>
<span class="d-bull">──────────── ╲</span>         <span class="d-mute">下轨（支撑）</span>
<span class="d-mute">   上升通道（每次回踩下轨 = 买点）</span>`,
      howTo: [
        '<b>画主趋势线</b>：连接 2-3 个 Swing 低点（上升通道）。',
        '<b>画平行副线</b>：平行平移，经过中间 Swing 高点。',
        '<b>检查平行度</b>：两条线近似平行（允许 ±10° 差异）。',
        '<b>验证触碰次数</b>：通道内至少 3 次触碰上下沿。',
      ],
      strategy: `<ul>
<li><b>下轨买 + 上轨卖</b>：通道交易的经典策略，胜率高。</li>
<li><b>突破上轨</b>：上升通道加速（强势），但后期可能反转。</li>
<li><b>跌破下轨</b>：通道结束，趋势变化。</li>
</ul>`,
      quotes: [
        { text: '通道是趋势的"港湾"，内部操作简单，突破后需重新评估。', source: 'trend Ch5' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs::Channel</code>',
      },
      tags: ['通道', 'channel', '上升通道', '下降通道', '平行'],
    },
    {
      id: 'tr-log-scale',
      group: 'trend',
      title: '对数坐标（Log Scale）',
      badges: [{ text: 'E29', kind: 'iron' }, { text: '长周期必备', kind: 'warn' }],
      desc: '长周期 / 大波动的图表必须使用对数坐标，否则形态识别完全扭曲。BTC 从 100 到 60000 的走势，线性坐标根本看不出来。',
      whatIs: `<p>想象 BTC 从 100 涨到 1000 是 <b>10 倍</b>，从 10000 涨到 100000 也是 10 倍。</p>
<ul>
<li><b>线性坐标</b>：第二段（+90000）会显示为巨大涨幅，第一段（+900）几乎看不到。</li>
<li><b>对数坐标</b>：两段涨幅（10 倍）会显示为相同高度，正确反映<b>百分比变化</b>。</li>
</ul>
<p>所以分析长期走势必须用对数坐标。AURA 的 log-scale 开关可在图表上一键切换。</p>
<p>对数坐标下，支撑阻力、趋势线、头肩形态等都会呈现"正确的"几何关系。</p>`,
      howTo: [
        '<b>周期选择</b>：周线 / 月线 / 年线必须用对数。',
        '<b>价格跨度</b>：如果图表最高价 / 最低价比 > 3 倍，必用对数。',
        '<b>形态识别</b>：在对数坐标下画趋势线 / 识别头肩。',
      ],
      strategy: '<p>长期持仓 / 大周期分析时切换到对数坐标；日内交易可保持线性。</p>',
      mistakes: [
        '<b>月线用线性坐标</b>：画出来的趋势线和真实完全不符，误判严重。',
      ],
      quotes: [
        { text: '对数坐标是长周期分析的必须品。', source: 'trend E29 Sprint 2' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/log_scale.rs</code>',
        '前端': '<code class="kb-file">app.js</code> log-scale 开关',
      },
      tags: ['对数坐标', 'log_scale', 'E29', '长周期'],
    },
    {
      id: 'tr-fib-extension',
      group: 'trend',
      title: 'Fibonacci 扩展（1.272 / 1.618 / 2.618）',
      badges: [{ text: '目标位', kind: 'bull' }],
      desc: '除了回撤比例（0.236-0.786），Fib 还有扩展比例，用于预测突破后的<b>目标位</b>。',
      whatIs: `<p>如果一波上涨从 100 到 150（幅度 50），那么基于 Fib 扩展：</p>
<ul>
<li><b>1.272 扩展</b>：100 + 50 × 1.272 = 163.6</li>
<li><b>1.618 扩展</b>（黄金目标）：100 + 50 × 1.618 = 180.9</li>
<li><b>2.618 扩展</b>：100 + 50 × 2.618 = 230.9</li>
</ul>
<p>通常价格回调到 0.382 / 0.618 后继续上涨，目标位就在这些扩展位。</p>`,
      strategy: `<ul>
<li><b>首要目标 1.272</b>：第一次止盈位。</li>
<li><b>次要目标 1.618</b>：黄金目标位，止盈主力。</li>
<li><b>终极目标 2.618</b>：极端行情下的潜在终点。</li>
</ul>`,
      quotes: [
        { text: 'Fibonacci 扩展是量化目标位的黄金工具。', source: 'trend Ch6' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/fib.rs::extension</code>',
      },
      tags: ['Fibonacci', 'extension', '扩展', '1.618', '目标位'],
    },
    {
      id: 'tr-psych-price',
      group: 'trend',
      title: '心理价位（整数关口）',
      badges: [{ text: '市场心理', kind: 'warn' }],
      desc: '人类对整数特别敏感：BTC 10000 / 20000 / 50000 / 100000 都是重要心理关口，突破前往往有激烈多空博弈。',
      whatIs: `<p>市场参与者的止损单、止盈单、挂单都倾向于集中在整数位。因此：</p>
<ul>
<li><b>整数阻力</b>：如 BTC 50000 / 60000 / 100000，突破前会多次受阻。</li>
<li><b>整数支撑</b>：跌破前会多次支撑。</li>
<li><b>"千元大关" / "万元大关"</b>等口语都有心理意义。</li>
</ul>
<p>AURA 的 Confluence 识别中，整数关口是一个独立的"类别"，可与 MA / SR / Fib 共同构成多合一。</p>`,
      strategy: '<p>接近整数关口时减仓 / 分批；突破 + 放量确认后重新加仓。</p>',
      quotes: [
        { text: '整数关口是人类心理的天然支撑阻力。', source: 'candle Ch1 市场心理' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/confluence.rs::PsychologicalPrice</code>',
      },
      tags: ['心理价位', '整数关口', 'psychological'],
    },
    {
      id: 'tr-trendline-invert',
      group: 'trend',
      title: '趋势线角色翻转',
      badges: [{ text: 'E30 扩展', kind: 'iron' }],
      desc: '跟水平 SR 类似，<b>趋势线也会角色翻转</b>：原上升支撑线被跌破并站稳后，变成阻力线。',
      whatIs: `<p>举例：BTC 从 10000 涨到 60000 的长期上升趋势线，有一天价格跌破这条线并在它下方站稳。之后如果 BTC 反弹到这条线附近，往往会受阻回落 —— 这就是<b>原支撑变阻力</b>。</p>
<p>反向同理：下降趋势线被有效突破后变成支撑。</p>`,
      strategy: '<p>趋势线被跌破后，反抽该线是做空的好机会；同理，下降趋势线被突破后，回踩是做多的好机会。</p>',
      quotes: [
        { text: '角色翻转是技术分析的普适原理。', source: 'trend E30' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs</code>',
      },
      tags: ['角色翻转', '趋势线', 'E30'],
    },
    {
      id: 'tr-multi-timeframe',
      group: 'trend',
      title: '多时间框共振（Multi-Timeframe Analysis）',
      badges: [{ text: '进阶', kind: 'iron' }],
      desc: '同一币种在不同时间框（月/周/日/4h/1h）上的方向是否一致？<b>多时间框共振</b>是 L8 级别信号的标配。',
      whatIs: `<p>操作任何一笔交易前，应检查多个时间框：</p>
<ol>
<li><b>月线</b>：长期趋势（牛市/熊市/震荡）。</li>
<li><b>周线</b>：中期趋势。</li>
<li><b>日线</b>：主要操作级别。</li>
<li><b>4h</b>：精确入场位置。</li>
<li><b>1h</b>：短线确认。</li>
</ol>
<p>如果 5 个时间框都看涨 = 绝佳买入机会（L8 级别共振）。如果方向冲突 = 放弃或只做最小时间框的短线。</p>`,
      howTo: [
        '<b>从大到小依次看</b>：先月线再 1h，而不是相反。',
        '<b>记录方向</b>：每个时间框当前是上升/下降/震荡？',
        '<b>统计一致性</b>：5 个都一致 = L8；4 个一致 = L6；3 个 = L4。',
      ],
      strategy: `<ul>
<li><b>5 框共振</b>：重仓（80-100%），持有到趋势反转。</li>
<li><b>3-4 框共振</b>：中型仓位（40-70%）。</li>
<li><b>冲突</b>：观望或只做短线。</li>
</ul>`,
      quotes: [
        { text: '多时间框共振是最强的信号层次。', source: 'R-P1-34 多级趋势' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/multi_timeframe.rs</code>',
        '测试': '10 个测试用例',
      },
      tags: ['多时间框', 'MTF', '共振', '多级趋势'],
    },
    {
      id: 'tr-elliott',
      group: 'trend',
      title: '艾略特波浪理论（Elliott Wave）',
      badges: [{ text: '经典理论', kind: 'iron' }],
      desc: 'Ralph Elliott 提出的"市场波浪结构"理论：趋势由 <b>5 浪推动 + 3 浪调整</b> 交替构成。',
      whatIs: `<p>核心结构：一个完整的"大周期"由 <b>8 浪</b> 组成。</p>
<ul>
<li><b>推动浪（1-5）</b>：</li>
<ul>
<li>浪 1：初始推动（常不被注意）</li>
<li>浪 2：回调（不跌破浪 1 起点）</li>
<li>浪 3：最强烈的推动浪（通常最长）</li>
<li>浪 4：再次回调（不跌破浪 1 终点）</li>
<li>浪 5：最后推动，常伴随背离</li>
</ul>
<li><b>调整浪（A-B-C）</b>：</li>
<ul>
<li>A：反向的第一波</li>
<li>B：反弹</li>
<li>C：破位下跌</li>
</ul>
</ul>
<p>三大铁律：<b>(1) 浪 2 不低于浪 1 起点；(2) 浪 3 不是最短；(3) 浪 4 不重叠浪 1 区间</b>。</p>`,
      strategy: `<ul>
<li><b>识别浪 3 入场</b>：最好的买点（动能最强）。</li>
<li><b>浪 5 出场</b>：看到背离 + 成交量衰竭时清仓。</li>
<li><b>C 浪结束抄底</b>：调整完成的最后一次机会。</li>
</ul>`,
      mistakes: [
        '<b>强行数浪</b>：艾略特浪在事后看很清晰，事前判断难度极大。新手容易主观臆断。',
        '<b>以艾略特浪为主</b>：建议与均线 / SR / 形态组合使用，不作为唯一依据。',
      ],
      quotes: [
        { text: '艾略特浪描述了市场的自然节奏。', source: 'Ralph Elliott 1938' },
      ],
      meta: {
        '参考': '艾略特《The Wave Principle》',
      },
      tags: ['艾略特', '波浪', 'Elliott', '5浪', 'ABC'],
    },
    {
      id: 'tr-wyckoff',
      group: 'trend',
      title: '威科夫方法（Wyckoff Method）',
      badges: [{ text: '经典理论', kind: 'iron' }, { text: '主力行为', kind: 'warn' }],
      desc: 'Richard Wyckoff 在 20 世纪初提出的 <b>主力周期理论</b>：4 个阶段（吸筹 / 拉升 / 出货 / 打压）。',
      whatIs: `<p>威科夫方法把市场看作<b>主力的游戏</b>，大资金走完完整周期需要 4 个阶段：</p>
<ol>
<li><b>吸筹（Accumulation）</b>：主力在低位横盘吸筹，量能先放大后萎缩。</li>
<li><b>拉升（Markup）</b>：价格突破横盘，放量上涨。</li>
<li><b>出货（Distribution）</b>：主力在高位横盘派发，量能维持高位但价格滞涨。</li>
<li><b>打压（Markdown）</b>：价格跌破横盘，放量下跌。</li>
</ol>
<p>每个阶段有可识别的子阶段（如 Spring / Shakeout / Sign of Strength 等）。识别阶段 = 知道主力意图 = 选对方向。</p>`,
      strategy: `<ul>
<li><b>吸筹末期（Spring）</b>：重仓买入。</li>
<li><b>拉升中期</b>：顺势持有。</li>
<li><b>出货末期（UpThrust）</b>：清仓。</li>
<li><b>打压末期</b>：再次吸筹的起点。</li>
</ul>`,
      quotes: [
        { text: 'Wyckoff 方法是"看懂主力"的系统框架。', source: 'Richard Wyckoff 1930s' },
      ],
      meta: {
        '参考': 'Wyckoff 经典教材',
      },
      tags: ['威科夫', 'Wyckoff', '主力', '4阶段', '吸筹', '出货'],
    },
    {
      id: 'tr-chan',
      group: 'trend',
      title: '缠论（简介）',
      badges: [{ text: '中国特色', kind: '' }],
      desc: '"缠中说禅"提出的技术分析体系：<b>中枢 / 走势类型 / 三买三卖</b>。以数学形式描述 K 线结构。',
      whatIs: `<p>缠论的核心概念：</p>
<ul>
<li><b>分型</b>：相邻 3 根 K 线的顶 / 底分型。</li>
<li><b>笔</b>：两个相反分型之间的连线。</li>
<li><b>线段</b>：至少 3 笔构成，方向统一。</li>
<li><b>中枢</b>：连续 3 段的重叠区间，是"平衡点"。</li>
<li><b>走势类型</b>：盘整 / 趋势（上升 / 下降）。</li>
<li><b>三买三卖</b>：基于中枢位置和走势类型的 6 种买卖点。</li>
</ul>
<p>缠论在中国市场流行，与传统道氏理论互补。AURA 未直接实现缠论，但其 Swing / 趋势判断与缠论的"分型 / 线段"部分兼容。</p>`,
      meta: {
        '参考': '缠中说禅《教你炒股票》',
      },
      tags: ['缠论', '缠中说禅', '中枢', '三买三卖'],
    },
    {
      id: 'tr-elliott-fib',
      group: 'trend',
      title: '艾略特浪 × Fibonacci 组合',
      badges: [{ text: '进阶', kind: 'iron' }],
      desc: '艾略特浪的幅度通常符合 Fibonacci 比例：<b>浪 3 ≈ 浪 1 × 1.618</b>、<b>浪 4 回调 ≈ 浪 3 × 0.382</b> 等。',
      whatIs: `<p>艾略特本人的发现：波浪的幅度不是随机的，而是符合斐波那契数列。常见关系：</p>
<ul>
<li>浪 3 幅度 = 浪 1 幅度 × 1.618（最常见）</li>
<li>浪 5 幅度 = 浪 1 幅度 × 0.618 或 × 1.0</li>
<li>浪 2 回调 = 浪 1 × 0.5 或 × 0.618</li>
<li>浪 4 回调 = 浪 3 × 0.382（较浅）</li>
<li>浪 A 幅度 = 浪 5 × 0.618</li>
<li>浪 C 幅度 = 浪 A × 1.618</li>
</ul>
<p>这种组合是<b>高阶交易员</b>常用的目标位预测工具。</p>`,
      strategy: `<p>识别波浪后，用 Fib 比例计算目标位，作为止盈依据。</p>`,
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/fib.rs</code>',
      },
      tags: ['艾略特Fib', 'wave_fib', '目标位', 'fibonacci'],
    },
    {
      id: 'tr-trendline-rules',
      group: 'trend',
      title: '趋势线画法 3 规则（E31）',
      badges: [{ text: '铁证', kind: 'iron' }],
      desc: '<b>(1) 至少 2 点确认，3 点验证</b>；<b>(2) 不穿越实体只穿影线</b>；<b>(3) 对数坐标画长周期</b>。',
      whatIs: `<p>趋势线看起来简单，实则有严格规则：</p>
<ol>
<li><b>2 + 1 法则</b>：至少 2 个 Swing 点画初始趋势线。当价格再次触碰该线（第 3 点）且反弹，趋势线<b>确认</b>。</li>
<li><b>不穿实体</b>：趋势线应连接 K 线的低点（上升）或高点（下降），<b>不能切入实体</b>。允许影线穿越但收盘不可。</li>
<li><b>对数坐标</b>：长周期图（月线 / 年线）必须用对数坐标画趋势线，否则长期走势的倾斜度被扭曲。</li>
</ol>`,
      mistakes: [
        '<b>用 1 个点画趋势线</b>：单点"画"不出趋势线，那只是支撑位。',
        '<b>让趋势线穿 K 线实体</b>：这样画出的线不反映真实支撑，没有参考价值。',
        '<b>随意调整趋势线</b>：为了让价格"验证"你的趋势线而反复调整 —— 这是自欺欺人。',
      ],
      quotes: [
        { text: '趋势线必须严格按 3 规则绘制，否则误导。', source: 'trend E31 Sprint 3' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/lines.rs</code>',
        '修复': 'E31 Sprint 3',
      },
      tags: ['趋势线画法', 'E31', '3规则'],
    },
    {
      id: 'tr-wyckoff-spring',
      group: 'trend',
      title: '威科夫 Spring（弹簧）',
      badges: [{ text: '吸筹末端', kind: 'bull' }, { text: '顶级买点', kind: 'iron' }],
      desc: '吸筹阶段末期，主力故意打压价格<b>短暂跌破支撑</b>触发散户止损，然后迅速拉回 —— 是吸筹完成的标志。',
      whatIs: `<p>Spring（弹簧）是威科夫方法中的经典终极买点：</p>
<ol>
<li>价格在吸筹区横盘多日</li>
<li>某天价格突然跌破下沿（触发散户止损）</li>
<li>当日或 1-2 日内快速拉回支撑区 —— <b>像弹簧压下又弹回</b></li>
<li>之后进入拉升阶段</li>
</ol>
<p>Spring 的意义：主力完成最后的吸筹（吃掉止损盘），开始拉升。</p>`,
      strategy: '<p>Spring 确认后立即建仓 70-100%，止损设 Spring 最低点。这是吸筹阶段<b>最精确的买点</b>。</p>',
      mistakes: [
        '<b>看到跌破就做空</b>：Spring 的跌破是假的，做空会被迅速拉回扫损。',
      ],
      example: `<b>BTC 2023/8/17 Spring</b>：BTC 在 29000-30000 横盘 3 周，8/17 跌破 28900 到 28650（跌破支撑），当日拉回 29100。随后进入拉升到 44000（+50%）。`,
      quotes: [
        { text: 'Spring 是吸筹的完成信号。', source: 'Wyckoff Method' },
      ],
      tags: ['Spring', '弹簧', 'Wyckoff', '吸筹', '顶级买点'],
    },
    {
      id: 'tr-wyckoff-upthrust',
      group: 'trend',
      title: '威科夫 UpThrust（上推）',
      badges: [{ text: '出货末端', kind: 'bear' }, { text: '顶级空点', kind: 'iron' }],
      desc: 'Spring 的反向：出货阶段末期，主力故意拉高<b>短暂突破阻力</b>吸引散户追涨，然后快速跌回 —— 是出货完成的标志。',
      whatIs: `<p>UpThrust（上推）的步骤：</p>
<ol>
<li>价格在分销区横盘多日</li>
<li>某天价格突然突破上沿（触发散户追涨）</li>
<li>当日或 1-2 日内快速跌回分销区 —— 假突破</li>
<li>之后进入打压阶段</li>
</ol>
<p>UpThrust 是主力"最后的派发"—— 把剩余筹码抛给追涨的散户。</p>`,
      strategy: '<p>UpThrust 确认后立即清仓 / 做空，止损设 UpThrust 最高点。</p>',
      mistakes: [
        '<b>追涨突破</b>：上推的突破是假的，追进去立即被套。',
      ],
      example: `<b>BTC 2021/11/8 UpThrust</b>：BTC 在 60000-67000 横盘 5 周，11/8 突破至 69000（历史新高），当日跌回 67000。随后进入熊市至 2022 年底 15500（-77%）。`,
      tags: ['UpThrust', '上推', 'Wyckoff', '出货', '顶级空点'],
    },
    {
      id: 'tr-gann',
      group: 'trend',
      title: '江恩理论（Gann Theory）',
      badges: [{ text: '时空理论', kind: '' }],
      desc: 'W.D. Gann 的价格 × 时间理论：<b>1×1 角度线（45°）</b>是最重要的支撑。价格和时间有特定几何关系。',
      whatIs: `<p>江恩核心观点：</p>
<ul>
<li>价格 × 时间 = 市场几何。时间和价格同等重要。</li>
<li><b>1×1 线（45°）</b>：每单位时间价格变动 1 单位，是最强趋势线。</li>
<li><b>角度线 1×2, 1×4, 2×1, 4×1</b>等：对应不同角度（63°, 75°, 26°, 14°）。</li>
<li>价格突破或跌破关键角度线 → 趋势改变。</li>
</ul>
<p>江恩理论复杂且主观性强，AURA 未直接实现。感兴趣者可作为进阶研究。</p>`,
      quotes: [
        { text: '时间是市场最重要的因素。', source: 'W.D. Gann' },
      ],
      tags: ['江恩', 'Gann', '角度线', '1×1'],
    },
    {
      id: 'tr-market-cycle',
      group: 'trend',
      title: '市场周期（Market Cycle）',
      badges: [{ text: '宏观视角', kind: 'iron' }],
      desc: '市场按 4 阶段循环：<b>底部积累 → 牛市拉升 → 顶部分配 → 熊市下跌</b>。识别当前阶段比择时更重要。',
      whatIs: `<p>这是威科夫方法的宏观版本，适用于所有市场：</p>
<ol>
<li><b>底部积累期</b>：价格横盘低位，量能萎缩。绝望情绪。机构开始吸筹。</li>
<li><b>牛市拉升期</b>：价格突破横盘，量能放大，情绪由怀疑转兴奋。</li>
<li><b>顶部分配期</b>：价格高位横盘或微创新高，量能高位萎缩。狂热情绪。机构开始派发。</li>
<li><b>熊市下跌期</b>：价格下跌，量能跟随放大。恐惧情绪。</li>
</ol>
<p>BTC 历史：2019 积累 → 2020-2021 牛市 → 2021 Q4 分配 → 2022 熊市 → 2023 积累 → 2024 牛市...</p>`,
      strategy: `<ul>
<li><b>底部积累</b>：分批建仓，耐心等待。</li>
<li><b>牛市拉升</b>：顺势持有，主仓位最重。</li>
<li><b>顶部分配</b>：分批减仓，开始观望。</li>
<li><b>熊市下跌</b>：空仓或小仓做空，等待下一轮积累。</li>
</ul>`,
      quotes: [
        { text: '市场总是在 4 阶段间循环。', source: 'Charles Dow' },
      ],
      tags: ['市场周期', 'cycle', '4阶段', '积累', '分配'],
    },

    // ==================== K 线形态（candle）====================
    {
      id: 'ck-single',
      group: 'candle',
      title: '单 K 线（锤头 / 流星 / 十字星 / 大阳 / 大阴 / T 字 / 倒 T 字）',
      badges: [],
      desc: '基于实体/影线比例和开收盘位置识别。<b>锤头</b>（下影长 ≥ 2×实体）= 底部反弹；<b>流星</b>（上影长 ≥ 2×实体）= 顶部警示；<b>十字星</b>（实体 ≤ 5% ATR）= 变盘。',
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs</code>',
        'API': '<span class="kb-chip api">/api/candle_patterns</span>',
        '数量': '17 种单 K 线形态',
      },
      tags: ['锤头', '流星', '十字星', '大阳', '单K线'],
    },
    {
      id: 'ck-double',
      group: 'candle',
      title: '双 K 线（吞没 / 乌云盖顶 / 刺透 / 镊子）',
      badges: [],
      desc: '<b>看涨吞没 BullishEngulfing</b>：阳线实体完全包住前一阴线实体。<b>乌云盖顶</b>：大阴线跌破前阳线 ≥ 50%。<b>刺透</b> 为其反向。<b>镊子顶/底</b>：连续两根 K 线形成相同高点或低点。',
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs</code>',
        '数量': '8 种双 K 线',
      },
      tags: ['吞没', 'engulfing', '乌云', '刺透', '镊子'],
    },
    {
      id: 'ck-triple',
      group: 'candle',
      title: '三 K 线（晨星 / 黄昏 / 三兵 / 三鸦）',
      badges: [{ text: '强信号', kind: 'iron' }],
      desc: '<b>早晨之星 MorningStar</b>：大阴 + 十字星/小实体 + 大阳（底部反转）。<b>黄昏之星</b> 为其反向。<b>红三兵</b>：连续 3 根大阳递进；<b>黑三鸦</b>：连续 3 根大阴递进。',
      quotes: [
        { text: '黄昏之星 > 早晨之星（顶部信号更可靠）。', source: 'candle p.320' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs</code>',
        '数量': '10 种三 K 线',
      },
      tags: ['晨星', '黄昏', 'morning', 'evening', '三兵'],
    },
    {
      id: 'ck-advanced',
      group: 'candle',
      title: '高级组合（岛形反转 / 倒置 V / 镊子 + 时间映射）',
      badges: [],
      desc: '<b>岛形反转</b>：岛与大陆间有双跳空。时间跨度越长，反转级别越高（1-5 天 = 短线，6-20 天 = 中线，>20 天 = 长线）。',
      quotes: [
        { text: '岛形反转的时间跨度映射到反转级别。', source: 'candle p.660' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/advanced.rs</code>',
        '测试': '19 个测试用例',
        '数量': '6 种高级组合',
      },
      tags: ['岛形', '倒置V', '时间映射'],
    },
    {
      id: 'ck-chart-hs',
      group: 'candle',
      title: '头肩顶 / 头肩底（经典反转形态）',
      badges: [{ text: '经典', kind: 'iron' }, { text: '见顶 / 见底', kind: 'warn' }, { text: 'R-P1-23', kind: '' }],
      desc: '技术分析教科书中最著名的反转形态。由 <b>左肩 - 头 - 右肩</b> 三个峰（或三个谷）构成，两个肩之间的低点（或高点）连成 <b>颈线</b>。颈线有效跌破（或突破）即形态完成确认。',
      whatIs: `<p>把图想象成一个<b>人形</b>：两边是肩膀（同样高），中间是头（比肩膀高）。头肩顶出现在上涨末期，是<b>见顶反转信号</b>；反之头肩底是见底反转。</p>
<p>为什么有效？从市场心理看：<b>左肩</b> = 多头最后一次强势冲高（正常上涨）；<b>头</b> = 多头最后一次创新高但已显疲态（量能往往比左肩低）；<b>右肩</b> = 反弹力度不足，无法创新高，多头衰竭。当颈线被跌破，所有之前的"高位买入者"都套牢，形成抛压雪崩。</p>
<p>AURA 在原书基础上实施 <b>R-P1-23 量价对称原则</b>：头部量能必须 ≤ 左肩量能，右肩量能必须 < 头部量能。不符合量价关系的"视觉头肩"会被降级或丢弃。</p>`,
      diagram: `<span class="d-mute">          头肩顶（见顶反转）</span>

<span class="d-mute">   价格 ↑       </span><span class="d-bear">头</span>
<span class="d-mute">        │      ╱╲</span>
<span class="d-mute">        │     ╱  ╲</span>          <span class="d-warn">量递减</span>
<span class="d-mute">        │ 左肩╱    ╲右肩</span>     <span class="d-warn">左肩量 > 头量 > 右肩量</span>
<span class="d-mute">        │   ╱╲  ⚫  ╱╲</span>
<span class="d-mute">        │  ╱  ╲   ╱  ╲</span>
<span class="d-mute">        │─╱────╲─╱────╲──</span>     <span class="d-bear">← 颈线（支撑）</span>
<span class="d-mute">        │        ⚫     ╲</span>
<span class="d-mute">        │                ╲</span>     <span class="d-bear">💥 颈线跌破 ≥ 3%</span>
<span class="d-mute">        │                 ╲___</span>  形态确认
<span class="d-mute">        └───────────────────→</span>

<span class="d-mute">   目标位：颈线 - (头 - 颈线) × 1.0</span>`,
      howTo: [
        '<b>找到 3 个高点</b>：中间高点（头）明显高于两边（左右肩），两肩高度大致相同（允许 ±10% 差异）。',
        '<b>画颈线</b>：连接两肩之间的两个低点，得到一条水平或略斜的支撑线。',
        '<b>验证对称性</b>：左肩到头、头到右肩的时间应大致相等（允许 ±30% 差异），太不对称则不是标准头肩。',
        '<b>验证量价关系</b>（R-P1-23）：左肩量 > 头量 > 右肩量 —— 头不创新量、右肩量更萎缩。',
        '<b>等待颈线跌破</b>：<b>收盘价跌破颈线 ≥ 3%</b>（或下影穿越但收盘回拉不算），且<b>放量</b>（≥ 1.5× 均量）。',
        '<b>形态确认</b>：跌破后若 3 根内不回抽颈线或回抽不过颈线，形态完全确认。',
      ],
      params: {
        '肩高差异': '<code>≤ 10%</code>',
        '时间对称': '<code>±30%</code>',
        '颈线跌破': '<code>≥ 3%</code>（收盘价）',
        '跌破放量': '<code>≥ 1.5 × vol_ma20</code>',
        '回抽验证': '<code>跌破后 3 根内不回颈线</code>',
        '量价规则': '左肩量 > 头量 > 右肩量（R-P1-23）',
      },
      strategy: `<ul>
<li><b>做空时机</b>：收盘价跌破颈线 ≥ 3% 且放量 → 第二日开盘做空，止损设在颈线上方 1 ATR。</li>
<li><b>持仓者</b>：看到头部形成（右肩开始出现）即减仓 30-50%，颈线跌破立即清仓。</li>
<li><b>量度目标位</b>：颈线价格 <code>-</code> (头高 <code>-</code> 颈线) × 1.0 = 第一目标位。多数情况下能达到。</li>
<li><b>回抽机会</b>：颈线跌破后 3-5 根可能反抽颈线但不过，是第二次做空机会（更安全）。</li>
<li><b>头肩底</b>：完全反向 —— 颈线突破 ≥ 3% 且放量 → 做多，目标 = 颈线 + (颈线 - 头) × 1.0。</li>
</ul>`,
      mistakes: [
        '<b>没有颈线跌破就做空</b>：在右肩形成时做空属预判，未形成有效跌破前形态可能失败（演变为 W 底 / 三重顶）。',
        '<b>颈线跌破 < 3%</b>：跌破不足 3% 容易假突破（多头陷阱之反，空头陷阱）。AURA 自动过滤。',
        '<b>不验证量价关系</b>：视觉上的头肩如果量能反向（头量 > 左肩量），通常是陷阱，后续往往继续上涨。',
        '<b>忽略时间对称</b>：左肩用 5 根、头用 30 根、右肩用 3 根的"不对称头肩"，可靠度极低。',
        '<b>对数 vs 线性坐标</b>：长周期图（>50 根）应使用对数坐标识别头肩，线性坐标会扭曲形态。AURA 的 log-scale 开关可切换。',
        '<b>忽视上下文</b>：如果价格本在下跌趋势中出现"头肩底"，其可靠度远低于横盘或反弹中的头肩底。',
      ],
      example: `<b>BTCUSDT 日线 2021/5 头肩顶经典案例</b>：左肩 59000（4/14）→ 头 65000（4/14-4/23，量萎缩 20%）→ 右肩 58000（5/8，量继续萎缩）→ 颈线 55000 跌破（5/19，放量 3×）。AURA 识别并发出 L7 级卖出信号。后续 BTC 从 55000 跌至 28000（-49%），完美达到量度目标 45000（颈线 - 10000）并继续下破。`,
      quotes: [
        { text: '头肩顶是最经典的见顶反转形态，三峰成型、颈线确认、量价对称。', source: 'candle Ch5 p.500' },
        { text: '头肩底的右肩在颈线上方回踩，是极佳买点。', source: 'candle p.540' },
        { text: '头肩顶的量价关系：左肩量最大，头量次之，右肩量最小（量能递减）。', source: 'candle R-P1-23' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/head_shoulder.rs</code>',
        'API': '<span class="kb-chip api">/api/chart_patterns</span> <span class="kb-chip">HeadAndShouldersTop / HeadAndShouldersBottom</span>',
        '测试': '11 个测试用例',
        '修复': 'R-P1-23 量价对称（Sprint 7）',
      },
      tags: ['头肩顶', '头肩底', 'HeadAndShoulders', '颈线', 'neckline', '反转', 'R-P1-23'],
    },
    {
      id: 'ck-chart-double',
      group: 'candle',
      title: '双顶 / 双底',
      badges: [],
      desc: '价格两次测试相同高/低位后失败。<b>E32 时间规则</b>：两峰/两谷间隔 ≥ 20 根 K 线才算有效双顶（否则为普通盘整）。',
      quotes: [
        { text: '双底形成需要时间沉淀。', source: 'candle Ch7' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/double.rs</code>',
        '修复': 'E32 (Sprint 2.5)',
      },
      tags: ['双顶', '双底', 'double_top', 'double_bottom'],
    },
    {
      id: 'ck-chart-triangle',
      group: 'candle',
      title: '三角形（上升 / 下降 / 等腰 / 扩散）',
      badges: [],
      desc: '<b>上升三角</b>：水平上轨 + 上升下轨 = 看涨蓄势。<b>下降三角</b>：下降上轨 + 水平下轨 = 看跌蓄势。<b>扩散三角</b> = 主力过顶吸筹洗盘（R-P2-02）。',
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/triangle.rs</code>',
        'API': '<span class="kb-chip api">/api/chart_patterns</span>',
      },
      tags: ['三角', 'triangle', '上升', '下降', '扩散'],
    },
    {
      id: 'ck-chart-flag',
      group: 'candle',
      title: '旗形（7 条铁证）',
      badges: [{ text: '7 条规则', kind: 'iron' }],
      desc: 'candle p.770 给出旗形的 7 条完整规则：(1) 旗杆 ≥ 20% (2) 旗面回撤 ≤ 50% (3) 突破 ≥ 3% (4) 时间 ≤ 3 周 (5) 量能萎缩 (6) 方向与旗杆同向 (7) 目标位 = 旗杆高度 × 1 + 突破价。AURA 完整验证。',
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/flag_validator.rs</code>',
        '测试': '7 个测试用例',
        '修复': 'R-P1-39 (Sprint 5)',
      },
      tags: ['旗形', 'flag', '7条', '旗杆', '旗面'],
    },
    {
      id: 'ck-chart-rounding',
      group: 'candle',
      title: '圆顶 / 圆底（3 阶段）',
      badges: [],
      desc: 'R-P1-28 / R-P1-48 完整实现：<b>阶段 1</b> 缓慢下跌（左弧）；<b>阶段 2</b> 横盘见底（弧底）；<b>阶段 3</b> 缓慢上升（右弧）。每阶段 ≥ 10 根 K 线且量能递减-递增。',
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/advanced.rs</code>',
        '测试': '4 个测试用例',
      },
      tags: ['圆底', '圆顶', 'rounding', '3阶段'],
    },
    {
      id: 'ck-chart-diamond',
      group: 'candle',
      title: '菱形形态（扩散 + 收敛合璧）',
      badges: [],
      desc: 'R-P1-38：前半段扩散三角（不确定放大），后半段收敛三角（能量凝聚）。通常为大级别反转前奏。',
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/diamond.rs</code>',
      },
      tags: ['菱形', 'diamond'],
    },
    {
      id: 'ck-chart-wedge',
      group: 'candle',
      title: '楔形（上升 / 下降）',
      badges: [],
      desc: '两条收敛的趋势线，但<b>两条同向倾斜</b>（与三角不同）。<b>上升楔</b> = 虽然创新高但动能衰竭，看跌。<b>下降楔</b> = 虽然创新低但动能衰竭，看涨。',
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/wedge.rs</code>',
      },
      tags: ['楔形', 'wedge'],
    },
    {
      id: 'ck-hammer',
      group: 'candle',
      title: '锤头线（Hammer）',
      badges: [{ text: '见底信号', kind: 'bull' }, { text: '单 K 线', kind: '' }],
      desc: '下跌趋势末端出现的一种特殊 K 线，形如<b>锤子</b>（实体小、下影长）—— 市场被打到底后反弹，预示底部临近。',
      whatIs: `<p>锤头线的样子：</p>
<ul>
<li><b>下影线</b>非常长（至少是实体的 2 倍）</li>
<li><b>上影线</b>很短或几乎没有</li>
<li><b>实体</b>很小，颜色（阴/阳）不是最重要，位置靠近 K 线顶部</li>
</ul>
<p>出现在下跌末期：开盘后价格继续下跌（形成长下影），但尾盘买盘进场把价格拉回近开盘价 —— 说明<b>空头力竭、多头开始反攻</b>。</p>`,
      diagram: `<span class="d-mute">    ┬</span>             <span class="d-mute">← 上影极短或无</span>
<span class="d-mute">  ┌─┴─┐</span>           <span class="d-mute">实体小（阴阳均可，阳更强）</span>
<span class="d-bull">  │   │</span>
<span class="d-mute">  └───┘</span>
<span class="d-mute">    │</span>               <span class="d-mute">下影长 ≥ 2× 实体</span>
<span class="d-mute">    │</span>
<span class="d-mute">    │</span>
<span class="d-mute">    ┴</span>
<span class="d-bull">  锤头（见底信号）</span>`,
      howTo: [
        '<b>下跌背景</b>：此前至少 5-10 根 K 线呈下降趋势。',
        '<b>测量下影</b>：下影 ≥ 2 × 实体长度。',
        '<b>测量上影</b>：上影 ≤ 10% K 线总长。',
        '<b>次日确认</b>：次日收阳线（或至少不创新低），确认反转。',
        '<b>配合支撑位</b>：锤头出现在重要支撑位（MA60 / Fib 0.618 / 历史低点）可靠度最高。',
      ],
      params: {
        '下影比': '<code>≥ 2.0 × 实体</code>',
        '上影比': '<code>≤ 10%</code>',
        '前置趋势': '下跌 ≥ 5 根',
      },
      strategy: `<ul>
<li><b>次日阳线确认</b>：分批建仓 30-50%，止损设锤头下影最低点。</li>
<li><b>止盈</b>：第一目标 = 前期反弹高点或 MA20。</li>
</ul>`,
      mistakes: [
        '<b>没有下跌背景就当锤头</b>：震荡期的类似形态不是锤头。',
        '<b>不等确认直接买</b>：锤头单独 50% 可信，加上次日阳线确认升至 70%。',
      ],
      example: `<b>BTC 2023/1/8</b>：16600 @ 下跌末期出现完美锤头（下影 1200，实体 150），次日阳线确认。随后 3 月涨至 29000（+75%）。`,
      quotes: [
        { text: '锤头线的下影是多头反攻的信号弹。', source: 'candle Ch2 p.50' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Hammer</code>',
        'API': '<span class="kb-chip">candle_patterns.kind=Hammer</span>',
      },
      tags: ['锤头', 'hammer', '见底', '下影', 'bull'],
    },
    {
      id: 'ck-shooting-star',
      group: 'candle',
      title: '流星线（Shooting Star）',
      badges: [{ text: '见顶信号', kind: 'bear' }, { text: '单 K 线', kind: '' }],
      desc: '上涨趋势末端的"倒锤头"：上影长、下影短、实体小 —— 多头冲高受阻，见顶信号。',
      whatIs: `<p>流星是锤头的上下颠倒版：</p>
<ul>
<li><b>上影线</b>非常长（≥ 2 × 实体）</li>
<li><b>下影线</b>很短</li>
<li><b>实体</b>很小，位于 K 线底部</li>
</ul>
<p>出现在上涨末期：开盘后价格继续冲高（形成长上影），但尾盘抛压出现把价格打回开盘附近 —— <b>多头力竭、空头开始反攻</b>。</p>`,
      howTo: [
        '<b>上涨背景</b>：此前至少 5-10 根 K 线呈上升趋势。',
        '<b>上影 ≥ 2 × 实体</b>',
        '<b>下影 ≤ 10% K 线长</b>',
        '<b>次日阴线确认</b>',
      ],
      strategy: `<ul>
<li><b>次日阴线确认 → 减仓 50%</b>，止损设流星上影最高点。</li>
<li><b>不直接做空</b>：流星的"破坏力"比锤头的"反弹力"弱，空头建议等更多信号共振。</li>
</ul>`,
      quotes: [
        { text: '流星线的上影是空头反攻的第一声号角。', source: 'candle p.55' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::ShootingStar</code>',
      },
      tags: ['流星', 'shooting_star', '见顶', '上影'],
    },
    {
      id: 'ck-doji',
      group: 'candle',
      title: '十字星 Doji（6 变体）',
      badges: [{ text: '变盘信号', kind: 'warn' }, { text: '单 K 线', kind: '' }],
      desc: '开盘价 ≈ 收盘价 的 K 线，实体极小（实体 < 5% 全长）—— 多空势均力敌，趋势可能变盘。',
      whatIs: `<p>十字星是<b>最重要的单 K 线形态</b>之一：开盘和收盘几乎一样，意味着当日多空力量相等，价格没有明确方向。</p>
<p>出现在不同位置意义不同：</p>
<ol>
<li><b>顶部十字星</b>：上升趋势末期，可能见顶。</li>
<li><b>底部十字星（晨星中的中间 K）</b>：下跌末期，可能见底。</li>
<li><b>趋势中十字星</b>：犹豫，等待方向选择。</li>
</ol>
<p>AURA 实现 6 种变体：标准十字、长腿十字（上下影都长）、蜻蜓十字（无上影）、墓碑十字（无下影）、四价十字（无影线）、高/低浪十字。</p>`,
      diagram: `<span class="d-mute">  标准 Doji    蜻蜓 Doji     墓碑 Doji</span>
<span class="d-mute">     │            ─           │     </span>
<span class="d-mute">     ┼            ┼           │     </span>
<span class="d-mute">     │            │           ┼     </span>
<span class="d-mute">     │            │           ─     </span>`,
      howTo: [
        '<b>实体判断</b>：|close - open| / (high - low) < 5%。',
        '<b>识别变体</b>：看上下影线比例分类。',
        '<b>位置判断</b>：在趋势什么位置（顶 / 底 / 中间）。',
        '<b>下一根确认</b>：十字星后下一根方向决定变盘方向。',
      ],
      params: {
        '实体比例': '<code>&lt; 5%</code> 全长',
        '变体种类': '6 种（Standard / LongLegged / Dragonfly / Gravestone / FourPrice / HighWave）',
      },
      strategy: `<ul>
<li><b>顶部蜻蜓 / 墓碑</b>：强见顶信号，减仓 50%。</li>
<li><b>底部蜻蜓</b>：强见底信号，试探性建仓 30%。</li>
<li><b>趋势中标准十字</b>：观望，等下一根方向。</li>
</ul>`,
      quotes: [
        { text: '十字星是最纯粹的"犹豫"信号。', source: 'candle Ch2 p.65' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Doji</code>',
      },
      tags: ['十字星', 'doji', '变盘', '蜻蜓', '墓碑'],
    },
    {
      id: 'ck-marubozu',
      group: 'candle',
      title: '大阳线 / 大阴线（Marubozu）',
      badges: [{ text: '强势信号', kind: 'warn' }],
      desc: '极长实体 + 几乎没有影线的 K 线 —— 一方势力完全压制，当日"一路下跌"或"一路上涨"。',
      whatIs: `<p>Marubozu 日文意为"光头"—— 这种 K 线没有上下影线（或非常短），完全由实体构成。</p>
<p><b>大阳线 Bullish Marubozu</b>：开盘即最低、收盘即最高，多头全程碾压空头。</p>
<p><b>大阴线 Bearish Marubozu</b>：开盘即最高、收盘即最低，空头全程碾压。断头铡刀的原型。</p>`,
      howTo: [
        '<b>实体占比</b>：实体 / 全长 ≥ 90%。',
        '<b>影线</b>：上下影均 ≤ 5%。',
        '<b>尺寸</b>：实体 ≥ 1.5 × ATR(14)。',
      ],
      strategy: `<ul>
<li><b>趋势中大阳</b>：继续看涨，顺势加仓。</li>
<li><b>顶部大阴</b>：警报级卖出（可能是断头铡刀）。</li>
<li><b>底部大阳</b>：反转信号，分批买入。</li>
</ul>`,
      quotes: [
        { text: '光头光脚的 K 线代表某一方的绝对胜利。', source: 'candle p.70' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Marubozu</code>',
      },
      tags: ['大阳线', '大阴线', 'marubozu', '光头', '光脚'],
    },
    {
      id: 'ck-engulfing',
      group: 'candle',
      title: '吞没形态（Engulfing）',
      badges: [{ text: '强反转', kind: 'iron' }, { text: '双 K 线', kind: '' }],
      desc: '第二根 K 线的实体<b>完全包住</b>第一根的实体 —— 力量彻底翻转的经典反转信号。',
      whatIs: `<p>吞没形态的条件：</p>
<ul>
<li>前一根 K 线实体较小</li>
<li>后一根 K 线实体<b>完全包住</b>前一根实体（开盘 < 前收，收盘 > 前开；或反之）</li>
<li>方向相反（前阳后阴 = 看跌吞没；前阴后阳 = 看涨吞没）</li>
</ul>
<p>意义：后一根 K 线的力量完全超越了前一根，意味着<b>多空主导权发生了切换</b>。</p>`,
      diagram: `<span class="d-mute">  看涨吞没（底部反转）</span>      <span class="d-mute">看跌吞没（顶部反转）</span>
<span class="d-bear">    ┌─┐</span>                      <span class="d-bull">    ┌───┐</span>
<span class="d-bear">    │ │</span>   <span class="d-mute">小阴</span>                <span class="d-bull">    │   │</span>  <span class="d-mute">小阳</span>
<span class="d-bear">    └─┘</span>                      <span class="d-bull">    └───┘</span>
<span class="d-bull">  ┌─────┐</span>                    <span class="d-bear">  ┌───────┐</span>
<span class="d-bull">  │     │</span>  <span class="d-mute">大阳包住</span>           <span class="d-bear">  │       │</span>  <span class="d-mute">大阴包住</span>
<span class="d-bull">  └─────┘</span>                    <span class="d-bear">  └───────┘</span>`,
      howTo: [
        '<b>背景趋势</b>：前有至少 3 根同向 K 线。',
        '<b>第一根小实体</b>：前一根实体 < 30% 全长（相对较小）。',
        '<b>第二根大实体</b>：后一根实体 > 60% 全长，完全包住前一根。',
        '<b>方向相反</b>：阳阴或阴阳。',
        '<b>放量确认</b>：第二根量能 ≥ 1.5 × 均量。',
      ],
      strategy: `<ul>
<li><b>看涨吞没</b>：底部出现 → 重仓建多（70-100%）。</li>
<li><b>看跌吞没</b>：顶部出现 → 清仓或做空。</li>
<li><b>止损</b>：跌破 / 突破吞没 K 线的开盘价。</li>
</ul>`,
      quotes: [
        { text: '吞没是双 K 线形态中最强的反转信号。', source: 'candle Ch3 p.120' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Engulfing</code>',
        'API': '<span class="kb-chip">BullishEngulfing / BearishEngulfing</span>',
      },
      tags: ['吞没', 'engulfing', '反转', '双K线', 'bull', 'bear'],
    },
    {
      id: 'ck-dark-cloud',
      group: 'candle',
      title: '乌云盖顶（Dark Cloud Cover）',
      badges: [{ text: '见顶信号', kind: 'bear' }, { text: '双 K 线', kind: '' }],
      desc: '上涨末期：先阳后阴，阴线深入阳线实体 ≥ 50% —— "乌云"压在前一根阳线上方，多头受挫。',
      whatIs: `<p>乌云盖顶的构成：</p>
<ol>
<li>前一根是大阳线，多头仍占主导</li>
<li>第二根阴线<b>开盘跳高</b>（高于前阳线收盘），但尾盘暴跌</li>
<li>阴线收盘价<b>深入前阳线实体 ≥ 50%</b></li>
</ol>
<p>比吞没形态弱一级，但比普通阴线强。适合在顶部区域识别见顶。</p>`,
      howTo: [
        '<b>前有阳线</b>：前一根是明显的大阳线（实体 ≥ 60%）。',
        '<b>跳高开盘</b>：第二根开盘价 > 前阳线最高价。',
        '<b>深入阳体</b>：第二根收盘 < 前阳线实体中点（即 ≥ 50% 深入）。',
      ],
      strategy: `<p>减仓 50%，止损设乌云盖顶最高点。若次日再收阴线 → 清仓。</p>`,
      quotes: [
        { text: '乌云盖顶是顶部的经典形态之一。', source: 'candle p.135' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::DarkCloudCover</code>',
      },
      tags: ['乌云盖顶', 'dark_cloud', '见顶', '双K线'],
    },
    {
      id: 'ck-piercing',
      group: 'candle',
      title: '刺透形态（Piercing Pattern）',
      badges: [{ text: '见底信号', kind: 'bull' }, { text: '双 K 线', kind: '' }],
      desc: '下跌末期的"乌云反向"：先阴后阳，阳线收盘刺入前阴线实体 ≥ 50%。',
      whatIs: `<p>刺透形态是乌云盖顶的镜像：</p>
<ol>
<li>前一根是大阴线</li>
<li>第二根阳线<b>跳低开盘</b>（低于前阴线最低价）</li>
<li>阳线收盘价<b>深入前阴线实体 ≥ 50%</b></li>
</ol>
<p>比看涨吞没弱一级，但比锤头强。是下跌末期的试探性买点。</p>`,
      strategy: `<p>试探建仓 30-50%，止损设刺透形态最低点。</p>`,
      quotes: [
        { text: '刺透形态的 50% 是关键阈值。', source: 'candle p.140' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::PiercingPattern</code>',
      },
      tags: ['刺透', 'piercing', '见底', '双K线'],
    },
    {
      id: 'ck-tweezers',
      group: 'candle',
      title: '镊子顶 / 镊子底（Tweezers）',
      badges: [{ text: '短期反转', kind: 'warn' }, { text: '双 K 线', kind: '' }],
      desc: '连续两根 K 线形成<b>相同的高点（镊子顶）或相同的低点（镊子底）</b>—— 价格在此位反复受阻 / 受支撑。',
      whatIs: `<p>镊子形态极简：两根相邻 K 线的最高点（或最低点）相差 &lt; 0.5%，像镊子的两个"爪子"夹在同一位置。</p>
<p><b>镊子顶</b>：上涨中两根 K 线高点相同 —— 上方有强阻力。</p>
<p><b>镊子底</b>：下跌中两根 K 线低点相同 —— 下方有强支撑。</p>`,
      howTo: [
        '<b>高点差异</b>：&lt; 0.5%。',
        '<b>背景趋势</b>：上涨 / 下跌 ≥ 5 根。',
        '<b>次日确认</b>：次日方向决定。',
      ],
      strategy: `<ul>
<li><b>镊子顶 + 次日阴线</b>：短线清仓或减仓，中长线需等更强信号。</li>
<li><b>镊子底 + 次日阳线</b>：短线试探性建仓。</li>
</ul>`,
      quotes: [
        { text: '镊子顶短线清仓，中长线减仓。', source: 'candle p.180' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Tweezers</code>',
      },
      tags: ['镊子', 'tweezers', '短期反转', '双K线'],
    },
    {
      id: 'ck-harami',
      group: 'candle',
      title: '母子线（Harami，孕线）',
      badges: [{ text: '中继 / 反转', kind: 'warn' }, { text: '双 K 线', kind: '' }],
      desc: '第一根大 K 线"怀抱"第二根小 K 线（第二根完全包在第一根实体内）—— 市场犹豫、等待方向。',
      whatIs: `<p>Harami 日语意为"怀孕"。一根大 K 线里"藏着"一根小 K 线：</p>
<ul>
<li>第一根：大阳或大阴（实体 ≥ 60%）</li>
<li>第二根：小实体（实体 ≤ 30%），完全包含在前一根实体内</li>
<li>通常两根方向相反（阳后阴 或 阴后阳）</li>
</ul>
<p>意义：原本的单边推进被<b>暂停</b>，市场开始犹豫。是反转的前兆但不是确认。</p>`,
      howTo: [
        '<b>第一根大实体</b>：≥ 60%。',
        '<b>第二根完全包含</b>：小实体的最高 / 最低 都在大实体内。',
        '<b>等待第三根</b>：第三根方向决定（顺势或反转）。',
      ],
      strategy: '<p>Harami 后：观望，等第三根 K 线确认方向。</p>',
      quotes: [
        { text: 'Harami 是"市场在屏息"的形态。', source: 'candle p.155' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Harami</code>',
      },
      tags: ['母子线', 'harami', '孕线', '中继'],
    },
    {
      id: 'ck-morning-evening-star',
      group: 'candle',
      title: '早晨之星 / 黄昏之星（Star）',
      badges: [{ text: '顶级反转', kind: 'iron' }, { text: '三 K 线', kind: '' }],
      desc: '底部/顶部三根 K 线的"经典组合"：大阴（大阳）+ 十字星 + 大阳（大阴）—— 三本书中最可靠的反转形态之一。',
      whatIs: `<p><b>早晨之星（见底）</b>：</p>
<ol>
<li>第一根：大阴线（延续下跌）</li>
<li>第二根：跳低开盘的十字星或小 K 线（多空胶着，"黎明前的黑暗"）</li>
<li>第三根：大阳线，收盘深入第一根阴线实体 ≥ 50%</li>
</ol>
<p><b>黄昏之星（见顶）</b>：完全反向，大阳 + 十字星 + 大阴。<b>黄昏之星比早晨之星更强</b>（顶部反转比底部更可靠）。</p>`,
      diagram: `<span class="d-mute">早晨之星（见底）：</span>
<span class="d-bear">  ┌─┐</span>         <span class="d-mute">│        </span>          <span class="d-bull">  ┌───┐</span>
<span class="d-bear">  │ │</span>         <span class="d-mute">┼        </span>          <span class="d-bull">  │   │</span>
<span class="d-bear">  │ │</span>  ←       <span class="d-mute">│        </span>   →      <span class="d-bull">  │   │</span>
<span class="d-bear">  │ │</span>                                     <span class="d-bull">  │   │</span>
<span class="d-bear">  └─┘</span>                                     <span class="d-bull">  └───┘</span>
<span class="d-mute">  大阴</span>     <span class="d-mute">十字星（跳低）</span>     <span class="d-mute">  大阳</span>`,
      howTo: [
        '<b>第一根大阴</b>：实体 ≥ 60%。',
        '<b>第二根小实体</b>：实体 ≤ 30%，且跳空开盘（跳低）。',
        '<b>第三根大阳</b>：实体 ≥ 60%，收盘价 ≥ 第一根实体中点。',
        '<b>量能对比</b>：第三根量能 > 第一根（多头主导确认）。',
      ],
      strategy: `<ul>
<li><b>早晨之星</b>：重仓建多（70-100%），止损设第二根星的最低点。</li>
<li><b>黄昏之星</b>：清仓 + 顺势做空，止损设第二根星的最高点。</li>
</ul>`,
      quotes: [
        { text: '黄昏之星的可靠度高于早晨之星（顶部卖出更重要）。', source: 'candle p.320' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::MorningStar / EveningStar</code>',
      },
      tags: ['早晨之星', '黄昏之星', 'morning_star', 'evening_star', '三K线', '反转'],
    },
    {
      id: 'ck-three-soldiers',
      group: 'candle',
      title: '红三兵 / 黑三鸦（Three Soldiers / Crows）',
      badges: [{ text: '趋势加速', kind: '' }, { text: '三 K 线', kind: '' }],
      desc: '连续三根大阳线递进（红三兵）或三根大阴线递进（黑三鸦）—— 趋势强烈加速的信号。',
      whatIs: `<p><b>红三兵</b>：连续 3 根阳线，每根都创新高且实体较大。表示多头连续进攻，看涨非常强势。</p>
<p><b>黑三鸦</b>：相反，3 根阴线连续下跌创新低。看跌强势。</p>
<p>但有一个重要例外：如果连续三阴线出现在<b>很高的位置</b>，且每根实体都很大，这反而可能是主力<b>倒三阳出货</b>（大阳接力结束，多头衰竭）—— 这是 ma p.400 原书特别指出的主力行为学现象。</p>`,
      howTo: [
        '<b>三根同向</b>：全阳或全阴。',
        '<b>递进</b>：第 2 / 第 3 根的最高价 > 前一根最高价（红三兵）。',
        '<b>实体充实</b>：每根实体 ≥ 50%。',
        '<b>上下影短</b>：上下影 ≤ 20%（防止涨/跌无力）。',
      ],
      strategy: `<ul>
<li><b>低位红三兵</b>：强势看多信号，加仓。</li>
<li><b>高位红三兵（末期）</b>：可能是主力出货前的最后拉升，反而需警惕。</li>
<li><b>黑三鸦</b>：顺势做空或清仓。</li>
</ul>`,
      mistakes: [
        '<b>高位无脑跟多</b>：红三兵在 L4 超买位出现可能是顶部，不是延续。',
      ],
      quotes: [
        { text: '倒三阳 = 主力出货。', source: 'candle p.400' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::ThreeSoldiers / ThreeCrows</code>',
      },
      tags: ['红三兵', '黑三鸦', 'three_soldiers', 'three_crows', '趋势加速'],
    },
    {
      id: 'ck-triple-top-bottom',
      group: 'candle',
      title: '三重顶 / 三重底',
      badges: [{ text: '反转形态', kind: 'warn' }],
      desc: '价格三次测试同一高点（或低点）失败 —— 比双顶 / 双底更强的反转确认。',
      whatIs: `<p>三重顶 / 三重底是双顶 / 双底的加强版：</p>
<ol>
<li>价格反复触碰同一阻力 / 支撑 3 次</li>
<li>三个峰（或三个谷）高度相近（± 2% 差异）</li>
<li>三次触碰之间形成 2 个回撤低点（或反弹高点）</li>
<li>跌破颈线（支撑）= 形态确认</li>
</ol>
<p>为什么比双顶强？因为 3 次尝试都失败，市场已充分"消化"过这个阻力，突破难度指数级上升。</p>`,
      strategy: `<ul>
<li><b>三重顶 + 颈线跌破 ≥ 3%</b>：做空，目标位 = 颈线 - (峰高 - 颈线)。</li>
<li><b>三重底 + 颈线突破 ≥ 3%</b>：做多。</li>
</ul>`,
      quotes: [
        { text: '三次测试失败是重要的心理转折点。', source: 'candle Ch6' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/triple.rs</code>',
      },
      tags: ['三重顶', '三重底', 'triple_top', 'triple_bottom'],
    },
    {
      id: 'ck-island-reversal',
      group: 'candle',
      title: '岛形反转（Island Reversal）',
      badges: [{ text: '强反转', kind: 'iron' }, { text: '时间映射', kind: 'warn' }],
      desc: '价格先跳空上涨（或下跌）形成一个"岛"，随后跳空下跌（或上涨）回到"大陆"—— 岛的时间跨度决定反转级别。',
      whatIs: `<p>想象 K 线图有一块"小岛"（一组 K 线）与两侧的"大陆"（主要价格区）之间被两个<b>跳空缺口</b>隔开。</p>
<p>岛顶反转：上涨中跳空向上进入小岛区，然后跳空向下回到下方 —— 见顶。</p>
<p>岛底反转：下跌中跳空向下进入小岛区，然后跳空向上回到上方 —— 见底。</p>
<p><b>原书铁证</b>（candle p.660）：岛的时间跨度决定反转级别。1-5 天 = 短线反转；6-20 天 = 中线反转；> 20 天 = 大级别反转。</p>`,
      howTo: [
        '<b>两个跳空</b>：进入岛 + 离开岛各一个跳空缺口（缺口未被填补）。',
        '<b>岛区 K 线</b>：通常 3-20 根 K 线。',
        '<b>方向相反</b>：进入跳空方向 + 离开跳空方向相反。',
      ],
      strategy: `<ul>
<li><b>岛顶反转</b>：离开跳空确认后立即清仓 / 做空。</li>
<li><b>岛底反转</b>：离开跳空确认后建仓做多。</li>
<li><b>级别对应仓位</b>：短线反转 30%，中线 50%，大线 80%+。</li>
</ul>`,
      quotes: [
        { text: '岛形反转的时间跨度映射到反转级别。', source: 'candle p.660' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/advanced.rs::IslandReversal</code>',
      },
      tags: ['岛形', 'island', '跳空', '反转', '时间映射'],
    },
    {
      id: 'ck-inverted-v',
      group: 'candle',
      title: '倒置 V（Inverted V）',
      badges: [{ text: '顶部尖顶', kind: 'bear' }],
      desc: '价格急速上涨后立即急速下跌，形如倒 V —— 主力快速拉高出货的经典图形。三次减仓铁证。',
      whatIs: `<p>倒置 V 是最锋利的顶部形态：价格几乎没有"顶部横盘"过程，而是一个尖顶。通常伴随：</p>
<ul>
<li>巨量拉升（2-3 倍均量）</li>
<li>尖顶当日或次日急速反转</li>
<li>没有时间给散户决策，追高者立刻被套</li>
</ul>
<p><b>三次减仓铁证</b>（candle p.605）：第一次警报减仓 30%，第二次减仓 50%，第三次清仓 100%。原书强调：<b>"踏空是保障资金安全必须付出的代价"</b>。</p>`,
      strategy: `<ul>
<li><b>确认倒 V 后立即清仓</b>，不等反弹。</li>
<li><b>分段减仓</b>：顶部起不怀疑先减 30%，第二次反弹再减 50%，突破前低时全清。</li>
</ul>`,
      example: `<b>DOGE 2021/5/8</b>：0.72 USDT 尖顶后 2 天跌至 0.45（-38%），1 个月跌至 0.20（-72%）。未及时减仓的追高者损失惨重。`,
      quotes: [
        { text: '倒置 V 三次减仓：30% / 50% / 100%。', source: 'candle p.605' },
        { text: '踏空是保障资金安全必须付出的代价。', source: 'candle p.605' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/advanced.rs::InvertedV</code>',
      },
      tags: ['倒置V', 'inverted_v', '尖顶', '出货', '三次减仓'],
    },
    {
      id: 'ck-v-bottom',
      group: 'candle',
      title: 'V 底反转',
      badges: [{ text: '尖底', kind: 'bull' }],
      desc: '价格急速下跌后立即急速反弹，形如 V —— 恐慌性抛售后的快速反转，极强买点。',
      whatIs: `<p>V 底是倒置 V 的镜像：价格跌到谷底后几乎没有"底部盘整"，立刻反弹。常伴随：</p>
<ul>
<li>巨量砸盘（恐慌性抛售）</li>
<li>一根长下影或锤头</li>
<li>次日放量阳线直接反包</li>
</ul>
<p>V 底与淡友反攻（次日阳反包）、岛底反转（带跳空）属"底部三形态互通"（R-P1-36）。</p>`,
      strategy: `<ul>
<li><b>V 底 + 次日阳线确认</b>：重仓建多（70-100%）。</li>
<li><b>不等回抽</b>：V 底往往"一路涨"，等回抽容易踏空。</li>
</ul>`,
      quotes: [
        { text: '底部三形态互通：V / 淡友 / 岛形。', source: 'R-P1-36' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/advanced.rs::VBottom</code>',
      },
      tags: ['V底', 'v_bottom', '尖底', '反弹', 'bull'],
    },
    {
      id: 'ck-chart-rect',
      group: 'candle',
      title: '矩形（Rectangle，箱体整理）',
      badges: [{ text: '中继 / 出货', kind: '' }, { text: 'R-P1-40', kind: '' }],
      desc: '价格在水平高点和低点之间反复横盘 —— 主力吸筹或出货的典型特征，关键看突破方向。',
      whatIs: `<p>矩形形态：</p>
<ol>
<li>价格在 2 条水平线之间波动（上沿阻力 + 下沿支撑）</li>
<li>至少 2 次触碰上下沿</li>
<li>持续 10-50 根 K 线</li>
</ol>
<p><b>位置决定含义</b>：</p>
<ul>
<li><b>低位矩形</b> = 主力吸筹</li>
<li><b>高位矩形</b> = 主力出货</li>
<li><b>趋势中矩形</b> = 短期休整</li>
</ul>`,
      strategy: `<ul>
<li><b>低位突破上沿</b>：做多，目标位 = 突破价 + 矩形高度。</li>
<li><b>高位跌破下沿</b>：做空，目标位 = 跌破价 - 矩形高度。</li>
<li><b>不猜方向</b>：矩形内不操作，等突破。</li>
</ul>`,
      quotes: [
        { text: '矩形 = 主力高抛低吸 / 囤积。', source: 'candle p.795' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/rectangle.rs</code>',
      },
      tags: ['矩形', 'rectangle', '箱体', '吸筹', '出货'],
    },
    {
      id: 'ck-dumpling-top',
      group: 'candle',
      title: '圆弧顶 / 圆弧底（Dumpling / Rounding）',
      badges: [{ text: '慢反转', kind: '' }, { text: '3 阶段', kind: 'iron' }],
      desc: '价格呈圆弧形缓慢反转，没有尖锐的顶或底 —— 大级别趋势转换的标志，可靠但需要耐心。',
      whatIs: `<p>圆弧形态是"慢反转"，与倒置 V 和 V 底相对。三个阶段：</p>
<ol>
<li><b>阶段 1</b>：缓慢下跌（左弧），量能递减</li>
<li><b>阶段 2</b>：横盘见底（弧底），量能极低</li>
<li><b>阶段 3</b>：缓慢上升（右弧），量能递增</li>
</ol>
<p>每个阶段 ≥ 10 根 K 线，总持续 30-60 根。圆底形成后的上升通常很持续（主力充分吸筹）。</p>`,
      strategy: `<p><b>阶段 3 量能放大</b>时分批建仓。目标位 = 弧顶价 + 弧深。</p>`,
      quotes: [
        { text: '圆弧反转需时间验证，是大级别信号。', source: 'R-P1-28 / R-P1-48' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/advanced.rs::Rounding</code>',
      },
      tags: ['圆弧顶', '圆弧底', 'rounding', '3阶段', '慢反转'],
    },
    {
      id: 'ck-cup-handle',
      group: 'candle',
      title: '杯柄形态（Cup and Handle）',
      badges: [{ text: '看涨', kind: 'bull' }],
      desc: '圆底 + 右侧小幅回调（把手）—— 经典的看涨延续形态，由 William O\'Neil 提出。',
      whatIs: `<p>杯柄形态：</p>
<ol>
<li><b>杯体</b>：U 形底部，类似圆弧底</li>
<li><b>把手</b>：杯子右沿的小幅回调（5-10%）</li>
<li><b>突破</b>：价格突破杯沿上沿 + 放量</li>
</ol>
<p>杯柄是主力在"测试最后的抛压"—— 杯柄期主力最后一次打压，清洗浮筹，然后拉升。</p>`,
      strategy: `<p><b>把手中建仓</b>，突破杯沿 + 放量时加仓，目标位 = 杯沿 + 杯深。</p>`,
      quotes: [
        { text: 'Cup and Handle 是 CANSLIM 策略的核心形态。', source: 'O\'Neil' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/cup_handle.rs</code>',
      },
      tags: ['杯柄', 'cup_handle', '看涨'],
    },
    {
      id: 'ck-gap',
      group: 'candle',
      title: '跳空缺口（Gap）4 分类',
      badges: [{ text: '缺口理论', kind: 'iron' }],
      desc: '价格跳空产生缺口，4 种类型：<b>普通</b> / <b>突破</b> / <b>延续</b> / <b>衰竭</b>。每种含义不同，不可混淆。',
      whatIs: `<p>缺口是指某根 K 线开盘价与前一根收盘价之间出现的空白区域。4 种分类：</p>
<ol>
<li><b>普通缺口（Common Gap）</b>：横盘中的随机缺口，无方向意义，通常很快被填补。</li>
<li><b>突破缺口（Breakaway Gap）</b>：突破关键阻力 / 支撑时的缺口 —— <b>最重要</b>，通常不会被快速填补，是新趋势的起点。</li>
<li><b>延续缺口（Runaway Gap）</b>：趋势中期出现的缺口，往往距离起点和终点各一半。是趋势加速信号。</li>
<li><b>衰竭缺口（Exhaustion Gap）</b>：趋势末期的最后一个缺口，标志力竭。很快被填补，随后反转。</li>
</ol>`,
      howTo: [
        '<b>位置判断</b>：在趋势的什么阶段？',
        '<b>缺口填补速度</b>：当天填补 = 普通；几天填补 = 可能突破；长期不填 = 确认突破。',
        '<b>量能配合</b>：突破 / 延续缺口通常有放量；衰竭缺口后量能萎缩。',
      ],
      strategy: `<ul>
<li><b>突破缺口</b>：追涨/追跌，止损设缺口下沿 / 上沿。</li>
<li><b>延续缺口</b>：顺势加仓。</li>
<li><b>衰竭缺口</b>：准备反向操作。</li>
<li><b>普通缺口</b>：忽略。</li>
</ul>`,
      quotes: [
        { text: '缺口不闭合即为真，缺口闭合则翻盘。', source: 'candle Ch8 缺口理论' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/gap.rs</code>',
      },
      tags: ['跳空', 'gap', '缺口', '突破缺口', '衰竭缺口'],
    },
    {
      id: 'ck-three-methods',
      group: 'candle',
      title: '上升 / 下降三法（Rising / Falling Three Methods）',
      badges: [{ text: '中继', kind: '' }, { text: '五 K 线', kind: '' }],
      desc: '趋势中的"喘息"形态：第 1 / 5 根大阳（大阴），中间 3 根小阴（小阳）—— 趋势延续确认。',
      whatIs: `<p><b>上升三法</b>（看涨）：</p>
<ol>
<li>第 1 根：大阳线</li>
<li>第 2-4 根：3 根小阴线，但都在第 1 根阳线实体内</li>
<li>第 5 根：大阳线，收盘 > 第 1 根收盘</li>
</ol>
<p>意义：主力"洗盘"后继续拉升。小阴线是散户卖出，但都被第 5 根大阳吞没。</p>`,
      strategy: `<p>第 5 根大阳收盘时加仓，止损设 3 根小阴的最低点。</p>`,
      quotes: [
        { text: '三法是最经典的中继形态。', source: 'candle p.380' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::ThreeMethods</code>',
      },
      tags: ['上升三法', '三法', 'three_methods', '中继'],
    },
    {
      id: 'ck-abandoned-baby',
      group: 'candle',
      title: '弃婴形态（Abandoned Baby）',
      badges: [{ text: '极强反转', kind: 'iron' }],
      desc: '底部/顶部的 3 K 线形态：大阴 + 十字星（两侧都跳空）+ 大阳 —— 中间的十字星被"抛弃"在两个缺口之间。',
      whatIs: `<p>弃婴是早晨之星 / 黄昏之星的强化版：</p>
<ul>
<li>第 2 根十字星的最高价 < 第 1 根的最低价（向下跳空）</li>
<li>第 3 根阳线的最低价 > 第 2 根的最高价（向上跳空）</li>
<li>第 2 根"孤零零"悬在两个缺口之间 —— 故名"弃婴"</li>
</ul>
<p>极罕见但可靠度最高的反转形态之一。</p>`,
      strategy: `<p>弃婴确认后重仓建仓（看涨 100% / 看跌清仓）。</p>`,
      quotes: [
        { text: '弃婴形态罕见但极准确。', source: 'candle p.305' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::AbandonedBaby</code>',
      },
      tags: ['弃婴', 'abandoned_baby', '跳空', '反转'],
    },
    {
      id: 'ck-single-needle',
      group: 'candle',
      title: '单针探底 / 单针探顶（Single Needle）',
      badges: [{ text: '主力行为', kind: 'iron' }],
      desc: '一根 K 线有极长的下影（或上影），像"探针"快速扎到关键支撑再拉回 —— 主力试盘或扫货的典型手法。',
      whatIs: `<p>单针探底是典型的"主力测试抛压"行为：</p>
<ul>
<li>开盘后价格正常</li>
<li>盘中被主力瞬间打压（或拉升）到关键支撑 / 阻力位</li>
<li>尾盘迅速恢复到正常水平，只留下一根长影线</li>
</ul>
<p>关键特征：<b>影线 ≥ 全长 60%，实体很小</b>。与锤头 / 流星的区别：单针的影线更夸张，几乎只有一根线。</p>`,
      howTo: [
        '<b>下影比例</b>：下影长度 / K 线全长 ≥ 60%（探底）。',
        '<b>探到关键位</b>：下影最低点 ≈ MA60 / 前期支撑 / 心理位。',
        '<b>快速恢复</b>：收盘 > (开盘 + 最低) / 2（拉回超过 50%）。',
        '<b>量能配合</b>：下影形成时的量通常很大（主力动作）。',
      ],
      strategy: `<ul>
<li><b>单针探底 + 收阳</b>：主力在此位有买盘，支撑有效。次日建仓，止损设单针最低点。</li>
<li><b>单针探顶</b>：主力试出上方抛压，见顶信号。减仓。</li>
</ul>`,
      mistakes: [
        '<b>追单针最低点止损</b>：价格可能再次测试此位，太贴的止损易被扫。设在最低点下方 0.5-1 ATR。',
      ],
      example: `<b>BTC 2023/3/10 单针探底</b>：在 19500 出现一根超长下影（下影占 65%），下探 19100 但迅速拉回至 19800。随后反弹至 28000（+43%）。主力在此位扫货。`,
      quotes: [
        { text: '单针是主力意图的公开展示。', source: 'candle 主力行为学 Ch8' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::SingleNeedle</code>',
      },
      tags: ['单针探底', 'single_needle', '长影线', '主力试盘'],
    },
    {
      id: 'ck-belt-hold',
      group: 'candle',
      title: '捉腰带线（Belt Hold）',
      badges: [{ text: '反转信号', kind: 'warn' }],
      desc: '开盘价即最低（最高）价，一根没有下影（上影）的大阳（大阴）线 —— 表示开盘就强势主导，无反向阻碍。',
      whatIs: `<p>捉腰带线是一种特殊的 Marubozu 变体：</p>
<ul>
<li><b>看涨捉腰带</b>：开盘价 = 最低价（无下影），大阳线。</li>
<li><b>看跌捉腰带</b>：开盘价 = 最高价（无上影），大阴线。</li>
</ul>
<p>意义：开盘即由一方完全主导，对方连试探都没能成功。出现在底部 / 顶部时是强烈反转信号。</p>`,
      strategy: '<p>低位看涨捉腰带：建仓；高位看跌捉腰带：减仓。</p>',
      quotes: [
        { text: '捉腰带线是一根 K 线决胜负的形态。', source: 'candle p.95' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::BeltHold</code>',
      },
      tags: ['捉腰带', 'belt_hold', '反转'],
    },
    {
      id: 'ck-counter-attack',
      group: 'candle',
      title: '淡友反攻 / 空方反攻（Counter Attack）',
      badges: [{ text: '反转', kind: 'warn' }, { text: '底部三形态', kind: 'iron' }],
      desc: '下跌末期阴线后的大阳反包，但开盘跳空低开（与刺透形态不同，开盘价更低）—— 底部反转信号。',
      whatIs: `<p>淡友反攻：</p>
<ol>
<li>第 1 根：大阴线，延续下跌</li>
<li>第 2 根：大幅跳低开盘（比前阴低点更低），但尾盘迅速拉回，<b>收盘价 ≈ 第 1 根阴线的收盘价</b></li>
</ol>
<p>意义：空头本想继续砸盘（跳低），但多头反攻把价格拉回前一日收盘附近。力量出现转折。</p>
<p>淡友反攻属于"底部三形态互通"（R-P1-36）：V 底 / 淡友反攻 / 岛底反转 —— 三者在原理上等价。</p>`,
      strategy: '<p>淡友反攻 + 放量 → 试探建仓，止损设第 2 根最低点。</p>',
      quotes: [
        { text: '底部三形态互通：V / 淡友 / 岛形。', source: 'R-P1-36' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::CounterAttack</code>',
      },
      tags: ['淡友反攻', '空方反攻', 'counter_attack', '底部'],
    },
    {
      id: 'ck-stick-sandwich',
      group: 'candle',
      title: '条形三明治（Stick Sandwich）',
      badges: [{ text: '反转', kind: '' }, { text: '三 K 线', kind: '' }],
      desc: '阴 - 阳 - 阴 / 阳 - 阴 - 阳 的 3 K 线组合，且第 1 根和第 3 根的收盘价几乎相同 —— 像夹心三明治。',
      whatIs: `<p>第 1 根和第 3 根的收盘价相同（±0.3%），中间一根方向相反。形成"两片面包 + 一片火腿"的视觉效果。</p>
<p>底部出现 = 看涨（阴阳阴，两阴的收盘支撑相同）；顶部出现 = 看跌（阳阴阳）。</p>
<p>是较弱的反转信号，需要其他指标确认。</p>`,
      strategy: '<p>单独出现不足以交易，需配合量价 / 趋势背景。</p>',
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::StickSandwich</code>',
      },
      tags: ['三明治', 'stick_sandwich', '反转'],
    },
    {
      id: 'ck-hanging-man',
      group: 'candle',
      title: '上吊线（Hanging Man）',
      badges: [{ text: '顶部警报', kind: 'bear' }],
      desc: '形状与锤头完全一样，但出现在<b>上涨末期</b> —— 是顶部反转警报（与底部锤头相反）。',
      whatIs: `<p>同样的一根"下影长 + 实体小"的 K 线：</p>
<ul>
<li>出现在下跌末期 = <b>锤头</b>（看涨）</li>
<li>出现在上涨末期 = <b>上吊线</b>（看跌）</li>
</ul>
<p>为什么上涨末期的长下影反而看跌？因为说明盘中出现了<b>空头强烈砸盘</b>，虽然被多头拉回，但这是空头开始发力的信号。就像一个人被吊起来了。</p>`,
      howTo: [
        '<b>上涨背景</b>：前 5-10 根 K 线上涨。',
        '<b>下影长</b>：≥ 2 × 实体。',
        '<b>次日阴线确认</b>：是反转的必要条件。',
      ],
      strategy: '<p>次日阴线确认后减仓 50%，止损设上吊线最高点。</p>',
      quotes: [
        { text: '上吊线的下影是空头进攻的第一击。', source: 'candle p.78' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::HangingMan</code>',
      },
      tags: ['上吊线', 'hanging_man', '顶部反转'],
    },
    {
      id: 'ck-inverted-hammer',
      group: 'candle',
      title: '倒锤头 / 纸伞（Inverted Hammer）',
      badges: [{ text: '底部信号', kind: 'bull' }],
      desc: '形状与流星相同（长上影 + 小实体），但出现在<b>下跌末期</b> —— 底部反转信号。',
      whatIs: `<p>倒锤头是流星的底部版：</p>
<ul>
<li>出现在上涨末期 = <b>流星</b>（看跌）</li>
<li>出现在下跌末期 = <b>倒锤头</b>（看涨）</li>
</ul>
<p>下跌末期的长上影意味着<b>多头试图反攻</b>（虽然被空头打回），但接下来空头力量会衰竭。次日阳线确认后买入。</p>`,
      strategy: '<p>次日阳线确认 + 放量 → 试探建仓。</p>',
      quotes: [
        { text: '倒锤头是多头反攻的号角。', source: 'candle p.80' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::InvertedHammer</code>',
      },
      tags: ['倒锤头', 'inverted_hammer', '纸伞', '底部'],
    },
    {
      id: 'ck-kicking',
      group: 'candle',
      title: '反冲形态（Kicking）',
      badges: [{ text: '极强信号', kind: 'iron' }],
      desc: '连续两根 Marubozu 但方向完全相反，中间有<b>巨大跳空</b> —— 极罕见但可靠度极高的反转形态。',
      whatIs: `<p>Kicking 形态：</p>
<ol>
<li>第 1 根：Marubozu（大阳 或 大阴，无影线）</li>
<li>第 2 根：跳空 + Marubozu，方向与第 1 根完全相反</li>
</ol>
<p>例如：大阳 Marubozu 后，次日大幅跳空低开，收盘大阴 Marubozu。或相反。</p>
<p>为什么可靠？因为两根都是"光头光脚"，表示两天内市场情绪 180° 反转，中间还有跳空。这是极大的心理转变。</p>`,
      strategy: '<p>反冲形态出现后立即操作（重仓 70-100%），止损设反向。</p>',
      quotes: [
        { text: 'Kicking 是 K 线形态中罕见的高确定性信号。', source: 'candle p.310' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Kicking</code>',
      },
      tags: ['反冲', 'kicking', '跳空', '反转'],
    },
    {
      id: 'ck-separating-lines',
      group: 'candle',
      title: '分手线（Separating Lines）',
      badges: [{ text: '中继', kind: '' }],
      desc: '两根相反方向的 K 线，但开盘价相同（不是吞没，不是反冲，而是从同一点分道扬镳）—— 趋势延续信号。',
      whatIs: `<p>分手线：第 2 根与第 1 根的方向相反，但两根的<b>开盘价相同</b>（±0.3%）。</p>
<p>例如：上涨中出现阴线但次日阳线在同一开盘价继续上涨。意味着"昨天的阴线只是小插曲，多头继续前进"。</p>`,
      strategy: '<p>分手线确认趋势延续，顺势加仓。</p>',
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::SeparatingLines</code>',
      },
      tags: ['分手线', 'separating_lines', '中继'],
    },
    {
      id: 'ck-breakaway',
      group: 'candle',
      title: '脱离形态（Breakaway）',
      badges: [{ text: '5 K 线', kind: '' }],
      desc: '5 根 K 线组合：第 1 根大（同向）+ 3 根小（同向延续）+ 最后一根反向大 K 线 —— 典型的转折点。',
      whatIs: `<p>底部脱离形态：</p>
<ol>
<li>第 1 根：大阴（下跌中）</li>
<li>第 2-4 根：3 根小 K 线（继续下跌但幅度递减）</li>
<li>第 5 根：大阳，吞没前 3 根小阴</li>
</ol>
<p>意义：下跌动能逐步衰竭（第 2-4 根），最后被大阳反攻吞没 —— 反转确认。</p>`,
      strategy: '<p>第 5 根大阳收盘时建仓，止损设第 2-4 根最低点。</p>',
      meta: {
        '代码': '<code class="kb-file">src/engine/candle/patterns.rs::Breakaway</code>',
      },
      tags: ['脱离', 'breakaway', '5K线', '反转'],
    },
    {
      id: 'ck-volume-symmetry',
      group: 'candle',
      title: '头肩底量价对称（R-P1-23 铁证）',
      badges: [{ text: '铁证', kind: 'iron' }, { text: '量价规则', kind: '' }],
      desc: '头肩底的量能规则：<b>左肩量 > 头量 > 右肩量</b>，且<b>颈线突破时必须放量</b>。',
      whatIs: `<p>头肩底（见底反转）的量价对称原则：</p>
<ul>
<li><b>左肩阶段</b>：下跌到第 1 次低点，量能大（恐慌抛售）</li>
<li><b>头部阶段</b>：下跌到更低的底，但量能已减少（空头衰竭）</li>
<li><b>右肩阶段</b>：反弹后回踩，但低点高于头部，量能最小（空头无力）</li>
<li><b>颈线突破</b>：价格上破颈线时，量能必须再次放大（多头确认）</li>
</ul>
<p>对称性：量能从大 → 中 → 小，再在突破时变大。不符合此对称的"视觉头肩底"可靠度低。</p>`,
      howTo: [
        '<b>测量 3 段量能</b>：左肩量 / 头量 / 右肩量。',
        '<b>验证递减</b>：左肩 > 头 > 右肩（每段差异 ≥ 20%）。',
        '<b>突破量能</b>：颈线突破根量 ≥ 1.5 × 头部量。',
      ],
      strategy: '<p>量价对称的头肩底是 L7 级买入信号；不符合量价对称的"假头肩底"不交易。</p>',
      quotes: [
        { text: '头肩底的量价对称是形态有效性的最高标准。', source: 'candle R-P1-23' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/chartpattern/head_shoulder.rs</code>',
        '测试': '3 个量价对称测试用例',
      },
      tags: ['头肩底', '量价对称', 'R-P1-23', '铁证'],
    },

    // ==================== 信号分级 ====================
    {
      id: 'sig-levels',
      group: 'signal',
      title: 'L1-L8 信号分级体系',
      badges: [{ text: '核心', kind: 'iron' }],
      desc: '<b>L1</b> 弱信号（单 K 线） → <b>L8</b> 顶级共振（多形态多时间框）。每个信号都附 level/book_source/severity。AURA 的 <code>SignalLevel</code> 统一所有模块的分级。',
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/level.rs</code>',
        '测试': '10 个测试用例',
        '修复': 'R-P1-02/03/10/11 (Sprint 6)',
      },
      tags: ['L1', 'L8', '分级', 'level', 'severity'],
    },
    {
      id: 'sig-confluence',
      group: 'signal',
      title: 'Confluence 多合一识别',
      badges: [],
      desc: '识别在 ±0.3% 价格带内同时存在的多个独立信号源：<b>MA / 趋势线 / S/R / Fib / 心理价位</b>。数量越多、种类越分散，Confluence 分数越高。',
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/confluence.rs</code>',
        'API': '<span class="kb-chip api">/api/signals</span>',
        '字段': 'confluences: Vec&lt;Confluence&gt;',
      },
      tags: ['confluence', '共振', '多合一'],
    },
    {
      id: 'sig-stealth',
      group: 'signal',
      title: '潜伏突破 / 穿头破脚',
      badges: [{ text: '主力行为', kind: 'iron' }],
      desc: '<b>潜伏突破</b>（R-P1-30）：连续小阳线缩量，悄然突破关键阻力 = 主力吸筹完成。<b>穿头破脚</b>（R-P1-31）：一根大阳线吞没前 N 根小阴线 = 主力强势拉升。',
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/stealth.rs</code>',
        '测试': '6 个测试用例',
        'API': '<span class="kb-chip api">/api/signals</span> stealth_breakouts',
      },
      tags: ['潜伏', 'stealth', '穿头破脚', '主力'],
    },
    {
      id: 'sig-traps',
      group: 'signal',
      title: '多头陷阱 / 空头陷阱',
      badges: [{ text: '假突破', kind: 'bear' }],
      desc: 'R-P1-17 多头陷阱：价格突破但未满足 3% 有效性阈值，随后快速回落。AURA 会在 <b>突破后 3-5 根 K 线内持续检测</b>，失效则标记为陷阱，降低信号权重。',
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/bull_trap.rs</code>',
        '测试': '7 个测试用例',
        'API': '<span class="kb-chip api">/api/signals</span> bull_traps',
      },
      tags: ['陷阱', 'trap', '假突破'],
    },
    {
      id: 'sig-volume',
      group: 'signal',
      title: '量能异常（放量 / 缩量）',
      badges: [],
      desc: '当前 bar 成交量相对 20 均量的倍数：<b>≥ 3× = 放量</b>，<b>≤ 0.5× = 缩量</b>。AURA 的 <code>VolumeAnomalyEvent</code> 与价格形态联合验证信号有效性。',
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/volume.rs</code>',
        'API': '<span class="kb-chip api">/api/signals</span> volume_anomalies',
      },
      tags: ['量能', 'volume', '放量', '缩量'],
    },
    {
      id: 'sig-router',
      group: 'signal',
      title: 'Priority 路由',
      badges: [{ text: 'Sprint 10', kind: '' }],
      desc: 'R-P1-05 信号路由系统：根据每个信号的 <b>level / book_source / time_frame</b> 自动分优先级。高优先级信号（如 L8 + 铁证 + 多时间框共振）会首先触发通知/交易。',
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/router.rs</code>',
        '测试': '9 个测试用例',
      },
      tags: ['router', '路由', 'priority', '优先级'],
    },

    // ==================== 风险管理 ====================
    {
      id: 'rm-position',
      group: 'risk',
      title: 'PositionLimit 仓位校验',
      badges: [{ text: '强制约束', kind: 'iron' }],
      desc: '所有 Playbook 决策必须经过 <code>PositionLimit</code> 校验：<b>L4_MAX = 30%</b>、<b>BULL_MAX = 100%</b>、<b>SELL_MAX = 0%</b>。超出约束的决策会被强制截断。',
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/strategy.rs:238</code>',
        '测试': '8 个测试用例',
      },
      tags: ['仓位', 'position', 'limit', '30%', '校验'],
    },
    {
      id: 'rm-stop',
      group: 'risk',
      title: 'ATR 止损',
      badges: [],
      desc: 'Average True Range × 倍数 = 止损距离。默认 <b>2 × ATR</b>。远离均线或 Swing 极值的入场，使用 ATR 自适应调整以避免过紧/过松。',
      meta: {
        '代码': '<code class="kb-file">src/engine/trend/strategy.rs</code>',
        '参数': 'atr_mult (默认 2.0)',
      },
      tags: ['ATR', '止损', 'stop_loss'],
    },
    {
      id: 'rm-rr',
      group: 'risk',
      title: 'R:R 盈亏比',
      badges: [],
      desc: '止盈距离 / 止损距离 = R:R。默认 <b>2:1</b>，即最低盈亏比。低于此比例的交易机会会被 Playbook 过滤，保证长期正期望。',
      meta: {
        '代码': '<code class="kb-file">src/backtest/playbook.rs</code>',
        '参数': 'rr (默认 2.0)',
      },
      tags: ['RR', '盈亏比', '止盈'],
    },
    {
      id: 'rm-staged',
      group: 'risk',
      title: '分级减仓路径（30% / 50% / 100%）',
      badges: [{ text: '铁证', kind: 'iron' }],
      desc: '倒置 V 三次减仓：第一次卖出 30%（警示），第二次卖出 50%（确认），第三次清仓（失守）。AURA 的 <code>StagedExit</code> 模块按这个路径执行。',
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/staged_exit.rs</code>',
        '测试': '7 个测试用例',
        '修复': 'R-P1-42/32 (Sprint 6)',
      },
      tags: ['分级减仓', 'staged_exit', '30/50/100'],
    },
    {
      id: 'rm-risk-reward',
      group: 'risk',
      title: '盈亏比最低 2:1（R:R ≥ 2）',
      badges: [{ text: '铁律', kind: 'iron' }],
      desc: '每笔交易的<b>止盈距离 / 止损距离 ≥ 2</b>。这不是策略而是生存底线 —— 只有高 R:R 才能在胜率 40-50% 时仍然盈利。',
      whatIs: `<p>假设你的胜率是 50%（即 10 笔中 5 笔赚、5 笔亏）。</p>
<ul>
<li><b>R:R = 1:1</b>：赚 5 × 100 = 500；亏 5 × 100 = 500。净利润 0，还不算手续费。</li>
<li><b>R:R = 2:1</b>：赚 5 × 200 = 1000；亏 5 × 100 = 500。净利润 +500。</li>
<li><b>R:R = 3:1</b>：赚 5 × 300 = 1500；亏 5 × 100 = 500。净利润 +1000。</li>
</ul>
<p>数学铁律：<b>长期盈利的交易者，胜率不一定高，但 R:R 一定 ≥ 2</b>。AURA 的 Playbook 强制要求 R:R ≥ 2，否则不发信号。</p>`,
      howTo: [
        '<b>确定止损</b>：入场前先确定止损价（如 MA20 下方 1 ATR）。',
        '<b>计算距离</b>：止损距离 = |入场价 - 止损价|。',
        '<b>设置止盈</b>：止盈距离 ≥ 2 × 止损距离，即 R:R ≥ 2。',
        '<b>检查可行性</b>：止盈位是否在合理位置（前高 / Fib 扩展 / 目标位）？若是 → 执行；若不是 → 放弃该交易。',
      ],
      params: {
        '最低 R:R': '<code>2.0</code>（AURA 默认）',
        '建议 R:R': '<code>2.5 - 3.0</code>',
      },
      strategy: '<p><b>R:R < 2 的交易直接放弃</b>，等更好的机会。</p>',
      mistakes: [
        '<b>为了"冲单量"降低 R:R</b>：赚得少亏得大，长期必亏。',
        '<b>不设止盈只设止损</b>：无 R:R 概念，容易在盈利时舍不得卖。',
      ],
      example: `<b>BTC 4h 交易示例</b>：入场 60000，止损 58800（距离 1200 = 2%），止盈 62400（距离 2400 = 4%）。R:R = 2:1。符合最低要求。`,
      quotes: [
        { text: 'R:R 是交易数学的铁律，不是策略选择。', source: 'trend Ch7 资金管理' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/backtest/playbook.rs</code>',
        '参数': 'rr (默认 2.0)',
      },
      tags: ['RR', '盈亏比', 'risk_reward', '2:1', '铁律'],
    },
    {
      id: 'rm-single-risk',
      group: 'risk',
      title: '单笔风险 ≤ 2%（2% Rule）',
      badges: [{ text: '铁律', kind: 'iron' }, { text: '资金管理', kind: 'warn' }],
      desc: '每笔交易的最大亏损 ≤ 账户总资金的 2%。这是避免"一笔爆仓"的铁律。',
      whatIs: `<p>为什么是 2%？做个数学：</p>
<table class="kb-api-table">
<tr><th>连续亏损次数</th><th>账户剩余（从 100% 起）</th></tr>
<tr><td>5 次 × 2%</td><td>90.4%</td></tr>
<tr><td>10 次 × 2%</td><td>81.7%</td></tr>
<tr><td>20 次 × 2%</td><td>66.8%</td></tr>
<tr><td>50 次 × 2%</td><td>36.4%</td></tr>
</table>
<p>即使连续亏 50 次（极端情况）你还有 36% 本金，有机会翻身。</p>
<p>反观：<b>单笔风险 20%，连亏 3 次你就只剩 51%</b>，你需要翻倍才能回本。这就是为什么<b>重仓交易的人迟早爆仓</b>。</p>`,
      howTo: [
        '<b>账户总值 × 2%</b> = 单笔最大允许亏损。',
        '<b>止损距离</b>：根据技术位（MA / Swing）确定。',
        '<b>头寸计算</b>：头寸 = 最大亏损 / 止损距离。',
      ],
      params: {
        '单笔风险': '<code>≤ 2%</code>（AURA 默认）',
        '激进': '<code>≤ 3%</code>',
        '保守': '<code>≤ 1%</code>',
      },
      example: `<b>示例</b>：账户 10000 USDT，最大单笔亏损 200 USDT（2%）。BTC @ 60000 入场，止损 58800（距离 1200）。头寸 = 200 / 1200 = 0.167 BTC，价值 10020 USDT —— 正好覆盖账户。`,
      quotes: [
        { text: '2% 法则是专业交易员的第一准则。', source: 'trend p.240 资金管理' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/backtest/playbook_runner.rs</code>',
        '参数': 'max_risk_pct (默认 2.0)',
      },
      tags: ['2%法则', '单笔风险', 'max_risk', '资金管理'],
    },
    {
      id: 'rm-dd',
      group: 'risk',
      title: '最大回撤（Max Drawdown）',
      badges: [{ text: '关键指标', kind: 'warn' }],
      desc: '账户历史最高值到最低值的最大跌幅。<b>控制回撤比追求收益更重要</b>。',
      whatIs: `<p>回撤（Drawdown）= 从峰值到谷值的百分比下跌。例如账户从 10000 跌到 7500，回撤 25%。</p>
<p>最大回撤（Max DD）是这个策略 / 账户历史上经历过的最大回撤。它决定：</p>
<ul>
<li><b>心理承受</b>：DD 30% 很痛苦；DD 50% 会让人放弃策略。</li>
<li><b>资金复利</b>：回撤 50% 需要涨 100% 才能回本。</li>
<li><b>策略健康度</b>：Sharpe / Calmar 比率都依赖 DD。</li>
</ul>
<p>专业交易员宁可 15% 年化收益 + 10% DD，也不要 50% 年化 + 40% DD。<b>稳健比爆发重要</b>。</p>`,
      strategy: `<ul>
<li><b>设置 DD 警戒</b>：DD 超过 10% 减少仓位，超过 20% 停止新单复盘。</li>
<li><b>分散风险</b>：不把所有资金放在同一币同一策略上。</li>
<li><b>降低杠杆</b>：杠杆会放大 DD。</li>
</ul>`,
      quotes: [
        { text: '回撤是交易员的噩梦，也是试金石。', source: 'trend p.245' },
      ],
      meta: {
        '指标': 'AURA 回测报告提供 MaxDD / Calmar 比率',
      },
      tags: ['回撤', 'drawdown', 'MaxDD', '风险'],
    },
    {
      id: 'rm-sharpe',
      group: 'risk',
      title: '夏普比率（Sharpe Ratio）',
      badges: [{ text: '策略衡量', kind: '' }],
      desc: '衡量"每承担 1 单位风险获得多少超额收益"的经典指标。> 1 合格 / > 2 优秀 / > 3 顶级。',
      whatIs: `<p>公式：<b>Sharpe = (年化收益 - 无风险利率) / 年化波动率</b>。</p>
<p>简单说：你获得的超额收益与你承受的波动之比。</p>
<ul>
<li><b>Sharpe < 1</b>：风险回报不划算，不如指数基金。</li>
<li><b>Sharpe 1-2</b>：合格，可持续运营。</li>
<li><b>Sharpe 2-3</b>：优秀，职业交易水准。</li>
<li><b>Sharpe > 3</b>：顶级，可能过拟合或短期走运。</li>
</ul>
<p>AURA 的回测报告中 Sharpe / Sortino / Calmar 是核心 KPI。</p>`,
      strategy: `<p>策略优化的目标是 <b>提高 Sharpe 而非收益率</b>。Sharpe 2 + 20% 年化，比 Sharpe 0.8 + 50% 年化更值得选择。</p>`,
      quotes: [
        { text: 'Sharpe 是专业交易员的试金石。', source: 'quant literature' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/backtest/metrics.rs::sharpe</code>',
      },
      tags: ['Sharpe', '夏普比率', '风险调整', '策略'],
    },
    {
      id: 'sig-reliability',
      group: 'signal',
      title: '信号可靠度等级（Reliability）',
      badges: [{ text: '分级体系', kind: '' }],
      desc: 'AURA 对每个信号都给出可靠度评分：<b>低 / 中 / 高 / 极高</b>，帮助决策时做取舍。',
      whatIs: `<p>可靠度评估 4 个维度：</p>
<ol>
<li><b>原书铁证</b>：是否有书名 + 页码的明确引用？</li>
<li><b>历史验证</b>：是否在 AURA 回测中通过真实数据验证？</li>
<li><b>共振数量</b>：有多少独立信号共同指向同一结论？</li>
<li><b>上下文</b>：在当前趋势 / 时间框 / 波动率下是否典型？</li>
</ol>
<p>4 维全满 = 极高可靠（L8 级）；3 维满 = 高可靠（L6）；2 维满 = 中（L4）；1 维 = 低（L1-L3）。</p>`,
      strategy: `<ul>
<li><b>极高可靠</b>：全仓建立 / 清仓。</li>
<li><b>高可靠</b>：重仓（50-70%）。</li>
<li><b>中可靠</b>：中仓（30-50%）。</li>
<li><b>低可靠</b>：小仓（10-20%）或观察。</li>
</ul>`,
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/level.rs</code>',
      },
      tags: ['可靠度', 'reliability', '分级'],
    },
    {
      id: 'sig-divergence',
      group: 'signal',
      title: '顶背离 / 底背离（Divergence）',
      badges: [{ text: '反转预警', kind: 'warn' }],
      desc: '价格创新高但指标（RSI / MACD）未创新高 = 顶背离（看跌）；价格创新低但指标未创新低 = 底背离（看涨）。',
      whatIs: `<p>背离是"价量背离"的专业版：</p>
<ul>
<li><b>顶背离</b>：近期价格高点一次比一次高，但 RSI / MACD 的高点一次比一次低。意味着上涨动能衰竭 —— 即将见顶。</li>
<li><b>底背离</b>：价格低点一次比一次低，但指标低点一次比一次高。意味着下跌动能衰竭 —— 即将见底。</li>
</ul>
<p>背离是领先指标，但可能持续较长时间（"背离能延续"），所以应配合其他信号使用。</p>`,
      strategy: `<ul>
<li><b>顶背离 + K 线见顶形态</b>：减仓 / 做空。</li>
<li><b>底背离 + K 线见底形态</b>：加仓 / 做多。</li>
<li><b>单独背离</b>：仅作为预警，不作为主要依据。</li>
</ul>`,
      quotes: [
        { text: '背离是动能衰竭的指标。', source: 'ma / candle 综合' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/indicator/divergence.rs</code>',
      },
      tags: ['背离', 'divergence', '顶背离', '底背离', 'RSI', 'MACD'],
    },
    {
      id: 'rm-kelly',
      group: 'risk',
      title: 'Kelly 公式（仓位最优解）',
      badges: [{ text: '数学', kind: 'iron' }],
      desc: '数学化计算"单笔最优仓位"的公式：<b>f = (p × b - q) / b</b>，其中 p=胜率、q=败率、b=盈亏比。',
      whatIs: `<p>Kelly Criterion 是 1956 年 John Kelly 提出的数学最优下注量公式。</p>
<p>例：胜率 60%（p=0.6），败率 40%（q=0.4），盈亏比 2:1（b=2）：</p>
<p><b>f = (0.6 × 2 - 0.4) / 2 = 0.4 = 40%</b></p>
<p>即每次用账户 40% 下注是数学最优。但实际应用中建议用 <b>Half-Kelly（20%）</b>，因为：</p>
<ul>
<li>胜率和盈亏比的估计通常偏乐观</li>
<li>全 Kelly 仓位的波动极大，心理难以承受</li>
<li>Half-Kelly 的期望收益是全 Kelly 的 75%，但波动降低 50%</li>
</ul>`,
      strategy: `<p><b>实用建议</b>：估算胜率和 R:R，用 Half-Kelly = (f) / 2 作为仓位上限。</p>`,
      mistakes: [
        '<b>用全 Kelly 下单</b>：数学最优 ≠ 实用最优。波动会让你崩溃。',
      ],
      quotes: [
        { text: 'Kelly 公式是仓位管理的数学基石。', source: 'Ed Thorp' },
      ],
      tags: ['Kelly', '公式', '仓位', '数学', '最优'],
    },
    {
      id: 'rm-compound',
      group: 'risk',
      title: '复利的力量（与回撤杀伤力）',
      badges: [{ text: '数学', kind: 'iron' }, { text: '反直觉', kind: 'warn' }],
      desc: '回本需要的涨幅比亏损幅度大：<b>亏 50% 需要涨 100%；亏 70% 需要涨 233%</b>。所以控制回撤 = 保护复利。',
      whatIs: `<p>复利公式铁律：</p>
<table class="kb-api-table">
<tr><th>回撤幅度</th><th>回本所需涨幅</th></tr>
<tr><td>10%</td><td>11%</td></tr>
<tr><td>20%</td><td>25%</td></tr>
<tr><td>30%</td><td>43%</td></tr>
<tr><td>40%</td><td>67%</td></tr>
<tr><td>50%</td><td>100%</td></tr>
<tr><td>70%</td><td>233%</td></tr>
<tr><td>90%</td><td>900%</td></tr>
</table>
<p>这就是为什么<b>控制回撤比追求收益更重要</b>。10% 回撤只要 11% 反弹就回本；50% 回撤要翻倍。</p>`,
      strategy: `<p><b>绝不让单笔亏损 > 10%。</b> 回撤 10% 内是可修复的，更深就越来越难。</p>`,
      tags: ['复利', 'compound', '回撤', '回本', '数学'],
    },
    {
      id: 'rm-leverage',
      group: 'risk',
      title: '杠杆危险（Leverage Risk）',
      badges: [{ text: '高危', kind: 'bear' }],
      desc: '杠杆 10x = 账户波动放大 10 倍 = <b>10% 反向就爆仓</b>。加密市场 90% 的爆仓用户都是杠杆过度。',
      whatIs: `<p>杠杆常被误解为"放大收益"，实际是<b>放大风险</b>：</p>
<ul>
<li><b>2x 杠杆</b>：账户波动翻倍。反向 50% 爆仓。</li>
<li><b>5x 杠杆</b>：反向 20% 爆仓。</li>
<li><b>10x 杠杆</b>：反向 10% 爆仓。</li>
<li><b>100x 杠杆</b>：反向 1% 爆仓！</li>
</ul>
<p>加密日内波动 5-10% 是常态。5x 以上杠杆在加密市场几乎都会爆仓。</p>`,
      strategy: `<ul>
<li><b>新手不用杠杆</b>：先用现货熟悉市场。</li>
<li><b>进阶 2-3x</b>：有经验后适度使用。</li>
<li><b>永远不超 5x</b>：即便全职交易员也很少超。</li>
</ul>`,
      mistakes: [
        '<b>满仓 100x 赌方向</b>：赌博而非交易。迟早归零。',
        '<b>用杠杆"回本"</b>：亏损后加杠杆想快速回本 —— 最快的清零路径。',
      ],
      example: `<b>SEC 数据</b>：2022 年加密市场超 90% 的爆仓用户使用 5x 以上杠杆。主力经常通过"插针"行情专门扫杠杆仓。`,
      tags: ['杠杆', 'leverage', '爆仓', '风险'],
    },
    {
      id: 'rm-diversify',
      group: 'risk',
      title: '分散化（Diversification）',
      badges: [{ text: '风险分散', kind: '' }],
      desc: '不把所有鸡蛋放在一个篮子里 —— 但加密市场的相关性特殊，BTC 与大部分山寨币的相关性 > 0.7，分散效果有限。',
      whatIs: `<p>传统分散化：买 10 个不同行业的股票，降低单一风险。</p>
<p>加密分散化：BTC / ETH / SOL / BNB / AVAX —— <b>大多数 > 0.7 相关性</b>。BTC 跌，几乎所有都跌。真正的分散要跨资产类别：加密 + 股票 + 黄金 + 现金。</p>`,
      strategy: `<ul>
<li><b>加密内</b>：BTC 50% + ETH 30% + 其他 20%（降低"山寨大跌"风险）。</li>
<li><b>跨类别</b>：加密 50% + 股票 30% + 现金 20%（抵御系统性风险）。</li>
<li><b>再平衡</b>：每季度调整回目标比例，高抛低吸。</li>
</ul>`,
      tags: ['分散化', 'diversification', '组合', '风险'],
    },
    {
      id: 'rm-black-swan',
      group: 'risk',
      title: '黑天鹅应对（Black Swan）',
      badges: [{ text: '极端风险', kind: 'bear' }],
      desc: '罕见但冲击巨大的事件：LUNA 崩盘、FTX 爆雷、USDC 脱锚。<b>不可预测，但可预案</b>。',
      whatIs: `<p>黑天鹅的特点：</p>
<ol>
<li><b>罕见</b>：5-10 年一次</li>
<li><b>冲击大</b>：市场腰斩或归零</li>
<li><b>事后可解释</b>：但事前无法预测</li>
</ol>
<p>加密历史黑天鹅：</p>
<ul>
<li>2014 Mt.Gox 交易所倒闭</li>
<li>2017 ICO 泡沫破裂</li>
<li>2022/5 LUNA 归零</li>
<li>2022/11 FTX 爆雷</li>
<li>2023/3 USDC 短暂脱锚</li>
</ul>`,
      strategy: `<ul>
<li><b>留足现金</b>：账户至少保留 20% 现金，永不全仓。</li>
<li><b>分散交易所</b>：不把所有资产放一个平台。</li>
<li><b>硬件钱包</b>：长期资产放冷钱包，远离交易所。</li>
<li><b>设总风险上限</b>：整个账户最大亏损 ≤ 30%，触发即全清。</li>
</ul>`,
      quotes: [
        { text: '黑天鹅无法预测，只能预案。', source: 'Nassim Taleb' },
      ],
      tags: ['黑天鹅', 'black_swan', '极端风险', '预案'],
    },
    {
      id: 'rm-correlation',
      group: 'risk',
      title: '相关性风险（Correlation Risk）',
      badges: [{ text: '隐性风险', kind: 'warn' }],
      desc: '持有看似分散的资产，但<b>相关性极高</b>（都跟 BTC），实际上=满仓单一资产。',
      whatIs: `<p>"假分散化"的常见误区：</p>
<ul>
<li>同时持有 5 个山寨币 —— 它们相关性 > 0.9，BTC 跌全跌。</li>
<li>只买科技股 —— 一场加息全部下跌。</li>
</ul>
<p>衡量：相关系数（Correlation）。</p>
<ul>
<li>+1.0：完全同向</li>
<li>0：不相关</li>
<li>-1.0：完全反向</li>
</ul>
<p>真正的分散需要 <b>相关性 < 0.3 的资产</b>。</p>`,
      strategy: '<p>加密 + 黄金 + 美股 + 现金 是相对好的组合（相关性较低）。纯加密组合不算分散。</p>',
      tags: ['相关性', 'correlation', '分散', '风险'],
    },
    {
      id: 'rm-time-diversify',
      group: 'risk',
      title: '时间分散（Time Diversification）',
      badges: [{ text: 'DCA', kind: '' }],
      desc: '不择时，<b>定期定额</b>买入 —— 是散户最好的长期策略。避免"抄底情绪化"决策。',
      whatIs: `<p>DCA（Dollar Cost Averaging）：每周 / 每月固定金额买入，不管价格高低。</p>
<p>优势：</p>
<ul>
<li>消除择时焦虑</li>
<li>平均成本 < 最高价，接近中位价</li>
<li>长期持有 + 分批进场 = 稳健</li>
</ul>
<p>适合：<b>长期看好的资产</b>（如 BTC / ETH 等主流）+ <b>时间周期 1-3 年以上</b>。</p>`,
      strategy: '<p>每月 1 号买入固定金额的 BTC/ETH，不管价格。3 年后回头看会发现成本远低于"情绪化择时"的平均值。</p>',
      tags: ['时间分散', 'DCA', '定投', '策略'],
    },
    {
      id: 'rm-hedge',
      group: 'risk',
      title: '对冲策略（Hedging）',
      badges: [{ text: '进阶', kind: '' }],
      desc: '持有现货多头 + 期货空头 = 对冲下跌风险。保护利润 / 过渡熊市。',
      whatIs: `<p>对冲不是"看空"，是"保险"。例如：</p>
<ul>
<li>你持有 1 BTC 现货（成本 30000，当前 60000，盈利 30000）。</li>
<li>担心短期回调，但不想卖现货（税务 / 长期看好）。</li>
<li>开 1 BTC 期货空头 —— 若跌 20%，空头赚 12000 抵消现货损失。</li>
</ul>
<p>对冲的成本：期货手续费 + 资金费率。长期对冲会侵蚀收益，适合<b>短期避险</b>（1-4 周）。</p>`,
      strategy: `<p>出现 L4 级顶部警报但不想卖现货时，开等值期货空头对冲 2-4 周。L4 警报解除后平掉空头。</p>`,
      tags: ['对冲', 'hedging', '期货', '保护'],
    },
    {
      id: 'sig-rsi',
      group: 'signal',
      title: 'RSI 相对强弱指数（0-100）',
      badges: [{ text: '动能指标', kind: '' }],
      desc: '衡量近期涨跌动能的指标，值 0-100。<b>> 70 超买</b>、<b>< 30 超卖</b>、<b>50 中轴线</b>（牛熊分界）。',
      whatIs: `<p>RSI（Relative Strength Index）由 Welles Wilder 1978 年提出。公式：<b>RSI = 100 - 100 / (1 + RS)</b>，其中 RS = 平均涨幅 / 平均跌幅。</p>
<p>通俗理解：过去 N 根 K 线（默认 14）里，涨了多少、跌了多少。如果全涨了，RSI = 100（极度超买）；全跌了 RSI = 0（极度超卖）。</p>
<p><b>3 个关键阈值</b>：</p>
<ul>
<li><b>50</b>：牛熊分界线。RSI > 50 偏多，< 50 偏空。</li>
<li><b>70 / 30</b>：超买 / 超卖阈值（经典）。</li>
<li><b>80 / 20</b>：极端超买 / 超卖（强势市场可以用）。</li>
</ul>`,
      strategy: `<ul>
<li><b>RSI > 70</b>：短期超买，谨慎追涨。但在强牛市中 RSI 可长期 > 70。</li>
<li><b>RSI < 30</b>：短期超卖，可试探买入。</li>
<li><b>RSI 顶背离 / 底背离</b>：见 sig-divergence 卡片，是 RSI 最高价值用法。</li>
<li><b>RSI 50 穿越</b>：RSI 上穿 50 = 多头确认；下穿 = 空头确认。</li>
</ul>`,
      mistakes: [
        '<b>看到 RSI > 70 就做空</b>：强势牛市中 RSI 可长期保持 80-90。',
      ],
      quotes: [
        { text: 'RSI 是所有技术指标中最重要的一个。', source: 'Wilder 1978' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/indicator/rsi.rs</code>',
        'API': '<span class="kb-chip api">/api/indicators/series?kinds=rsi</span>',
        '默认周期': '14',
      },
      tags: ['RSI', '相对强弱', '超买', '超卖', '14周期'],
    },
    {
      id: 'sig-macd',
      group: 'signal',
      title: 'MACD 异同移动平均线',
      badges: [{ text: '趋势 + 动能', kind: '' }],
      desc: '结合均线交叉与动能的综合指标。由 <b>DIF</b>、<b>DEA</b>、<b>MACD 柱</b> 三部分组成。',
      whatIs: `<p>MACD 的三要素：</p>
<ul>
<li><b>DIF（快线）</b>= EMA12 - EMA26，反映短期与中期均线的差距。</li>
<li><b>DEA（慢线）</b>= DIF 的 9 周期 EMA，对快线进行平滑。</li>
<li><b>MACD 柱</b> = (DIF - DEA) × 2，柱状图展示。</li>
</ul>
<p><b>4 个经典信号</b>：</p>
<ol>
<li><b>金叉</b>：DIF 上穿 DEA = 买入信号。</li>
<li><b>死叉</b>：DIF 下穿 DEA = 卖出信号。</li>
<li><b>0 轴穿越</b>：DIF 上穿 0 轴 = 牛市确认；下穿 = 熊市确认。</li>
<li><b>背离</b>：价格创新高但 MACD 柱未创新高 = 顶背离。</li>
</ol>`,
      strategy: `<ul>
<li><b>金叉 + 在 0 轴上方</b>：强势买点。</li>
<li><b>金叉 + 在 0 轴下方</b>：弱势反弹，短线试探。</li>
<li><b>死叉 + 在 0 轴上方</b>：牛市回调，减仓。</li>
<li><b>死叉 + 在 0 轴下方</b>：熊市加速，加仓做空。</li>
</ul>`,
      quotes: [
        { text: 'MACD 是唯一同时包含趋势和动能的指标。', source: 'Appel 1979' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/indicator/macd.rs</code>',
        'API': '<span class="kb-chip api">/api/indicators/series?kinds=macd</span>',
        '默认参数': '<code>12/26/9</code>',
      },
      tags: ['MACD', 'DIF', 'DEA', '金叉', '死叉', '0轴'],
    },
    {
      id: 'sig-stoch-rsi',
      group: 'signal',
      title: 'Stochastic RSI（随机 RSI）',
      badges: [{ text: '灵敏动能', kind: '' }],
      desc: '对 RSI 再做 Stochastic 计算，比纯 RSI 更灵敏。适合日内短线交易。',
      whatIs: `<p>Stoch RSI = (RSI - min(RSI, N)) / (max(RSI, N) - min(RSI, N))，值域 0-1（或 0-100）。</p>
<p>它把 RSI 的 0-100 范围再拉伸到 0-100（或 0-1），所以同样的 RSI 波动会变得"更夸张"。</p>
<p>优势：<b>更早发现超买超卖</b>。劣势：<b>假信号更多</b>。</p>`,
      strategy: `<p>适合<b>短线 4h 以下</b>使用，配合 RSI 过滤噪音。不建议日线及以上单独使用。</p>`,
      meta: {
        '代码': '<code class="kb-file">src/engine/indicator/stoch_rsi.rs</code>',
        'API': '<span class="kb-chip api">/api/indicators/series?kinds=stoch_rsi</span>',
      },
      tags: ['StochRSI', '随机RSI', '动能', '短线'],
    },
    {
      id: 'sig-atr',
      group: 'signal',
      title: 'ATR 真实波动幅度（Average True Range）',
      badges: [{ text: '波动率', kind: '' }],
      desc: '衡量市场波动大小的指标。ATR 越大 = 波动越剧烈 = 止损需设更远。',
      whatIs: `<p>ATR = 近 N 根 K 线的 True Range 的平均值。True Range = max(high - low, |high - prev_close|, |low - prev_close|)。</p>
<p>简单说：平均每根 K 线的波动幅度是多少。例如 BTC 4h ATR = 500 USDT 意味着每根 4h 平均波动 500 点。</p>
<p>用途：</p>
<ul>
<li><b>止损距离</b>：通常设 <b>2 × ATR</b>（避免正常波动触发止损）。</li>
<li><b>仓位计算</b>：头寸 = 单笔风险 / (ATR × 倍数)。</li>
<li><b>趋势强度</b>：ATR 扩大 = 波动增强；ATR 缩小 = 盘整。</li>
</ul>`,
      params: {
        '默认周期': '<code>14</code>',
        '常用倍数': '<code>1.5 - 3.0</code>',
      },
      strategy: '<p>ATR 的主要用途是<b>动态止损</b>：不是固定点数，而是随波动调整的止损距离。</p>',
      meta: {
        '代码': '<code class="kb-file">src/engine/indicator/atr.rs</code>',
        '应用': '<code>Playbook.stop_loss_atr_mult</code>',
      },
      tags: ['ATR', '波动率', '真实波幅', '止损'],
    },
    {
      id: 'sig-obv',
      group: 'signal',
      title: 'OBV 能量潮（On-Balance Volume）',
      badges: [{ text: '量价结合', kind: '' }],
      desc: '累积成交量指标：上涨日加上当日量，下跌日减去当日量。反映<b>资金流向</b>。',
      whatIs: `<p>OBV 的核心思想：价格涨跌背后是否有<b>真实的资金支持</b>？</p>
<ul>
<li>上涨日（close > prev close）：OBV += volume</li>
<li>下跌日（close < prev close）：OBV -= volume</li>
<li>平盘日：OBV 不变</li>
</ul>
<p>正常情况下，OBV 和价格方向应一致。如果价格涨但 OBV 跌 = <b>OBV 背离</b>，说明上涨缺乏量能支撑，可能假涨。</p>`,
      strategy: `<ul>
<li><b>OBV 创新高</b>：量能支撑足，趋势健康。</li>
<li><b>OBV 背离</b>：警惕趋势反转。</li>
</ul>`,
      meta: {
        '代码': '<code class="kb-file">src/engine/indicator/obv.rs</code>',
      },
      tags: ['OBV', '能量潮', '量价', '背离'],
    },
    {
      id: 'sig-volume-confirm',
      group: 'signal',
      title: '量价配合（Volume-Price Confirmation）',
      badges: [{ text: '基础原理', kind: 'iron' }],
      desc: '<b>"价涨量增 / 价跌量缩"</b> 是健康趋势；<b>"价涨量缩 / 价跌量增"</b> 是反转预警。',
      whatIs: `<p>量价关系是所有技术分析的基石：</p>
<table class="kb-api-table">
<tr><th>价</th><th>量</th><th>含义</th></tr>
<tr><td>↑</td><td>↑</td><td>健康上涨 ✓</td></tr>
<tr><td>↑</td><td>↓</td><td>上涨无力 ⚠️（可能假涨）</td></tr>
<tr><td>↓</td><td>↑</td><td>恐慌抛售 ⚠️（可能见底）</td></tr>
<tr><td>↓</td><td>↓</td><td>盘整缩量 （观望）</td></tr>
</table>
<p>关键判断：<b>突破必放量，否则是假突破</b>。这就是有效突破三要素中的"放量 1.5 倍"。</p>`,
      strategy: `<ul>
<li><b>价涨量增</b>：顺势加仓。</li>
<li><b>价涨量缩</b>：减仓警惕。</li>
<li><b>价跌量增</b>：观察，恐慌抛售后可能是底。</li>
<li><b>价跌量缩</b>：盘整，等待方向。</li>
</ul>`,
      quotes: [
        { text: '量是因，价是果。', source: 'candle Ch1 量价论' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/signal/volume.rs</code>',
      },
      tags: ['量价', 'volume_price', '量能', '背离'],
    },
    {
      id: 'sig-long-term-levels',
      group: 'signal',
      title: '长期压力 / 支撑位（120 / 240 日均线）',
      badges: [{ text: '铁证', kind: 'iron' }, { text: 'R-P1-29', kind: '' }],
      desc: '<b>120 日均线</b>是半年期长期线，<b>240 日均线</b>是年线。两者是最后的牛熊分水岭。',
      whatIs: `<p>如果 60 日均线是"定性线"，那 120 日和 240 日就是"终极底线"：</p>
<ul>
<li><b>120 日</b>：半年时间框。突破意味着重大牛熊转换。</li>
<li><b>240 日</b>：整年时间框。终极压力 / 支撑，突破 = 大周期反转。</li>
</ul>
<p>原书铁证（R-P1-29）：<b>120 日和 240 日被称为"主力防线"</b>。主力在这些位置集中了大量资金，散户突破它们需要巨量资金配合。</p>`,
      strategy: `<ul>
<li><b>价格测试 120 日</b>：第一次测试通常反弹，第二次 / 第三次突破概率增大。</li>
<li><b>价格突破 240 日</b>：L8 级信号，重仓建立。</li>
<li><b>价格跌破 240 日</b>：长期熊市确认，清仓等待。</li>
</ul>`,
      quotes: [
        { text: '120 日是半年的主力防线，240 日是年度终极压力。', source: 'ma p.310 R-P1-29' },
      ],
      meta: {
        '代码': '<code class="kb-file">src/engine/ma/special.rs::LongTermLevel</code>',
        'API': '<span class="kb-chip api">/api/signals</span> long_term_hits',
      },
      tags: ['120日', '240日', '年线', '长期', '主力防线'],
    },
    {
      id: 'sig-bollinger',
      group: 'signal',
      title: '布林带（Bollinger Bands）',
      badges: [{ text: '常用工具', kind: '' }],
      desc: 'MA20 ± 2 倍标准差形成的上下轨 —— 同时表达<b>均值 + 波动率 + 支撑阻力</b>的"三合一"工具。',
      whatIs: `<p>布林带的构成：</p>
<ul>
<li><b>中轨</b> = MA20（20 周期均线）</li>
<li><b>上轨</b> = MA20 + 2 × 20 周期标准差</li>
<li><b>下轨</b> = MA20 - 2 × 20 周期标准差</li>
</ul>
<p>统计上，约 95% 的价格波动会落在上下轨之间。所以：</p>
<ul>
<li><b>价格触及上轨</b> = 2σ 偏离，超买。</li>
<li><b>价格触及下轨</b> = 2σ 偏离，超卖。</li>
<li><b>上下轨收窄（Squeeze）</b> = 波动率降到低位，蓄势突破。</li>
<li><b>上下轨扩张</b> = 波动率增大，趋势加速。</li>
</ul>`,
      strategy: `<ul>
<li><b>布林带 Squeeze</b>：等待突破方向，突破即建仓。</li>
<li><b>价格穿越中轨</b>：趋势方向判断。</li>
<li><b>价格沿上轨走</b>：强势趋势，顺势持有。</li>
</ul>`,
      meta: {
        '代码': '<code class="kb-file">src/engine/indicator/bollinger.rs</code>',
        '默认参数': '20 / 2σ',
      },
      tags: ['布林带', 'bollinger', 'squeeze', '波动率'],
    },
    {
      id: 'sig-kdj',
      group: 'signal',
      title: 'KDJ 随机指标',
      badges: [{ text: '东亚流行', kind: '' }],
      desc: 'K / D / J 三线组合的动能指标。<b>J > 100 超买；J < 0 超卖；KD 金叉/死叉</b> 是经典信号。',
      whatIs: `<p>KDJ 是 Stoch 的扩展版：</p>
<ul>
<li><b>K 值</b>：快速随机值（默认 9 周期）</li>
<li><b>D 值</b>：K 的平滑均线</li>
<li><b>J 值</b>：3K - 2D（加速版，超买超卖更明显）</li>
</ul>
<p>使用要点：</p>
<ul>
<li><b>J > 100</b>：极度超买，价格大概率短期回落。</li>
<li><b>J < 0</b>：极度超卖，反弹概率大。</li>
<li><b>K 上穿 D（金叉）</b>：买入信号，特别是在 20 以下的金叉最可靠。</li>
<li><b>K 下穿 D（死叉）</b>：卖出信号，80 以上的死叉最可靠。</li>
</ul>
<p>注意：KDJ 在强趋势中容易"钝化"（长期留在 80+ 或 20-），需与其他指标组合。</p>`,
      strategy: '<p>与 RSI / MACD 组合使用。单独依赖 KDJ 假信号较多。</p>',
      tags: ['KDJ', '随机指标', '超买', '超卖'],
    },
    {
      id: 'sig-cci',
      group: 'signal',
      title: 'CCI 顺势指标（Commodity Channel Index）',
      badges: [{ text: '动量', kind: '' }],
      desc: '衡量价格偏离统计均值的程度。<b>CCI > +100 = 超买；< -100 = 超卖</b>。擅长识别趋势反转。',
      whatIs: `<p>CCI 由 Donald Lambert 1980 年提出。公式：<b>CCI = (TP - SMA_TP) / (0.015 × MAD)</b>，其中 TP = (高 + 低 + 收) / 3。</p>
<p>值域不限（理论无边界），但常用：</p>
<ul>
<li><b>|CCI| > 200</b>：极端超买 / 超卖</li>
<li><b>|CCI| > 100</b>：趋势信号</li>
<li><b>|CCI| < 100</b>：盘整区间</li>
</ul>`,
      strategy: '<p>适合<b>捕捉趋势反转</b>。CCI 从 +200 跌回 +100 = 短期见顶信号。</p>',
      tags: ['CCI', '顺势指标', '动量'],
    },
    {
      id: 'sig-adx',
      group: 'signal',
      title: 'ADX 趋势强度指数',
      badges: [{ text: '趋势强度', kind: 'iron' }],
      desc: '衡量<b>趋势强度</b>（不分方向）。ADX > 25 = 强趋势；< 20 = 震荡盘整。配合 +DI / -DI 判断方向。',
      whatIs: `<p>Wilder 1978 年的经典指标，解决"什么时候该用趋势策略，什么时候该用震荡策略"的问题：</p>
<ul>
<li><b>ADX</b>：趋势强度，0-100，与方向无关。</li>
<li><b>+DI</b>：上升方向的动能。</li>
<li><b>-DI</b>：下降方向的动能。</li>
</ul>
<p>规则：</p>
<ul>
<li><b>ADX > 25 且 +DI > -DI</b>：上升趋势强。</li>
<li><b>ADX > 25 且 -DI > +DI</b>：下降趋势强。</li>
<li><b>ADX < 20</b>：无趋势，改用震荡策略。</li>
</ul>`,
      strategy: '<p>ADX 是决定"用何种策略"的导航：趋势策略（海龟 / MA 跟踪）vs 震荡策略（箱体 / 布林带均值回归）。</p>',
      tags: ['ADX', '趋势强度', 'DMI', 'Wilder'],
    },
    {
      id: 'sig-ichimoku',
      group: 'signal',
      title: '一目均衡表（Ichimoku Cloud）',
      badges: [{ text: '日式经典', kind: '' }],
      desc: '由 5 条线构成的综合系统：<b>转换线 / 基准线 / 延迟线 / 先行上下带</b>。云图（Kumo）是核心。',
      whatIs: `<p>Goichi Hosoda 1960 年代设计，综合趋势 / 动量 / 支撑阻力于一图。5 条线：</p>
<ul>
<li><b>转换线（Tenkan-sen）</b>：9 周期高低平均</li>
<li><b>基准线（Kijun-sen）</b>：26 周期高低平均</li>
<li><b>延迟线（Chikou Span）</b>：收盘价后移 26 周期</li>
<li><b>先行带 A（Senkou Span A）</b>：(转换线 + 基准线) / 2，前移 26 周期</li>
<li><b>先行带 B（Senkou Span B）</b>：52 周期高低平均，前移 26 周期</li>
</ul>
<p>先行带 A / B 之间形成"云图"（Kumo）。<b>价格在云上 = 牛市；云下 = 熊市；云内 = 不确定</b>。</p>`,
      strategy: `<ul>
<li><b>云图变绿</b>：转多信号。</li>
<li><b>价格突破云上沿</b>：买入。</li>
<li><b>价格跌破云下沿</b>：卖出。</li>
</ul>`,
      tags: ['一目', 'Ichimoku', '云图', 'Kumo', '日式'],
    },
    {
      id: 'sig-dxy',
      group: 'signal',
      title: '美元指数（DXY）与加密',
      badges: [{ text: '宏观', kind: 'iron' }],
      desc: '美元指数与 BTC 长期负相关：<b>DXY 上升 → BTC 下跌；DXY 下跌 → BTC 上涨</b>。是宏观大方向参考。',
      whatIs: `<p>DXY（U.S. Dollar Index）= 美元对 6 种主要货币（欧元 / 日元 / 英镑 / 加元 / 瑞典克朗 / 瑞士法郎）的加权汇率。</p>
<p>与 BTC 的关系（相关系数约 -0.6 到 -0.8）：</p>
<ul>
<li><b>美联储加息</b>：美元走强 → DXY 上涨 → 全球风险资产（含 BTC）下跌。</li>
<li><b>美联储降息 / QE</b>：美元走弱 → DXY 下跌 → 风险资产（含 BTC）上涨。</li>
<li><b>地缘冲突</b>：美元避险 → DXY 上涨 → BTC 短期压力。</li>
</ul>
<p>所以判断 BTC 大方向前，先看 DXY 趋势。</p>`,
      strategy: '<p>DXY 形成顶部 → BTC 看涨；DXY 突破新高 → BTC 谨慎。</p>',
      tags: ['DXY', '美元指数', '宏观', '负相关'],
    },
    {
      id: 'sig-etf-flow',
      group: 'signal',
      title: 'BTC 现货 ETF 资金流',
      badges: [{ text: '机构资金', kind: 'iron' }, { text: '2024+', kind: '' }],
      desc: '2024 年 1 月起 BTC 现货 ETF 正式运行。<b>ETF 每日净流入是机构资金的直接指标</b>。',
      whatIs: `<p>BlackRock IBIT / Fidelity FBTC / Grayscale GBTC 等 ETF 每日披露申购 / 赎回。意义：</p>
<ul>
<li><b>净流入为正</b>：机构在买入，看涨。</li>
<li><b>净流入为负</b>：机构在抛售，看跌。</li>
<li><b>大额流入（单日 > 5 亿美元）</b>：重大看涨信号。</li>
<li><b>大额流出（单日 > 5 亿美元）</b>：警惕，可能预示顶部。</li>
</ul>
<p>观察工具：SoSoValue.com / Farside.co.uk</p>`,
      strategy: '<p>作为宏观判断：连续 5 天流入 > 流出 = 多头加强；反之减仓。</p>',
      example: `<b>BTC 2024/3 ETF 狂潮</b>：单日净流入超 10 亿美元多次，配合减半预期，BTC 从 40000 涨至 73000（+82%）。`,
      tags: ['ETF', '资金流', '机构', 'BlackRock'],
    },
    {
      id: 'sig-sopr',
      group: 'signal',
      title: 'SOPR 已花费产出比率',
      badges: [{ text: '链上', kind: '' }],
      desc: 'Spent Output Profit Ratio —— 衡量<b>平均每笔卖出交易的盈亏</b>。> 1 = 卖出获利；< 1 = 亏损卖出。',
      whatIs: `<p>SOPR = 卖出时价值 / 买入时价值。</p>
<ul>
<li><b>SOPR > 1</b>：平均卖出有盈利，市场健康。</li>
<li><b>SOPR = 1</b>：盈亏平衡。</li>
<li><b>SOPR < 1</b>：平均卖出亏损，恐慌情绪。</li>
</ul>
<p>底部特征：SOPR 长期 < 1 + 开始反弹到 1。</p>
<p>顶部特征：SOPR > 1.1 且下降（虽有盈利但开始减少）。</p>`,
      strategy: '<p>SOPR 低位反弹是抄底信号；SOPR 高位滞涨是减仓信号。</p>',
      tags: ['SOPR', '链上', '盈亏', '情绪'],
    },

    // ==================== 交易心理 ====================
    {
      id: 'psych-greed-fear',
      group: 'psych',
      title: '贪婪与恐惧（Greed & Fear）',
      badges: [{ text: '两大敌人', kind: 'iron' }],
      desc: '交易中 90% 的亏损源于贪婪（追涨 / 不止盈）和恐惧（割肉 / 不敢买）。<b>反人性</b>才是交易盈利的核心。',
      whatIs: `<p>市场是人性的放大镜：</p>
<p><b>贪婪</b>：价格涨了还想涨更多，不止盈；看到别人赚钱自己想追涨；亏损时不愿止损想"解套"。贪婪让你追在顶部，等在底部。</p>
<p><b>恐惧</b>：价格跌了害怕再跌就割肉；底部不敢买；好机会不敢下手。恐惧让你卖在底部，错过起点。</p>
<p><b>大师的反人性</b>：巴菲特说"别人贪婪时我恐惧，别人恐惧时我贪婪"。这就是为什么 <b>L4 超买时要减仓，L8 超卖时要加仓</b> —— AURA 的信号体系就是帮你<b>机械化执行反人性操作</b>。</p>`,
      howTo: [
        '<b>识别自己的情绪</b>：下单前问自己"我是在恐惧还是贪婪？"',
        '<b>遵守规则</b>：用固定的信号体系触发交易，不凭感觉。',
        '<b>远离盘面</b>：情绪化操作最容易发生在频繁盯盘时。',
      ],
      strategy: `<p><b>机械化 + 分级</b>：L1 观察、L4 减仓、L8 全仓。让规则代替情绪。</p>`,
      quotes: [
        { text: '别人贪婪时我恐惧，别人恐惧时我贪婪。', source: 'Warren Buffett' },
        { text: '市场由两种情绪驱动：贪婪与恐惧。', source: 'Jesse Livermore' },
      ],
      tags: ['贪婪', '恐惧', 'greed', 'fear', '心理'],
    },
    {
      id: 'psych-loss-aversion',
      group: 'psych',
      title: '损失厌恶（Loss Aversion）',
      badges: [{ text: '行为金融', kind: 'warn' }],
      desc: '心理学铁证：<b>失去 100 元的痛苦 ≈ 得到 200 元的快乐</b>。这解释了为什么新手"亏损不止损、盈利早止盈"。',
      whatIs: `<p>Kahneman 诺贝尔奖研究：人对亏损的痛苦感是盈利愉悦感的 2 倍。这导致交易者：</p>
<ul>
<li><b>盈利时急于兑现</b>：怕"煮熟的鸭子飞了"，早早止盈。结果错过主升浪。</li>
<li><b>亏损时不愿止损</b>：割肉的痛太大，宁可"拿住"。结果小亏变大亏。</li>
</ul>
<p>这完全违反了 <b>R:R 2:1</b> 原则。正确做法是 <b>"让盈利奔跑，让亏损截断"</b>（Cut your losses, let your winners run）。</p>`,
      strategy: `<ul>
<li><b>预设止损位</b>：入场前设好，触发自动执行，不给自己"考虑"的机会。</li>
<li><b>移动止盈</b>：用 Trailing Stop（跟踪止损）让盈利自动奔跑。</li>
<li><b>分批止盈</b>：30% 到第一目标、70% 让它跑，平衡心理与收益。</li>
</ul>`,
      mistakes: [
        '<b>"回本就走"</b>：这是损失厌恶的典型。结果往往是永远回不了本。',
        '<b>"我就知道会跌"</b>：亏损时的心理防御，不是真实判断。',
      ],
      quotes: [
        { text: '损失的痛苦是盈利快乐的 2 倍。', source: 'Kahneman & Tversky 1979' },
      ],
      tags: ['损失厌恶', 'loss_aversion', 'Kahneman', '行为金融'],
    },
    {
      id: 'psych-anchoring',
      group: 'psych',
      title: '锚定效应（Anchoring）',
      badges: [{ text: '认知偏见', kind: 'warn' }],
      desc: '不自觉地把某个"历史价格"当作判断基准 —— 例如"我是 60000 买的 BTC，跌到 50000 感觉很便宜"。<b>这种锚定让你做出错误决策</b>。',
      whatIs: `<p>人脑天生需要"参考点"。交易中最常见的锚点：</p>
<ul>
<li><b>买入成本</b>："我 60000 买的，现在 50000 太便宜了"——实际上市场不关心你的成本。</li>
<li><b>历史高点</b>："BTC 最高 69000 过，50000 肯定还会回去"——历史不保证未来。</li>
<li><b>别人的观点</b>："某某大 V 说会涨到 100000"——这只是别人的锚。</li>
</ul>
<p>正确的判断应基于 <b>当前的技术面和基本面</b>，而非历史价格锚。</p>`,
      strategy: '<p>每次入场重新评估：<b>基于当前数据，此刻合适吗？</b> 忽略你的买入成本。</p>',
      mistakes: [
        '<b>"成本锚定不肯卖"</b>：永远在亏损中等回本。',
        '<b>"历史高点锚定"</b>：坚信"会创新高"，错过反转信号。',
      ],
      quotes: [
        { text: '市场只看现在，不看你的买入成本。', source: '交易心理学常识' },
      ],
      tags: ['锚定', 'anchoring', '认知偏见', '心理'],
    },
    {
      id: 'psych-overconfidence',
      group: 'psych',
      title: '过度自信（Overconfidence）',
      badges: [{ text: '认知偏见', kind: 'warn' }],
      desc: '连赢几次后觉得自己"懂市场"、加大仓位、放松止损 —— <b>这是爆仓的前奏</b>。',
      whatIs: `<p>过度自信的典型表现：</p>
<ul>
<li>连续 5 次盈利后，觉得自己"手感好"，满仓操作。</li>
<li>判断对一次大行情后，把运气当能力。</li>
<li>忽视风险管理规则（"这次稳赢"）。</li>
<li>不再做回测，凭感觉下单。</li>
</ul>
<p>心理学研究：交易者的实际胜率通常比他们自认的低 10-20%。专业交易员都有"胜后更谨慎"的习惯。</p>`,
      strategy: `<ul>
<li><b>胜后降仓</b>：连赢 3 次后主动减仓一半，等回归正常节奏。</li>
<li><b>写交易日志</b>：把判断理由写下来，事后对照自己到底懂不懂。</li>
<li><b>每周复盘</b>：检查这一周的判断是否真的基于规则。</li>
</ul>`,
      quotes: [
        { text: '一次大胜后，最大的敌人是你自己。', source: 'Jesse Livermore' },
      ],
      tags: ['过度自信', 'overconfidence', '心理'],
    },
    {
      id: 'psych-fomo',
      group: 'psych',
      title: 'FOMO（错失恐惧）',
      badges: [{ text: '最危险情绪', kind: 'bear' }],
      desc: 'Fear Of Missing Out —— 看到别人赚钱或某币暴涨，害怕错过而<b>追高买入</b>。FOMO 是新手最大的陷阱。',
      whatIs: `<p>加密市场 FOMO 尤其严重：</p>
<ul>
<li>某币 1 天涨 50%，害怕错过"下一个 SHIB"，追高买入。</li>
<li>社交媒体看到别人 10 倍收益截图，激动地全仓追。</li>
<li>某利好消息发布，秒级追买。</li>
</ul>
<p>事实：FOMO 让你<b>总是买在最高点</b>。市场"人多的地方别去"—— 当所有人都在追涨，说明这波上涨接近尾声。</p>`,
      strategy: `<ul>
<li><b>不追没有计划的交易</b>：所有交易前先有信号依据。</li>
<li><b>错过就错过</b>：永远有下一个机会，不要为错过一次 pump 焦虑。</li>
<li><b>用限价单代替市价单</b>：挂在回调位等买入，避免追高。</li>
</ul>`,
      mistakes: [
        '<b>"再不上车就永远上不去了"</b>：这是 FOMO 的典型自我欺骗。',
      ],
      quotes: [
        { text: 'FOMO 让你永远买在顶部。', source: '加密交易常识' },
      ],
      tags: ['FOMO', '错失恐惧', '追高', '心理'],
    },
    {
      id: 'psych-sunk-cost',
      group: 'psych',
      title: '沉没成本谬误（Sunk Cost Fallacy）',
      badges: [{ text: '认知偏见', kind: 'warn' }],
      desc: '因为"已经投入了时间 / 金钱"而继续持有明显错误的交易 —— <b>"都亏这么多了，再等等总会回来"</b>。',
      whatIs: `<p>人性让我们无法接受"沉没成本"。典型表现：</p>
<ul>
<li>一个交易已经亏 30%，所有技术面都转空，但"都亏这么多了，卖了就是真亏"。</li>
<li>研究了 3 天的币种结果技术面变差了，但"研究了这么久不买可惜"。</li>
<li>某策略已连亏 10 次，但"都用了这么久了，再试试"。</li>
</ul>
<p>理性的视角：<b>已发生的亏损与未来决策无关</b>。只问"如果我现在没持仓，我会买吗？" 答案不是"会" → 立即平仓。</p>`,
      strategy: '<p>定期问自己：<b>若我此刻重新开始，这个持仓还值得吗？</b> 答否 → 立即平仓。</p>',
      quotes: [
        { text: '沉没成本不是你的成本，是你的枷锁。', source: '行为经济学' },
      ],
      tags: ['沉没成本', 'sunk_cost', '谬误', '心理'],
    },
    {
      id: 'psych-journal',
      group: 'psych',
      title: '交易日志与复盘（Trading Journal）',
      badges: [{ text: '必备习惯', kind: 'iron' }],
      desc: '记录每笔交易的 <b>时间 / 价格 / 理由 / 情绪 / 结果 / 复盘</b> —— 这是从新手进阶为职业交易员的唯一道路。',
      whatIs: `<p>不写日志的交易员 = 不总结的学生。你会反复犯同样的错。</p>
<p>日志应包含：</p>
<ul>
<li><b>入场时间 + 价格</b></li>
<li><b>信号理由</b>（如：4h 断头铡刀 + 周线空头排列）</li>
<li><b>当时情绪</b>（恐惧 / 贪婪 / 中性）</li>
<li><b>预期</b>（目标位 / 止损位 / 持有周期）</li>
<li><b>实际结果</b>（盈亏 / 持有时间 / 偏离预期多少）</li>
<li><b>复盘反思</b>：判断对了吗？执行对了吗？下次改进什么？</li>
</ul>
<p>每周 / 每月统计：胜率、平均 R:R、最大回撤、连胜 / 连亏次数。数据驱动的反馈循环。</p>`,
      strategy: `<p>AURA 的 <b>订阅提醒功能</b> 自动记录触发历史（含时间/价格），可视为简化版交易日志。完整日志建议用 Notion / Excel 单独记录。</p>`,
      quotes: [
        { text: '不写日志的交易员，亏损是必然的。', source: 'Mark Douglas 《Trading in the Zone》' },
      ],
      tags: ['日志', 'journal', '复盘', '习惯'],
    },
    {
      id: 'psych-stop-habit',
      group: 'psych',
      title: '止损纪律（Stop-Loss Discipline）',
      badges: [{ text: '生存本能', kind: 'iron' }],
      desc: '99% 的失败交易员都有同一个共性：<b>不止损</b>。职业交易员的区别：止损是本能，不需要思考。',
      whatIs: `<p>止损难在哪里？</p>
<ul>
<li><b>心理难</b>：承认自己判断错了。</li>
<li><b>情绪难</b>：亏损的痛苦是双倍的（损失厌恶）。</li>
<li><b>希望难</b>：总想"可能会反弹"。</li>
</ul>
<p>解决方法：<b>入场前设置止损 + 自动化执行</b>。不依赖临场判断。</p>
<p>职业交易员的心态："止损是<b>交易的成本</b>，如同租金、水电。不是失败，是<b>必然发生的生意成本</b>。" 接受这个心态，止损就不再痛苦。</p>`,
      strategy: `<ul>
<li><b>挂止损单</b>：入场同时挂好，不给自己改变的机会。</li>
<li><b>技术位止损</b>：止损应在技术位外（如 MA20 下方 1 ATR），不是固定百分比。</li>
<li><b>绝不加码</b>：亏损时加码是赌博，不是交易。</li>
</ul>`,
      mistakes: [
        '<b>看到止损被扫愤怒</b>：把止损当"失败"，而不是"成本"。心态不对。',
        '<b>止损后立即反向</b>：被扫止损的一瞬间冲动反手，通常再被扫一次。',
      ],
      quotes: [
        { text: '止损是交易的租金，不是失败。', source: 'Mark Douglas' },
      ],
      tags: ['止损', 'stop_loss', '纪律', '心理'],
    },

    // ==================== 实战策略 ====================
    {
      id: 'prac-pullback-ma',
      group: 'practice',
      title: '回踩 MA20 买入（Pullback to MA20）',
      badges: [{ text: '最经典策略', kind: 'bull' }, { text: '胜率高', kind: 'iron' }],
      desc: '牛市中价格回调至 MA20 附近支撑后反弹 —— 是<b>最稳定可复制</b>的买入机会。',
      whatIs: `<p>这是葛南维 L1 / L2 的实战版。在明确的上升趋势中，价格不会一路直冲，而是"涨一段 → 回调到 MA20 → 再涨一段"。回调到 MA20 就是<b>机械化买点</b>。</p>`,
      howTo: [
        '<b>前提检查</b>：日线 MA60 向上 + 价格 > MA60 + 多头排列。',
        '<b>等待回调</b>：价格回到 MA20 ±1% 范围内。',
        '<b>确认反弹</b>：触碰 MA20 后出现实体向上的阳线（或小时级别阳线）。',
        '<b>放量配合</b>：反弹那根量能 ≥ 平均量。',
        '<b>入场</b>：阳线收盘买入。',
      ],
      params: {
        '均线': '<code>MA20</code>（节奏线）',
        '触碰范围': '<code>±1%</code>',
        '止损': 'MA20 下方 <code>1.5 × ATR</code>',
        '止盈': 'R:R = 2:1 或前高',
      },
      strategy: `<ul>
<li><b>仓位</b>：50-70%（牛市加仓机会）。</li>
<li><b>止损</b>：MA20 下方 1.5 × ATR。若 MA20 本身跌破不算 MA20 回踩。</li>
<li><b>止盈</b>：前期 Swing High 或 Fib 1.618 扩展位。</li>
</ul>`,
      example: `<b>BTC 2024/11 回踩案例</b>：BTC @ 68000 上涨后回调至 MA20 @ 64500，出现阳线反弹 + 放量。买入后 3 周涨至 73000（+13%）。止损 62500（-3%），R:R 3.3。`,
      quotes: [
        { text: '趋势中的回调是主力给你上车的机会。', source: 'trend Ch4' },
      ],
      tags: ['回踩MA20', 'pullback', '买入', '策略', 'L1'],
    },
    {
      id: 'prac-breakout-neckline',
      group: 'practice',
      title: '突破颈线 + 回抽确认（Breakout + Retest）',
      badges: [{ text: '头肩底 / 双底适用', kind: 'bull' }],
      desc: '头肩底 / 双底 / 矩形等反转形态完成后，价格突破颈线并<b>回抽验证</b> —— 回抽不破是最佳买点。',
      whatIs: `<p>反转形态完成 → 突破颈线 → 回抽颈线 —— 这个"突破 + 回抽"模式比直接追突破更安全：</p>
<ul>
<li><b>突破确认</b>：颈线已被有效突破（3% + 放量）。</li>
<li><b>回抽验证</b>：价格回来测试颈线，证明"原阻力变支撑"成立。</li>
<li><b>反弹确认</b>：从颈线位反弹，第二次起点更明确。</li>
</ul>`,
      howTo: [
        '<b>识别形态</b>：头肩底 / 双底 / 矩形 + 颈线可见。',
        '<b>突破判断</b>：价格突破颈线 ≥ 3% + 放量。',
        '<b>等待回抽</b>：突破后 3-10 根内价格回到颈线附近。',
        '<b>验证不破</b>：回抽低点 > 颈线价格（可允许短暂下影穿越）。',
        '<b>入场</b>：反弹阳线收盘买入。',
      ],
      strategy: `<ul>
<li><b>仓位</b>：70-100%（L6-L8 级信号）。</li>
<li><b>止损</b>：颈线下方 3%（跌破即形态失败）。</li>
<li><b>止盈</b>：量度目标位 = 颈线 + 形态高度。</li>
</ul>`,
      example: `<b>BTC 2023/10 双底突破案例</b>：双底颈线 26500，突破至 28000（+5.7%），回抽至 26800 未破，反弹至 32000（+19%）。`,
      quotes: [
        { text: '回抽不破是形态完成的金标准。', source: 'candle Ch6' },
      ],
      tags: ['突破颈线', 'retest', '回抽', '策略'],
    },
    {
      id: 'prac-bottom-divergence',
      group: 'practice',
      title: '底背离建仓（Bullish Divergence Entry）',
      badges: [{ text: '反转信号', kind: 'bull' }],
      desc: '下跌末期价格创新低但 RSI / MACD 不创新低 —— 底背离是<b>抄底的最早信号</b>。',
      whatIs: `<p>底背离意味着：价格虽在跌但下跌动能已衰竭。是反转的"前兆"而非"确认"。</p>
<p>完整信号链：<b>底背离 → K 线见底形态（锤头/早晨之星）→ 突破下降趋势线 → 建仓确认</b>。4 步都到位才是顶级买点。</p>`,
      howTo: [
        '<b>识别底背离</b>：价格最近 2 个低点 A < B（A 更低）；RSI 对应的低点 a > b（a 更高）。',
        '<b>等 K 线确认</b>：背离后出现锤头 / 早晨之星 / 看涨吞没。',
        '<b>突破确认</b>：价格突破最近下降趋势线或 MA20。',
        '<b>入场</b>：突破根阳线收盘买入。',
      ],
      params: {
        '指标': 'RSI / MACD / OBV 任一',
        '低点间隔': '≥ 5 根 K 线',
        '背离确认': 'K 线见底形态',
      },
      strategy: `<p>分 3 批建仓：背离出现买 30%；K 线确认买 30%；突破趋势线买 40%。风险可控。</p>`,
      mistakes: [
        '<b>只看背离就买</b>：背离可延续很久，必须等 K 线确认。',
      ],
      example: `<b>ETH 2022/11 底背离抄底</b>：ETH 11 月创新低 1070，但 RSI 14 从 9 月的 20 抬到 28（底背离）。随后 12 月出现锤头 + 早晨之星确认，分批买入后涨至 2100（+96%）。`,
      tags: ['底背离', '建仓', '抄底', 'divergence', 'RSI'],
    },
    {
      id: 'prac-dca-bottom',
      group: 'practice',
      title: '分批抄底（Bottom DCA）',
      badges: [{ text: '稳健策略', kind: 'bull' }],
      desc: '在熊市末期将预算分 3-5 次，按<b>BIAS 阈值</b>或<b>固定百分比</b>分批买入 —— 不追求抄在最低点，追求"够低就好"。',
      whatIs: `<p>市场没人能精准抄底。分批抄底的思路：</p>
<ul>
<li><b>BIAS 分批</b>：BIAS 达到 -8% 买 1/3；-12% 再买 1/3；-20% 最后 1/3。</li>
<li><b>百分比分批</b>：价格下跌 15% 买 1/3；再跌 15% 再买 1/3；再跌 15% 最后买。</li>
<li><b>时间分批</b>：熊市末期每周固定买入 1/12，一年完成建仓。</li>
</ul>
<p>好处：无论最低点在哪儿，你的平均成本都接近最低。坏处：如果是 V 底则只买到 1/3。</p>`,
      strategy: `<ul>
<li><b>总预算</b>：账户 20-30%（留部分现金应对黑天鹅）。</li>
<li><b>每批间隔</b>：BIAS 每 4% 或价格每 15% 加一批。</li>
<li><b>止损线</b>：所有批次的平均止损 = 前期重要低点下方 3%。</li>
</ul>`,
      example: `<b>BTC 2022/6-11 分批抄底案例</b>：在 30000 / 20000 / 16000 各买 1/3，平均成本 22000。后续 2023 年涨至 44000，账户翻倍。`,
      tags: ['分批抄底', 'DCA', '熊市', '建仓'],
    },
    {
      id: 'prac-trailing-stop',
      group: 'practice',
      title: '移动止损（Trailing Stop）',
      badges: [{ text: '让盈利奔跑', kind: 'iron' }],
      desc: '持仓盈利后，止损<b>跟随价格向上移动</b>（做多）—— 锁定部分利润，同时不限制上涨空间。',
      whatIs: `<p>普通止损是固定价格，移动止损是"跟随"价格：</p>
<ul>
<li>初始：入场 60000，止损 58000（-3.3%）。</li>
<li>价格涨到 63000，止损上移到 61000（跟着涨 3000）。</li>
<li>价格涨到 68000，止损上移到 66000。</li>
<li>价格回落触发 66000 止损 → 退出，实际盈利 10%（而不是 -3.3%）。</li>
</ul>
<p>优势：<b>让盈利奔跑的同时锁定利润</b>。是趋势跟踪策略的核心技术。</p>`,
      howTo: [
        '<b>初始止损</b>：入场价下方 2 × ATR 或 MA20 下方。',
        '<b>移动规则</b>：每当价格创新高，止损上移到 "新高 - (初始止损距离)"。',
        '<b>或用 MA 移动</b>：止损始终 = MA20（向上跟随）。',
      ],
      params: {
        '移动间距': '<code>2 × ATR</code> 或 <code>MA20</code>',
      },
      strategy: '<p>适合大趋势行情。震荡市不适合（容易被扫然后错过延续）。</p>',
      tags: ['移动止损', 'trailing_stop', '跟踪止损', '策略'],
    },
    {
      id: 'prac-scaled-profit',
      group: 'practice',
      title: '分级止盈（Scaled Take-Profit）',
      badges: [{ text: '心理友好', kind: 'bull' }],
      desc: '将持仓分 2-3 批止盈：<b>20% 到第 1 目标、40% 到第 2 目标、40% 让它跑</b> —— 兼顾心理与收益。',
      whatIs: `<p>单次全仓止盈 vs 分级止盈：</p>
<table class="kb-api-table">
<tr><th>方式</th><th>优点</th><th>缺点</th></tr>
<tr><td>单次全止盈</td><td>简单</td><td>卖早了踏空、卖晚了回吐</td></tr>
<tr><td>分级止盈</td><td>心理压力小、兼顾奔跑</td><td>操作复杂</td></tr>
</table>
<p>分级止盈的心理优势：第一批止盈后已锁定部分利润，剩余仓位的心态会更放松，不会因小回调恐慌平仓。</p>`,
      strategy: `<p><b>推荐配比 20-40-40</b>：</p>
<ul>
<li>第 1 目标（R:R 1）：止盈 20%。</li>
<li>第 2 目标（R:R 2）：止盈 40%。</li>
<li>剩余 40%：开启移动止损，让它奔跑。</li>
</ul>`,
      tags: ['分级止盈', 'scaled_profit', 'take_profit', '策略'],
    },
    {
      id: 'prac-pyramid',
      group: 'practice',
      title: '金字塔加仓（Pyramid Adding）',
      badges: [{ text: '趋势加速', kind: 'bull' }],
      desc: '顺趋势盈利时<b>递减式加仓</b>：第 1 次加仓 50%、第 2 次 30%、第 3 次 20% —— 控制风险同时放大收益。',
      whatIs: `<p>金字塔加仓的原则：<b>越涨加得越少</b>。这样即便最后一次加仓在高点被套，整体成本仍然可控。</p>
<p>相反的"倒金字塔加仓"（越涨加越多）是大忌 —— 那是散户的典型错误。</p>`,
      howTo: [
        '<b>初始仓位 50%</b>：主力入场。',
        '<b>价格上涨 5% + 新信号</b>：加仓 30%。',
        '<b>价格再上涨 5% + 新信号</b>：加仓 20%。',
        '<b>移动止损</b>：每次加仓同时调整止损到新的技术位。',
      ],
      params: {
        '加仓比例': '<code>50% / 30% / 20%</code>',
        '触发条件': '每次 +5% + 新信号',
      },
      mistakes: [
        '<b>倒金字塔（越涨加越多）</b>：大忌。成本中心高，一回调就大亏。',
      ],
      tags: ['金字塔', 'pyramid', '加仓', '策略'],
    },
    {
      id: 'prac-top-down',
      group: 'practice',
      title: '自上而下分析（Top-Down Analysis）',
      badges: [{ text: '多时间框', kind: 'iron' }],
      desc: '从大时间框到小时间框依次分析：<b>月 → 周 → 日 → 4h → 1h</b>。确保操作方向与大趋势一致。',
      whatIs: `<p>新手常见错误是"从 1h 图就下单"—— 完全忽略大趋势。正确做法：</p>
<ol>
<li><b>月线</b>：判断大周期（牛市 / 熊市 / 震荡）。</li>
<li><b>周线</b>：判断中期阶段（建仓 / 拉升 / 出货 / 打压）。</li>
<li><b>日线</b>：决定是否进场 + 大方向。</li>
<li><b>4h</b>：精确入场区间。</li>
<li><b>1h / 15m</b>：精确入场时刻。</li>
</ol>
<p>这是职业交易员的标准流程，比纯短线准确度提升 50%+。</p>`,
      strategy: '<p><b>任何时间框的决策都不能违反更大时间框的方向</b>。1h 看涨但日线看跌 → 只做小仓短线。</p>',
      tags: ['自上而下', 'top_down', '多时间框', '流程'],
    },
    {
      id: 'prac-box-trade',
      group: 'practice',
      title: '箱体交易（Range Trading）',
      badges: [{ text: '震荡市适用', kind: '' }],
      desc: '横盘市场的策略：<b>上沿卖、下沿买</b>，反复高抛低吸 —— 比单边趋势难做但机会多。',
      whatIs: `<p>矩形 / 箱体是主力"吸筹"或"出货"期间的常见形态。既然主力在高抛低吸，跟着做同样可行：</p>
<ul>
<li>下沿附近有支撑 → 买入，止损设下沿下方 3%。</li>
<li>上沿附近有阻力 → 卖出（或做空），止损设上沿上方 3%。</li>
<li>突破方向 → 按矩形高度计算目标位。</li>
</ul>`,
      howTo: [
        '<b>确认箱体</b>：上下沿各至少 2 次触碰。',
        '<b>箱体高度 / 当前价 ≥ 3%</b>：否则操作空间太小。',
        '<b>不猜方向</b>：只做区间内高抛低吸。',
      ],
      strategy: '<p>仓位 30-50%（震荡市风险大）。严格止损，亏损超 3% 退出。</p>',
      mistakes: [
        '<b>把短期回调当箱体</b>：必须横盘 ≥ 10 根且有明确上下沿。',
      ],
      tags: ['箱体交易', 'range_trade', '震荡', '策略'],
    },
    {
      id: 'prac-turtle',
      group: 'practice',
      title: '海龟交易法（Turtle System 简介）',
      badges: [{ text: '经典系统', kind: 'iron' }],
      desc: '1983 年 Richard Dennis 的传奇系统：<b>20 日突破买、10 日反向破卖</b> + ATR 仓位管理。纯机械化、无主观判断。',
      whatIs: `<p>海龟系统的核心规则：</p>
<ol>
<li><b>入场</b>：价格突破过去 20 日最高价 → 买入（多头）；突破 20 日最低价 → 卖出（空头）。</li>
<li><b>止损</b>：入场价 ± 2 × ATR(20)。</li>
<li><b>加仓</b>：每涨 0.5 × ATR 加仓 1 单位（最多 4 单位）。</li>
<li><b>出场</b>：价格跌破过去 10 日最低价 → 平仓（多头）；或反向。</li>
</ol>
<p>海龟系统适合<b>强趋势</b>市场。震荡市可能连续亏损。Dennis 用这套系统把一群新手（海龟）训练成千万富翁。</p>`,
      strategy: `<p>海龟思路可简化：<b>20 日新高买、10 日新低卖</b>。配合 ATR 止损。是"跟随趋势"最简单的体系。</p>`,
      quotes: [
        { text: '海龟系统证明：纪律比聪明更重要。', source: 'Richard Dennis' },
      ],
      tags: ['海龟', 'turtle', 'trend_follow', '经典系统'],
    },
    {
      id: 'prac-rsi-macd-combo',
      group: 'practice',
      title: 'RSI + MACD 双指标组合',
      badges: [{ text: '经典组合', kind: '' }],
      desc: '结合 RSI（动能）+ MACD（趋势）双重确认 —— 相比单指标假信号减少 60%+。',
      whatIs: `<p>为什么要组合指标？因为每个指标都有盲区：</p>
<ul>
<li><b>RSI 在强趋势中失灵</b>：RSI > 70 可长期持续。</li>
<li><b>MACD 反应慢</b>：低周期时滞后严重。</li>
<li><b>两者组合</b>：互补短板，确认度提升。</li>
</ul>
<p>经典规则：<b>RSI > 50 + MACD 金叉 + MACD 柱翻红 = L5 级买入</b>。三条件同时满足，胜率 65%+。</p>`,
      howTo: [
        '<b>RSI 过滤方向</b>：RSI > 50 = 偏多，< 50 = 偏空。',
        '<b>MACD 确认</b>：金叉/死叉 + 0 轴穿越。',
        '<b>柱状验证</b>：MACD 柱从红转绿（多转空）或反之。',
        '<b>三条件一致</b>：确认才下单。',
      ],
      strategy: `<ul>
<li><b>买入</b>：RSI > 50 + MACD 金叉 + MACD 柱翻红。</li>
<li><b>卖出</b>：RSI < 50 + MACD 死叉 + MACD 柱翻绿。</li>
<li><b>仓位</b>：L5 级（50-70%）。</li>
</ul>`,
      tags: ['RSI', 'MACD', '组合', '策略'],
    },
    {
      id: 'prac-bollinger-squeeze',
      group: 'practice',
      title: '布林带 Squeeze 突破策略',
      badges: [{ text: '蓄势突破', kind: '' }],
      desc: '布林带收窄到历史低位（Squeeze）后，<b>突破方向 = 新趋势方向</b>。机械化操作，胜率约 60%。',
      whatIs: `<p>Squeeze（挤压）是布林带特有的形态：</p>
<ul>
<li>上轨和下轨之间的距离（bandwidth）收窄到近 50 根的最低 20%</li>
<li>表明波动率降到低位，市场蓄势待发</li>
<li>突破方向 = 未来 1-4 周的主要方向</li>
</ul>
<p>类似"压缩的弹簧"，能量积累越多，释放越猛。</p>`,
      howTo: [
        '<b>识别 Squeeze</b>：bandwidth / MA20 < 2%，持续 ≥ 5 根 K 线。',
        '<b>等待突破</b>：价格突破上轨或下轨 + 放量（≥ 1.5× 均量）。',
        '<b>入场</b>：突破当根收盘买/卖。',
        '<b>目标</b>：移动止损或到达上一 Swing。',
      ],
      strategy: '<p>仓位 50-70%。止损设 Squeeze 中轴（MA20）反向 1.5 × ATR。</p>',
      example: `<b>ETH 2023/10 Squeeze 突破</b>：ETH 在 1550-1650 横盘 3 周（bandwidth 5.7% 历史低位），10/26 突破 1700 + 量 2.3×。后续涨至 2200（+30%）。`,
      tags: ['Squeeze', '布林带', '突破', '蓄势'],
    },
    {
      id: 'prac-fomc-scalp',
      group: 'practice',
      title: '宏观事件时段策略（FOMC / CPI）',
      badges: [{ text: '事件驱动', kind: 'warn' }],
      desc: '加密市场与美联储政策高度相关。<b>FOMC 会议 / CPI 数据发布前 2 小时空仓</b>，避免剧烈波动。',
      whatIs: `<p>重要宏观事件（每月或每季度 1 次）：</p>
<ul>
<li><b>FOMC 会议</b>（美联储利率决议）：每 6 周一次，北京时间凌晨 2:00。</li>
<li><b>CPI 数据</b>（通胀）：每月 13-15 日，北京时间晚 20:30。</li>
<li><b>非农数据</b>：每月第一个周五，北京时间晚 20:30。</li>
<li><b>Jackson Hole 会议</b>：每年 8 月。</li>
</ul>
<p>这些事件发布时，加密市场往往在 30 分钟内波动 5-10%，方向难料。<b>最好的策略是空仓观望</b>。</p>`,
      strategy: `<ul>
<li><b>事件前 2 小时</b>：清仓或大幅减仓。</li>
<li><b>事件发布</b>：不交易，观察市场反应。</li>
<li><b>事件后 1-2 小时</b>：等新趋势明确再入场。</li>
</ul>`,
      tags: ['FOMC', 'CPI', '宏观', '事件驱动', '加息'],
    },
    {
      id: 'prac-weekly-monthly',
      group: 'practice',
      title: '周末 / 月末特殊策略',
      badges: [{ text: '时间规律', kind: '' }],
      desc: '加密市场 <b>周末流动性低</b>，容易剧烈波动。<b>月末主力调仓</b>，容易砸盘或拉升。有规律可循。',
      whatIs: `<p>加密市场时间规律：</p>
<ul>
<li><b>周末</b>（六日）：流动性仅工作日的 50-70%，波动放大。小资金容易被砸盘或拉升。</li>
<li><b>月末最后 2-3 日</b>：基金调仓、期货交割，行情波动大。</li>
<li><b>美股开盘前 30 分钟</b>（北京 21:00）：BTC 经常被动跟随美股。</li>
<li><b>亚洲盘（北京 9-11 点）</b>：相对安静。</li>
</ul>`,
      strategy: `<ul>
<li><b>周末</b>：减仓至 50%，不做高杠杆。</li>
<li><b>月末</b>：等待调仓完成再判断方向。</li>
<li><b>重要事件前</b>：观望不操作。</li>
</ul>`,
      tags: ['周末', '月末', '时间', '规律', '特殊'],
    },
    {
      id: 'prac-mm-accumulation',
      group: 'practice',
      title: '识别主力吸筹的 5 个特征',
      badges: [{ text: '主力行为', kind: 'iron' }],
      desc: '主力吸筹期的典型特征：<b>(1) 价格横盘；(2) 量能萎缩；(3) 小阴小阳；(4) 关键支撑不破；(5) 短暂假跌破</b>。',
      whatIs: `<p>主力吸筹需要不惊动市场 —— 他们会表现得"市场很烂"：</p>
<ol>
<li><b>价格横盘</b>：低位窄幅波动（±3-5%），不创新低也不创新高。</li>
<li><b>量能持续萎缩</b>：日均量低于前期 50%，显示无人交易（实际是散户退出）。</li>
<li><b>小阴小阳交替</b>：无大长阳 / 大阴线。</li>
<li><b>关键支撑不破</b>：价格测试低位多次，但总有神秘买盘托住。</li>
<li><b>偶有假跌破（Spring）</b>：最后的吸筹，触发散户止损。</li>
</ol>
<p>这 5 条同时满足 → <b>吸筹确认</b>。突破上沿即拉升开始。</p>`,
      strategy: '<p>吸筹期分批建仓（每周买 10-20%），上沿突破时加到满仓。</p>',
      example: '<b>BTC 2023 上半年</b>：全部 5 个吸筹特征都满足，从 16500 积累到 30000 （+80%）。耐心持有者大幅受益。',
      tags: ['吸筹', '主力', 'accumulation', '识别'],
    },
    {
      id: 'prac-mm-distribution',
      group: 'practice',
      title: '识别主力出货的 5 个特征',
      badges: [{ text: '主力行为', kind: 'iron' }, { text: '顶部警报', kind: 'bear' }],
      desc: '主力出货期的特征：<b>(1) 高位横盘；(2) 量能维持高位但价格滞涨；(3) 上影频出；(4) 突破不持续；(5) UpThrust 假突破</b>。',
      whatIs: `<p>主力出货需要散户接盘，会制造"还要涨"的假象：</p>
<ol>
<li><b>高位横盘</b>：价格在高位窄幅波动，常有微创新高。</li>
<li><b>量能维持高位但滞涨</b>：散户追涨量能大，但价格不涨 → 有人在抛。</li>
<li><b>上影线频繁</b>：冲高被压制，收盘跌回。</li>
<li><b>突破不持续</b>：每次突破 1-2 根就跌回。</li>
<li><b>UpThrust 假突破</b>：最后一次"创新高"后迅速跌回。</li>
</ol>`,
      strategy: '<p>出货特征确认 → 分批减仓。UpThrust 出现 → 立即清仓。</p>',
      example: '<b>BTC 2021 Q4</b>：全部 5 个特征，最终从 69000 跌至 28000（-59%）。识别者提前离场。',
      tags: ['出货', '主力', 'distribution', '识别'],
    },
    {
      id: 'prac-vwap',
      group: 'practice',
      title: 'VWAP 成交量加权均价策略',
      badges: [{ text: '机构常用', kind: '' }],
      desc: 'VWAP（Volume Weighted Average Price）= 当日加权平均价。是机构交易员的<b>标准基准线</b>。',
      whatIs: `<p>VWAP 公式：(∑ 价格 × 成交量) / ∑ 成交量。当日从开盘开始累积计算。</p>
<p>机构用 VWAP 衡量自己的交易质量：<b>买入价 < VWAP = 好单；卖出价 > VWAP = 好单</b>。</p>
<p>对散户而言 VWAP 是一条动态支撑 / 阻力：</p>
<ul>
<li>价格 > VWAP 且向上 = 多头强势</li>
<li>价格 < VWAP 且向下 = 空头强势</li>
<li>回踩 VWAP 反弹 = 买点（L1 短线版）</li>
</ul>`,
      strategy: '<p>日内交易者用 VWAP 作为短线支撑阻力，与 MA20 组合使用。</p>',
      tags: ['VWAP', '加权均价', '机构', '日内'],
    },
    {
      id: 'prac-order-book',
      group: 'practice',
      title: '盘口深度（Order Book）分析',
      badges: [{ text: '微观结构', kind: '' }],
      desc: '看<b>买卖盘挂单</b>判断支撑阻力：厚的买单墙 = 支撑；厚的卖单墙 = 阻力。大单撤销 = 主力操纵警告。',
      whatIs: `<p>盘口深度显示当前所有未成交订单。观察技巧：</p>
<ul>
<li><b>买单墙</b>：某价位挂单巨大（如 100 BTC），形成支撑。</li>
<li><b>卖单墙</b>：同理，高位挂单 = 阻力。</li>
<li><b>撤单行为</b>：大单突然消失 = 主力"钓鱼"（伪装支撑然后撤单砸盘）。</li>
<li><b>成交量突增</b>：主力主动吸货或抛售的信号。</li>
</ul>
<p>但加密市场盘口常被<b>冰山单 / 刷单机器人</b>干扰，不能单独依赖。</p>`,
      strategy: '<p>作为辅助参考，不单独使用。结合 K 线和成交量综合判断。</p>',
      mistakes: [
        '<b>相信明显的买/卖墙</b>：往往是主力伪装，真要成交时会撤单。',
      ],
      tags: ['盘口', 'order_book', '挂单', '深度'],
    },
    {
      id: 'prac-funding-rate',
      group: 'practice',
      title: '资金费率（Funding Rate）',
      badges: [{ text: '加密特有', kind: 'iron' }],
      desc: '永续合约的资金费率反映多空力量：<b>正费率 = 多方拥挤</b>（风险），<b>负费率 = 空方拥挤</b>（反转机会）。',
      whatIs: `<p>永续合约每 8 小时收取资金费率，平衡合约价与现货价：</p>
<ul>
<li><b>正费率（常见 0.01-0.1%/8h）</b>：多方付钱给空方。说明多头更拥挤。</li>
<li><b>负费率</b>：空方付钱给多方。说明空头更拥挤。</li>
</ul>
<p>极端费率（> 0.1%/8h 或 < -0.05%/8h）= 市场情绪过于一致 = <b>反向操作的机会</b>。</p>`,
      strategy: `<ul>
<li><b>费率 > 0.1%/8h（多头极度拥挤）</b>：顶部概率高，考虑减仓或做空。</li>
<li><b>费率 < -0.05%/8h（空头极度拥挤）</b>：底部概率高，考虑抄底。</li>
<li><b>持续正费率</b>：避免多头持仓（付费给空方）。</li>
</ul>`,
      example: `<b>BTC 2024/3 费率警报</b>：BTC @ 73000，费率达 0.15%/8h（年化 164%）。随后 3 周回调至 61000（-16%），多头拥挤被清算。`,
      tags: ['资金费率', 'funding_rate', '永续合约', '情绪'],
    },
    {
      id: 'prac-halving-cycle',
      group: 'practice',
      title: 'BTC 减半周期理论',
      badges: [{ text: '加密特有', kind: 'iron' }, { text: '4 年周期', kind: '' }],
      desc: 'BTC 每 4 年减半一次（区块奖励砍半），历史上<b>减半后 12-18 月出现牛市顶点</b>。是加密市场最大的宏观周期。',
      whatIs: `<p>BTC 减半是协议规定的通胀递减机制：</p>
<ul>
<li><b>2012/11/28 第 1 次减半</b>（12.5 → 25 BTC/块，笔误：50→25）：减半后价格从 12 涨到 1100（2013 峰值）。</li>
<li><b>2016/7/9 第 2 次减半</b>（25 → 12.5）：减半后涨到 20000（2017 峰值）。</li>
<li><b>2020/5/11 第 3 次减半</b>（12.5 → 6.25）：减半后涨到 69000（2021 峰值）。</li>
<li><b>2024/4/19 第 4 次减半</b>（6.25 → 3.125）：峰值预计 2025-2026 年。</li>
</ul>
<p><b>减半效应</b>：供给减半 + 需求不变 → 价格倾向上涨。但这种效应需要 12-18 个月兑现。</p>`,
      strategy: `<ul>
<li><b>减半前 6-12 月</b>：积累期，分批买入。</li>
<li><b>减半后 12-18 月</b>：拉升期，满仓持有。</li>
<li><b>峰值后 6-12 月</b>：熊市，清仓或定投反向。</li>
</ul>`,
      mistakes: [
        '<b>认为减半当天必涨</b>：事件价格往往已定价在内，短期未必涨。',
        '<b>这次不一样</b>：每轮减半都有人这么说，结果周期规律依然。',
      ],
      tags: ['减半', 'halving', 'BTC', '4年周期', '加密'],
    },
    {
      id: 'prac-fear-greed',
      group: 'practice',
      title: '恐慌与贪婪指数（Fear & Greed Index）',
      badges: [{ text: '情绪指标', kind: '' }],
      desc: '0-100 综合情绪指标：<b>极度恐慌（0-25）= 抄底机会；极度贪婪（75-100）= 顶部警告</b>。',
      whatIs: `<p>加密恐慌贪婪指数（alternative.me 发布）综合 6 个因子：</p>
<ul>
<li>波动率（25%）</li>
<li>动能 / 交易量（25%）</li>
<li>社交媒体情绪（15%）</li>
<li>市场主导率（10%）</li>
<li>Google 趋势（10%）</li>
<li>问卷调查（15%）</li>
</ul>
<p>值的含义：</p>
<ul>
<li><b>0-25 极度恐慌</b>：往往是底部。2020/3 黑天鹅期间指数长时间 < 10。</li>
<li><b>26-45 恐慌</b>：逢低买入机会。</li>
<li><b>46-55 中性</b>：观望。</li>
<li><b>56-74 贪婪</b>：谨慎。</li>
<li><b>75-100 极度贪婪</b>：顶部警告。2021/5 指数长期 > 80。</li>
</ul>`,
      strategy: '<p>指数 < 25 时分批买入；> 80 时分批卖出。与 L4 / L8 信号组合使用。</p>',
      tags: ['恐慌贪婪指数', 'fear_greed', '情绪', '市场温度'],
    },
    {
      id: 'prac-btc-dominance',
      group: 'practice',
      title: 'BTC 市值占比（BTC Dominance）',
      badges: [{ text: '加密特有', kind: '' }],
      desc: 'BTC 市值 / 总加密市值 的百分比。<b>占比上升 = 资金流向 BTC（避险）；下降 = 资金流向山寨（Alt Season）</b>。',
      whatIs: `<p>BTC Dominance（简称 BTC.D）是加密市场独有的指标：</p>
<ul>
<li><b>BTC.D > 60%</b>：BTC 主导，山寨表现差。常见于熊市或避险期。</li>
<li><b>BTC.D 50-60%</b>：BTC 稳居主导，牛市中期。</li>
<li><b>BTC.D < 50%</b>：山寨强势（Alt Season），山寨币暴涨。常见于牛市末期。</li>
</ul>
<p>规律：牛市初期 BTC 先涨（资金进场），然后 ETH 跟涨，最后散户追山寨 → BTC.D 从高位下降，标志牛市进入末期。</p>`,
      strategy: `<ul>
<li><b>BTC.D 上升</b>：BTC 优先持有，减少山寨仓位。</li>
<li><b>BTC.D 跌破 50%</b>：可以增加部分山寨仓位。</li>
<li><b>BTC.D 急跌</b>：通常牛市末期，警惕顶部。</li>
</ul>`,
      tags: ['BTC.D', 'dominance', '市值占比', 'Alt Season'],
    },
    {
      id: 'prac-stablecoin-flow',
      group: 'practice',
      title: '稳定币流入 / 流出（Stablecoin Flow）',
      badges: [{ text: '链上数据', kind: '' }],
      desc: '交易所稳定币余额变化：<b>流入 = 准备买入 = 看涨；流出 = 提现 / 离场 = 看跌</b>。',
      whatIs: `<p>稳定币（USDT / USDC）是加密市场的"现金"。它们的流动反映实际资金动向：</p>
<ul>
<li><b>交易所稳定币余额上升</b>：有人把稳定币存入交易所，准备买币。看涨信号。</li>
<li><b>交易所稳定币余额下降</b>：有人把稳定币提走，减少交易意愿。看跌信号。</li>
</ul>
<p>观察工具：CryptoQuant、Glassnode 等链上数据平台。</p>
<p>历史案例：2020/10 交易所 USDT 余额从 50 亿涨到 100 亿（翻倍），预示牛市启动。</p>`,
      strategy: '<p>作为宏观信号使用，与技术面共振时增强置信度。</p>',
      tags: ['稳定币', 'stablecoin', 'USDT', '链上'],
    },
    {
      id: 'prac-onchain-btc',
      group: 'practice',
      title: 'BTC 链上关键指标（On-Chain Metrics）',
      badges: [{ text: '链上数据', kind: 'iron' }],
      desc: '几个关键链上指标：<b>MVRV / NUPL / 长期持有者余额 / 交易所流出</b>。是宏观顶部 / 底部的参考。',
      whatIs: `<p>重要链上指标：</p>
<ul>
<li><b>MVRV（Market Value / Realized Value）</b>：市值 / 已实现市值。> 3.5 = 顶部警告；< 1 = 底部区域。</li>
<li><b>NUPL（Net Unrealized Profit/Loss）</b>：未实现盈亏率。> 0.7 = 贪婪；< 0 = 恐慌底部。</li>
<li><b>长期持有者（LTH）供给</b>：长期持有 > 155 天的 BTC 数量。上升 = 筹码锁定 = 看涨。</li>
<li><b>交易所 BTC 余额</b>：下降 = 提到冷钱包 = 看涨。2020 以来持续下降。</li>
<li><b>矿工抛压</b>：矿工卖出量。减半后 12 个月内矿工抛压最大。</li>
</ul>`,
      strategy: '<p>多个链上指标同时到极端（MVRV < 1 + NUPL < 0）= 极强抄底信号。</p>',
      tags: ['链上', 'on_chain', 'MVRV', 'NUPL', 'glassnode'],
    },
    {
      id: 'prac-trade-flow',
      group: 'practice',
      title: '完整交易流程（Pre → Entry → Hold → Exit → Post）',
      badges: [{ text: '流程', kind: 'iron' }],
      desc: '职业交易员的 5 步流程：<b>Pre（分析）→ Entry（入场）→ Hold（持仓）→ Exit（出场）→ Post（复盘）</b>。每步都有纪律。',
      whatIs: `<p>业余交易员往往只关注"入场"，职业交易员重视整个流程：</p>
<ol>
<li><b>Pre（分析）</b>：多时间框扫描 + 确认信号 + 计算风险回报比。不达标则跳过。</li>
<li><b>Entry（入场）</b>：确认信号 + 设置止损止盈（挂单自动化）+ 记录交易理由。</li>
<li><b>Hold（持仓）</b>：不频繁查看价格，遵守既定止损止盈。出现反向信号才调整。</li>
<li><b>Exit（出场）</b>：止损 / 止盈 / 新反向信号触发。不"再等等"。</li>
<li><b>Post（复盘）</b>：记录结果。判断对错？执行对错？下次改进？</li>
</ol>`,
      strategy: '<p>每笔交易都走完整 5 步。跳过任何一步都是漏洞。</p>',
      mistakes: [
        '<b>只有 Entry 没有 Pre</b>：没有计划的交易 = 赌博。',
        '<b>频繁 Hold 时操作</b>：情绪化调整止损止盈是常见死因。',
        '<b>不 Post</b>：不复盘就无法进步。',
      ],
      tags: ['交易流程', 'pre_trade', '流程', '纪律'],
    },
    {
      id: 'prac-target-calc',
      group: 'practice',
      title: '目标位计算（3 种方法）',
      badges: [{ text: '量度法则', kind: '' }],
      desc: '估算止盈目标的 3 种方法：<b>(1) 形态高度 + 突破点；(2) Fib 扩展 1.618；(3) 前期 Swing 高/低</b>。',
      whatIs: `<p>好的目标位让你知道"赚够就走"。3 种经典方法：</p>
<ol>
<li><b>形态量度</b>：头肩底 / 双底 / 矩形的目标 = 突破价 + 形态高度。例：颈线 30000，头部 26000，高度 4000 → 目标 34000。</li>
<li><b>Fib 扩展</b>：从 Swing Low 到 Swing High 测量，1.618 扩展位 = 目标。例：从 30000 涨到 40000 的幅度 10000，1.618 × 10000 = 16180 → 目标 46180。</li>
<li><b>前期 Swing</b>：上一次的高点 / 低点往往成为下次的阻力 / 支撑。</li>
</ol>
<p>通常取 <b>3 种方法中最保守的值</b>作为第一目标位。</p>`,
      strategy: '<p>设置两个目标：T1 = 保守（40% 仓位止盈）；T2 = Fib 1.618（40% 仓位止盈）；剩余 20% 移动止损。</p>',
      tags: ['目标位', '止盈', '量度', 'Fib扩展'],
    },
    {
      id: 'prac-news-reaction',
      group: 'practice',
      title: '消息面反应（News Reaction）',
      badges: [{ text: '事件驱动', kind: '' }],
      desc: '利好消息发布后价格反应模式：<b>利好反涨 = 健康；利好反跌 = 顶部警报；利空反涨 = 底部信号</b>。',
      whatIs: `<p>市场对消息的反应比消息本身更重要：</p>
<ul>
<li><b>利好 + 上涨</b>：市场积极，多头健康。</li>
<li><b>利好 + 下跌</b>：利好已定价在内，顶部警告。参考 2017/12/17 芝加哥期货上市 BTC，当时视为大利好，但也是牛市顶部。</li>
<li><b>利空 + 下跌</b>：恐慌，可能延续。</li>
<li><b>利空 + 上涨</b>：市场消化完毕，底部信号。参考 2022/11/11 FTX 爆雷当日 BTC 暴跌 25%，但 2 天后企稳。</li>
</ul>`,
      strategy: '<p>关注<b>反应</b>而非消息本身。"利好不涨"和"利空不跌"是最强的反向信号。</p>',
      tags: ['消息面', 'news', '反应', '情绪'],
    },
    {
      id: 'prac-multi-asset',
      group: 'practice',
      title: '多币联动分析',
      badges: [{ text: '组合视角', kind: '' }],
      desc: '同时看 BTC / ETH / SOL / 大型山寨 的表现，判断<b>资金流向</b>。同涨同跌 = 系统性行情；分化 = 选筹机会。',
      whatIs: `<p>加密市场同步性高，但细微差异能提供信号：</p>
<ul>
<li><b>BTC 涨但 ETH 不跟涨</b>：ETH 弱势，短线避开。</li>
<li><b>BTC 盘整，ETH 先拉升</b>：ETH 领涨，山寨季启动信号。</li>
<li><b>BTC 跌但 SOL 抗跌</b>：SOL 有独立动能，可能是领涨候选。</li>
<li><b>全线暴跌</b>：系统性事件，清仓避险。</li>
</ul>`,
      strategy: '<p>每日早盘扫描 BTC / ETH / 前 10 市值山寨。找出"与大盘偏离"的品种，作为选筹候选。</p>',
      tags: ['多币联动', '组合', '资金流向'],
    },
    {
      id: 'prac-dex-cex',
      group: 'practice',
      title: 'CEX vs DEX 价差套利',
      badges: [{ text: '套利', kind: '' }],
      desc: '中心化交易所 vs 去中心化交易所间的短暂价差可用于<b>无风险套利</b>（但需考虑 gas 成本和滑点）。',
      whatIs: `<p>同一代币在不同交易所可能有短暂价差：</p>
<ul>
<li><b>CEX（如 Binance / OKX）</b>：高流动性，价格收敛快。</li>
<li><b>DEX（如 Uniswap / Raydium）</b>：流动性池，价格受滑点影响大。</li>
<li><b>价差套利</b>：DEX 买便宜 → 转到 CEX 卖贵，或反之。</li>
</ul>
<p>注意：</p>
<ul>
<li>Gas 成本（ETH 主网可能 20-100 刀）</li>
<li>转账时间（链上确认 15 秒-10 分钟）</li>
<li>滑点（大额交易会恶化价格）</li>
</ul>
<p>实际操作需价差 > 2-3% 才有利可图。</p>`,
      strategy: '<p>专业套利者用机器人监控 10+ 交易所，小额高频。散户很难做到。</p>',
      tags: ['套利', 'arbitrage', 'CEX', 'DEX'],
    },

    // ==================== API 索引 ====================
    {
      id: 'api-list',
      group: 'api',
      title: '全部 HTTP API 端点',
      badges: [{ text: '24 个', kind: '' }],
      desc: '以下是所有已注册的 HTTP GET 端点。在浏览器地址栏直接访问可看到 JSON 响应（添加 <code>?symbol=BTCUSDT&interval=4h&limit=500</code>）。',
      meta: {
        'WebSocket': '<span class="kb-chip api">/ws</span> 实时 K 线推送',
      },
      extra: 'api-table',
      tags: ['API', '端点', 'endpoint'],
    },
  ];

  // ---------- 渲染 ----------
  function renderBlock(cls, emoji, title, body) {
    if (!body) return '';
    // body 支持字符串（原样）或数组（自动 <ol>）
    let bodyHtml;
    if (Array.isArray(body)) {
      bodyHtml = `<ol>${body.map((x) => `<li>${x}</li>`).join('')}</ol>`;
    } else {
      bodyHtml = body;
    }
    return `
      <div class="kb-section-block ${cls}">
        <div class="kb-sb-title"><span class="kb-sb-emoji">${emoji}</span>${escHtml(title)}</div>
        <div class="kb-sb-body">${bodyHtml}</div>
      </div>
    `;
  }

  function renderParams(params) {
    if (!params) return '';
    const entries = Object.entries(params);
    if (!entries.length) return '';
    return `
      <dl class="kb-params">
        ${entries.map(([k, v]) => `<dt>${escHtml(k)}</dt><dd>${v}</dd>`).join('')}
      </dl>
    `;
  }

  function renderCard(sec) {
    const group = GROUPS.find((g) => g.key === sec.group);
    const emoji = group?.emoji || '';
    const badgesHtml = (sec.badges || []).map((b) =>
      `<span class="kb-card-badge ${b.kind || ''}">${escHtml(b.text)}</span>`
    ).join('');
    const quotesHtml = (sec.quotes || []).map((q) =>
      `<div class="kb-quote">${q.text}<span class="kb-quote-source">${escHtml(q.source || '')}</span></div>`
    ).join('');
    const metaHtml = Object.entries(sec.meta || {}).map(([k, v]) =>
      `<div class="kb-meta-key">${escHtml(k)}</div><div class="kb-meta-val">${v}</div>`
    ).join('');
    let extraHtml = '';
    if (sec.extra === 'api-table') {
      extraHtml = renderApiTable();
    }

    // 教程级字段
    const whatIsHtml = sec.whatIs ? renderBlock('', '📖', '小白解读', sec.whatIs) : '';
    const howToHtml = sec.howTo ? renderBlock('howto', '👁️', '怎么识别（分步）', sec.howTo) : '';
    const strategyHtml = sec.strategy ? renderBlock('strategy', '✅', '交易策略', sec.strategy) : '';
    const mistakesHtml = sec.mistakes ? renderBlock('mistakes', '⚠️', '常见误判 / 易错点', sec.mistakes) : '';
    const exampleHtml = sec.example ? renderBlock('example', '💡', '举例说明', sec.example) : '';
    const diagramHtml = sec.diagram ? `<div class="kb-diagram">${sec.diagram}</div>` : '';
    const paramsHtml = sec.params ? renderParams(sec.params) : '';

    return `
      <section class="kb-card" id="${sec.id}">
        <div class="kb-card-head">
          <span>${emoji}</span>
          <h3 class="kb-card-title">${escHtml(sec.title)}</h3>
          ${badgesHtml}
        </div>
        <div class="kb-card-desc">${sec.desc}</div>
        ${whatIsHtml}
        ${diagramHtml}
        ${howToHtml}
        ${paramsHtml}
        ${strategyHtml}
        ${mistakesHtml}
        ${exampleHtml}
        ${quotesHtml}
        ${metaHtml ? `<div class="kb-meta">${metaHtml}</div>` : ''}
        ${extraHtml}
      </section>
    `;
  }

  function renderGroup(group) {
    const secsInGroup = SECTIONS.filter((s) => s.group === group.key);
    if (!secsInGroup.length) return '';
    return `
      <h2 class="kb-group-title" data-group="${group.key}">
        <span class="kg-emoji">${group.emoji}</span> ${escHtml(group.title)}
      </h2>
      <p class="kb-group-intro">${escHtml(group.intro)}</p>
      ${secsInGroup.map(renderCard).join('')}
    `;
  }

  function renderAll() {
    const container = $('kb-body');
    if (!container) return;
    container.innerHTML = GROUPS.map(renderGroup).join('');
    const stat = $('kb-stat-sections');
    if (stat) stat.textContent = String(SECTIONS.length);
    renderNav();
  }

  // 动态生成左侧导航（覆盖原 HTML 硬编码）
  function renderNav() {
    const nav = document.getElementById('kb-nav');
    if (!nav) return;
    nav.innerHTML = GROUPS.map((g) => {
      const secsInGroup = SECTIONS.filter((s) => s.group === g.key);
      if (!secsInGroup.length) return '';
      const links = secsInGroup.map((s) =>
        `<li><a href="#${s.id}" data-sec title="${escHtml(s.desc || '').replace(/<[^>]+>/g, '').slice(0, 80)}">${escHtml(s.title)}</a></li>`
      ).join('');
      return `
        <div class="kb-nav-group">
          <h3>${g.emoji} ${escHtml(g.title)} <span class="kb-nav-count">${secsInGroup.length}</span></h3>
          <ul>${links}</ul>
        </div>
      `;
    }).join('');
  }

  function renderApiTable() {
    const apis = [
      { path: '/api/ping', desc: '健康检查' },
      { path: '/api/version', desc: '版本信息（name/version/phase）' },
      { path: '/api/symbols', desc: 'Binance 所有 USDT 现货交易对列表（缓存 30 分钟）' },
      { path: '/api/klines', desc: 'K 线数据（从 Binance 拉取，本地缓存）' },
      { path: '/api/ma_state', desc: '完整均线状态（排列 / 交叉 / 粘合 / 葛南维 / 斜率 / BIAS）' },
      { path: '/api/trend_state', desc: '趋势状态（Swing / 趋势线 / SR / Fib / 阶段）' },
      { path: '/api/candle_patterns', desc: '单/双/三 K 线形态识别' },
      { path: '/api/chart_patterns', desc: '图表形态（头肩 / 双顶 / 旗形 / 三角 / 圆底 / 菱形 / 楔形）' },
      { path: '/api/resonance', desc: '四维共振评分 + 交易建议（入场 / 止损 / 止盈 / 建议仓位）' },
      { path: '/api/signals', desc: '高级信号（Confluence / 断头铡刀 / 陷阱 / 潜伏 / 放量 / 反转）' },
      { path: '/api/decision', desc: 'AI 决策聚合（action / confidence / risk_level / 多书源引用）' },
      { path: '/api/indicators/series', desc: 'RSI / MACD / StochRSI / Volume 等技术指标序列' },
      { path: '/api/backtest/run', desc: '回测运行（权益曲线 / 17 项绩效指标）' },
      { path: '/api/backtest/playbook', desc: '按原书铁证策略模板回测（断头铡刀 / 旱地拔葱 等）' },
      { path: '/api/effectiveness', desc: '形态历史胜率统计（打标数据 + 置信区间）' },
      { path: '/api/bandit/state', desc: 'Thompson Bandit 当前 arm 权重' },
      { path: '/api/bandit/train', desc: '在指定 symbol+interval 上增量训练 Bandit' },
      { path: '/api/bandit/reset', desc: '重置 Bandit 到初始状态' },
      { path: '/api/bandit/decide', desc: '给定当前特征，由 Bandit 选出最优 arm' },
      { path: '/api/system/*', desc: '体系实验室全套：components / definitions / evaluate / discovery / seeds' },
      { path: '/api/benchmark/run', desc: '跨 symbol × interval 基准矩阵' },
      { path: '/ws', desc: 'WebSocket 实时 K 线推送（close_time / kline / depth）' },
    ];
    return `
      <table class="kb-api-table">
        <thead>
          <tr><th>端点</th><th>说明</th></tr>
        </thead>
        <tbody>
          ${apis.map((a) => `
            <tr>
              <td>${escHtml(a.path)}</td>
              <td class="desc">${escHtml(a.desc)}</td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    `;
  }

  // ---------- 搜索 ----------
  function escHtml(s) {
    return String(s ?? '').replace(/[&<>"']/g, (c) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    }[c]));
  }

  function doSearch(q) {
    const query = (q || '').trim().toLowerCase();
    const hint = $('kb-search-hint');
    const countEl = $('kb-search-count');
    const navItems = document.querySelectorAll('.kb-nav-group li');
    const cards = document.querySelectorAll('.kb-card');
    const groupTitles = document.querySelectorAll('.kb-group-title');
    const groupIntros = document.querySelectorAll('.kb-group-intro');

    if (!query) {
      // 恢复全部
      navItems.forEach((li) => li.classList.remove('hidden'));
      cards.forEach((c) => {
        c.classList.remove('hidden');
        c.classList.remove('search-hit');
      });
      groupTitles.forEach((g) => g.classList.remove('hidden'));
      groupIntros.forEach((g) => g.classList.remove('hidden'));
      hint.hidden = true;
      clearHighlights();
      return;
    }

    // 匹配规则：标题/简介/引用/标签/meta 值
    const matched = new Set();
    for (const sec of SECTIONS) {
      const hay = [
        sec.title,
        sec.desc,
        ...(sec.quotes || []).map((x) => x.text + ' ' + (x.source || '')),
        ...(sec.tags || []),
        ...Object.values(sec.meta || {}),
      ].join(' ').toLowerCase();
      if (hay.includes(query)) matched.add(sec.id);
    }

    // 应用到 DOM
    cards.forEach((c) => {
      if (matched.has(c.id)) {
        c.classList.remove('hidden');
        c.classList.add('search-hit');
      } else {
        c.classList.add('hidden');
        c.classList.remove('search-hit');
      }
    });
    navItems.forEach((li) => {
      const a = li.querySelector('a[data-sec]');
      if (!a) return;
      const id = a.getAttribute('href').replace('#', '');
      li.classList.toggle('hidden', !matched.has(id));
    });
    // 隐藏无匹配的 group
    GROUPS.forEach((g) => {
      const any = SECTIONS.some((s) => s.group === g.key && matched.has(s.id));
      const title = document.querySelector(`.kb-group-title[data-group="${g.key}"]`);
      const intro = title ? title.nextElementSibling : null;
      if (title) title.classList.toggle('hidden', !any);
      if (intro && intro.classList.contains('kb-group-intro')) intro.classList.toggle('hidden', !any);
    });

    countEl.textContent = String(matched.size);
    hint.hidden = false;
    highlightInDom(query);
  }

  function clearHighlights() {
    document.querySelectorAll('.kb-highlight').forEach((el) => {
      const p = el.parentNode;
      if (!p) return;
      p.replaceChild(document.createTextNode(el.textContent || ''), el);
      p.normalize();
    });
  }

  function highlightInDom(query) {
    clearHighlights();
    if (!query) return;
    const cards = document.querySelectorAll('.kb-card:not(.hidden)');
    const re = new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi');
    cards.forEach((card) => {
      highlightInNode(card, re);
    });
  }

  function highlightInNode(node, re) {
    if (node.nodeType === 3) {
      const text = node.nodeValue || '';
      if (re.test(text)) {
        const span = document.createElement('span');
        span.innerHTML = text.replace(re, '<mark class="kb-highlight">$1</mark>');
        node.parentNode.replaceChild(span, node);
      }
    } else if (node.nodeType === 1 && node.childNodes && !['MARK', 'CODE', 'SCRIPT', 'STYLE'].includes(node.tagName)) {
      const children = Array.from(node.childNodes);
      children.forEach((c) => highlightInNode(c, re));
    }
  }

  // ---------- 滚动激活导航 ----------
  let observer = null;
  function setupScrollSpy() {
    const navLinks = document.querySelectorAll('.kb-nav a[data-sec]');
    const linkById = new Map();
    navLinks.forEach((a) => {
      const id = a.getAttribute('href').replace('#', '');
      linkById.set(id, a);
    });
    observer = new IntersectionObserver((entries) => {
      entries.forEach((e) => {
        if (e.isIntersecting) {
          navLinks.forEach((a) => a.classList.remove('active'));
          const link = linkById.get(e.target.id);
          if (link) link.classList.add('active');
        }
      });
    }, { rootMargin: '-45% 0px -50% 0px', threshold: 0 });
    document.querySelectorAll('.kb-card').forEach((c) => observer.observe(c));
  }

  // ---------- 事件 ----------
  function bindEvents() {
    const searchInput = $('kb-search-input');
    let searchTimer = null;
    searchInput.addEventListener('input', () => {
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => doSearch(searchInput.value), 120);
    });
    $('kb-search-clear').addEventListener('click', () => {
      searchInput.value = '';
      doSearch('');
      searchInput.focus();
    });

    // 点击文件路径 => 复制到剪贴板
    document.addEventListener('click', (ev) => {
      const f = ev.target.closest('code.kb-file');
      if (!f) return;
      const text = f.textContent || '';
      if (navigator.clipboard) {
        navigator.clipboard.writeText(text).then(() => {
          f.style.color = 'var(--bull)';
          const old = f.textContent;
          f.textContent = '已复制 ✓';
          setTimeout(() => { f.textContent = old; f.style.color = ''; }, 900);
        });
      }
    });
  }

  // ---------- 启动 ----------
  window.addEventListener('DOMContentLoaded', () => {
    renderAll();
    bindEvents();
    setupScrollSpy();
    // 若 URL 带锚点，滚动过去
    if (location.hash) {
      setTimeout(() => {
        const el = document.querySelector(location.hash);
        if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }, 150);
    }
  });
})();
