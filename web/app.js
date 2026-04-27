// Aura-Trade 前端主脚本（Phase 1.5）
// - Lightweight Charts 渲染 K线 + 均线 + 成交量 + 形态标注
// - 右侧信号面板：均线状态、葛南维信号、均线交叉、K线形态

(function () {
  'use strict';

  // ---------- 常量 ----------
  // 读取 CSS 变量（主题同步）
  function cssVar(name, fallback) {
    try {
      const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
      return v || fallback;
    } catch (_) { return fallback; }
  }

  // 将十六进制色（#rrggbb / #rgb）转为 rgba(r,g,b,a)
  function hexToRgba(hex, alpha) {
    if (!hex) return `rgba(0,0,0,${alpha})`;
    let h = hex.trim();
    if (h.startsWith('#')) h = h.slice(1);
    if (h.length === 3) h = h.split('').map((c) => c + c).join('');
    if (h.length !== 6) return `rgba(128,128,128,${alpha})`;
    const r = parseInt(h.slice(0, 2), 16);
    const g = parseInt(h.slice(2, 4), 16);
    const b = parseInt(h.slice(4, 6), 16);
    return `rgba(${r},${g},${b},${alpha})`;
  }

  // 默认周期：加入 MA30（新基准），绘图顺序从短到长
  const DEFAULT_MA_PERIODS = [5, 10, 20, 30, 60, 120, 250];
  // 交易员友好色板：避开 bull(绿)/bear(红) 以防和 K 线撞色
  //   - MA30 基准用亮金色突出
  //   - 其余用不同色相的中等饱和色，便于区分
  const MA_COLORS = {
    5:   '#e8a04a',   // 暖金（短期快线）
    10:  '#ec7dbe',   // 粉红
    20:  '#5a8bc2',   // 稳蓝（中期）
    30:  '#d9b94e',   // 柔和亮金 —— 基准 MA30（突出）
    60:  '#a78bfa',   // 淡紫
    90:  '#5dc9d6',   // 青（半木夏常用）
    120: '#b57a4e',   // 褐
    250: '#8a968f',   // 中性灰（长期）
    256: '#8a968f',   // 半木夏常用
  };
  const MA_COLOR_POOL = ['#e8a04a', '#ec7dbe', '#5a8bc2', '#d9b94e', '#a78bfa', '#5dc9d6', '#b57a4e', '#8a968f'];
  function colorForPeriod(p, idx) {
    return MA_COLORS[p] || MA_COLOR_POOL[idx % MA_COLOR_POOL.length];
  }

  // 参数预设：切换下拉时一键应用
  const PRESETS = {
    aura: {
      periods: '5,10,20,30,60,120,250',
      base: '30',
      weights: { ma: '0.3', trend: '0.3', candle: '0.2', chart: '0.2' },
      atrMult: '1.5',
      risk: '2',
      rr: '2',
    },
    // 半木夏（微博 @半木夏btc）公开方法论：
    //   - MA 5/10/30/90/120/256
    //   - 低倍杠杆（2-3×，极端 1×）→ 单笔风险 1%
    //   - 强均线 + 辅助 K 线形态，技术图形相对次要
    banmuxia: {
      periods: '5,10,30,90,120,256',
      base: '30',
      weights: { ma: '0.4', trend: '0.3', candle: '0.2', chart: '0.1' },
      atrMult: '2.0',
      risk: '1',
      rr: '2',
    },
  };
  // 涨跌色：从 CSS 变量读取（支持 light/dark 自动切换）
  const BULL_COLOR = cssVar('--bull', '#3d7f5f');
  const BEAR_COLOR = cssVar('--bear', '#b85c56');
  const WARN_COLOR = cssVar('--warn', '#c99e4e');
  const INFO_COLOR = cssVar('--info', '#5a7fa8');
  const MUTED_COLOR = cssVar('--text-muted', '#9a9486');

  // ============ 形态分类映射（前端硬编码，供子菜单过滤） ============
  // 未列出的 kind 默认归为 'single'
  const PATTERN_CATEGORY = {
    // 单根
    BigBullCandle: 'single', BigBearCandle: 'single',
    Doji: 'single', DragonflyDoji: 'single', GravestoneDoji: 'single',
    FourPriceDoji: 'single', LongLeggedDoji: 'single', SpinningTop: 'single',
    FlatLine: 'single', TShape: 'single', InvTShape: 'single',
    Hammer: 'single', HangingMan: 'single',
    InvertedHammer: 'single', ShootingStar: 'single',
    MarubozuBull: 'single', MarubozuBear: 'single',
    // 双根
    BullishEngulfing: 'double', BearishEngulfing: 'double',
    BullishHarami: 'double', BearishHarami: 'double',
    PiercingLine: 'double', DarkCloudCover: 'double',
    TweezersTop: 'double', TweezersBottom: 'double',
    BullishInsideBar: 'double', BearishInsideBar: 'double',
    BullishOutsideBar: 'double', BearishOutsideBar: 'double',
    // 三根
    MorningStar: 'triple', EveningStar: 'triple',
    MorningDojiStar: 'triple', EveningDojiStar: 'triple',
    ThreeWhiteSoldiers: 'triple', ThreeBlackCrows: 'triple',
    ThreeInsideUp: 'triple', ThreeInsideDown: 'triple',
    ThreeOutsideUp: 'triple', ThreeOutsideDown: 'triple',
    AbandonedBabyBull: 'triple', AbandonedBabyBear: 'triple',
    // 高级（多根组合 / 开盘光头光脚 / 岛形 / 三明治）
    IslandReversalTop: 'advanced', IslandReversalBottom: 'advanced',
    StickSandwichBull: 'advanced', StickSandwichBear: 'advanced',
    OpenMarubozuBull: 'advanced', OpenMarubozuBear: 'advanced',
  };

  // 形态历史表现映射（基于 PATTERN_EFFECTIVENESS_REPORT.md 9 数据集真实评估）
  const RANK_STYLE = {
    '强可用':   { color: BULL_COLOR,  icon: '★★★' },
    '可用':     { color: '#6aa587',   icon: '★★' },
    '一般':     { color: WARN_COLOR,  icon: '★' },
    '无偏':     { color: MUTED_COLOR, icon: '○' },
    '反向失效': { color: BEAR_COLOR,  icon: '✕' },
  };
  const PATTERN_STATS = {
    // === K 线形态 ===
    // 强可用（跨 3 级别或 3/3 全正）
    '看涨反击线': { rank: '强可用', hit: '64-68%', alpha: '+0.76~1.18%', note: '1d/4h 3/3 全正' },
    '看跌反击线': { rank: '强可用', hit: '61%',    alpha: '+2.05%',      note: '日线 3/3 全正' },
    '光脚阴线':   { rank: '强可用', hit: '54-60%', alpha: '+0.64~12.5%', note: '5/5 最稳，周线极强' },
    '塔形顶':     { rank: '强可用', hit: '58.3%',  alpha: '+1.73%',      note: '日线 3/3 正' },
    '光头光脚大阳线': { rank: '可用', hit: '56.1%', alpha: '+1.76%',     note: '日线强' },
    '光头光脚大阴线': { rank: '可用', hit: '53.5%', alpha: '+0.63%',     note: '3/5 正' },
    // 可用
    '对应顶':     { rank: '可用', hit: '56.6%', alpha: '+0.37%', note: '4/5 正' },
    '曙光初现':   { rank: '可用', hit: '54.1%', alpha: '+0.35%', note: '4/5 正' },
    '早晨十字星': { rank: '可用', hit: '53.9%', alpha: '+0.32%', note: '4/5 正' },
    '三内部下跌': { rank: '可用', hit: '54.0%', alpha: '+0.13%', note: '日线 3/3' },
    // 一般
    '射击之星':   { rank: '一般', hit: '54.8%', alpha: '+0.25%', note: '5/5 但幅度小' },
    '倒锤头线':   { rank: '一般', hit: '51.7%', alpha: '+0.22%', note: '3/5' },
    '对应底':     { rank: '一般', hit: '52.9%', alpha: '+0.14%', note: '' },
    '红三兵':     { rank: '一般', hit: '66-76%', alpha: '跨集不稳', note: '周线强但 σ 大' },
    '黑三兵（三只乌鸦）': { rank: '一般', hit: '54.1%', alpha: '+0.59%', note: '日线 3/3' },
    // 无偏（中性或无预测力）
    '十字星':     { rank: '无偏', hit: '≈0', alpha: '0', note: '仅指示波动率' },
    '长十字线':   { rank: '无偏', hit: '≈0', alpha: '0', note: '仅指示波动率' },
    '螺旋桨':     { rank: '无偏', hit: '≈0', alpha: '0', note: '仅指示波动率' },
    '内含线':     { rank: '无偏', hit: '≈0', alpha: '0', note: '仅指示波动率' },
    '外包线':     { rank: '无偏', hit: '≈0', alpha: '0', note: '仅指示波动率' },
    '镊子顶（平顶）': { rank: '无偏', hit: '49.5%', alpha: '≈0',  note: '' },
    '镊子底（平底）': { rank: '无偏', hit: '51.0%', alpha: '≈0',  note: '' },
    // 反向失效（加密市场反例）
    '大阳线':     { rank: '反向失效', hit: '45.9%', alpha: '-1.15%', note: '加密追涨杀跌' },
    '锤头线':     { rank: '反向失效', hit: '47.7%', alpha: '-0.24%', note: '跨级别反向' },
    '吊颈线':     { rank: '反向失效', hit: '47.9%', alpha: '-0.89%', note: '跨级别反向' },
    '早晨之星':   { rank: '反向失效', hit: '46.7%', alpha: '-0.20%', note: '加密反例' },
    '看涨夹心饼': { rank: '反向失效', hit: '47.5%', alpha: '-0.50%', note: '跨级别反向' },
    '看跌夹心饼': { rank: '反向失效', hit: '40.9%', alpha: '-0.65%', note: '跨级别反向' },
    '多方炮':     { rank: '反向失效', hit: '44.3%', alpha: '-0.48%', note: '跨级别反向' },
    '看涨吞没（穿头破脚）': { rank: '反向失效', hit: '49.4%', alpha: '-0.05%', note: '加密反例' },
    '看跌吞没（穿头破脚）': { rank: '反向失效', hit: '48.1%', alpha: '+0.05%', note: '效果弱' },
    '三外部上涨': { rank: '反向失效', hit: '44.7%', alpha: '-0.16%', note: '跨集不稳' },

    // === 技术图形 ===
    '菱形顶':     { rank: '强可用', hit: '85.7%', alpha: '+11.87%', note: '日线最强图形' },
    '菱形底':     { rank: '强可用', hit: '70%',   alpha: '+2.30%',  note: '3/4 正' },
    '上升楔形':   { rank: '可用',   hit: '64.7%', alpha: '+1.35%',  note: '3/5 正' },
    '下降楔形':   { rank: '可用',   hit: '100%',  alpha: '+2.81%',  note: '样本少' },
    '头肩顶':     { rank: '可用',   hit: '80%',   alpha: '+1.70%',  note: '4/4' },
    '头肩底':     { rank: '可用',   hit: '60%',   alpha: '+2.77%',  note: '' },
    '三重顶':     { rank: '可用',   hit: '100%',  alpha: '+2.55%',  note: '样本少' },
    'V 形顶':     { rank: '可用',   hit: '75%',   alpha: '+3.46%',  note: 'P0 修复后' },
    'V 形底':     { rank: '可用',   hit: '75%',   alpha: '+3.46%',  note: 'P0 修复后' },
    '对称三角形': { rank: '无偏',   hit: '≈0',    alpha: '0',       note: '中性' },
    '矩形（箱体）': { rank: '无偏', hit: '≈0',    alpha: '0',       note: '中性' },
    '双顶 M':     { rank: '反向失效', hit: '50%',  alpha: '-0.45%', note: '颈线伪突破' },
    '双底 W':     { rank: '反向失效', hit: '40%',  alpha: '-1.43%', note: 'P0 后仍反向' },
    '上升三角形': { rank: '反向失效', hit: '25%',  alpha: '-3.57%', note: '伪突破' },
    '多头旗形':   { rank: '反向失效', hit: '0-25%', alpha: '-4%',   note: 'P0 严格化后触发减少' },
    '空头旗形':   { rank: '反向失效', hit: '0-25%', alpha: '-4%',   note: 'P0 严格化后触发减少' },
  };

  function statsFor(label) {
    return PATTERN_STATS[label] || null;
  }

  // ---------- 全局状态 ----------
  const state = {
    chart: null,
    candleSeries: null,
    maSeries: {}, // {period: series}
    volumeChart: null,
    volumeSeries: null,
    // 指标副图（MACD / RSI / 成交量）
    indicatorChart: null,
    indicatorSeries: {},      // {key: series} 当前渲染的子系列
    indicatorKind: 'macd',    // 当前展示指标
    indicatorData: null,      // 最近一次 /api/indicators/series 的 data
    // 画线工具
    drawMode: null,           // null | 'hline' | 'trendline' | 'measure'
    drawPending: null,        // 需要两步的工具第一点暂存这里 {time, price}
    drawings: [],             // 已完成的画线：{ id, kind, points:[{time,price}], color, _series?, _priceLine? }
    drawSeq: 0,               // 自增 id
    measureMarkers: [],       // 测量工具的 marker 池 [{ id, marker }]
    // —— 画线升级：预览线 + 端点拖拽 + 颜色切换 ——
    drawColor: null,          // 当前绘制颜色；null → 用默认 DRAW_COLOR
    drawPreview: null,        // 两步工具的橡皮筋预览：{ series, pt1 }
    snapHint: null,           // OHLC 吸附提示 DOM 节点
    drawDrag: null,           // 拖拽中：{ drawingId, pointIdx, origPoint }
    selectedDrawingId: null,  // 当前选中的 drawing id（端点显示圆点）
    // 子菜单过滤状态（默认全开；从 localStorage 恢复）
    patternCats: null,        // { single, double, triple, advanced }
    trendParts: null,         // { trendline, sr, swings }
    currentKlines: [],
    currentMaState: null,
    currentPatterns: [],
    currentTrend: null,
    trendLineSeries: [], // [{series, priceLines:[]}]
    srPriceLines: [], // [primitive]
    // 黄金分割（Fibonacci 回撤）：candleSeries 上的 priceLine 列表
    fibPriceLines: [],
    // 技术图形点击可视化：id -> { series:[LineSeries], priceLines:[PriceLine], li?:HTMLElement }
    chartPatternOverlays: new Map(),
    // --- 实时推送 ---
    ws: null,
    wsSymbol: null,
    wsInterval: null,
    wsRetry: 0,
    wsReconnectTimer: null,
    lastBarOpenTime: 0,
    lastLivePrice: null,
  };

  // ---------- DOM 引用 ----------
  const $ = (id) => document.getElementById(id);

  // ---------- 配置持久化（localStorage） ----------
  // 所有需要保存的 DOM id，按类型区分，以便正确还原与保存
  const CFG_KEYS = {
    // 顶栏
    value: ['symbol', 'interval', 'ma-kind', 'limit', 'pattern-density'],
    checked: ['log-scale', 'show-patterns', 'show-trend', 'show-fib', 'live-stream'],
    // 配置 Tab
    cfg_value: [
      'cfg-w-ma', 'cfg-w-trend', 'cfg-w-candle', 'cfg-w-chart',
      'cfg-equity', 'cfg-max-risk', 'cfg-rr', 'cfg-atr-mult',
      'cfg-periods', 'cfg-base', 'cfg-preset',
    ],
  };
  const CFG_STORAGE_KEY = 'aura_trade_cfg_v1';

  // 解析当前配置的均线周期（从 cfg-periods 输入框），失败回退到默认值
  function currentMaPeriods() {
    const raw = $('cfg-periods')?.value || '';
    const arr = raw.split(',')
      .map((s) => parseInt(s.trim(), 10))
      .filter((n) => Number.isFinite(n) && n >= 2 && n <= 1000);
    // 去重 + 升序
    const dedup = Array.from(new Set(arr)).sort((a, b) => a - b);
    return dedup.length ? dedup : DEFAULT_MA_PERIODS.slice();
  }

  function saveConfig() {
    const data = {};
    for (const id of [...CFG_KEYS.value, ...CFG_KEYS.cfg_value]) {
      const el = $(id);
      if (el) data[id] = el.value;
    }
    for (const id of CFG_KEYS.checked) {
      const el = $(id);
      if (el) data[id] = el.checked;
    }
    try { localStorage.setItem(CFG_STORAGE_KEY, JSON.stringify(data)); } catch (_) {}
  }

  function loadConfig() {
    let data;
    try { data = JSON.parse(localStorage.getItem(CFG_STORAGE_KEY) || '{}'); } catch (_) { data = {}; }
    for (const id of [...CFG_KEYS.value, ...CFG_KEYS.cfg_value]) {
      if (data[id] == null) continue;
      const el = $(id);
      if (!el) continue;
      let v = data[id];
      // 防御：localStorage 里可能残留之前污染的 symbol（如 "BYBIT:BTCUSDTBTC"）
      if (id === 'symbol' && typeof v === 'string') {
        const cleaned = (v || '').trim().toUpperCase().match(/^([A-Z]+:)?([A-Z0-9]+USDT)/);
        if (cleaned) v = (cleaned[1] || '') + cleaned[2];
      }
      // interval 必须非空（hidden select 默认 4h）
      if (id === 'interval' && (!v || !String(v).length)) v = '4h';
      el.value = v;
    }
    for (const id of CFG_KEYS.checked) {
      if (data[id] == null) continue;
      const el = $(id);
      if (el) el.checked = !!data[id];
    }
  }

  function resetConfig() {
    try { localStorage.removeItem(CFG_STORAGE_KEY); } catch (_) {}
    location.reload();
  }

  // ---------- 工具 ----------
  function fetchJson(url, init = {}) {
    const opts = {
      headers: { Accept: 'application/json', ...(init.headers || {}) },
      ...init,
    };
    return fetch(url, opts)
      .then((r) => r.json())
      .then((body) => {
        if (!body.ok) throw new Error(body.error || 'API error');
        return body.data;
      });
  }

  function fmtPrice(v) {
    if (!isFinite(v)) return '—';
    if (v > 1000) return v.toFixed(2);
    if (v > 10) return v.toFixed(3);
    if (v > 0.1) return v.toFixed(4);
    return v.toFixed(6);
  }
  function fmtPct(v) { return isFinite(v) ? (v * 100).toFixed(2) + '%' : '—'; }
  function fmtTs(ms) {
    const d = new Date(ms);
    const pad = (n) => n.toString().padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` +
           `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  // ---------- 图表初始化 ----------
  function initCharts() {
    const chartEl = $('chart');
    const volumeEl = $('volume');
    chartEl.innerHTML = '';
    volumeEl.innerHTML = '';

    // 图表主题色（从 CSS 变量读取，随主题切换）
    const chartBg     = cssVar('--surface', '#ffffff');
    const chartText   = cssVar('--text-dim', '#6b6558');
    const chartGrid   = cssVar('--border', '#e8e6e0');
    const chartBorder = cssVar('--border-strong', '#d4d0c4');

    const common = {
      layout: {
        background: { type: 'solid', color: chartBg },
        textColor: chartText,
        fontSize: 11,
        fontFamily: cssVar('--font-sans', 'system-ui, sans-serif'),
      },
      grid: {
        vertLines: { color: chartGrid },
        horzLines: { color: chartGrid },
      },
      rightPriceScale: {
        borderColor: chartBorder,
        mode: $('log-scale').checked ? 1 : 0,
      },
      timeScale: {
        borderColor: chartBorder,
        timeVisible: true,
        secondsVisible: false,
      },
      crosshair: {
        mode: 0,
        vertLine: { color: cssVar('--text-muted', '#9a9486'), labelBackgroundColor: cssVar('--accent', '#cc785c') },
        horzLine: { color: cssVar('--text-muted', '#9a9486'), labelBackgroundColor: cssVar('--accent', '#cc785c') },
      },
    };

    const chart = LightweightCharts.createChart(chartEl, {
      ...common,
      height: chartEl.clientHeight,
      width: chartEl.clientWidth,
    });
    const candleSeries = chart.addCandlestickSeries({
      upColor: BULL_COLOR,
      downColor: BEAR_COLOR,
      borderUpColor: BULL_COLOR,
      borderDownColor: BEAR_COLOR,
      wickUpColor: BULL_COLOR,
      wickDownColor: BEAR_COLOR,
    });

    // 均线系列由 rebuildMaSeries() 动态创建（因为周期可切换）
    const maSeries = {};

    const volumeChart = LightweightCharts.createChart(volumeEl, {
      ...common,
      height: volumeEl.clientHeight,
      width: volumeEl.clientWidth,
      rightPriceScale: {
        borderColor: chartBorder,
        scaleMargins: { top: 0.1, bottom: 0 },
      },
      timeScale: { visible: false },
    });
    const volumeSeries = volumeChart.addHistogramSeries({
      priceFormat: { type: 'volume' },
      priceScaleId: '',
    });

    // 指标副图（MACD/RSI/成交量）
    const indicatorEl = $('indicator');
    indicatorEl.innerHTML = '';
    const indicatorChart = LightweightCharts.createChart(indicatorEl, {
      ...common,
      height: indicatorEl.clientHeight || 100,
      width: indicatorEl.clientWidth,
      rightPriceScale: {
        borderColor: chartBorder,
        scaleMargins: { top: 0.1, bottom: 0.1 },
      },
      timeScale: { visible: false },
    });

    // 同步时间轴：主图缩放时刷新 volume 和 indicator 的可见范围
    chart.timeScale().subscribeVisibleLogicalRangeChange((r) => {
      if (!r) return;
      volumeChart.timeScale().setVisibleLogicalRange(r);
      indicatorChart.timeScale().setVisibleLogicalRange(r);
      // 画线握点跟随视口变化
      updateAllHandlesDom();
    });

    // 画线工具：监听点击事件（带 Shift 键信息，供 OHLC 吸附使用）
    chart.subscribeClick((param) => {
      param._shiftKey = !!state._lastShiftKey;
      handleChartClick(param);
    });

    // 十字光标信息 + 指标 + 顶部 OHLC + MA overlay + VOL meta 全面联动
    chart.subscribeCrosshairMove((param) => {
      // 画线预览/拖拽/吸附提示：只要 param 存在就尝试更新（不受 K 线可用性影响）
      updateDrawingPreview(param);
      const kl = state.currentKlines;
      if (!param.time || !kl.length) {
        // 鼠标离开图表：恢复到最新 bar 显示
        $('price-info').textContent = '—';
        $('time-info').textContent = '';
        if (kl.length > 0) {
          updateChartTicker(kl[kl.length - 1], kl.length > 1 ? kl[kl.length - 2] : null);
          updateChartMaValuesForBar(null);
          updateVolumeMeta(kl, null);
          updateIndicatorMetaByTime(null);
        }
        return;
      }
      const idx = kl.findIndex((k) => Math.floor(k.open_time / 1000) === param.time);
      if (idx < 0) return;
      const kd = kl[idx];
      const prev = idx > 0 ? kl[idx - 1] : null;
      $('price-info').textContent =
        `O ${fmtPrice(kd.open)}  H ${fmtPrice(kd.high)}  L ${fmtPrice(kd.low)}  C ${fmtPrice(kd.close)}  V ${fmtPrice(kd.volume)}`;
      $('time-info').textContent = fmtTs(kd.open_time);
      // 顶部信息条跟随鼠标 bar
      updateChartTicker(kd, prev);
      // MA overlay 显示此 bar 的均线值
      updateChartMaValuesForBar(idx);
      // 成交量 meta 条跟随鼠标 bar（当根量 + 滚动 MA20 + 量比）
      updateVolumeMeta(kl, idx);
      // 副图指标（MACD / RSI / StochRSI / 成交量）右上读数联动
      updateIndicatorMetaByTime(param.time);
    });

    // 自适应窗口
    window.addEventListener('resize', () => {
      chart.applyOptions({ width: chartEl.clientWidth, height: chartEl.clientHeight });
      volumeChart.applyOptions({ width: volumeEl.clientWidth, height: volumeEl.clientHeight });
      indicatorChart.applyOptions({ width: indicatorEl.clientWidth, height: indicatorEl.clientHeight });
      updateAllHandlesDom();
    });
    // 页面滚动时也刷新握点（chart 在页面中可能有偏移）
    window.addEventListener('scroll', () => updateAllHandlesDom(), { passive: true });

    state.chart = chart;
    state.candleSeries = candleSeries;
    state.maSeries = maSeries;
    state.volumeChart = volumeChart;
    state.volumeSeries = volumeSeries;
    state.indicatorChart = indicatorChart;
    rebuildMaSeries();
  }

  // 根据当前 cfg-periods 创建/重建 MA 线条（切换预设或手动改周期时调用）
  function rebuildMaSeries() {
    if (!state.chart) return;
    // 移除旧序列
    for (const p of Object.keys(state.maSeries)) {
      try { state.chart.removeSeries(state.maSeries[p]); } catch (_) { /* noop */ }
    }
    state.maSeries = {};
    const periods = currentMaPeriods();
    periods.forEach((p, idx) => {
      state.maSeries[p] = state.chart.addLineSeries({
        color: colorForPeriod(p, idx),
        lineWidth: (p <= 30) ? 2 : 1.5,
        lineStyle: 0,
        lineType: 0,
        priceLineVisible: false,
        lastValueVisible: false,
        crosshairMarkerVisible: false,
        // 不设 title：MA 名称与值已显示在图表左上角 .chart-ma-overlay，避免右侧价格轴堆标签
      });
    });
  }

  // ---------- 数据喂给图表 ----------
  function applyKlines(klines) {
    state.currentKlines = klines;
    const cs = klines.map((k) => ({
      time: Math.floor(k.open_time / 1000),
      open: k.open,
      high: k.high,
      low: k.low,
      close: k.close,
    }));
    const vs = klines.map((k) => ({
      time: Math.floor(k.open_time / 1000),
      value: k.volume,
      color: k.close >= k.open ? hexToRgba(BULL_COLOR, 0.4) : hexToRgba(BEAR_COLOR, 0.4),
    }));
    state.candleSeries.setData(cs);
    state.volumeSeries.setData(vs);
    state.chart.timeScale().fitContent();
    // P0-3：同步更新顶部大号价格（以最新 K 线收盘价为基准）
    updatePriceDisplay();
    // AiCoin 风：更新图表顶部信息条（OHLC + 涨跌）为最新 bar
    const last = klines[klines.length - 1];
    const prev = klines.length > 1 ? klines[klines.length - 2] : null;
    updateChartTicker(last, prev);
    // 成交量 meta 条（默认最新 bar）
    updateVolumeMeta(klines, null);
  }

  // ---------- AiCoin 风：图表顶部 OHLC 信息条（可传任意 bar，供 crosshair 联动） ----------
  function updateChartTicker(bar, prevBar) {
    if (!bar) return;
    const sym = $('symbol')?.value || '—';
    const intv = $('interval')?.value || '';

    const symEl = $('ct-symbol');
    if (symEl) symEl.textContent = `${sym} · ${intv}`;

    const timeEl = $('ct-time');
    if (timeEl) timeEl.textContent = fmtDatetime(bar.open_time);

    const setField = (id, val, cls) => {
      const el = $(id);
      if (!el) return;
      el.textContent = fmtPrice(val);
      el.className = cls || '';
    };
    const upDown = bar.close >= bar.open ? 'up' : 'down';
    setField('ct-open',  bar.open);
    setField('ct-high',  bar.high, 'up');
    setField('ct-low',   bar.low, 'down');
    setField('ct-close', bar.close, upDown);

    // 涨幅（相对前一根 close）
    const changeEl = $('ct-change');
    if (changeEl) {
      if (prevBar && prevBar.close > 0) {
        const diff = bar.close - prevBar.close;
        const pct = (diff / prevBar.close) * 100;
        const cls = diff >= 0 ? 'up' : 'down';
        const sign = diff >= 0 ? '+' : '';
        changeEl.textContent = `${sign}${pct.toFixed(2)}%`;
        changeEl.className = cls;
      } else {
        changeEl.textContent = '—';
        changeEl.className = '';
      }
    }
    // 振幅（相对前一根 close 的高低范围）
    const ampEl = $('ct-amp');
    if (ampEl) {
      if (prevBar && prevBar.close > 0) {
        const amp = ((bar.high - bar.low) / prevBar.close) * 100;
        ampEl.textContent = `${amp.toFixed(2)}%`;
      } else {
        ampEl.textContent = '—';
      }
    }
  }

  // 格式化 UTC ms → YYYY-MM-DD HH:MM
  function fmtDatetime(ms) {
    const d = new Date(ms);
    const pad = (n) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  // 更新成交量 meta（当根 + MA20 比）
  // @param {Array} klines
  // @param {number|null} barIdx  null 表示最新一根（供 crosshair 联动）
  function updateVolumeMeta(klines, barIdx) {
    const el = $('volume-meta-fields');
    if (!el || !klines.length) return;
    const idx = (barIdx == null || barIdx >= klines.length) ? klines.length - 1 : barIdx;
    const cur = klines[idx];
    if (!cur) return;
    // 以 idx 为右端、向前取 20 根（含自身）作均量
    const start = Math.max(0, idx - 19);
    const tail = klines.slice(start, idx + 1);
    const avg = tail.reduce((s, k) => s + (k.volume || 0), 0) / (tail.length || 1);
    const ratio = avg > 0 ? cur.volume / avg : 0;
    const ratioTag = ratio >= 1.5 ? '<b class="up">放量</b>' : ratio <= 0.5 ? '<b class="down">缩量</b>' : '';
    el.innerHTML = `当根 <b>${fmtPrice(cur.volume)}</b>  均量(20) <b>${fmtPrice(avg)}</b>  量比 <b>${ratio.toFixed(2)}</b>  ${ratioTag}`;
  }

  // ---------- P0-3：顶部当前价大号显示 ----------
  // 24h 前的 close 作为涨跌基准，失败回退到第一根 K 线的 close
  function priceBaseline() {
    const kl = state.currentKlines;
    if (!kl.length) return null;
    const barsIn24h = Math.max(1, Math.round(86400 / tfSeconds()));
    const last = kl.length - 1;
    const refIdx = Math.max(0, last - barsIn24h);
    const ref = kl[refIdx]?.close;
    return Number.isFinite(ref) ? ref : null;
  }

  // 刷新价格大号显示
  // @param {number=} livePrice   可选，若传入则用此作为当前价（WS tick）
  // @param {number=} liveOpen    可选，WS 当根 K 线的开盘价（用于 flash 方向判定）
  function updatePriceDisplay(livePrice, liveOpen) {
    const kl = state.currentKlines;
    const priceEl = $('current-price');
    const changeEl = $('price-change');
    const symEl = $('price-symbol');
    if (!priceEl) return;

    const symbol = $('symbol')?.value;
    if (symEl) symEl.textContent = symbol || '';

    let price = livePrice;
    if (!Number.isFinite(price)) {
      const lastK = kl.length ? kl[kl.length - 1] : null;
      price = lastK ? lastK.close : NaN;
    }
    if (!Number.isFinite(price)) {
      priceEl.textContent = '—';
      changeEl.textContent = '—';
      changeEl.className = 'price-change';
      return;
    }

    // 涨跌：优先用 24h 基准；若 tick 场景下 livePrice 来自 WS，也用同基准
    const base = priceBaseline();
    let pct = null, diff = null;
    if (Number.isFinite(base) && base > 0) {
      diff = price - base;
      pct = diff / base;
    }

    const prev = parseFloat(priceEl.dataset.lastPrice || '');
    priceEl.textContent = fmtPrice(price);
    priceEl.dataset.lastPrice = String(price);
    // 颜色按 24h 方向
    priceEl.classList.remove('up', 'down', 'flash-up', 'flash-down');
    if (pct != null && pct > 0) priceEl.classList.add('up');
    else if (pct != null && pct < 0) priceEl.classList.add('down');
    // 闪烁按相邻 tick 方向
    if (Number.isFinite(prev) && prev !== price) {
      const cls = price > prev ? 'flash-up' : 'flash-down';
      // 强制 reflow 触发动画
      priceEl.classList.add(cls);
      setTimeout(() => priceEl.classList.remove(cls), 260);
    }

    if (pct == null) {
      changeEl.textContent = '—';
      changeEl.className = 'price-change';
    } else {
      const sign = pct >= 0 ? '▲' : '▼';
      changeEl.textContent = `${sign} ${Math.abs(diff).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} (${(pct * 100).toFixed(2)}%)`;
      changeEl.className = 'price-change ' + (pct >= 0 ? 'up' : 'down');
    }
    // 避免未使用变量告警
    void liveOpen;
  }

  function applyMa(maState) {
    state.currentMaState = maState;
    // 清空所有 ma 系列（隐藏未包含的）
    for (const p of Object.keys(state.maSeries)) state.maSeries[p].setData([]);
    const periods = maState.periods;
    const series = maState.series;
    for (let idx = 0; idx < periods.length; idx++) {
      const p = periods[idx];
      if (!(p in state.maSeries)) continue;
      const data = [];
      for (let i = 0; i < series[idx].length; i++) {
        const v = series[idx][i];
        if (!isFinite(v) || v == null) continue;
        const ts = state.currentKlines[i]?.open_time;
        if (!ts) continue;
        data.push({ time: Math.floor(ts / 1000), value: v });
      }
      state.maSeries[p].setData(data);
    }
    // 左上角 MA overlay 显示最新 bar 的 MA 值
    updateChartMaValuesForBar(null);
  }

  // 图表左上角 MA 浮层（MA(5) 0.4396  MA(10) 0.4125 …）
  // @param {number|null} barIdx  null 表示最新一根
  function updateChartMaValuesForBar(barIdx) {
    const el = $('chart-ma-overlay');
    const maState = state.currentMaState;
    if (!el || !maState?.periods || !maState?.series) { return; }
    const parts = [];
    for (let idx = 0; idx < maState.periods.length; idx++) {
      const p = maState.periods[idx];
      const arr = maState.series[idx];
      if (!Array.isArray(arr) || arr.length === 0) continue;
      // 指定 bar：若该 bar 无值（warmup 期），回退向前查找最近一个有效值；null 表示最新
      let v = null;
      const upper = (barIdx == null || barIdx >= arr.length) ? arr.length - 1 : barIdx;
      for (let i = upper; i >= 0; i--) {
        if (isFinite(arr[i]) && arr[i] != null) { v = arr[i]; break; }
      }
      if (v == null) continue;
      const color = colorForPeriod(p, idx);
      parts.push(`<span class="ma-item" style="color:${color}">MA(${p}) <b>${fmtPrice(v)}</b></span>`);
    }
    el.innerHTML = parts.join('');
  }

  // 形态标注渲染（P0-2 增强）：
  //   1. 按 pattern-density 过滤最小强度
  //   2. 同一根 bar 上多个形态 → 最强的 1 个 + `+N` 后缀；方向投票决定颜色
  //   3. 5 根 K 线窗口内多个标注 → 聚合为"●×N"圆点，避免文字重叠
  //   4. 低强度（弱于当前 density）的标注使用淡化颜色
  //   5. 按 strength 降序取前 MAX_MARKERS，再按 time 升序（LWC 要求）
  const MAX_PATTERN_MARKERS = 80;
  const CLUSTER_WINDOW_BARS = 5; // 5 根窗口内聚合
  const CLUSTER_THRESHOLD = 3;   // 达到 N 个才聚合

  // 方向分色 + 强度分级 -> 颜色
  function markerColor(direction, strength, isCluster) {
    // cluster 永远用主色调
    // 强度：5-6=实色，3-4=稍淡，<3=灰
    const alpha = isCluster ? 1 : (strength >= 5 ? 1 : strength >= 3 ? 0.75 : 0.5);
    if (direction > 0) return `rgba(38, 166, 154, ${alpha})`;
    if (direction < 0) return `rgba(239, 83, 80, ${alpha})`;
    return `rgba(139, 148, 158, ${alpha})`;
  }

  function applyPatternMarkers(patterns, extraMarkers) {
    state.currentPatterns = patterns || state.currentPatterns;
    const show = $('show-patterns').checked;
    const minStrength = parseInt($('pattern-density')?.value || '3', 10);

    let markers = [];
    if (show && patterns && patterns.length) {
      // Step 1: 过滤 + 按 index 分组（加入分类过滤）
      const cats = state.patternCats;
      const hasCatFilter = cats && (!cats.single || !cats.double || !cats.triple || !cats.advanced);
      const byIndex = new Map();
      for (const p of patterns) {
        if ((p.strength || 0) < minStrength) continue;
        if (!state.currentKlines[p.index]) continue;
        if (hasCatFilter) {
          const cat = PATTERN_CATEGORY[p.kind] || 'single';
          if (!cats[cat]) continue;
        }
        const arr = byIndex.get(p.index);
        if (arr) arr.push(p); else byIndex.set(p.index, [p]);
      }

      // Step 2: 每个 bar 挑最强的一个；方向投票决定颜色
      const picks = [];
      for (const [index, arr] of byIndex) {
        arr.sort((a, b) => (b.strength || 0) - (a.strength || 0));
        const strongest = arr[0];
        let vote = 0;
        for (const p of arr) vote += (p.direction || 0) * (p.strength || 1);
        const direction = vote > 0 ? 1 : vote < 0 ? -1 : 0;
        picks.push({
          index,
          label: strongest.label + (arr.length > 1 ? ` +${arr.length - 1}` : ''),
          strength: strongest.strength,
          direction,
          memberCount: arr.length,
        });
      }

      // Step 3: 按 index 升序 → 5 根窗口内聚合
      picks.sort((a, b) => a.index - b.index);
      const clusters = [];
      let cur = [];
      for (const p of picks) {
        if (!cur.length || p.index - cur[cur.length - 1].index <= CLUSTER_WINDOW_BARS) {
          cur.push(p);
        } else {
          clusters.push(cur);
          cur = [p];
        }
      }
      if (cur.length) clusters.push(cur);

      // Step 4: 每个 cluster 输出 0/1 个 marker
      const renderable = [];
      for (const cluster of clusters) {
        if (cluster.length >= CLUSTER_THRESHOLD) {
          // 聚合：用中心点位置 + "●×N"（中心偏向最强那个）
          cluster.sort((a, b) => (b.strength || 0) - (a.strength || 0));
          const anchor = cluster[0];
          // 方向投票（按强度加权）
          let vote = 0;
          for (const p of cluster) vote += (p.direction || 0) * (p.strength || 1);
          const direction = vote > 0 ? 1 : vote < 0 ? -1 : 0;
          const totalMembers = cluster.reduce((s, p) => s + p.memberCount, 0);
          renderable.push({
            index: anchor.index,
            label: `×${totalMembers}`,
            strength: anchor.strength,
            direction,
            isCluster: true,
          });
        } else {
          // 单点：逐个保留
          for (const p of cluster) renderable.push({ ...p, isCluster: false });
        }
      }

      // Step 5: 限制总量
      renderable.sort((a, b) => (b.strength || 0) - (a.strength || 0));
      const kept = renderable.slice(0, MAX_PATTERN_MARKERS);

      // Step 6: 转为 marker
      markers = kept.map((p) => {
        const k = state.currentKlines[p.index];
        const bull = p.direction > 0;
        const bear = p.direction < 0;
        return {
          time: Math.floor(k.open_time / 1000),
          position: p.isCluster ? 'inBar' : (bull ? 'belowBar' : bear ? 'aboveBar' : 'inBar'),
          color: markerColor(p.direction, p.strength, p.isCluster),
          shape: p.isCluster ? 'circle' : (bull ? 'arrowUp' : bear ? 'arrowDown' : 'circle'),
          text: p.label,
          size: p.isCluster ? 1 : (p.strength >= 5 ? 1 : 0),
        };
      });
    }

    if (extraMarkers && extraMarkers.length) {
      markers = markers.concat(extraMarkers);
    }
    // 合并测量工具的 markers（独立池，跨 renderDrawing 持久显示）
    if (state.measureMarkers.length) {
      markers = markers.concat(state.measureMarkers.map((x) => x.marker));
    }
    // LWC 要求按 time 升序
    markers.sort((a, b) => a.time - b.time);
    state.candleSeries.setMarkers(markers);
  }

  // ---------- 右侧面板渲染 ----------
  function renderMaPanel(ma) {
    $('alignment-val').textContent = `${ma.alignment}`;
    $('alignment-aliases').textContent = (ma.alignment_aliases || []).join(' / ');
    const aCls = ma.alignment === 'Bullish' ? 'bull' : ma.alignment === 'Bearish' ? 'bear' : '';
    $('alignment-val').className = 'v ' + aCls;

    $('bias-val').textContent = `MA${ma.bias_base_period}  ${fmtPct(ma.bias_base)}`;
    $('bias-val').className = 'v ' + (ma.bias_base > 0 ? 'bull' : ma.bias_base < 0 ? 'bear' : '');

    $('price-pos').textContent =
      ma.price_vs_base === 'above' ? '在基准均线之上 ▲' :
      ma.price_vs_base === 'below' ? '在基准均线之下 ▼' :
      ma.price_vs_base === 'near' ? '紧贴基准均线 ─' : '—';

    $('spread-state').textContent = ma.spread_state || '平稳';

    // MA 值表
    const tbody = $('ma-values').querySelector('tbody');
    tbody.innerHTML = '';
    for (let i = 0; i < ma.periods.length; i++) {
      const p = ma.periods[i];
      const v = ma.last_values[i];
      const s = ma.slopes[i];
      const slopeCls = s > 0 ? 'bull' : s < 0 ? 'bear' : '';
      const tr = document.createElement('tr');
      tr.innerHTML = `<td class="name">MA${p}</td><td>${fmtPrice(v)}</td><td class="${slopeCls}">${fmtPct(s)}</td>`;
      tbody.appendChild(tr);
    }
  }

  function renderGranville(ma) {
    const ul = $('granville-list');
    ul.innerHTML = '';
    if (!ma.granville.length) {
      ul.innerHTML = '<li class="dim">近期无葛南维信号</li>';
      return;
    }
    for (const g of ma.granville.slice(-12).reverse()) {
      const rule = g.rule; // enum name e.g. "B1BreakoutBuy"
      const isBuy = rule.startsWith('B');
      const li = document.createElement('li');
      li.className = (isBuy ? 'buy' : 'sell') + ' explainable';
      const k = state.currentKlines[g.index];
      const ts = k ? fmtTs(k.open_time) : '';
      li.setAttribute('title', `${prettyGranville(rule)} · 点击查看详解并在图表定位`);
      li.innerHTML = `<span>${prettyGranville(rule)}</span><span class="sig-time">${ts}</span>`;
      li.addEventListener('click', () => {
        window.AuraExplainer?.explain('granville', {
          rule,
          timeMs: k?.open_time,
          price: k?.close,
          bar: g.index,
        });
      });
      ul.appendChild(li);
    }
  }

  function prettyGranville(rule) {
    const map = {
      B1BreakoutBuy: 'B1 突破买入',
      B2PullbackBuy: 'B2 回踩买入',
      B3FalseBreakBuy: 'B3 假跌买入',
      B4DivergenceBuy: 'B4 乖离买入',
      S1BreakdownSell: 'S1 跌破卖出',
      S2ReboundSell: 'S2 反弹卖出',
      S3FalseBreakSell: 'S3 假涨卖出',
      S4DivergenceSell: 'S4 乖离卖出',
    };
    return map[rule] || rule;
  }

  function renderCrosses(ma) {
    const ul = $('cross-list');
    ul.innerHTML = '';
    if (!ma.crosses.length) {
      ul.innerHTML = '<li class="dim">近期无均线交叉</li>';
      return;
    }
    for (const c of ma.crosses.slice(-12).reverse()) {
      const isGolden = c.kind === 'Golden';
      const li = document.createElement('li');
      li.className = (isGolden ? 'buy' : 'sell') + ' explainable';
      const k = state.currentKlines[c.index];
      const ts = k ? fmtTs(k.open_time) : '';
      const label = `${isGolden ? '金叉' : '死叉'}  MA${c.fast_period} × MA${c.slow_period}`;
      li.setAttribute('title', `${label} · 点击查看详解并在图表定位`);
      li.innerHTML = `<span>${label}</span><span class="sig-time">${ts}</span>`;
      li.addEventListener('click', () => {
        window.AuraExplainer?.explain('cross', {
          kind: c.kind,
          fast: c.fast_period,
          slow: c.slow_period,
          timeMs: k?.open_time,
          price: k?.close,
          bar: c.index,
        });
      });
      ul.appendChild(li);
    }
  }

  /// 渲染一个形态 list item：附加历史评级徽章 + hover tooltip
  function renderPatternItem(p, ul) {
    const li = document.createElement('li');
    li.className = (p.direction > 0 ? 'buy' : p.direction < 0 ? 'sell' : 'dim') + ' explainable';
    const stars = '★'.repeat(p.strength);
    const stats = statsFor(p.label);
    let badge = '';
    let tooltip = `${p.label}（强度 ${stars}） · 点击查看详解并在图表定位`;
    if (stats) {
      const style = RANK_STYLE[stats.rank] || {};
      badge = ` <span class="rank-badge" style="color:${style.color};border-color:${style.color}" title="历史胜率">${stats.rank}</span>`;
      tooltip = `${p.label}\n评级: ${stats.rank}\n历史胜率: ${stats.hit}\n历史 alpha: ${stats.alpha}` + (stats.note ? `\n备注: ${stats.note}` : '') + '\n\n点击查看详解并在图表定位';
    }
    li.setAttribute('title', tooltip);
    li.innerHTML = `<span>${p.label} ${stars}${badge}</span><span class="sig-time">${fmtTs(p.open_time)}</span>`;
    // 取对应 K 线的收盘价
    const k = state.currentKlines.find((kl) => kl.open_time === p.open_time);
    li.addEventListener('click', () => {
      window.AuraExplainer?.explain('pattern', {
        label: p.label,
        direction: p.direction,
        strength: p.strength,
        timeMs: p.open_time,
        price: k?.close,
        bar: k ? state.currentKlines.indexOf(k) : null,
      });
    });
    ul.appendChild(li);
  }

  function renderPatterns(patterns) {
    const ul = $('pattern-list');
    ul.innerHTML = '';
    if (!patterns.length) {
      ul.innerHTML = '<li class="dim">无识别到的形态</li>';
      return;
    }
    // 按 pattern-density 过滤强度，按时间倒序取最近 20 条
    const minStrength = parseInt($('pattern-density')?.value || '3', 10);
    const recent = patterns
      .filter((p) => p.strength >= minStrength)
      .slice(-20)
      .reverse();
    if (!recent.length) {
      ul.innerHTML = `<li class="dim">近期无强度 ≥ ${minStrength} 的形态</li>`;
      return;
    }
    for (const p of recent) renderPatternItem(p, ul);
  }

  // ---------- 主刷新流程 ----------
  async function reload() {
    // 防御性清理 symbol（处理浏览器 autocomplete 污染）
    const symRaw = $('symbol').value;
    const symbol = sanitizeSymbol(symRaw);
    if (symbol !== symRaw) $('symbol').value = symbol;
    // interval 默认回退到 4h（hidden select，某些情况可能为空）
    const interval = $('interval').value || '4h';
    if (!$('interval').value) $('interval').value = interval;
    const kind = $('ma-kind').value;
    const limit = parseInt($('limit').value || '500', 10);
    const periods = currentMaPeriods().join(',');

    // 简单的加载提示
    $('alignment-val').textContent = '加载中…';

    try {
      const [kl, ma, pat, tr, cp, rs, sig, decision, ind] = await Promise.all([
        fetchJson(`/api/klines?symbol=${symbol}&interval=${interval}&limit=${limit}`),
        fetchJson(`/api/ma_state?symbol=${symbol}&interval=${interval}&limit=${limit}&periods=${periods}&kind=${kind}`),
        fetchJson(`/api/candle_patterns?symbol=${symbol}&interval=${interval}&limit=${limit}`),
        fetchJson(`/api/trend_state?symbol=${symbol}&interval=${interval}&limit=${limit}`),
        fetchJson(`/api/chart_patterns?symbol=${symbol}&interval=${interval}&limit=${limit}`),
        fetchJson(`/api/resonance?symbol=${symbol}&interval=${interval}&limit=${limit}`
          + `&equity=${cfgEquity()}&max_risk=${cfgRisk()}&rr=${cfgRR()}&atr_mult=${cfgAtrMult()}`
          + `&w_ma=${cfgW('ma')}&w_trend=${cfgW('trend')}&w_candle=${cfgW('candle')}&w_chart=${cfgW('chart')}`),
        // Sprint 9：高级信号 API
        fetchJson(`/api/signals?symbol=${symbol}&interval=${interval}&limit=${limit}`).catch(() => null),
        // P0-1：AI 决策聚合 API
        fetchJson(`/api/decision?symbol=${symbol}&interval=${interval}&limit=${limit}`).catch(() => null),
        // 指标副图：RSI + MACD + StochRSI + 成交量
        fetchJson(`/api/indicators/series?symbol=${symbol}&interval=${interval}&limit=${limit}&kinds=rsi,macd,stoch_rsi,volume`).catch(() => null),
      ]);
      applyKlines(kl.klines);
      applyMa(ma);
      renderMaPanel(ma);
      renderGranville(ma);
      renderCrosses(ma);
      renderPatterns(pat.patterns);
      applyPatternMarkers(pat.patterns);
      applyTrend(tr.state);
      renderTrendPanel(tr.state);
      renderChartPatterns(cp.patterns);
      renderResonance(rs);
      if (sig) renderSignals(sig);
      if (decision) renderDecision(decision);
      if (ind) applyIndicators(ind);
      // 画线恢复（依赖 candleSeries 已就绪）
      restoreDrawings();
      // 启动/切换实时推送
      startWs(symbol, interval);
    } catch (e) {
      console.error(e);
      $('alignment-val').textContent = '加载失败: ' + e.message;
    }
  }

  // ---------- P0-1：AI 决策横条渲染（紧凑版） ----------
  // 主行：图标 + 标签 + 置信度 + 风险 + 首条理由 + 展开按钮
  // 详情区：完整理由列表 + 操作按钮 + 原书依据（默认折叠）
  // 响应字段参考 src/server/routes.rs::DecisionResp
  function renderDecision(d) {
    if (!d) return;
    const bar = $('decision-bar');
    if (!bar) return;

    // Claude Design v3：decision-strip 单行 + action-* 色调
    bar.className = 'decision-strip action-' + (d.action || 'hold');

    const iconMap = { buy: '🚀', sell: '⚠️', watch: '👀', hold: '✋' };
    const iconEl = $('decision-icon');
    if (iconEl) iconEl.textContent = iconMap[d.action] || '🎯';

    const labelEl = $('decision-label');
    if (labelEl) labelEl.textContent = d.action_label || '—';

    const riskEl = $('decision-risk');
    if (riskEl) {
      riskEl.textContent = d.risk_label || '—';
      riskEl.className = 'ds-risk-badge risk-' + (d.risk_level || 'medium');
    }

    const conf = Math.max(0, Math.min(100, Number(d.confidence) || 0));
    const confNumEl = $('decision-confidence-num');
    if (confNumEl) confNumEl.textContent = `${conf}%`;
    const confFillEl = $('decision-confidence-fill');
    if (confFillEl) confFillEl.style.width = conf + '%';

    const reasons = Array.isArray(d.reasons) && d.reasons.length ? d.reasons : ['暂无明显信号'];

    // 主行：首条理由预览（省略号处理）
    const reasonOneEl = $('decision-reason-one');
    if (reasonOneEl) {
      const suffix = reasons.length > 1 ? `  <span style="color:var(--text-dim);">+${reasons.length - 1}</span>` : '';
      reasonOneEl.innerHTML = escapeHtml(reasons[0]) + suffix;
    }

    // 详情区：完整列表
    const reasonUl = $('decision-reasons');
    if (reasonUl) {
      reasonUl.innerHTML = reasons.slice(0, 6).map(r => `<li>${escapeHtml(r)}</li>`).join('');
    }

    // 操作按钮
    const actEl = $('decision-actions');
    if (actEl) {
      const actions = Array.isArray(d.suggested_actions) ? d.suggested_actions : [];
      actEl.innerHTML = actions.length === 0
        ? '<span style="color:var(--text-dim);font-size:11px;">暂无建议操作</span>'
        : actions.map((a, i) => {
            const kind = a.kind === 'primary' ? 'primary' : a.kind === 'danger' ? 'danger' : '';
            const hint = a.hint ? `title="${escapeHtml(a.hint)}"` : '';
            return `<button class="decision-action-btn ${kind}" data-idx="${i}" ${hint}>${escapeHtml(a.label)}</button>`;
          }).join('');
    }

    const bookEl = $('decision-book');
    if (bookEl) {
      const srcs = Array.isArray(d.book_sources) ? d.book_sources : [];
      bookEl.innerHTML = srcs.length
        ? `📖 原书依据：<span>${srcs.map(escapeHtml).join(' · ')}</span>`
        : '';
    }
  }

  // ---------- 指标副图（MACD / RSI / 成交量） ----------
  //
  // state.indicatorData = { times: [ms], rsi?: [f64], macd?: {line, signal, hist}, volume?: [f64], volume_ma?: [f64] }
  // 仅在 tab 切换或重载数据时重建 series，避免残留。
  //
  // LWC 需 time 升序；NaN 位置通过 null 跳过。

  const RSI_OVERBOUGHT = 70;
  const RSI_OVERSOLD = 30;
  const STOCH_OVERBOUGHT = 80;
  const STOCH_OVERSOLD = 20;

  function applyIndicators(resp) {
    if (!resp) return;
    state.indicatorData = resp;
    renderIndicator(state.indicatorKind);
  }

  // 根据 kind 清空旧 series 并重建新 series
  function renderIndicator(kind) {
    const chart = state.indicatorChart;
    const d = state.indicatorData;
    if (!chart || !d || !Array.isArray(d.times) || d.times.length === 0) return;

    // 清理旧 series
    for (const k of Object.keys(state.indicatorSeries)) {
      try { chart.removeSeries(state.indicatorSeries[k]); } catch (_) { /* noop */ }
    }
    state.indicatorSeries = {};
    state.indicatorKind = kind;

    // 时间（ms → sec）
    const toSec = (ms) => Math.floor(ms / 1000);
    const times = d.times;

    if (kind === 'rsi' && Array.isArray(d.rsi)) {
      const rsiSeries = chart.addLineSeries({
        color: WARN_COLOR, lineWidth: 2,
        priceLineVisible: false, lastValueVisible: true,
        priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
      });
      const data = [];
      for (let i = 0; i < times.length; i++) {
        const v = d.rsi[i];
        if (Number.isFinite(v)) data.push({ time: toSec(times[i]), value: v });
      }
      rsiSeries.setData(data);
      // 70/30 参考线
      rsiSeries.createPriceLine({
        price: RSI_OVERBOUGHT, color: hexToRgba(BEAR_COLOR, 0.6), lineWidth: 1,
        lineStyle: 2, axisLabelVisible: true, title: '70',
      });
      rsiSeries.createPriceLine({
        price: RSI_OVERSOLD, color: hexToRgba(BULL_COLOR, 0.6), lineWidth: 1,
        lineStyle: 2, axisLabelVisible: true, title: '30',
      });
      rsiSeries.createPriceLine({
        price: 50, color: hexToRgba(MUTED_COLOR, 0.35), lineWidth: 1, lineStyle: 1,
        axisLabelVisible: false, title: '',
      });
      state.indicatorSeries.rsi = rsiSeries;
    } else if (kind === 'stoch_rsi' && d.stoch_rsi) {
      // K（金黄）+ D（稳蓝）+ 80/20 参考线
      const kSeries = chart.addLineSeries({
        color: WARN_COLOR, lineWidth: 2,
        priceLineVisible: false, lastValueVisible: true,
        priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
      });
      const dSeries = chart.addLineSeries({
        color: INFO_COLOR, lineWidth: 1,
        priceLineVisible: false, lastValueVisible: true,
        priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
      });
      const kData = [], dData = [];
      for (let i = 0; i < times.length; i++) {
        const t = toSec(times[i]);
        const kv = d.stoch_rsi.k?.[i];
        const dv = d.stoch_rsi.d?.[i];
        if (Number.isFinite(kv)) kData.push({ time: t, value: kv });
        if (Number.isFinite(dv)) dData.push({ time: t, value: dv });
      }
      kSeries.setData(kData);
      dSeries.setData(dData);
      kSeries.createPriceLine({
        price: STOCH_OVERBOUGHT, color: hexToRgba(BEAR_COLOR, 0.6),
        lineWidth: 1, lineStyle: 2, axisLabelVisible: true, title: '80',
      });
      kSeries.createPriceLine({
        price: STOCH_OVERSOLD, color: hexToRgba(BULL_COLOR, 0.6),
        lineWidth: 1, lineStyle: 2, axisLabelVisible: true, title: '20',
      });
      kSeries.createPriceLine({
        price: 50, color: hexToRgba(MUTED_COLOR, 0.35),
        lineWidth: 1, lineStyle: 1, axisLabelVisible: false, title: '',
      });
      state.indicatorSeries.stochK = kSeries;
      state.indicatorSeries.stochD = dSeries;
    } else if (kind === 'macd' && d.macd) {
      // Histogram（柱）+ DIF（实线）+ DEA（虚线）
      const histSeries = chart.addHistogramSeries({
        priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
        priceScaleId: '',
      });
      const lineSeries = chart.addLineSeries({
        color: WARN_COLOR, lineWidth: 2, priceLineVisible: false, lastValueVisible: true,
        priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
      });
      const signalSeries = chart.addLineSeries({
        color: INFO_COLOR, lineWidth: 1, priceLineVisible: false, lastValueVisible: true,
        priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
      });

      const histData = [], lineData = [], signalData = [];
      for (let i = 0; i < times.length; i++) {
        const t = toSec(times[i]);
        const h = d.macd.hist?.[i];
        const l = d.macd.line?.[i];
        const s = d.macd.signal?.[i];
        if (Number.isFinite(h)) {
          histData.push({
            time: t, value: h,
            color: h >= 0 ? hexToRgba(BULL_COLOR, 0.7) : hexToRgba(BEAR_COLOR, 0.7),
          });
        }
        if (Number.isFinite(l)) lineData.push({ time: t, value: l });
        if (Number.isFinite(s)) signalData.push({ time: t, value: s });
      }
      histSeries.setData(histData);
      lineSeries.setData(lineData);
      signalSeries.setData(signalData);
      state.indicatorSeries.hist = histSeries;
      state.indicatorSeries.line = lineSeries;
      state.indicatorSeries.signal = signalSeries;
    } else if (kind === 'volume' && Array.isArray(d.volume)) {
      // 成交量柱（涨跌色）+ 20 周期均线
      const volHist = chart.addHistogramSeries({
        priceFormat: { type: 'volume' },
        priceScaleId: '',
      });
      const volData = [];
      for (let i = 0; i < times.length; i++) {
        const v = d.volume[i];
        if (!Number.isFinite(v)) continue;
        // 用 currentKlines 获取 open/close 判定涨跌色
        const kd = state.currentKlines[i];
        const up = kd && kd.close >= kd.open;
        volData.push({
          time: toSec(times[i]), value: v,
          color: up ? hexToRgba(BULL_COLOR, 0.6) : hexToRgba(BEAR_COLOR, 0.6),
        });
      }
      volHist.setData(volData);
      state.indicatorSeries.vol = volHist;

      if (Array.isArray(d.volume_ma)) {
        const maSeries = chart.addLineSeries({
          color: WARN_COLOR, lineWidth: 2, priceLineVisible: false, lastValueVisible: false,
          priceFormat: { type: 'volume' },
        });
        const maData = [];
        for (let i = 0; i < times.length; i++) {
          const v = d.volume_ma[i];
          if (Number.isFinite(v)) maData.push({ time: toSec(times[i]), value: v });
        }
        maSeries.setData(maData);
        state.indicatorSeries.volMa = maSeries;
      }
    }

    // 同步主图时间轴
    try {
      const r = state.chart?.timeScale().getVisibleLogicalRange();
      if (r) chart.timeScale().setVisibleLogicalRange(r);
    } catch (_) { /* noop */ }
    updateIndicatorMetaByTime(null);
  }

  // 光标悬停（time 为 sec）时，在右上角显示当前 bar 的指标读数
  function updateIndicatorMetaByTime(timeSec) {
    const el = $('indicator-meta');
    const d = state.indicatorData;
    if (!el || !d || !Array.isArray(d.times)) return;

    // 若未传 time，则展示最后一根
    let idx = d.times.length - 1;
    if (timeSec != null) {
      for (let i = 0; i < d.times.length; i++) {
        if (Math.floor(d.times[i] / 1000) === timeSec) { idx = i; break; }
      }
    }

    const kind = state.indicatorKind;
    if (kind === 'rsi' && Array.isArray(d.rsi)) {
      const v = d.rsi[idx];
      if (Number.isFinite(v)) {
        const tag = v >= RSI_OVERBOUGHT ? '⚠️ 超买' : v <= RSI_OVERSOLD ? '🌱 超卖' : '';
        el.textContent = `RSI(14) = ${v.toFixed(2)} ${tag}`;
      } else el.textContent = 'RSI(14) = —';
    } else if (kind === 'stoch_rsi' && d.stoch_rsi) {
      const kv = d.stoch_rsi.k?.[idx];
      const dv = d.stoch_rsi.d?.[idx];
      const parts = [];
      if (Number.isFinite(kv)) {
        const tag = kv >= STOCH_OVERBOUGHT ? '⚠️ 超买'
          : kv <= STOCH_OVERSOLD ? '🌱 超卖' : '';
        parts.push(`K ${kv.toFixed(2)} ${tag}`.trim());
      }
      if (Number.isFinite(dv)) parts.push(`D ${dv.toFixed(2)}`);
      if (Number.isFinite(kv) && Number.isFinite(dv)) {
        // 金叉/死叉提示（本 bar 的 K-D 符号差异）
        const prevK = d.stoch_rsi.k?.[idx - 1];
        const prevD = d.stoch_rsi.d?.[idx - 1];
        if (Number.isFinite(prevK) && Number.isFinite(prevD)) {
          if (prevK < prevD && kv >= dv) parts.push('✨ 金叉');
          else if (prevK > prevD && kv <= dv) parts.push('❌ 死叉');
        }
      }
      el.textContent = parts.join('  ') || 'StochRSI = —';
    } else if (kind === 'macd' && d.macd) {
      const l = d.macd.line?.[idx];
      const s = d.macd.signal?.[idx];
      const h = d.macd.hist?.[idx];
      const histTag = Number.isFinite(h) ? (h >= 0 ? '▲' : '▼') : '';
      const parts = [];
      if (Number.isFinite(l)) parts.push(`DIF ${l.toFixed(2)}`);
      if (Number.isFinite(s)) parts.push(`DEA ${s.toFixed(2)}`);
      if (Number.isFinite(h)) parts.push(`HIST ${histTag} ${Math.abs(h).toFixed(2)}`);
      el.textContent = parts.join('  ');
    } else if (kind === 'volume' && Array.isArray(d.volume)) {
      const v = d.volume[idx];
      const ma = d.volume_ma?.[idx];
      const ratio = Number.isFinite(v) && Number.isFinite(ma) && ma > 0 ? (v / ma) : null;
      const parts = [];
      if (Number.isFinite(v)) parts.push(`VOL ${fmtPrice(v)}`);
      if (Number.isFinite(ma)) parts.push(`MA20 ${fmtPrice(ma)}`);
      if (ratio != null) parts.push(`比 ${ratio.toFixed(2)}x`);
      el.textContent = parts.join('  ');
    } else {
      el.textContent = '—';
    }
  }

  // 切换副图 tab
  function switchIndicator(kind) {
    if (!['macd', 'rsi', 'stoch_rsi', 'volume'].includes(kind)) return;
    document.querySelectorAll('.ind-tab').forEach((t) => {
      t.classList.toggle('active', t.dataset.ind === kind);
    });
    renderIndicator(kind);
  }

  // ---------- 画线工具 ----------
  //
  // 支持 水平线 / 趋势线 两种：
  //   水平线 hline     — 一次点击即完成，用 candleSeries.createPriceLine 渲染
  //   趋势线 trendline — 两次点击连成一条直线，用独立 lineSeries 渲染
  //
  // 绘制结果存 state.drawings，同步写入 localStorage。
  //
  // 存储结构：
  //   hline:     { id, kind:'hline',     points:[{time, price}] }
  //   trendline: { id, kind:'trendline', points:[{time, price}, {time, price}] }

  const DRAW_STORAGE_KEY = 'aura_drawings';
  const DRAW_COLOR = INFO_COLOR; // 稳重蓝（CSS --info）—— 默认色
  const DRAW_LINE_WIDTH = 2;
  // 调色板：点击颜色按钮循环切换
  const DRAW_PALETTE = [
    { name: '蓝', value: INFO_COLOR },
    { name: '红', value: BEAR_COLOR },
    { name: '绿', value: BULL_COLOR },
    { name: '橙', value: WARN_COLOR },
    { name: '白', value: '#e5e5e5' },
  ];
  // 命中检测：端点圆点半径（px）
  const DRAW_HIT_RADIUS_PX = 10;
  // 当前绘制颜色（从 localStorage 恢复）
  function currentDrawColor() {
    return state.drawColor || DRAW_COLOR;
  }

  // 点击主图时调用（param 来自 LWC subscribeClick 回调）
  //
  // 升级：**连续画模式** —— 画完一条不自动退出，Esc 或再点工具按钮才退出。
  //       **Shift 吸附** —— 按 Shift 时，价格自动吸附到该 bar 的 OHLC 最近者。
  function handleChartClick(param) {
    // 优先：如果是点在已有画线的端点上 → 进入拖拽，不画新线
    if (state.drawDrag || tryBeginDragFromChartClick(param)) return;

    if (!state.drawMode) {
      // 非画线模式下：点到画线端点附近 → 选中以显示圆点
      trySelectDrawing(param);
      return;
    }
    if (!param || !param.time || !param.point || !state.candleSeries) return;
    const rawPrice = state.candleSeries.coordinateToPrice(param.point.y);
    if (!Number.isFinite(rawPrice)) return;

    // Shift 吸附到 OHLC（若该 bar 在当前 K 线数据中）
    const price = maybeSnapPrice(rawPrice, param.time, param._shiftKey);
    const pt = { time: param.time, price };

    if (state.drawMode === 'hline') {
      try { commitDrawing({ kind: 'hline', points: [pt] }); }
      catch (e) { console.error('[draw] commit hline failed', e); }
      // 连续画：不退出
    } else if (state.drawMode === 'trendline' || state.drawMode === 'measure') {
      if (!state.drawPending) {
        state.drawPending = pt; // 第一点
      } else {
        // 同点防御：两点 time 相同会导致 LWC 异常
        if (state.drawPending.time === pt.time) {
          if (window.AuraToast) window.AuraToast.push('请选择不同时间的两点', 'warn');
          return;
        }
        try {
          commitDrawing({ kind: state.drawMode, points: [state.drawPending, pt] });
        } catch (e) {
          console.error('[draw] commit failed', e);
        } finally {
          state.drawPending = null;
          clearPreview();
        }
      }
    }
  }

  function commitDrawing(draft) {
    const id = ++state.drawSeq;
    const entry = {
      id, kind: draft.kind, points: draft.points,
      color: currentDrawColor(),
    };
    state.drawings.push(entry);
    renderDrawing(entry);
    saveDrawings();
  }

  // 渲染单个绘画对象，结果附到 entry 上便于后续删除
  function renderDrawing(entry) {
    if (!state.candleSeries || !state.chart) return;
    const color = entry.color || DRAW_COLOR;
    if (entry.kind === 'hline') {
      const pt = entry.points[0];
      const pl = state.candleSeries.createPriceLine({
        price: pt.price,
        color,
        lineWidth: DRAW_LINE_WIDTH,
        lineStyle: 0,
        axisLabelVisible: true,
        title: fmtPrice(pt.price),
      });
      entry._priceLine = pl;
    } else if (entry.kind === 'trendline') {
      const [a, b] = entry.points;
      const ser = state.chart.addLineSeries({
        color,
        lineWidth: DRAW_LINE_WIDTH,
        priceLineVisible: false,
        lastValueVisible: false,
        crosshairMarkerVisible: false,
        autoscaleInfoProvider: () => ({ priceRange: null }),
      });
      const data = a.time <= b.time
        ? [{ time: a.time, value: a.price }, { time: b.time, value: b.price }]
        : [{ time: b.time, value: b.price }, { time: a.time, value: a.price }];
      ser.setData(data);
      entry._series = ser;
    } else if (entry.kind === 'measure') {
      const [a, b] = entry.points;
      const pct = ((b.price - a.price) / Math.max(a.price, 1e-9)) * 100;
      const tfSec = tfSeconds();
      const bars = Math.max(1, Math.round((b.time - a.time) / tfSec));
      const sign = pct >= 0 ? '+' : '';
      const isUp = pct >= 0;
      const ser = state.chart.addLineSeries({
        color: color || WARN_COLOR,
        lineWidth: 2,
        lineStyle: 0,
        priceLineVisible: false,
        lastValueVisible: false,
        crosshairMarkerVisible: false,
        autoscaleInfoProvider: () => ({ priceRange: null }),
      });
      const data = a.time <= b.time
        ? [{ time: a.time, value: a.price }, { time: b.time, value: b.price }]
        : [{ time: b.time, value: b.price }, { time: a.time, value: a.price }];
      ser.setData(data);
      entry._series = ser;
      entry._measure = { pct, bars };
      const marker = {
        time: b.time,
        position: isUp ? 'aboveBar' : 'belowBar',
        color: isUp ? BULL_COLOR : BEAR_COLOR,
        shape: 'circle',
        text: `${sign}${pct.toFixed(2)}% · ${bars}根`,
        size: 1,
      };
      state.measureMarkers.push({ id: entry.id, marker });
      applyPatternMarkers(state.currentPatterns);
    }
    // 若此画线被选中，刷新端点圆点
    if (state.selectedDrawingId === entry.id) {
      renderDrawingHandles(entry);
    }
  }

  function removeDrawing(entry) {
    try {
      if (entry._priceLine && state.candleSeries) {
        state.candleSeries.removePriceLine(entry._priceLine);
      }
      if (entry._series && state.chart) {
        state.chart.removeSeries(entry._series);
      }
    } catch (_) { /* noop */ }
    entry._priceLine = null;
    entry._series = null;
    // 清理端点握点
    removeDrawingHandles(entry);
    // 若被选中则清除选中状态
    if (state.selectedDrawingId === entry.id) state.selectedDrawingId = null;
    // 测量：从 marker 池移除并刷新
    if (entry.kind === 'measure') {
      state.measureMarkers = state.measureMarkers.filter((m) => m.id !== entry.id);
      applyPatternMarkers(state.currentPatterns);
    }
  }

  function undoDrawing() {
    const last = state.drawings.pop();
    if (last) removeDrawing(last);
    saveDrawings();
  }

  function clearDrawings() {
    for (const d of state.drawings) removeDrawing(d);
    state.drawings = [];
    state.drawPending = null;
    saveDrawings();
  }

  // 删除单条画线（管理面板调用）
  function deleteDrawingById(id) {
    const idx = state.drawings.findIndex((d) => d.id === id);
    if (idx < 0) return;
    const [removed] = state.drawings.splice(idx, 1);
    removeDrawing(removed);
    saveDrawings();
  }

  // 跳转图表到指定 bar 的时间位置（事件定位 + 画线管理"定位"）
  // 策略：每次都把目标 bar 置于可视区左侧 1/3 处，右侧留白给后续 bar（更自然的阅读位置）
  function scrollToTime(timeSec) {
    if (!state.chart || !Number.isFinite(timeSec)) return;
    try {
      const tfSec = tfSeconds();
      // 目标放在左 1/3：视野内容 = [ts - 20根, ts + 40根]（目标在 20/60 ≈ 33%）
      state.chart.timeScale().setVisibleRange({
        from: timeSec - tfSec * 20,
        to: timeSec + tfSec * 40,
      });
    } catch (_) { /* noop */ }
  }

  // --- 事件解释器：临时高亮某根 K 线（价位线 + 图表内标记 + 10s 自动消失） ---
  function clearHighlightBar() {
    if (state._explainerPriceLine && state.candleSeries) {
      try { state.candleSeries.removePriceLine(state._explainerPriceLine); } catch (_) { /* noop */ }
      state._explainerPriceLine = null;
    }
    // 移除临时高亮 marker：恢复 applyPatternMarkers 后的稳态
    if (state._explainerMarkerAdded) {
      state._explainerMarkerAdded = false;
      try { applyPatternMarkers(state.currentPatterns); } catch (_) { /* noop */ }
    }
    if (state._explainerTimer) {
      clearTimeout(state._explainerTimer);
      state._explainerTimer = null;
    }
  }

  function highlightBar(timeSec, label, direction) {
    if (!state.chart || !state.candleSeries) return;
    // 先清除已有
    clearHighlightBar();
    // 定位到该 bar（open_time 以毫秒保存，timeSec 是秒）
    const k = state.currentKlines.find((kl) => Math.floor(kl.open_time / 1000) === timeSec);
    if (!k) return;
    const color = direction > 0 ? BULL_COLOR : direction < 0 ? BEAR_COLOR : WARN_COLOR;

    // 1. 水平虚线（价格线）
    try {
      state._explainerPriceLine = state.candleSeries.createPriceLine({
        price: k.close,
        color,
        lineWidth: 2,
        lineStyle: 2, // dashed
        axisLabelVisible: true,
        title: `◎ ${label || '事件'}`,
      });
    } catch (_) { /* noop */ }

    // 2. 图表内的临时"目标标记"（醒目大圆圈 + 文字标签）
    //    策略：在当前 markers 基础上追加，10s 后通过 applyPatternMarkers 还原
    try {
      const markerList = state.candleSeries && typeof state.candleSeries.markers === 'function'
        ? state.candleSeries.markers() : null;
      // 取当前 marker 池（若 API 不可用则走 fallback：直接 setMarkers 覆盖后 10s 再 apply）
      const existing = Array.isArray(markerList) ? markerList.slice() : [];
      const pulseMarker = {
        time: timeSec,
        position: direction > 0 ? 'belowBar' : direction < 0 ? 'aboveBar' : 'inBar',
        color,
        shape: 'circle',
        text: `🎯 ${label || '目标'}`,
        size: 2,
      };
      // 去除已有相同 time 的旧标记（避免和原池冲突），再追加 pulse
      const merged = existing
        .filter((m) => !(m.time === timeSec && m.text && m.text.startsWith('🎯')))
        .concat([pulseMarker])
        .sort((a, b) => a.time - b.time);
      state.candleSeries.setMarkers(merged);
      state._explainerMarkerAdded = true;
    } catch (_) { /* noop */ }

    // 10 秒后自动清除（图表定位 + 高亮足够用户观察）
    state._explainerTimer = setTimeout(clearHighlightBar, 10000);
  }

  // 对外暴露：供 eventExplainer.js 调用
  window.__AuraTradeApi = {
    scrollToTime,
    highlightBar,
    clearHighlight: clearHighlightBar,
  };

  // ---------- 画线管理面板 ----------
  function openDrawingManager() {
    const existing = document.getElementById('draw-manager');
    if (existing) { existing.remove(); return; } // 再次点击关闭
    const wrap = document.createElement('div');
    wrap.id = 'draw-manager';
    wrap.className = 'draw-manager';
    wrap.innerHTML = `
      <div class="dm-head">
        <span class="dm-title">📐 画线管理</span>
        <button class="dm-close" type="button" title="关闭">×</button>
      </div>
      <div class="dm-body"></div>
      <div class="dm-foot">
        <button class="dm-clear-all" type="button">🗑 清空全部</button>
      </div>
    `;
    document.body.appendChild(wrap);
    const close = () => wrap.remove();
    wrap.querySelector('.dm-close').addEventListener('click', close);
    wrap.querySelector('.dm-clear-all').addEventListener('click', () => {
      if (!state.drawings.length) return;
      clearDrawings();
      close();
    });
    // 渲染列表
    const body = wrap.querySelector('.dm-body');
    if (!state.drawings.length) {
      body.innerHTML = '<div class="dm-empty">暂无画线。使用 ━/／/📏 工具开始绘制。</div>';
      return;
    }
    state.drawings.forEach((d) => {
      const row = document.createElement('div');
      row.className = 'dm-row';
      const kindLabel = d.kind === 'hline' ? '水平线 ━' : d.kind === 'trendline' ? '趋势线 ／' : '测量 📏';
      const pts = d.points.map((p) => fmtPrice(p.price)).join(' → ');
      const extra = d.kind === 'measure' && d._measure
        ? `<span class="dm-extra">${d._measure.pct >= 0 ? '+' : ''}${d._measure.pct.toFixed(2)}% · ${d._measure.bars}根</span>`
        : '';
      row.innerHTML = `
        <span class="dm-kind">${kindLabel}</span>
        <span class="dm-pts">${pts}</span>
        ${extra}
        <button class="dm-goto" type="button" title="定位到该画线">↪</button>
        <button class="dm-del" type="button" title="删除">✕</button>
      `;
      row.querySelector('.dm-goto').addEventListener('click', () => {
        const t = d.points[d.points.length - 1].time;
        scrollToTime(t);
        close();
      });
      row.querySelector('.dm-del').addEventListener('click', () => {
        deleteDrawingById(d.id);
        // 重新渲染行（保持面板打开）
        row.remove();
        if (!state.drawings.length) {
          body.innerHTML = '<div class="dm-empty">暂无画线。</div>';
        }
      });
      body.appendChild(row);
    });
  }

  // ---------- 视图工具：重置视图 / 导出截图 ----------
  function resetChartView() {
    try {
      state.chart?.timeScale().fitContent();
      state.volumeChart?.timeScale().fitContent();
      state.indicatorChart?.timeScale().fitContent();
      if (window.AuraToast) window.AuraToast.push('已重置视图', 'info');
    } catch (e) {
      if (window.AuraToast) window.AuraToast.push('重置视图失败', 'error');
    }
  }

  // ---------- 形态/趋势 子菜单（caret dropdown）----------
  const PILL_SUBMENU_DEFS = {
    patterns: {
      title: '形态分类',
      stateKey: 'patternCats',
      storageKey: 'aura_pattern_cats',
      items: [
        { key: 'single',   label: '单根 K 线' },
        { key: 'double',   label: '双根组合' },
        { key: 'triple',   label: '三根组合' },
        { key: 'advanced', label: '高级（岛形/三明治等）' },
      ],
    },
    trend: {
      title: '趋势组件',
      stateKey: 'trendParts',
      storageKey: 'aura_trend_parts',
      items: [
        { key: 'trendline', label: '趋势线（斜线）' },
        { key: 'sr',        label: '支撑/阻力水平' },
        { key: 'swings',    label: '摆动点 H/L' },
      ],
    },
  };

  // 初始化子菜单状态（localStorage 恢复 / 默认全开）
  function initPillSubmenuState() {
    for (const name of Object.keys(PILL_SUBMENU_DEFS)) {
      const def = PILL_SUBMENU_DEFS[name];
      let saved = null;
      try {
        const raw = localStorage.getItem(def.storageKey);
        if (raw) saved = JSON.parse(raw);
      } catch (_) { /* ignore */ }
      const defaults = {};
      def.items.forEach((it) => { defaults[it.key] = true; });
      state[def.stateKey] = saved && typeof saved === 'object' ? { ...defaults, ...saved } : defaults;
    }
  }

  function togglePillSubmenu(name) {
    const existing = document.getElementById('pill-submenu-' + name);
    if (existing) { existing.remove(); return; }
    // 关闭其他已开的子菜单
    document.querySelectorAll('.pill-submenu').forEach((m) => m.remove());
    document.querySelectorAll('.pill-caret[aria-expanded="true"]').forEach((c) => c.setAttribute('aria-expanded', 'false'));

    const def = PILL_SUBMENU_DEFS[name];
    if (!def) return;
    const caret = document.querySelector(`.pill-caret[data-pill="${name}"]`);
    if (!caret) return;

    const menu = document.createElement('div');
    menu.className = 'pill-submenu';
    menu.id = 'pill-submenu-' + name;
    const cur = state[def.stateKey] || {};
    const allOn = def.items.every((it) => cur[it.key]);
    menu.innerHTML = `
      <div class="pill-sub-head">
        <span class="pill-sub-title">${def.title}</span>
        <button class="pill-sub-toggle-all" type="button">${allOn ? '全不选' : '全选'}</button>
      </div>
      <div class="pill-sub-body">
        ${def.items.map((it) => `
          <label class="pill-sub-item">
            <input type="checkbox" data-key="${it.key}" ${cur[it.key] ? 'checked' : ''}>
            <span>${it.label}</span>
          </label>
        `).join('')}
      </div>
    `;
    document.body.appendChild(menu);

    // 定位到 caret 下方
    const r = caret.getBoundingClientRect();
    menu.style.top = (r.bottom + 6) + 'px';
    menu.style.right = (window.innerWidth - r.right) + 'px';
    caret.setAttribute('aria-expanded', 'true');

    // 子复选框事件
    menu.querySelectorAll('input[type="checkbox"]').forEach((cb) => {
      cb.addEventListener('change', () => {
        state[def.stateKey][cb.dataset.key] = cb.checked;
        try { localStorage.setItem(def.storageKey, JSON.stringify(state[def.stateKey])); } catch (_) {}
        // 触发重新渲染
        rerenderOverlays();
        // 全选/全不选按钮文字随状态更新
        const nowAllOn = def.items.every((it) => state[def.stateKey][it.key]);
        const btn = menu.querySelector('.pill-sub-toggle-all');
        if (btn) btn.textContent = nowAllOn ? '全不选' : '全选';
      });
    });
    // 全选 / 全不选
    menu.querySelector('.pill-sub-toggle-all').addEventListener('click', () => {
      const anyOn = def.items.some((it) => state[def.stateKey][it.key]);
      const newVal = !anyOn; // 如果有任何开的，全关；否则全开
      def.items.forEach((it) => {
        state[def.stateKey][it.key] = newVal;
        const cb = menu.querySelector(`input[data-key="${it.key}"]`);
        if (cb) cb.checked = newVal;
      });
      try { localStorage.setItem(def.storageKey, JSON.stringify(state[def.stateKey])); } catch (_) {}
      rerenderOverlays();
      menu.querySelector('.pill-sub-toggle-all').textContent = newVal ? '全不选' : '全选';
    });

    // 点击外部关闭
    setTimeout(() => {
      const off = (ev) => {
        if (menu.contains(ev.target) || ev.target === caret) return;
        menu.remove();
        caret.setAttribute('aria-expanded', 'false');
        document.removeEventListener('click', off);
      };
      document.addEventListener('click', off);
    }, 0);
  }

  // 触发当前形态 / 趋势的重新渲染（用子状态变化后调用）
  function rerenderOverlays() {
    if (state.currentTrend) {
      applyTrend(state.currentTrend);
    } else {
      applyPatternMarkers(state.currentPatterns);
    }
  }

  function exportChartPng() {
    if (!state.chart) return;
    try {
      const canvas = state.chart.takeScreenshot();
      const dataUrl = canvas.toDataURL('image/png');
      const sym = $('symbol')?.value || 'chart';
      const intv = $('interval')?.value || '';
      const stamp = new Date().toISOString().replace(/[:T]/g, '-').slice(0, 16);
      const a = document.createElement('a');
      a.download = `aura-${sym}-${intv}-${stamp}.png`;
      a.href = dataUrl;
      a.click();
      if (window.AuraToast) window.AuraToast.push('已导出图表 PNG', 'success');
    } catch (e) {
      if (window.AuraToast) window.AuraToast.push(`导出失败：${e.message}`, 'error');
    }
  }

  function enterDrawMode(kind) {
    // 若点击的是已激活模式，则切回不绘制
    if (state.drawMode === kind) {
      exitDrawMode();
      return;
    }
    state.drawMode = kind;
    state.drawPending = null;
    document.querySelectorAll('.draw-btn[data-draw]').forEach((b) => {
      b.classList.toggle('active', b.dataset.draw === kind);
    });
    document.querySelector('.chart-area')?.classList.add('drawing');
  }

  function exitDrawMode() {
    state.drawMode = null;
    state.drawPending = null;
    clearPreview();
    hideSnapHint();
    document.querySelectorAll('.draw-btn[data-draw]').forEach((b) => b.classList.remove('active'));
    document.querySelector('.chart-area')?.classList.remove('drawing');
  }

  // ========== 画线升级：预览 / 吸附 / 选中 / 拖拽 ==========

  // 重入守卫 + rAF 节流：
  // LWC 的 setData 可能让十字线位置重新计算并 emit crosshairMove，若直接处理会导致递归死循环。
  // 方案：高频回调先暂存最新 param，下一帧再真正执行一次，并加重入 flag 兜底。
  let _previewBusy = false;
  let _previewPending = null;
  let _previewRaf = 0;

  function updateDrawingPreview(param) {
    if (_previewBusy) return; // 正在执行中，忽略递归调用
    _previewPending = param;
    if (_previewRaf) return;
    _previewRaf = requestAnimationFrame(() => {
      _previewRaf = 0;
      const p = _previewPending;
      _previewPending = null;
      if (!state.chart) return; // 图表已销毁
      _previewBusy = true;
      try { _applyDrawingPreview(p); }
      catch (e) { console.error('[draw] preview error', e); }
      finally { _previewBusy = false; }
    });
  }

  // —— 橡皮筋预览（两步工具的第一点后，实时跟随鼠标） ——
  function _applyDrawingPreview(param) {
    // 1) 拖拽中：直接更新被拖点
    if (state.drawDrag) {
      updateDragByCrosshair(param);
      return;
    }
    // 2) 画线吸附提示
    if (state.drawMode && param && param.time && param.point) {
      const rawPrice = state.candleSeries?.coordinateToPrice(param.point.y);
      if (Number.isFinite(rawPrice) && state._lastShiftKey) {
        const snapped = maybeSnapPrice(rawPrice, param.time, true);
        if (snapped !== rawPrice) {
          showSnapHint(snapped, param.point.x, param.point.y);
        } else {
          hideSnapHint();
        }
      } else {
        hideSnapHint();
      }
    } else {
      hideSnapHint();
    }
    // 3) 两步工具：橡皮筋预览
    if (!state.drawPending || !state.drawMode || state.drawMode === 'hline') {
      clearPreview();
      return;
    }
    if (!param || !param.time || !param.point || !state.candleSeries) return;
    const rawPrice = state.candleSeries.coordinateToPrice(param.point.y);
    if (!Number.isFinite(rawPrice)) return;
    const price = maybeSnapPrice(rawPrice, param.time, state._lastShiftKey);
    const a = state.drawPending;
    const b = { time: param.time, price };
    // 避免 time 相同导致 LWC 报错（用户可能点在第一点正下方，trendline 需要时间递增）
    if (a.time === b.time) { clearPreview(); return; }
    ensurePreviewSeries();
    const data = a.time <= b.time
      ? [{ time: a.time, value: a.price }, { time: b.time, value: b.price }]
      : [{ time: b.time, value: b.price }, { time: a.time, value: a.price }];
    try { state.drawPreview.series.setData(data); } catch (_) { /* noop */ }
  }

  function ensurePreviewSeries() {
    if (state.drawPreview && state.drawPreview.series) return;
    if (!state.chart) return;
    const series = state.chart.addLineSeries({
      color: currentDrawColor(),
      lineWidth: 1,
      lineStyle: 2, // 虚线
      priceLineVisible: false,
      lastValueVisible: false,
      crosshairMarkerVisible: false,
      autoscaleInfoProvider: () => ({ priceRange: null }),
    });
    state.drawPreview = { series };
  }

  function clearPreview() {
    if (state.drawPreview && state.drawPreview.series && state.chart) {
      try { state.chart.removeSeries(state.drawPreview.series); } catch (_) { /* noop */ }
    }
    state.drawPreview = null;
  }

  // —— OHLC 吸附（Shift）——
  // 点击时若按住 Shift，价格吸附到该 bar 的 O/H/L/C 最近者
  function maybeSnapPrice(rawPrice, time, shiftKey) {
    if (!shiftKey) return rawPrice;
    const k = state.currentKlines.find((kl) => Math.floor(kl.open_time / 1000) === time);
    if (!k) return rawPrice;
    const candidates = [
      { k: 'H', v: k.high }, { k: 'L', v: k.low },
      { k: 'O', v: k.open }, { k: 'C', v: k.close },
    ];
    let best = candidates[0], bestDist = Math.abs(candidates[0].v - rawPrice);
    for (let i = 1; i < candidates.length; i++) {
      const d = Math.abs(candidates[i].v - rawPrice);
      if (d < bestDist) { best = candidates[i]; bestDist = d; }
    }
    return best.v;
  }

  function showSnapHint(price, x, y) {
    if (!state.snapHint) {
      const div = document.createElement('div');
      div.className = 'snap-hint';
      document.body.appendChild(div);
      state.snapHint = div;
    }
    const chartEl = document.getElementById('chart');
    if (!chartEl) return;
    const rect = chartEl.getBoundingClientRect();
    state.snapHint.textContent = `→ ${fmtPrice(price)}`;
    state.snapHint.style.left = (rect.left + x + 14) + 'px';
    state.snapHint.style.top = (rect.top + y - 10) + 'px';
    state.snapHint.style.display = 'block';
  }
  function hideSnapHint() {
    if (state.snapHint) state.snapHint.style.display = 'none';
  }

  // —— 选中画线（显示端点握点）——
  // 放宽：只要 point 存在就尝试命中（即便 time 为 null，点到 hline 的 Y 区域仍能选中）
  function trySelectDrawing(param) {
    if (!param || !param.point || !state.candleSeries) {
      clearSelection();
      return;
    }
    const hit = hitTestDrawings(param);
    if (hit) {
      selectDrawing(hit.entry.id);
    } else {
      clearSelection();
    }
  }

  function selectDrawing(id) {
    if (state.selectedDrawingId === id) return;
    clearSelection();
    state.selectedDrawingId = id;
    const entry = state.drawings.find((d) => d.id === id);
    if (entry) renderDrawingHandles(entry);
  }

  function clearSelection() {
    if (state.selectedDrawingId) {
      const entry = state.drawings.find((d) => d.id === state.selectedDrawingId);
      if (entry) removeDrawingHandles(entry);
    }
    state.selectedDrawingId = null;
  }

  // 命中检测：返回 { entry, pointIdx } 或 null
  // 策略：点击坐标到 drawing 的任一端点距离 <= DRAW_HIT_RADIUS_PX 像素
  function hitTestDrawings(param) {
    if (!param || !param.point || !state.candleSeries || !state.chart) return null;
    const mx = param.point.x, my = param.point.y;
    const ts = state.chart.timeScale();
    let best = null, bestD2 = DRAW_HIT_RADIUS_PX * DRAW_HIT_RADIUS_PX;
    for (const entry of state.drawings) {
      for (let i = 0; i < entry.points.length; i++) {
        const pt = entry.points[i];
        const x = ts.timeToCoordinate(pt.time);
        const y = state.candleSeries.priceToCoordinate(pt.price);
        if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
        const d2 = (x - mx) * (x - mx) + (y - my) * (y - my);
        if (d2 <= bestD2) {
          best = { entry, pointIdx: i };
          bestD2 = d2;
        }
      }
      // 水平线：命中检测按 Y 距离（全宽敏感）
      if (entry.kind === 'hline') {
        const y = state.candleSeries.priceToCoordinate(entry.points[0].price);
        if (Number.isFinite(y) && Math.abs(y - my) <= 4) {
          if (!best) best = { entry, pointIdx: 0 };
        }
      }
    }
    return best;
  }

  // —— 端点握点（选中时显示）——
  function renderDrawingHandles(entry) {
    removeDrawingHandles(entry);
    if (!state.candleSeries || !entry.points) return;
    // 用 candleSeries.setMarkers 会影响形态 marker，这里改用独立的 markers API 不可行
    // 策略：把握点渲染为 DOM 圆点覆盖在图表上（在 updateHandlesDom 中定位）
    entry._handles = entry.points.map((pt, idx) => {
      const dot = document.createElement('div');
      dot.className = 'draw-handle';
      dot.dataset.drawingId = entry.id;
      dot.dataset.pointIdx = String(idx);
      document.body.appendChild(dot);
      return dot;
    });
    updateHandlesDom(entry);
  }

  function removeDrawingHandles(entry) {
    if (entry && Array.isArray(entry._handles)) {
      for (const h of entry._handles) {
        try { h.remove(); } catch (_) { /* noop */ }
      }
      entry._handles = null;
    }
  }

  // 在 crosshairMove / 缩放 / resize 后更新握点位置（LWC 没有 viewport-change 事件，靠主动触发）
  function updateHandlesDom(entry) {
    if (!entry || !entry._handles || !state.chart || !state.candleSeries) return;
    const chartEl = document.getElementById('chart');
    if (!chartEl) return;
    const rect = chartEl.getBoundingClientRect();
    const ts = state.chart.timeScale();
    entry.points.forEach((pt, i) => {
      const dot = entry._handles[i];
      if (!dot) return;
      const x = ts.timeToCoordinate(pt.time);
      const y = state.candleSeries.priceToCoordinate(pt.price);
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        dot.style.display = 'none';
        return;
      }
      dot.style.display = 'block';
      dot.style.left = (rect.left + x - 6) + 'px';
      dot.style.top = (rect.top + y - 6) + 'px';
    });
  }

  function updateAllHandlesDom() {
    for (const d of state.drawings) {
      if (d._handles) updateHandlesDom(d);
    }
  }

  // —— 拖拽：点击端点进入拖拽模式 ——
  function tryBeginDragFromChartClick(param) {
    // 只在"选中态 && 点到握点"时才拖拽；否则返回 false 让 handleChartClick 继续走画线/选中逻辑
    if (!state.selectedDrawingId) return false;
    const hit = hitTestDrawings(param);
    if (!hit || hit.entry.id !== state.selectedDrawingId) return false;
    // 开始拖拽
    state.drawDrag = {
      drawingId: hit.entry.id,
      pointIdx: hit.pointIdx,
      origPoint: { ...hit.entry.points[hit.pointIdx] },
    };
    document.body.classList.add('drag-drawing');
    return true;
  }

  // 在 crosshairMove 时，如果正在拖拽，更新对应端点
  function updateDragByCrosshair(param) {
    if (!state.drawDrag || !param || !param.time || !param.point) return;
    const entry = state.drawings.find((d) => d.id === state.drawDrag.drawingId);
    if (!entry) return;
    const rawPrice = state.candleSeries.coordinateToPrice(param.point.y);
    if (!Number.isFinite(rawPrice)) return;
    const price = maybeSnapPrice(rawPrice, param.time, state._lastShiftKey);
    // 更新端点数据
    entry.points[state.drawDrag.pointIdx] = { time: param.time, price };
    // 重绘该画线
    redrawSingleDrawing(entry);
    updateHandlesDom(entry);
  }

  function redrawSingleDrawing(entry) {
    // 先删旧系列/priceLine/marker
    try {
      if (entry._priceLine && state.candleSeries) state.candleSeries.removePriceLine(entry._priceLine);
      if (entry._series && state.chart) state.chart.removeSeries(entry._series);
    } catch (_) { /* noop */ }
    entry._priceLine = null;
    entry._series = null;
    // 如果是 measure，移除旧 marker
    if (entry.kind === 'measure') {
      state.measureMarkers = state.measureMarkers.filter((m) => m.id !== entry.id);
    }
    renderDrawing(entry);
  }

  function finishDrag() {
    if (!state.drawDrag) return;
    document.body.classList.remove('drag-drawing');
    state.drawDrag = null;
    saveDrawings();
  }

  // —— 颜色切换：循环调色板 ——
  function cycleDrawColor() {
    const cur = state.drawColor || DRAW_COLOR;
    const idx = DRAW_PALETTE.findIndex((p) => p.value === cur);
    const next = DRAW_PALETTE[(idx + 1) % DRAW_PALETTE.length];
    state.drawColor = next.value;
    try { localStorage.setItem('aura_draw_color', next.value); } catch (_) { /* noop */ }
    syncDrawColorSwatch();
    // 若有选中画线 → 应用到该线（方便快速改色）
    if (state.selectedDrawingId) {
      const entry = state.drawings.find((d) => d.id === state.selectedDrawingId);
      if (entry) {
        entry.color = next.value;
        redrawSingleDrawing(entry);
        saveDrawings();
      }
    }
  }

  function syncDrawColorSwatch() {
    const btn = document.getElementById('draw-color');
    if (!btn) return;
    const swatch = btn.querySelector('.draw-color-swatch');
    if (swatch) swatch.style.background = currentDrawColor();
    const cur = DRAW_PALETTE.find((p) => p.value === currentDrawColor());
    if (cur) btn.title = `画线颜色：${cur.name}（点击循环切换）`;
  }

  // 按 symbol+interval 分别保存，跨 symbol 切换不干扰
  function drawStorageKey() {
    const s = $('symbol')?.value || '';
    const i = $('interval')?.value || '';
    return `${DRAW_STORAGE_KEY}:${s}:${i}`;
  }
  function saveDrawings() {
    try {
      const payload = state.drawings.map((d) => ({
        id: d.id, kind: d.kind, points: d.points,
        color: d.color || null,
      }));
      localStorage.setItem(drawStorageKey(), JSON.stringify(payload));
    } catch (_) { /* ignore quota */ }
  }
  // 从 localStorage 恢复画线（在 initCharts 后、reload 完毕时调用）
  function restoreDrawings() {
    // 清理已有可视化（切换 symbol 时）
    for (const d of state.drawings) removeDrawing(d);
    state.drawings = [];
    try {
      const raw = localStorage.getItem(drawStorageKey());
      if (!raw) return;
      const arr = JSON.parse(raw);
      if (!Array.isArray(arr)) return;
      for (const d of arr) {
        if (!d || !Array.isArray(d.points)) continue;
        const entry = {
          id: ++state.drawSeq, kind: d.kind, points: d.points,
          color: d.color || null,
        };
        state.drawings.push(entry);
        renderDrawing(entry);
      }
    } catch (_) { /* malformed */ }
  }

  // 展开/收起决策详情
  function toggleDecisionDetails() {
    const btn = $('decision-expand');
    const panel = $('decision-details');
    if (!btn || !panel) return;
    const isOpen = btn.getAttribute('aria-expanded') === 'true';
    if (isOpen) {
      panel.hidden = true;
      btn.setAttribute('aria-expanded', 'false');
    } else {
      panel.hidden = false;
      btn.setAttribute('aria-expanded', 'true');
    }
  }

  // 简单 HTML 转义，避免 reason 文本里的特殊字符破坏 DOM
  function escapeHtml(s) {
    if (s == null) return '';
    return String(s).replace(/[&<>"']/g, (c) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    }[c]));
  }

  // ---------- Sprint 9：高级信号面板渲染 ----------
  function renderSignals(sig) {
    if (!sig) return;

    const setText = (id, text) => {
      const el = $(id);
      if (el) el.textContent = text;
    };

    setText('sig-alignment', sig.current_alignment || '—');
    setText('sig-ma-relation', sig.ma_relation || '—');

    const confluenceCount = (sig.confluences || []).length;
    const strongConfluences = (sig.confluences || []).filter(c => c.unique_kinds >= 3).length;
    setText('sig-confluence-count',
      confluenceCount === 0 ? '无' :
      `${confluenceCount} 个${strongConfluences ? `（强 ${strongConfluences}）` : ''}`);

    const advMa = sig.advanced_ma_events || [];
    const guillotines = advMa.filter(e => e.kind === 'Guillotine').length;
    const poisons = advMa.filter(e => e.kind === 'PoissonSpider').length;
    const scallions = advMa.filter(e => e.kind === 'HangingScallions').length;
    const bullDiv = advMa.filter(e => e.kind === 'BondUpwardDiverge').length;
    setText('sig-advanced-ma-count',
      advMa.length === 0 ? '无' :
      `断头铡刀×${guillotines} 毒蜘蛛×${poisons} 旱地拔葱×${scallions} 向上发散×${bullDiv}`);

    const traps = sig.bull_traps || [];
    const bullTraps = traps.filter(t => t.kind === 'Bull').length;
    const bearTraps = traps.filter(t => t.kind === 'Bear').length;
    setText('sig-traps-count',
      traps.length === 0 ? '无' :
      `${traps.length} 个（多头陷阱 ${bullTraps} / 空头陷阱 ${bearTraps}）`);

    const stealth = sig.stealth_breakouts || [];
    setText('sig-stealth-count', stealth.length === 0 ? '无' : `${stealth.length} 次潜伏突破`);

    const flags = sig.flag_validations || [];
    const validFlags = flags.filter(f => f.validation.fully_valid).length;
    const acceptable = flags.filter(f => f.validation.passed_count >= 5).length;
    setText('sig-flag-count',
      flags.length === 0 ? '无' :
      `${flags.length} 个（完美 ${validFlags} / 可接受 ${acceptable}）`);

    // Sprint 17：新字段
    const vols = sig.volume_anomalies || [];
    const limitDowns = vols.filter(v => v.kind === 'LimitDownNoVolume').length;
    const limitUps = vols.filter(v => v.kind === 'LimitUpNoVolume').length;
    setText('sig-volume-anomaly',
      vols.length === 0 ? '无' :
      `无量跌停 ${limitDowns} / 无量涨停 ${limitUps}`);

    const longTerms = sig.long_term_hits || [];
    const break240 = longTerms.filter(h => h.event === 'BreakAbove240' || h.event === 'BreakBelow240').length;
    const touch240 = longTerms.filter(h => h.event === 'TouchResistance240' || h.event === 'TouchSupport240').length;
    setText('sig-long-term',
      longTerms.length === 0 ? '无' :
      `${longTerms.length} 次（突破 ${break240} / 触及 ${touch240}）`);

    const transitions = sig.trend_transitions || [];
    setText('sig-trend-transitions',
      transitions.length === 0 ? '无' : `${transitions.length} 次转移`);

    const combos = sig.candle_combinations || [];
    setText('sig-combinations',
      combos.length === 0 ? '无' : `${combos.length} 组（${combos.slice(-1)[0]?.kind || ''}）`);

    // 最近事件列表（最多 8 条）
    // 每个 event 带 payload（供 Explainer 使用）：{ type, data }
    const events = [];
    advMa.slice(-4).forEach(e => {
      const label = {
        Guillotine: '⚠️ 断头铡刀（最强空头）',
        PoissonSpider: '🕷 毒蜘蛛',
        HangingScallions: '🌱 旱地拔葱',
        BondUpwardDiverge: '🚀 再次粘合向上发散',
      }[e.kind] || e.kind;
      events.push({
        index: e.index,
        label,
        type: 'advMa',
        payload: { kind: e.kind, bar: e.index },
      });
    });
    (sig.confluences || []).filter(c => c.is_strong || c.unique_kinds >= 3).slice(0, 3).forEach(c => {
      events.push({
        index: 9999,
        label: `🎯 强合流 @${c.center_price.toFixed(2)}（${c.unique_kinds} 类组件 ×${c.strength_multiplier.toFixed(1)}）`,
        type: 'confluence',
        payload: {
          centerPrice: c.center_price,
          uniqueKinds: c.unique_kinds,
          strengthMultiplier: c.strength_multiplier,
        },
      });
    });
    traps.slice(-3).forEach(t => {
      const label = t.kind === 'Bull' ? '🪤 多头陷阱' : '🪤 空头陷阱';
      events.push({
        index: t.breakout_index,
        label: `${label}（${t.breakout_index} → ${t.reversal_index}）`,
        type: 'trap',
        payload: { kind: t.kind, bar: t.breakout_index, reversalBar: t.reversal_index },
      });
    });

    const ul = $('sig-recent-events');
    if (ul) {
      ul.innerHTML = '';
      if (events.length === 0) {
        ul.innerHTML = '<li class="dim">无最近事件</li>';
      } else {
        events.sort((a, b) => b.index - a.index);
        for (const e of events.slice(0, 8)) {
          const li = document.createElement('li');
          li.className = 'explainable';
          const tail = e.index < 9999 ? ` <span class="dim">@${e.index}</span>` : '';
          li.innerHTML = `<span>${e.label}</span>${tail}`;
          li.setAttribute('title', `${e.label.replace(/[⚠️🕷🌱🚀🎯🪤]+\s*/g, '')} · 点击查看详解`);
          // 绑定 bar → 时间
          const k = (e.payload?.bar != null) ? state.currentKlines[e.payload.bar] : null;
          li.addEventListener('click', () => {
            const enriched = {
              ...e.payload,
              timeMs: k?.open_time,
              price: k?.close,
            };
            window.AuraExplainer?.explain(e.type, enriched);
          });
          ul.appendChild(li);
        }
      }
    }
  }

  // ---------- 实时 WebSocket（Binance 公共 kline stream） ----------
  //
  // Binance 端点：wss://stream.binance.com:9443/ws/<symbol>@kline_<interval>
  // 收到的消息格式见 https://developers.binance.com/docs/binance-spot-api-docs/web-socket-streams#klinecandlestick-streams
  //
  // 策略：
  //   - 每个 tick 收到未收盘的 K 线，增量 update 最后一根 K 线 + volume
  //   - 当 K 线 close (k.x === true)，触发一次 reload() 重算所有形态/共振
  //   - 切换 symbol / interval 时自动重连
  //   - 异常时带指数退避重连
  function setLiveDot(status, text) {
    const dot = $('live-dot');
    const label = $('live-label');
    if (!dot) return;
    dot.className = 'live-dot ' + (status || '');
    if (label) label.textContent = text || '实时';
  }

  function stopWs() {
    if (state.wsReconnectTimer) {
      clearTimeout(state.wsReconnectTimer);
      state.wsReconnectTimer = null;
    }
    if (state.ws) {
      try {
        state.ws.onopen = state.ws.onmessage = state.ws.onclose = state.ws.onerror = null;
        state.ws.close();
      } catch (_) { /* ignore */ }
      state.ws = null;
    }
    state.wsSymbol = null;
    state.wsInterval = null;
    setLiveDot('', '离线');
  }

  function startWs(symbol, interval) {
    if (!$('live-stream')?.checked) {
      stopWs();
      return;
    }
    // 已连在同一 stream 则不重连
    if (state.ws && state.wsSymbol === symbol && state.wsInterval === interval
        && state.ws.readyState === WebSocket.OPEN) {
      return;
    }
    stopWs();

    const stream = `${symbol.toLowerCase()}@kline_${interval}`;
    const url = `wss://stream.binance.com:9443/ws/${stream}`;
    state.wsSymbol = symbol;
    state.wsInterval = interval;
    setLiveDot('connecting', '连接中…');

    let ws;
    try {
      ws = new WebSocket(url);
    } catch (e) {
      setLiveDot('error', '失败');
      scheduleReconnect(symbol, interval);
      return;
    }
    state.ws = ws;

    ws.onopen = () => {
      state.wsRetry = 0;
      setLiveDot('connected', '实时');
    };

    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (msg.e === 'kline' && msg.k) handleKlineTick(msg.k);
      } catch (_) { /* ignore parse errors */ }
    };

    ws.onclose = () => {
      setLiveDot('error', '断开');
      scheduleReconnect(symbol, interval);
    };

    ws.onerror = () => {
      setLiveDot('error', '错误');
      // close 会随后触发，由 onclose 统一重连
    };
  }

  function scheduleReconnect(symbol, interval) {
    if (!$('live-stream')?.checked) return;
    state.wsRetry = Math.min((state.wsRetry || 0) + 1, 8);
    const delay = Math.min(1000 * 2 ** state.wsRetry, 30000);
    if (state.wsReconnectTimer) clearTimeout(state.wsReconnectTimer);
    state.wsReconnectTimer = setTimeout(() => {
      // 只在当前仍是同一 symbol/interval 且开关开着时重连
      const cur = currentStreamKey();
      if (cur.symbol === symbol && cur.interval === interval && $('live-stream')?.checked) {
        startWs(symbol, interval);
      }
    }, delay);
  }

  function currentStreamKey() {
    return { symbol: $('symbol').value, interval: $('interval').value };
  }

  function handleKlineTick(k) {
    // k.t = 开盘时间 (ms), k.T = 收盘时间, k.o/h/l/c/v, k.x = 是否收盘
    const openTime = k.t;
    const timeSec = Math.floor(openTime / 1000);
    const open = parseFloat(k.o);
    const high = parseFloat(k.h);
    const low = parseFloat(k.l);
    const close = parseFloat(k.c);
    const volume = parseFloat(k.v);
    if (!isFinite(close)) return;

    state.lastLivePrice = close;

    // 更新图表最后一根（新 bar 自动生成，相同 time 则更新）
    if (state.candleSeries) {
      state.candleSeries.update({ time: timeSec, open, high, low, close });
    }
    if (state.volumeSeries) {
      state.volumeSeries.update({
        time: timeSec,
        value: volume,
        color: close >= open ? hexToRgba(BULL_COLOR, 0.4) : hexToRgba(BEAR_COLOR, 0.4),
      });
    }

    // 更新右下角实时价格显示
    const info = $('price-info');
    if (info) {
      info.textContent = `● ${fmtPrice(close)}`;
      info.style.color = close >= open ? 'var(--bull)' : 'var(--bear)';
    }
    const ti = $('time-info');
    if (ti) ti.textContent = '实时 · ' + fmtTs(openTime);

    // 同步更新 currentKlines 最后一根（供其他面板参考）
    if (state.currentKlines.length) {
      const last = state.currentKlines[state.currentKlines.length - 1];
      if (last.open_time === openTime) {
        last.high = high; last.low = low; last.close = close; last.volume = volume;
      } else if (openTime > last.open_time) {
        // 新 bar 开启（上一根应已收盘），插入占位，稍后由 reload 覆盖
        state.currentKlines.push({
          open_time: openTime, close_time: k.T,
          open, high, low, close, volume,
          trades: 0, taker_buy_volume: 0,
        });
      }
    }

    // P0-3：顶栏大号价格跟随 WS tick 实时更新
    updatePriceDisplay(close, open);

    // K 线收盘：触发一次完整后端刷新
    if (k.x === true && openTime !== state.lastBarOpenTime) {
      state.lastBarOpenTime = openTime;
      // 稍微延迟避免 Binance 的关闭事件早于 REST 返回新 bar
      setTimeout(() => {
        // 只在当前仍是同一 symbol/interval 时才刷新
        const cur = currentStreamKey();
        if (cur.symbol === state.wsSymbol && cur.interval === state.wsInterval) {
          reload();
        }
      }, 400);
    }
  }

  // ---------- Config 读取 ----------
  function cfgEquity() { return parseFloat($('cfg-equity')?.value || '10000'); }
  function cfgRisk() { return (parseFloat($('cfg-max-risk')?.value || '2')) / 100; }
  function cfgRR() { return parseFloat($('cfg-rr')?.value || '2'); }
  function cfgAtrMult() { return parseFloat($('cfg-atr-mult')?.value || '1.5'); }
  function cfgW(dim) { return parseFloat($('cfg-w-' + dim)?.value || '0.25'); }

  function bindConfigEvents() {
    const sliders = ['cfg-w-ma', 'cfg-w-trend', 'cfg-w-candle', 'cfg-w-chart'];
    const updateWeightSum = () => {
      let sum = 0;
      for (const id of sliders) {
        const v = parseFloat($(id).value);
        sum += v;
        $(id + '-v').textContent = v.toFixed(2);
      }
      $('cfg-w-sum').textContent = sum.toFixed(2);
      $('cfg-w-sum').style.color = Math.abs(sum - 1.0) < 0.01 ? 'var(--bull)' : 'var(--warn)';
    };

    // 权重滑条：实时更新显示 + 保存 + 去抖刷新
    let weightReloadTimer = null;
    for (const id of sliders) {
      $(id).addEventListener('input', () => {
        updateWeightSum();
        saveConfig();
        clearTimeout(weightReloadTimer);
        weightReloadTimer = setTimeout(() => {
          if ($('realtime').classList.contains('active')) reload();
        }, 400);
      });
    }
    updateWeightSum();

    // 其他配置变更：立即保存 + 实时刷新
    //   - cfg-periods 变化时还要重建 MA 图表线
    const triggers = ['cfg-equity', 'cfg-max-risk', 'cfg-rr', 'cfg-atr-mult', 'cfg-periods', 'cfg-base'];
    for (const id of triggers) {
      $(id)?.addEventListener('change', () => {
        // 手动改周期 → 自动切到"自定义"
        if (id === 'cfg-periods' || id === 'cfg-base') {
          const psel = $('cfg-preset');
          if (psel) psel.value = 'custom';
        }
        if (id === 'cfg-periods') rebuildMaSeries();
        saveConfig();
        reload();
      });
    }

    // 预设下拉：一键应用一整套参数
    $('cfg-preset')?.addEventListener('change', (e) => {
      const key = e.target.value;
      applyPreset(key);
    });

    // 恢复默认按钮
    $('cfg-reset')?.addEventListener('click', resetConfig);
  }

  // 应用预设：填充所有相关输入并刷新
  function applyPreset(key) {
    const preset = PRESETS[key];
    if (!preset) return; // "custom" 或未知 → 什么都不做
    $('cfg-periods').value = preset.periods;
    $('cfg-base').value = preset.base;
    $('cfg-w-ma').value = preset.weights.ma;
    $('cfg-w-trend').value = preset.weights.trend;
    $('cfg-w-candle').value = preset.weights.candle;
    $('cfg-w-chart').value = preset.weights.chart;
    $('cfg-atr-mult').value = preset.atrMult;
    $('cfg-max-risk').value = preset.risk;
    $('cfg-rr').value = preset.rr;
    // 滑条显示值同步
    for (const dim of ['ma', 'trend', 'candle', 'chart']) {
      const el = $(`cfg-w-${dim}-v`);
      if (el) el.textContent = parseFloat(preset.weights[dim]).toFixed(2);
    }
    const sum = ['ma', 'trend', 'candle', 'chart']
      .reduce((s, d) => s + parseFloat(preset.weights[d]), 0);
    $('cfg-w-sum').textContent = sum.toFixed(2);
    // 重建均线 + 保存 + 刷新
    rebuildMaSeries();
    saveConfig();
    reload();
  }

  // ---------- 事件绑定 ----------
  function bindEvents() {
    $('reload').addEventListener('click', reload);
    // symbol 是 input（combobox），需要 normalize 成大写 + trim + 容错清理
    $('symbol').addEventListener('change', () => {
      const el = $('symbol');
      const v = sanitizeSymbol(el.value);
      if (v !== el.value) el.value = v;
      if (!v) return;
      saveConfig();
      reload();
    });
    ['interval', 'ma-kind', 'limit'].forEach((id) =>
      $(id).addEventListener('change', () => { saveConfig(); reload(); })
    );
    // 实时开关
    $('live-stream')?.addEventListener('change', () => {
      if ($('live-stream').checked) {
        const { symbol, interval } = currentStreamKey();
        startWs(symbol, interval);
      } else {
        stopWs();
      }
    });
    // 页面隐藏时关闭 WS（节省资源），恢复时重连
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        if (state.ws) state.ws.close();
      } else if ($('live-stream')?.checked) {
        const { symbol, interval } = currentStreamKey();
        startWs(symbol, interval);
      }
    });
    $('log-scale').addEventListener('change', () => {
      saveConfig();
      state.chart.priceScale('right').applyOptions({
        mode: $('log-scale').checked ? 1 : 0,
      });
    });
    $('show-patterns').addEventListener('change', () => {
      saveConfig();
      applyPatternMarkers(state.currentPatterns);
    });
    $('pattern-density')?.addEventListener('change', () => {
      saveConfig();
      applyPatternMarkers(state.currentPatterns);
    });
    $('show-trend').addEventListener('change', () => {
      saveConfig();
      if (state.currentTrend) applyTrend(state.currentTrend);
    });
    $('show-fib')?.addEventListener('change', () => {
      saveConfig();
      applyFibLevels();
    });
    $('live-stream')?.addEventListener('change', saveConfig);

    // 回测按钮
    $('bt-run').addEventListener('click', runBacktest);

    // Sprint 13：Playbook 回测按钮
    $('pb-run-btn')?.addEventListener('click', runPlaybookBacktest);
    // P0-4：Playbook 对比模式
    $('pb-compare-btn')?.addEventListener('click', runPlaybookCompare);
    // Sprint A：指标有效性分析
    $('eff-run')?.addEventListener('click', runEffectiveness);
    // Sprint B：Bandit 面板
    $('bandit-refresh')?.addEventListener('click', () => refreshBandit());
    $('bandit-train')?.addEventListener('click', () => trainBandit());
    $('bandit-reset')?.addEventListener('click', () => resetBandit());

    // P0-5：空白引导的快捷预设按钮
    document.querySelectorAll('.empty-preset-btn').forEach((btn) => {
      btn.addEventListener('click', () => {
        applyBacktestPreset(btn.dataset.preset);
        runBacktest();
      });
    });

    // 决策横条：展开/收起详情
    $('decision-expand')?.addEventListener('click', toggleDecisionDetails);

    // 指标副图 tab 切换
    document.querySelectorAll('.ind-tab').forEach((btn) => {
      btn.addEventListener('click', () => switchIndicator(btn.dataset.ind));
    });

    // 画线工具
    document.querySelectorAll('.draw-btn[data-draw]').forEach((btn) => {
      btn.addEventListener('click', () => enterDrawMode(btn.dataset.draw));
    });
    $('draw-undo')?.addEventListener('click', undoDrawing);
    $('draw-clear')?.addEventListener('click', clearDrawings);
    $('draw-manage')?.addEventListener('click', openDrawingManager);
    $('draw-color')?.addEventListener('click', cycleDrawColor);
    // 启动时从 localStorage 恢复颜色 + 同步按钮色块
    try {
      const saved = localStorage.getItem('aura_draw_color');
      if (saved) state.drawColor = saved;
    } catch (_) { /* noop */ }
    syncDrawColorSwatch();
    // 视图工具
    $('btn-reset-view')?.addEventListener('click', resetChartView);
    $('btn-export-png')?.addEventListener('click', exportChartPng);
    // 子菜单 caret
    initPillSubmenuState();
    document.querySelectorAll('.pill-caret').forEach((btn) => {
      btn.addEventListener('click', (ev) => {
        ev.stopPropagation();
        togglePillSubmenu(btn.dataset.pill);
      });
    });

    // 周期按钮条（AiCoin 风）：点击切换 #interval + 同步 active
    function syncPeriodBtns() {
      const cur = $('interval')?.value;
      document.querySelectorAll('.pd-btn').forEach((b) => {
        b.classList.toggle('active', b.dataset.period === cur);
      });
    }
    document.querySelectorAll('.pd-btn').forEach((btn) => {
      btn.addEventListener('click', () => {
        const p = btn.dataset.period;
        const sel = $('interval');
        if (sel.value === p) return;
        if (!sel || !p) return;
        // 只有 select 里存在该值才切换（15m/1h/4h/1d/1w），否则忽略
        const has = Array.from(sel.options).some((o) => o.value === p);
        if (!has) {
          // 补充 option：确保 1m/5m/30m 这些也能用
          sel.insertAdjacentHTML('beforeend', `<option value="${p}">${p}</option>`);
        }
        sel.value = p;
        // 派发 change 事件，驱动 updateCmdPillDisplay + saveConfig + reload 统一刷新
        // （change handler 里已绑定 saveConfig + reload）
        sel.dispatchEvent(new Event('change', { bubbles: true }));
        syncPeriodBtns();
      });
    });
    // 通过 cmd-pill 改 interval 也同步 pd-btn
    $('interval')?.addEventListener('change', syncPeriodBtns);
    syncPeriodBtns();

    // Tab 切换（图表 / 回测 / 学习）
    document.querySelectorAll('.tab').forEach((tab) => {
      tab.addEventListener('click', () => {
        if (tab.disabled) return;
        document.querySelectorAll('.tab').forEach((t) => t.classList.remove('active'));
        document.querySelectorAll('.view').forEach((v) => v.classList.remove('active'));
        tab.classList.add('active');
        const target = document.getElementById(tab.dataset.tab);
        target?.classList.add('active');
        if (tab.dataset.tab === 'backtest') {
          requestAnimationFrame(() => initBacktestChart());
        }
        if (tab.dataset.tab === 'effectiveness' && !state.effLoaded) {
          state.effLoaded = true;
          requestAnimationFrame(() => {
            runEffectiveness();
            refreshBandit({ autoTrainIfEmpty: true });
          });
        }
      });
    });

    // ==================== Command Pill ====================
    // 顶栏合成选择器：点击展开 popover；选择变更时更新显示摘要
    const cmdBtn = $('cmd-open');
    const cmdPop = $('cmd-pop');
    const cmdPill = $('cmd-pill');
    function updateCmdPillDisplay() {
      const rawSym = $('symbol')?.value || '';
      const intv = $('interval')?.value || '—';
      const preset = $('cfg-preset')?.value || 'custom';
      const presetLabel = {
        aura: 'Aura 预设',
        banmuxia: '半木夏预设',
        custom: '自定义',
      }[preset] || preset;
      // 胶囊显示（symbol 已独立出去，这里只剩 interval + preset）
      const cpSym = $('cp-symbol'); // 向后兼容：如果还存在旧节点，仍然更新
      if (cpSym) cpSym.textContent = rawSym || '—';
      $('cp-interval').textContent = intv;
      $('cp-preset').textContent = presetLabel;
      // 更新独立的 sym-badge 显示：交易所徽章 + symbol
      const badgeEx = $('sym-badge-ex');
      const badgeSym = $('sym-badge-sym');
      if (badgeEx && badgeSym) {
        const parts = rawSym.split(':');
        const exchange = parts.length > 1 ? parts[0] : 'BINANCE';
        const symPart = parts.length > 1 ? parts[1] : rawSym;
        const styleMap = (typeof EXCHANGE_STYLE !== 'undefined') ? EXCHANGE_STYLE : null;
        const style = (styleMap && styleMap[exchange]) || { label: exchange, color: '#888', short: '?' };
        badgeEx.textContent = style.short;
        badgeEx.style.color = style.color;
        badgeEx.style.borderColor = `${style.color}66`;
        badgeEx.style.background = `${style.color}22`;
        badgeEx.title = style.label;
        badgeSym.textContent = symPart || '—';
      }
    }
    function openCmd() {
      cmdBtn.setAttribute('aria-expanded', 'true');
      cmdPop.hidden = false;
    }
    function closeCmd() {
      cmdBtn.setAttribute('aria-expanded', 'false');
      cmdPop.hidden = true;
    }
    cmdBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      cmdBtn.getAttribute('aria-expanded') === 'true' ? closeCmd() : openCmd();
    });
    document.addEventListener('click', (e) => {
      if (cmdPop?.hidden) return;
      if (!cmdPill?.contains(e.target)) closeCmd();
    });
    // 选择变更立即更新 pill 显示（reload 由原有绑定触发）
    ['symbol', 'interval', 'cfg-preset'].forEach((id) => {
      $(id)?.addEventListener('change', updateCmdPillDisplay);
    });
    updateCmdPillDisplay();
    // 「应用并刷新」后自动收起
    $('reload')?.addEventListener('click', () => closeCmd());

    // ==================== Settings Drawer ====================
    const drawer = $('settings-drawer');
    const backdrop = $('settings-backdrop');
    function openSettings() {
      drawer.hidden = false;
      backdrop.hidden = false;
    }
    function closeSettings() {
      drawer.hidden = true;
      backdrop.hidden = true;
    }
    $('open-settings')?.addEventListener('click', openSettings);
    $('close-settings')?.addEventListener('click', closeSettings);
    backdrop?.addEventListener('click', closeSettings);

    // 快捷键：Esc 关闭 drawer/cmd/drawMode；Cmd/Ctrl+, 打开设置
    //         H/T/M 切换画线工具；Del/Backspace 删除选中画线；Shift 键状态实时跟踪（OHLC 吸附）
    document.addEventListener('keydown', (e) => {
      // 实时追踪 Shift 键（crosshairMove 时用于吸附判断）
      state._lastShiftKey = e.shiftKey;
      // 忽略来自输入框的键盘事件（避免打字时误触工具）
      const tag = (e.target && e.target.tagName) || '';
      const isInput = tag === 'INPUT' || tag === 'TEXTAREA' || e.target?.isContentEditable;
      if (e.key === 'Escape') {
        if (state.drawDrag) { finishDrag(); return; }
        if (state.selectedDrawingId) { clearSelection(); return; }
        if (state.drawMode) exitDrawMode();
        else if (!drawer.hidden) closeSettings();
        else if (!cmdPop?.hidden) closeCmd();
      } else if ((e.metaKey || e.ctrlKey) && e.key === ',') {
        e.preventDefault();
        drawer.hidden ? openSettings() : closeSettings();
      } else if (!isInput && !e.metaKey && !e.ctrlKey && !e.altKey) {
        // 画线工具快捷键（仅非输入焦点时生效）
        const k = e.key.toLowerCase();
        if (k === 'h') { e.preventDefault(); enterDrawMode('hline'); }
        else if (k === 't') { e.preventDefault(); enterDrawMode('trendline'); }
        else if (k === 'm') { e.preventDefault(); enterDrawMode('measure'); }
        else if (e.key === 'Delete' || e.key === 'Backspace') {
          if (state.selectedDrawingId) {
            e.preventDefault();
            deleteDrawingById(state.selectedDrawingId);
            state.selectedDrawingId = null;
          }
        }
      }
    });
    document.addEventListener('keyup', (e) => {
      state._lastShiftKey = e.shiftKey;
    });
    // 全局 mouseup → 结束拖拽（LWC 没有 mouseup 事件，靠 document 兜底）
    document.addEventListener('mouseup', () => {
      if (state.drawDrag) finishDrag();
    });
  }

  // ==================== 趋势叠加渲染 ====================
  function clearTrendOverlays() {
    for (const s of state.trendLineSeries) {
      try { state.chart.removeSeries(s); } catch (_) { /* noop */ }
    }
    state.trendLineSeries = [];
    for (const pl of state.srPriceLines) {
      try { state.candleSeries.removePriceLine(pl); } catch (_) { /* noop */ }
    }
    state.srPriceLines = [];
    // 清空 S/R HTML 图例
    const leg = $('sr-legend');
    if (leg) leg.innerHTML = '';
  }

  function applyTrend(trendState) {
    state.currentTrend = trendState;
    clearTrendOverlays();
    if (!$('show-trend').checked) {
      // 仅合并形态标记，不画趋势线/支撑阻力
      applyPatternMarkers(state.currentPatterns);
      // 黄金分割独立于趋势叠加开关
      applyFibLevels();
      return;
    }

    // 1) 趋势线：从 p1 画到最后一根 K 线外推
    const klines = state.currentKlines;
    const lastIdx = klines.length - 1;
    const tp = state.trendParts || { trendline: true, sr: true, swings: true };
    if (tp.trendline) for (const line of trendState.trend_lines || []) {
      // 趋势线保持视觉明显：饱和色 + 合适线宽；但 autoscaleInfoProvider 让它不影响 Y 轴
      const color = line.kind === 'Resistance' ? BEAR_COLOR : BULL_COLOR;
      const s = state.chart.addLineSeries({
        color, lineWidth: 2,
        lineStyle: 0, // 实线（趋势线 vs S/R 虚线，两者通过线型区分）
        lastValueVisible: false, priceLineVisible: false,
        crosshairMarkerVisible: false,
        // 关键：不让此辅助线参与 Y 轴自适应，避免 K 线被压扁
        autoscaleInfoProvider: () => ({ priceRange: null }),
      });
      const p1Idx = line.p1_index;
      const p1Time = Math.floor(line.p1_time / 1000);
      const p1Price = line.p1_price;
      const slope = line.slope_per_bar;
      // 外推 3 根（而不是 20）——避免时间轴为空而被拉宽
      const endIdx = lastIdx + 3;
      let endPrice = p1Price + slope * (endIdx - p1Idx);
      // 价格 clamp：防止极端斜率把线拉到离谱位置（虽然 autoscale 已经不影响 Y 轴，但视觉上也别太飘）
      const curClose = klines[lastIdx]?.close ?? p1Price;
      const priceMax = curClose * 1.15;
      const priceMin = curClose * 0.85;
      let endTime = klines[lastIdx]
        ? Math.floor(klines[lastIdx].open_time / 1000) + 3 * tfSeconds()
        : p1Time + 1;
      if (Math.abs(slope) > 1e-12 && (endPrice > priceMax || endPrice < priceMin)) {
        endPrice = endPrice > priceMax ? priceMax : priceMin;
      }
      s.setData([
        { time: p1Time, value: p1Price },
        { time: endTime, value: endPrice },
      ]);
      state.trendLineSeries.push(s);
    }

    // 2) S/R 水平位：**局部水平段 + ±0.3% 色带 + 柔色 + 左上 legend**
    //    原则：更简单（仅最强 1 阻力 + 1 支撑）、更明显（色带 + 柔色）、更克制（只在有效区段显示）
    //    ⚠️ 角色翻转：根据"当前价 vs 位价"动态判定（支撑跌破→变阻力，阻力突破→变支撑）
    if (tp.sr) {
    const SR_SOFT_R = 'rgb(229, 115, 115)';  // 柔粉红（阻力）
    const SR_SOFT_S = 'rgb(102, 187, 155)';  // 柔青绿（支撑）
    const SR_BAND_HALF_PCT = 0.003;          // 色带宽度 ±0.3%
    const SR_BAND_ALPHA = 0.10;              // 色带边缘透明度（降调：0.22 → 0.10）

    const currentPrice = klines[lastIdx]?.close ?? 0;
    // 按当前价动态重新分类为阻力（高于当前价）/ 支撑（低于当前价）
    const annotated = (trendState.sr_levels || []).map((l) => ({
      ...l,
      effectiveKind: l.price > currentPrice ? 'Resistance' : 'Support',
      // 距离当前价的相对比例（越近越重要）
      distPct: Math.abs(l.price - currentPrice) / Math.max(currentPrice, 1e-9),
    }));
    // 排序：先看 touches 降序，再看 distPct 升序（近的优先）
    const sortSR = (a, b) => {
      const t = (b.touches || 0) - (a.touches || 0);
      if (t !== 0) return t;
      return a.distPct - b.distPct;
    };
    // 阻力：高于当前价的最多 top-3（按 price 升序渲染"近→远"）
    const resistAll = annotated.filter((l) => l.effectiveKind === 'Resistance').sort(sortSR).slice(0, 3);
    const supportAll = annotated.filter((l) => l.effectiveKind === 'Support').sort(sortSR).slice(0, 3);
    // legend 里"近的放前面"：阻力升序（近→远），支撑降序（近→远）
    const resistByPrice = resistAll.slice().sort((a, b) => a.price - b.price);
    const supportByPrice = supportAll.slice().sort((a, b) => b.price - a.price);

    const tfSec = tfSeconds();
    const lastBarTime = klines[lastIdx] ? Math.floor(klines[lastIdx].open_time / 1000) : 0;

    // 每方向的分层透明度：整体降调，避免视觉过载
    const RANK_ALPHA = [0.6, 0.4, 0.25];
    const RANK_WIDTH = [1.5, 1, 1];

    const renderLevel = (lvl, rankIdx) => {
      const isR = lvl.effectiveKind === 'Resistance';
      const rgb = isR ? '229, 115, 115' : '102, 187, 155';
      const alpha = RANK_ALPHA[rankIdx] ?? 0.3;
      const width = RANK_WIDTH[rankIdx] ?? 1;
      const midColor = `rgba(${rgb}, ${alpha})`;
      // 起点：first_touch 前 1 根
      const fi = Math.max(0, (lvl.first_touch_index ?? 0) - 1);
      const startK = klines[fi];
      if (!startK) return;
      const startTime = Math.floor(startK.open_time / 1000);
      const li = Math.min(klines.length - 1, lvl.last_touch_index ?? lastIdx);
      const liTime = Math.floor(klines[li].open_time / 1000);
      // 外推 3 根（与趋势线统一）—— 避免时间轴被拉宽产生 ghost bar 标签
      const endTime = Math.max(liTime, lastBarTime) + 3 * tfSec;

      // 仅最强（rankIdx=0）画 ±0.3% 色带
      if (rankIdx === 0) {
        const bandColor = `rgba(${rgb}, ${SR_BAND_ALPHA})`;
        const upper = state.chart.addLineSeries({
          color: bandColor, lineWidth: 1, lineStyle: 2,
          lastValueVisible: false, priceLineVisible: false, crosshairMarkerVisible: false,
          autoscaleInfoProvider: () => ({ priceRange: null }),
        });
        upper.setData([
          { time: startTime, value: lvl.price * (1 + SR_BAND_HALF_PCT) },
          { time: endTime,   value: lvl.price * (1 + SR_BAND_HALF_PCT) },
        ]);
        state.trendLineSeries.push(upper);
        const lower = state.chart.addLineSeries({
          color: bandColor, lineWidth: 1, lineStyle: 2,
          lastValueVisible: false, priceLineVisible: false, crosshairMarkerVisible: false,
          autoscaleInfoProvider: () => ({ priceRange: null }),
        });
        lower.setData([
          { time: startTime, value: lvl.price * (1 - SR_BAND_HALF_PCT) },
          { time: endTime,   value: lvl.price * (1 - SR_BAND_HALF_PCT) },
        ]);
        state.trendLineSeries.push(lower);
      }

      // 主线：分层透明度 + 线宽（不参与 Y 轴自适应）
      const mid = state.chart.addLineSeries({
        color: midColor, lineWidth: width, lineStyle: 2,
        lastValueVisible: false, priceLineVisible: false, crosshairMarkerVisible: false,
        autoscaleInfoProvider: () => ({ priceRange: null }),
      });
      mid.setData([
        { time: startTime, value: lvl.price },
        { time: endTime,   value: lvl.price },
      ]);
      state.trendLineSeries.push(mid);
    };

    // 渲染：**图表只画每方向最强 1 条（共 2 条）**，保持最大克制
    //        legend 文字保留全部 top-3（共 6 条）供交易员细节参考
    resistAll.slice(0, 1).forEach((l, i) => renderLevel(l, i));
    supportAll.slice(0, 1).forEach((l, i) => renderLevel(l, i));

    // legend：按"近→远"展示（不按强度排序，符合交易员阅读习惯）
    //         按距离给 3 档命名：近端 / 中端 / 远端
    const pickLabel = (kind, totalCount, idx) => {
      const suffix = kind === 'Resistance' ? '阻力' : '支撑';
      if (totalCount <= 1) return suffix;
      if (totalCount === 2) return (idx === 0 ? '近端' : '远端') + suffix;
      // totalCount >= 3
      if (idx === 0) return '近端' + suffix;
      if (idx === totalCount - 1) return '远端' + suffix;
      return '中端' + suffix;
    };
    const legendParts = [];
    resistByPrice.forEach((lvl, i) => {
      const cls = 'r';
      const label = pickLabel('Resistance', resistByPrice.length, i);
      legendParts.push(
        `<div class="sr-row"><span class="sr-tag ${cls}">${label}</span>` +
        `<span class="sr-price">${fmtPrice(lvl.price)}</span>` +
        `<span class="sr-touches">✱${lvl.touches || 0}</span></div>`
      );
    });
    supportByPrice.forEach((lvl, i) => {
      const cls = 's';
      const label = pickLabel('Support', supportByPrice.length, i);
      legendParts.push(
        `<div class="sr-row"><span class="sr-tag ${cls}">${label}</span>` +
        `<span class="sr-price">${fmtPrice(lvl.price)}</span>` +
        `<span class="sr-touches">✱${lvl.touches || 0}</span></div>`
      );
    });
    // 更新 HTML 图例（放在左上 MA overlay 下方）
    const legendEl = $('sr-legend');
    if (legendEl) legendEl.innerHTML = legendParts.join('');
    } // end if (tp.sr)

    // 3) 摆动点 + 形态合并为 markers
    const swingMarkers = tp.swings
      ? (trendState.swings || []).map((sw) => ({
          time: Math.floor(sw.time / 1000),
          position: sw.kind === 'High' ? 'aboveBar' : 'belowBar',
          color: sw.kind === 'High' ? BEAR_COLOR : BULL_COLOR,
          shape: 'circle',
          text: sw.kind === 'High' ? 'H' : 'L',
          size: 0,
        }))
      : [];
    // 和形态标注合并
    applyPatternMarkers(state.currentPatterns, swingMarkers);

    // 叠加黄金分割
    applyFibLevels();
  }

  // ==================== 黄金分割（Fibonacci 回撤）====================
  //
  // 选取最近一对显著摆动高/低（从 currentTrend.swings 取），计算标准回撤水平：
  //   0.0 / 0.236 / 0.382 / 0.5 / 0.618（重点）/ 0.786 / 1.0
  // 以水平 priceLine 形式画在主图上；0.618 用金色加粗以突出"黄金分割"位。
  const FIB_LEVELS = [
    { ratio: 0.0,   color: hexToRgba(MUTED_COLOR, 0.55), style: 3, width: 1, label: '0.0' },
    { ratio: 0.236, color: hexToRgba(MUTED_COLOR, 0.55), style: 3, width: 1, label: '0.236' },
    { ratio: 0.382, color: hexToRgba(MUTED_COLOR, 0.6),  style: 2, width: 1, label: '0.382' },
    { ratio: 0.5,   color: hexToRgba(MUTED_COLOR, 0.6),  style: 2, width: 1, label: '0.5' },
    { ratio: 0.618, color: cssVar('--accent', '#cc785c'), style: 0, width: 2, label: '0.618 ★' },
    { ratio: 0.786, color: hexToRgba(MUTED_COLOR, 0.6),  style: 2, width: 1, label: '0.786' },
    { ratio: 1.0,   color: hexToRgba(MUTED_COLOR, 0.55), style: 3, width: 1, label: '1.0' },
  ];

  function clearFibLevels() {
    for (const pl of state.fibPriceLines) {
      try { state.candleSeries.removePriceLine(pl); } catch (_) { /* noop */ }
    }
    state.fibPriceLines = [];
  }

  function applyFibLevels() {
    clearFibLevels();
    if (!$('show-fib')?.checked) return;
    if (!state.candleSeries) return;

    // 数据来源：趋势引擎识别出的摆动点（按时间升序）
    const swings = state.currentTrend?.swings || [];
    if (swings.length < 2) return;

    // 取最后一个 High 和最后一个 Low（方向由先后决定）
    let lastHigh = null;
    let lastLow = null;
    for (let i = swings.length - 1; i >= 0; i--) {
      const sw = swings[i];
      if (sw.kind === 'High' && !lastHigh) lastHigh = sw;
      if (sw.kind === 'Low' && !lastLow) lastLow = sw;
      if (lastHigh && lastLow) break;
    }
    if (!lastHigh || !lastLow) return;

    const hi = lastHigh.price;
    const lo = lastLow.price;
    if (!(hi > lo)) return; // 异常数据保护

    // 方向：最近一个枢轴更新：若 High 更晚 → 下跌回撤（从高到低），0.0=high，1.0=low
    //                         若 Low 更晚  → 上涨回撤（从低到高），0.0=low ，1.0=high
    const highLatest = lastHigh.time >= lastLow.time;
    const anchor0 = highLatest ? hi : lo;
    const anchor1 = highLatest ? lo : hi;
    const range = anchor1 - anchor0; // 可正可负

    for (const lvl of FIB_LEVELS) {
      const price = anchor0 + range * lvl.ratio;
      const pl = state.candleSeries.createPriceLine({
        price,
        color: lvl.color,
        lineWidth: lvl.width,
        lineStyle: lvl.style, // 0 Solid / 2 Dashed / 3 Dotted
        axisLabelVisible: true,
        title: `FIB ${lvl.label}`,
      });
      state.fibPriceLines.push(pl);
    }
  }

  function tfSeconds() {
    const tfMap = { '1m': 60, '5m': 300, '15m': 900, '30m': 1800, '1h': 3600, '4h': 14400, '1d': 86400, '1w': 604800 };
    return tfMap[$('interval').value] || 3600;
  }

  function renderResonance(rs) {
    const score = rs.score;
    const sug = rs.suggestion;
    // 立场 Badge
    const stanceCls = {
      StrongBull: 'strong-bull',
      Bull: 'bull',
      WeakBull: 'weak-bull',
      Neutral: 'neutral',
      WeakBear: 'weak-bear',
      Bear: 'bear',
      StrongBear: 'strong-bear',
    }[score.stance] || 'neutral';
    const badge = $('stance-badge');
    badge.textContent = score.stance_label + ` · ${(score.alignment * 100).toFixed(0)}%`;
    badge.className = 'rh-stance ' + stanceCls;
    // 大分
    const big = $('score-big');
    big.textContent = `${score.total >= 0 ? '+' : ''}${score.total.toFixed(1)}`;
    big.className = 'rh-score-big ' + (score.total > 10 ? 'bull' : score.total < -10 ? 'bear' : '');
    // 进度条
    const bar = $('score-bar-fill');
    const pct = Math.min(100, Math.abs(score.total));
    if (score.total >= 0) {
      bar.style.left = '50%';
      bar.style.right = `${50 - pct / 2}%`;
      bar.style.background = 'var(--bull)';
    } else {
      bar.style.right = '50%';
      bar.style.left = `${50 - pct / 2}%`;
      bar.style.background = 'var(--bear)';
    }
    // 四个维度（防御：若 dimensions 为空/undefined，安全跳过）
    const dimsEl = $('dims-grid');
    if (dimsEl) {
      dimsEl.innerHTML = '';
      const dims = Array.isArray(score.dimensions) ? score.dimensions : [];
      for (const d of dims) {
        const el = document.createElement('div');
        el.className = 'dim-row';
        const cls = d.score > 3 ? 'bull' : d.score < -3 ? 'bear' : '';
        const scoreVal = Number.isFinite(d.score) ? d.score : 0;
        const weightVal = Number.isFinite(d.weight) ? d.weight : 0;
        const left = document.createElement('span');
        left.className = 'dim-left';
        const name = document.createElement('span');
        name.className = 'dim-name';
        name.textContent = d.name || '—';
        const weight = document.createElement('span');
        weight.className = 'dim-weight';
        weight.textContent = `w=${weightVal.toFixed(2)}`;
        const scoreNode = document.createElement('span');
        scoreNode.className = `dim-score ${cls}`;
        scoreNode.textContent = `${scoreVal >= 0 ? '+' : ''}${scoreVal.toFixed(1)}`;
        left.appendChild(name);
        left.appendChild(weight);
        el.appendChild(left);
        el.appendChild(scoreNode);
        el.title = Array.isArray(d.contributions) ? d.contributions.join('\n') : '';
        dimsEl.appendChild(el);
      }
    }
    // 建议
    const sb = $('sug-box');
    const dir = sug.direction > 0 ? '做多 ⬆' : sug.direction < 0 ? '做空 ⬇' : '观望';
    const dirCls = sug.direction > 0 ? 'bull' : sug.direction < 0 ? 'bear' : '';
    sb.innerHTML = `
      <div class="sug-main ${dirCls}">${dir} · 信心 ${(sug.confidence * 100).toFixed(0)}%</div>
      <div class="sug-metric"><span>入场</span><span class="v">${fmtPrice(sug.entry_price)}</span></div>
      <div class="sug-metric"><span>止损 / 止盈</span><span class="v">${fmtPrice(sug.stop_loss)} / ${fmtPrice(sug.take_profit)}</span></div>
      <div class="sug-metric"><span>建议仓位</span><span class="v">${sug.suggested_position_size.toFixed(4)} (${sug.suggested_notional.toFixed(0)} USD)</span></div>
      <div class="sug-metric"><span>风险 / 收益</span><span class="v">${sug.risk_amount.toFixed(2)} / ${sug.reward_amount.toFixed(2)} (1:${sug.rr_ratio})</span></div>
    `;
  }

  function chartPatternId(p) {
    return `${p.kind}-${p.completion_index}`;
  }

  function clearAllChartPatternOverlays() {
    for (const [, ov] of state.chartPatternOverlays) {
      for (const s of ov.series) {
        try { state.chart.removeSeries(s); } catch (_) { /* noop */ }
      }
      for (const pl of ov.priceLines) {
        try { state.candleSeries.removePriceLine(pl); } catch (_) { /* noop */ }
      }
      if (ov.li) ov.li.classList.remove('active');
    }
    state.chartPatternOverlays.clear();
  }

  function drawChartPatternOverlay(p) {
    const id = chartPatternId(p);
    if (!p.points || p.points.length < 2) return null;
    // 基础颜色：看涨绿 / 看跌红 / 中性黄
    const color = p.direction > 0 ? BULL_COLOR : p.direction < 0 ? BEAR_COLOR : WARN_COLOR;

    // 1) 连接摆动点的折线（Lightweight Charts 要求 time 升序，SwingPoint 本身已升序）
    const pts = p.points
      .map((sp) => ({ time: Math.floor(sp.time / 1000), value: sp.price }))
      .sort((a, b) => a.time - b.time);
    // 过滤同一秒重复 time（极少发生但会报错）
    const deduped = [];
    for (const pt of pts) {
      if (!deduped.length || deduped[deduped.length - 1].time !== pt.time) deduped.push(pt);
    }
    const polyline = state.chart.addLineSeries({
      color,
      lineWidth: 2,
      lineStyle: 0,           // 实线
      lastValueVisible: false,
      priceLineVisible: false,
      crosshairMarkerVisible: false,
      title: `${p.label}`,
    });
    polyline.setData(deduped);

    const series = [polyline];
    const priceLines = [];

    // 2) 颈线：水平 price line
    if (p.neckline != null && isFinite(p.neckline)) {
      priceLines.push(state.candleSeries.createPriceLine({
        price: p.neckline,
        color,
        lineWidth: 1,
        lineStyle: 2, // dashed
        axisLabelVisible: true,
        title: '颈线',
      }));
    }

    // 3) 目标价：水平 dotted line
    if (p.target_price != null && isFinite(p.target_price)) {
      priceLines.push(state.candleSeries.createPriceLine({
        price: p.target_price,
        color,
        lineWidth: 1,
        lineStyle: 3, // dotted
        axisLabelVisible: true,
        title: '目标',
      }));
    }

    const overlay = { series, priceLines };
    state.chartPatternOverlays.set(id, overlay);
    return overlay;
  }

  function removeChartPatternOverlay(id) {
    const ov = state.chartPatternOverlays.get(id);
    if (!ov) return;
    for (const s of ov.series) {
      try { state.chart.removeSeries(s); } catch (_) { /* noop */ }
    }
    for (const pl of ov.priceLines) {
      try { state.candleSeries.removePriceLine(pl); } catch (_) { /* noop */ }
    }
    state.chartPatternOverlays.delete(id);
  }

  function toggleChartPatternOverlay(p, li) {
    const id = chartPatternId(p);
    if (state.chartPatternOverlays.has(id)) {
      const ov = state.chartPatternOverlays.get(id);
      removeChartPatternOverlay(id);
      if (ov && ov.li) ov.li.classList.remove('active');
      li.classList.remove('active');
    } else {
      const ov = drawChartPatternOverlay(p);
      if (ov) {
        ov.li = li;
        li.classList.add('active');
      }
    }
  }

  function renderChartPatterns(patterns) {
    const ul = $('chart-pattern-list');
    // 重新渲染前：清空所有既有 overlay（摆动点/索引随数据刷新可能失效）
    clearAllChartPatternOverlays();
    ul.innerHTML = '';
    if (!patterns || !patterns.length) {
      ul.innerHTML = '<li class="dim">未识别到技术图形</li>';
      return;
    }
    const recent = [...patterns].sort((a, b) => b.completion_index - a.completion_index).slice(0, 15);
    for (const p of recent) {
      const li = document.createElement('li');
      li.className = (p.direction > 0 ? 'buy' : p.direction < 0 ? 'sell' : 'dim') + ' clickable';
      const stars = '★'.repeat(p.strength);
      const lastPoint = p.points && p.points.length ? p.points[p.points.length - 1] : null;
      const ts = lastPoint ? fmtTs(lastPoint.time) : '';
      const stats = statsFor(p.label);
      let badge = '';
      let tooltip = `${p.label}（强度 ${stars}） · 点击在图表上显示/隐藏`;
      if (stats) {
        const style = RANK_STYLE[stats.rank] || {};
        badge = ` <span class="rank-badge" style="color:${style.color};border-color:${style.color}">${stats.rank}</span>`;
        tooltip = `${p.label}\n评级: ${stats.rank}\n历史胜率: ${stats.hit}\n历史 alpha: ${stats.alpha}` + (stats.note ? `\n备注: ${stats.note}` : '') + '\n\n点击在图表上显示/隐藏';
      }
      li.setAttribute('title', tooltip);
      li.innerHTML = `<span>${p.label} ${stars}${badge}</span><span class="sig-time">${ts}</span>`;
      li.addEventListener('click', () => toggleChartPatternOverlay(p, li));
      ul.appendChild(li);
    }
  }

  function renderTrendPanel(trend) {
    const phaseEl = $('dow-phase');
    const phase = trend.dow?.phase || 'Unknown';
    const labels = { Uptrend: '上升趋势 ▲', Downtrend: '下降趋势 ▼', Consolidation: '整固 / 盘整 ─', Unknown: '—' };
    phaseEl.textContent = labels[phase] || '—';
    phaseEl.className = 'v ' + (phase === 'Uptrend' ? 'bull' : phase === 'Downtrend' ? 'bear' : '');
    $('dow-age').textContent = `${trend.dow?.structure_age_bars || 0} bars since last swing`;
    $('channel-pos').textContent = trend.channel_position != null
      ? `${(trend.channel_position * 100).toFixed(0)}%` + (trend.channel_position > 0.85 ? ' ⚠上轨' : trend.channel_position < 0.15 ? ' ⚠下轨' : '')
      : '无通道';
    $('sr-count').textContent = `${(trend.sr_levels || []).length} 条`;
    const unfilledGaps = (trend.gaps || []).filter((g) => !g.filled).length;
    $('gap-count').textContent = `${unfilledGaps} / ${(trend.gaps || []).length}`;
  }

  // ==================== 回测工作台 ====================
  const btState = {
    chart: null,
    equitySeries: null,
    ddSeries: null,
  };

  function initBacktestChart() {
    const el = $('bt-equity');
    if (btState.chart) return;
    if (!el || el.clientWidth === 0) return;
    el.innerHTML = '';
    const chart = LightweightCharts.createChart(el, {
      layout: {
        background: { type: 'solid', color: cssVar('--surface', '#ffffff') },
        textColor: cssVar('--text-dim', '#6b6558'),
        fontSize: 11,
        fontFamily: cssVar('--font-sans', 'system-ui, sans-serif'),
      },
      grid: {
        vertLines: { color: cssVar('--border', '#e8e6e0') },
        horzLines: { color: cssVar('--border', '#e8e6e0') },
      },
      rightPriceScale: { borderColor: cssVar('--border-strong', '#d4d0c4') },
      leftPriceScale: { borderColor: cssVar('--border-strong', '#d4d0c4'), visible: true, scaleMargins: { top: 0.7, bottom: 0 } },
      timeScale: { borderColor: cssVar('--border-strong', '#d4d0c4'), timeVisible: true, secondsVisible: false },
      crosshair: { mode: 0 },
      height: 320,
      width: el.clientWidth,
    });
    const equityColor = cssVar('--accent', '#cc785c');
    const equitySeries = chart.addAreaSeries({
      lineColor: equityColor, topColor: hexToRgba(equityColor, 0.35), bottomColor: hexToRgba(equityColor, 0.0),
      lineWidth: 2, priceScaleId: 'right', title: '权益',
    });
    const ddSeries = chart.addHistogramSeries({
      color: hexToRgba(cssVar('--bear', '#b85c56'), 0.6), priceScaleId: 'left', priceFormat: { type: 'percent' },
      title: '回撤',
    });
    window.addEventListener('resize', () => {
      chart.applyOptions({ width: el.clientWidth });
    });
    btState.chart = chart;
    btState.equitySeries = equitySeries;
    btState.ddSeries = ddSeries;
  }

  function fmtMoney(v) {
    if (!isFinite(v)) return '—';
    return v.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  }

  function classifyMetric(key, v) {
    const bad = key === 'max_drawdown_pct' ? v > 0.15 : v < 0;
    const good = key === 'max_drawdown_pct' ? v < 0.1 : v > 0;
    if (isFinite(v) && good) return 'good';
    if (isFinite(v) && bad) return 'bad';
    return '';
  }

  function renderMetrics(p) {
    const rows = [
      ['总收益', fmtPct(p.total_return_pct), classifyMetric('total_return_pct', p.total_return_pct)],
      ['年化收益', fmtPct(p.annualized_return_pct), classifyMetric('annualized_return_pct', p.annualized_return_pct)],
      ['最大回撤', fmtPct(p.max_drawdown_pct), classifyMetric('max_drawdown_pct', p.max_drawdown_pct)],
      ['回撤持续', `${p.max_drawdown_duration_bars} bars`, ''],
      ['胜率', fmtPct(p.win_rate), classifyMetric('win_rate', p.win_rate - 0.5)],
      ['盈亏比', isFinite(p.profit_factor) ? p.profit_factor.toFixed(2) : '∞', classifyMetric('pf', p.profit_factor - 1)],
      ['期望 R', p.expectancy_r.toFixed(3) + 'R', classifyMetric('er', p.expectancy_r)],
      ['Sharpe', p.sharpe.toFixed(2), classifyMetric('sh', p.sharpe - 1)],
      ['Sortino', p.sortino.toFixed(2), classifyMetric('st', p.sortino - 1)],
      ['Calmar', isFinite(p.calmar) ? p.calmar.toFixed(2) : '∞', classifyMetric('cl', p.calmar - 0.5)],
      ['总交易数', p.total_trades, ''],
      ['胜 / 负', `${p.wins} / ${p.losses}`, ''],
      ['最大连胜', p.max_consec_wins, ''],
      ['最大连亏', p.max_consec_losses, ''],
      ['平均盈利', fmtMoney(p.avg_win), 'good'],
      ['平均亏损', fmtMoney(-p.avg_loss), 'bad'],
      ['平均持仓', p.avg_hold_bars.toFixed(1) + ' bars', ''],
    ];
    const cont = $('bt-metrics');
    cont.innerHTML = '';
    for (const [k, v, cls] of rows) {
      const el = document.createElement('div');
      el.className = 'metric';
      el.innerHTML = `<span class="mk">${k}</span><span class="mv ${cls}">${v}</span>`;
      cont.appendChild(el);
    }
  }

  function renderPatternRanking(stats) {
    const tbody = $('bt-pattern-table').querySelector('tbody');
    tbody.innerHTML = '';
    stats.forEach((s, i) => {
      const tr = document.createElement('tr');
      const rCls = s.total_r > 0 ? 'pos' : 'neg';
      const avgCls = s.avg_r > 0 ? 'pos' : 'neg';
      tr.innerHTML = `
        <td>${i + 1}</td>
        <td>${s.label}</td>
        <td>${s.count}</td>
        <td>${(s.winrate * 100).toFixed(1)}%</td>
        <td class="${avgCls}">${s.avg_r >= 0 ? '+' : ''}${s.avg_r.toFixed(2)}R</td>
        <td class="${rCls}">${s.total_r >= 0 ? '+' : ''}${s.total_r.toFixed(2)}R</td>
      `;
      tbody.appendChild(tr);
    });
  }

  function renderTrades(trades) {
    const tbody = $('bt-trades-table').querySelector('tbody');
    tbody.innerHTML = '';
    // 最近 200 笔
    const recent = trades.slice(-200).reverse();
    for (const t of recent) {
      const rCls = t.r_multiple > 0 ? 'pos' : 'neg';
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>#${t.id}</td>
        <td class="${t.side === 'Long' ? 'pos' : 'neg'}">${t.side === 'Long' ? '多' : '空'}</td>
        <td>${fmtTs(t.entry_time)}</td>
        <td>${fmtPrice(t.entry_price)}</td>
        <td>${fmtPrice(t.stop_loss)}</td>
        <td>${fmtPrice(t.take_profit)}</td>
        <td>${t.exit_price ? fmtPrice(t.exit_price) : '—'}</td>
        <td class="${rCls}">${t.r_multiple >= 0 ? '+' : ''}${t.r_multiple.toFixed(2)}R</td>
        <td>${t.reasons.join(' + ')}</td>
        <td>${t.exit_reason || '—'}</td>
      `;
      tbody.appendChild(tr);
    }
  }

  // P0-5：空白引导的快捷预设
  // 参数名对应 bt-* 输入框
  const BT_PRESETS = {
    default:      { symbol: 'BTCUSDT', interval: '1d', risk: '0.02', rr: '2.0',  atr: '1.5' },
    conservative: { symbol: 'BTCUSDT', interval: '4h', risk: '0.01', rr: '2.0',  atr: '2.0' },
    aggressive:   { symbol: 'ETHUSDT', interval: '4h', risk: '0.02', rr: '3.0',  atr: '1.2' },
  };
  function applyBacktestPreset(key) {
    const pre = BT_PRESETS[key];
    if (!pre) return;
    $('bt-symbol').value = pre.symbol;
    $('bt-interval').value = pre.interval;
    $('bt-risk').value = pre.risk;
    $('bt-rr').value = pre.rr;
    $('bt-atr').value = pre.atr;
  }

  async function runBacktest() {
    const params = new URLSearchParams({
      symbol: $('bt-symbol').value,
      interval: $('bt-interval').value,
      limit: $('bt-limit').value,
      capital: $('bt-capital').value,
      risk: $('bt-risk').value,
      rr: $('bt-rr').value,
      atr_mult: $('bt-atr').value,
      min_strength: $('bt-strength').value,
      fee_bps: $('bt-fee').value,
      slip_bps: $('bt-slip').value,
      allow_short: $('bt-short').checked ? '1' : '0',
      // 将当前预设的均线周期 + 基准带入回测（保持与实时分析一致）
      periods: currentMaPeriods().join(','),
      base_period: $('cfg-base')?.value || '30',
    });
    const btn = $('bt-run');
    btn.disabled = true;
    btn.textContent = '⏳ 计算中…';
    $('bt-metrics').innerHTML = '<div class="metric"><span class="mk">请求发送中…</span></div>';
    // P0-5：切换空白引导状态（提前标记 has-data 以露出图表容器）
    const eqPanel = document.querySelector('.bt-equity-panel');
    if (eqPanel) eqPanel.classList.add('has-data');
    try {
      const result = await fetchJson(`/api/backtest/run?${params.toString()}`);
      initBacktestChart();
      // 权益曲线（每 N 个取样以避免过多点）
      const step = Math.max(1, Math.floor(result.equity.length / 1500));
      const eq = [];
      const dd = [];
      for (let i = 0; i < result.equity.length; i += step) {
        const p = result.equity[i];
        const t = Math.floor(p.time / 1000);
        eq.push({ time: t, value: p.equity });
        dd.push({ time: t, value: -p.drawdown * 100 }); // 负值显示回撤
      }
      btState.equitySeries.setData(eq);
      btState.ddSeries.setData(dd);
      btState.chart.timeScale().fitContent();

      renderMetrics(result.performance);
      renderPatternRanking(result.pattern_stats);
      renderTrades(result.trades);
    } catch (e) {
      console.error(e);
      $('bt-metrics').innerHTML = `<div class="metric"><span class="mk" style="color:var(--err)">回测失败: ${e.message}</span></div>`;
      // 回测失败 → 回退到空白引导
      if (eqPanel) eqPanel.classList.remove('has-data');
    } finally {
      btn.disabled = false;
      btn.textContent = '▶ 运行回测';
    }
  }

  // ---------- Sprint 13：Playbook 原书策略回测 ----------
  async function runPlaybookBacktest() {
    const btn = $('pb-run-btn');
    if (!btn) return;
    const symbol = $('symbol').value;
    const interval = $('interval').value;
    const limit = parseInt($('limit').value || '1000', 10);
    const strategy = $('pb-strategy').value || 'default';

    btn.disabled = true;
    btn.textContent = '⏳ 计算中…';
    const metricsEl = $('pb-metrics');
    metricsEl.innerHTML = '<div class="metric"><span class="mk">请求发送中…</span></div>';

    try {
      const resp = await fetchJson(
        `/api/backtest/playbook?symbol=${symbol}&interval=${interval}&limit=${limit}&strategy=${strategy}`,
      );
      const { strategy_name, book_source, result } = resp;
      const p = result.performance;

      const fmtPct = (v) => (v == null ? '—' : `${(v).toFixed(2)}%`);
      const fmtRate = (v) => (v == null ? '—' : `${(v * 100).toFixed(1)}%`);
      const fmtNum = (v, d = 2) => (v == null ? '—' : v.toFixed(d));

      metricsEl.innerHTML = `
        <div class="metric"><span class="mk">策略</span><span class="mv">${strategy_name}</span></div>
        <div class="metric"><span class="mk">原书出处</span><span class="mv">${book_source}</span></div>
        <div class="metric"><span class="mk">总收益率</span><span class="mv" style="color:${p.total_return_pct >= 0 ? 'var(--ok)' : 'var(--err)'}">${fmtPct(p.total_return_pct)}</span></div>
        <div class="metric"><span class="mk">胜率</span><span class="mv">${fmtRate(p.win_rate)}</span></div>
        <div class="metric"><span class="mk">最大回撤</span><span class="mv" style="color:var(--err)">${fmtPct(p.max_drawdown_pct)}</span></div>
        <div class="metric"><span class="mk">总交易数</span><span class="mv">${p.total_trades}</span></div>
        <div class="metric"><span class="mk">盈利</span><span class="mv" style="color:var(--ok)">${p.wins}</span></div>
        <div class="metric"><span class="mk">亏损</span><span class="mv" style="color:var(--err)">${p.losses}</span></div>
        <div class="metric"><span class="mk">盈亏比 PF</span><span class="mv">${fmtNum(p.profit_factor)}</span></div>
        <div class="metric"><span class="mk">Sharpe</span><span class="mv">${fmtNum(p.sharpe)}</span></div>
        <div class="metric"><span class="mk">期望 R</span><span class="mv">${fmtNum(p.expectancy_r)}</span></div>
        <div class="metric"><span class="mk">K 线数</span><span class="mv">${result.bars}</span></div>
      `;
    } catch (e) {
      metricsEl.innerHTML = `<div class="metric"><span class="mk" style="color:var(--err)">回测失败: ${e.message}</span></div>`;
    } finally {
      btn.disabled = false;
      btn.textContent = '运行单策略';
    }
  }

  // ---------- P0-4：Playbook 对比模式 ----------
  // 一次性跑完 5 个策略，按总收益率降序排，展示对比表 + 推荐冠军
  const PLAYBOOK_STRATEGIES = [
    { key: 'default',      label: '组合策略',        book: '三书综合' },
    { key: 'guillotine',   label: '断头铡刀清仓',    book: 'ma p.380' },
    { key: 'scallions',    label: '旱地拔葱轻仓',    book: 'ma p.340' },
    { key: 'staged_exit',  label: '三次减仓',        book: 'candle p.605' },
    { key: 'trend_matrix', label: '多级趋势线矩阵',  book: 'trend p.216' },
  ];

  async function runPlaybookCompare() {
    const btn = $('pb-compare-btn');
    if (!btn) return;
    const symbol = $('symbol').value;
    const interval = $('interval').value;
    const limit = parseInt($('limit').value || '1000', 10);

    btn.disabled = true;
    const origText = btn.textContent;
    btn.textContent = '⏳ 并行计算 5 个策略…';

    const wrap = $('pb-compare-wrap');
    const tbody = $('pb-compare-table').querySelector('tbody');
    const summaryEl = $('pb-compare-summary');
    wrap.style.display = 'block';
    tbody.innerHTML = `<tr><td colspan="9" style="text-align:center;color:var(--text-dim);">正在并发运行 ${PLAYBOOK_STRATEGIES.length} 个策略…</td></tr>`;
    summaryEl.innerHTML = '';

    const makeUrl = (s) => `/api/backtest/playbook?symbol=${symbol}&interval=${interval}&limit=${limit}&strategy=${s}`;

    try {
      const results = await Promise.all(
        PLAYBOOK_STRATEGIES.map((s) =>
          fetchJson(makeUrl(s.key))
            .then((r) => ({ ok: true, key: s.key, meta: s, resp: r }))
            .catch((e) => ({ ok: false, key: s.key, meta: s, error: e.message })),
        ),
      );

      // 按总收益率降序
      const scored = results.map((r) => {
        if (!r.ok) return { ...r, score: -Infinity };
        const p = r.resp.result.performance;
        return { ...r, score: p.total_return_pct ?? -Infinity, perf: p };
      });
      scored.sort((a, b) => b.score - a.score);

      // 渲染表格
      tbody.innerHTML = '';
      scored.forEach((row, rank) => {
        const tr = document.createElement('tr');
        if (!row.ok) {
          tr.innerHTML = `<td>${row.meta.label}</td><td colspan="8" style="color:var(--err);">❌ ${row.error}</td>`;
          tr.classList.add('loser');
          tbody.appendChild(tr);
          return;
        }
        const p = row.perf;
        if (rank === 0) tr.classList.add('winner');
        else if (rank === scored.length - 1 && scored.length > 2) tr.classList.add('loser');

        const retCls = p.total_return_pct >= 0 ? 'pos' : 'neg';
        const ddCls = 'neg';
        const shCls = p.sharpe >= 1 ? 'pos' : p.sharpe < 0 ? 'neg' : '';
        const erCls = p.expectancy_r >= 0 ? 'pos' : 'neg';
        const rankCls = rank === 0 ? 'rank-1' : rank === 1 ? 'rank-2' : '';

        tr.innerHTML = `
          <td><strong>${row.meta.label}</strong><br><span class="pb-book" style="font-size:10px;color:var(--text-dim);">${row.meta.book}</span></td>
          <td class="${retCls}">${p.total_return_pct >= 0 ? '+' : ''}${p.total_return_pct.toFixed(2)}%</td>
          <td>${(p.win_rate * 100).toFixed(1)}%</td>
          <td class="${ddCls}">${p.max_drawdown_pct.toFixed(2)}%</td>
          <td>${isFinite(p.profit_factor) ? p.profit_factor.toFixed(2) : '∞'}</td>
          <td class="${shCls}">${p.sharpe.toFixed(2)}</td>
          <td class="${erCls}">${p.expectancy_r.toFixed(2)}R</td>
          <td>${p.total_trades}</td>
          <td><span class="rank-num ${rankCls}">#${rank + 1}</span></td>
        `;
        tbody.appendChild(tr);
      });

      // 摘要
      const winners = scored.filter((r) => r.ok);
      if (winners.length === 0) {
        summaryEl.innerHTML = '❌ 所有策略均执行失败';
      } else {
        const best = winners[0];
        const worst = winners[winners.length - 1];
        summaryEl.innerHTML = `
          ✅ 推荐：<strong>${best.meta.label}</strong>（总收益 <strong>+${best.perf.total_return_pct.toFixed(2)}%</strong>，
          Sharpe ${best.perf.sharpe.toFixed(2)}，胜率 ${(best.perf.win_rate * 100).toFixed(1)}%）
          <br><span class="pb-book">📖 ${best.meta.book} · 样本 ${best.resp.result.bars} 根 K 线（${symbol} ${interval}）</span>
          <br><span style="color:var(--text-dim);font-size:11px;">
            最差策略：${worst.meta.label}（${worst.perf.total_return_pct.toFixed(2)}%） ·
            冠军相对最差 α = ${(best.perf.total_return_pct - worst.perf.total_return_pct).toFixed(2)}%
          </span>
        `;
      }
    } catch (e) {
      tbody.innerHTML = `<tr><td colspan="9" style="color:var(--err);text-align:center;">对比失败: ${e.message}</td></tr>`;
    } finally {
      btn.disabled = false;
      btn.textContent = origText;
    }
  }

  // ---------- Sprint A：指标有效性分析 ----------
  //
  // 调用 /api/effectiveness，展示"指标有效性排行榜"表格。
  // 列：# / arm / 类别 / 样本 / 胜率 / 平均收益 / Sharpe / α / 最大 / 最小 / 综合评分 / 原书
  //
  // 行样式：
  //   tier-1：综合评分最高且 n >= min_n
  //   tier-2：评分前 3 名且 n >= min_n
  //   low-sample：n < min_n（灰化，并沉到列表底部）

  async function runEffectiveness() {
    const btn = $('eff-run');
    if (!btn) return;
    const symbol = $('eff-symbol').value;
    const interval = $('eff-interval').value;
    const limit = parseInt($('eff-limit').value || '2000', 10);
    const horizon = parseInt($('eff-horizon').value || '20', 10);
    const minN = parseInt($('eff-min-n').value || '3', 10);

    const origText = btn.textContent;
    btn.disabled = true;
    btn.textContent = '⏳ 扫描历史…';
    const tbody = $('eff-table').querySelector('tbody');
    const statsEl = $('eff-stats');
    tbody.innerHTML = `<tr><td colspan="12" style="text-align:center;color:var(--text-dim);padding:18px;">正在扫描 ${limit} 根 K 线…</td></tr>`;
    statsEl.textContent = '评估中…';

    try {
      const resp = await fetchJson(
        `/api/effectiveness?symbol=${symbol}&interval=${interval}&limit=${limit}&horizon=${horizon}`,
      );
      renderEffectiveness(resp, minN);
    } catch (e) {
      console.error(e);
      tbody.innerHTML = `<tr><td colspan="12" style="color:var(--err);text-align:center;">评估失败: ${e.message}</td></tr>`;
      statsEl.textContent = '失败';
    } finally {
      btn.disabled = false;
      btn.textContent = origText;
    }
  }

  function renderEffectiveness(resp, minN) {
    const tbody = $('eff-table').querySelector('tbody');
    const statsEl = $('eff-stats');

    if (!resp || !Array.isArray(resp.rankings) || resp.rankings.length === 0) {
      tbody.innerHTML = `<tr><td colspan="12" style="text-align:center;color:var(--text-dim);padding:24px;">没有任何 arm 触发（尝试增大 K 线数或换交易对 / 周期）</td></tr>`;
      statsEl.innerHTML = `<strong>${resp.total_triggers || 0}</strong> 次触发 · 0 个 arm`;
      return;
    }

    // 分层：按 score 降序已排好；min_n 以下放到末尾
    const ok = resp.rankings.filter((r) => r.n >= minN);
    const lowSamples = resp.rankings.filter((r) => r.n < minN);
    const ordered = [...ok, ...lowSamples];

    const fmtPct = (v, d = 2) => (v == null || !isFinite(v) ? '—' : `${(v * 100).toFixed(d)}%`);
    const fmtPctRaw = (v, d = 2) => (v == null || !isFinite(v) ? '—' : `${v >= 0 ? '+' : ''}${v.toFixed(d)}%`);
    const fmtNum = (v, d = 2) => (v == null || !isFinite(v) ? '—' : (v >= 0 ? '+' : '') + v.toFixed(d));

    tbody.innerHTML = '';
    ordered.forEach((r, i) => {
      const tr = document.createElement('tr');
      if (r.n < minN) {
        tr.classList.add('low-sample');
      } else if (i === 0) {
        tr.classList.add('tier-1');
      } else if (i < 3) {
        tr.classList.add('tier-2');
      }

      const wrCls = r.win_rate >= 0.6 ? 'pos' : r.win_rate < 0.4 ? 'neg' : '';
      const retCls = r.avg_return_pct > 0 ? 'pos' : r.avg_return_pct < 0 ? 'neg' : '';
      const alphaCls = r.alpha_vs_market > 0 ? 'pos' : r.alpha_vs_market < 0 ? 'neg' : '';
      const sharpeCls = r.sharpe >= 0.5 ? 'pos' : r.sharpe < 0 ? 'neg' : '';
      const scoreCls = r.effectiveness_score > 0 ? 'pos' : r.effectiveness_score < 0 ? 'neg' : '';

      tr.innerHTML = `
        <td>${i + 1}</td>
        <td class="arm-name">
          <code>${escapeHtml(r.arm)}</code>
          <div class="arm-label">${escapeHtml(r.label)}</div>
        </td>
        <td><span class="eff-cat-badge ${r.category.toLowerCase()}">${r.category}</span></td>
        <td>${r.n} <span style="color:var(--text-dim);font-size:10px;">(${r.wins}W/${r.losses}L)</span></td>
        <td class="${wrCls}">${fmtPct(r.win_rate, 1)}</td>
        <td class="${retCls}">${fmtPctRaw(r.avg_return_pct, 2)}</td>
        <td class="${sharpeCls}">${fmtNum(r.sharpe)}</td>
        <td class="${alphaCls}">${fmtPctRaw(r.alpha_vs_market * 100, 2)}</td>
        <td class="pos">${fmtPctRaw(r.max_return * 100, 2)}</td>
        <td class="neg">${fmtPctRaw(r.min_return * 100, 2)}</td>
        <td class="${scoreCls}"><strong>${fmtNum(r.effectiveness_score)}</strong></td>
        <td class="eff-book">${escapeHtml(r.book_source || '—')}</td>
      `;
      tbody.appendChild(tr);
    });

    statsEl.innerHTML = `
      <strong>${resp.total_triggers}</strong> 次触发 · <strong>${resp.rankings.length}</strong> 个 arm<br>
      ${resp.bars} 根 K 线 · horizon=${resp.horizon}
    `;
  }

  // ---------- Sprint B：Bandit 面板 ----------

  async function refreshBandit({ autoTrainIfEmpty = false } = {}) {
    const tbody = $('bandit-table')?.querySelector('tbody');
    const statsEl = $('bandit-stats');
    if (!tbody || !statsEl) return;
    statsEl.textContent = '加载 Bandit state…';
    try {
      const resp = await fetchJson('/api/bandit/state');
      renderBandit(resp);
      // Sprint B 自动化 1/3：首次自动训练
      if (autoTrainIfEmpty && (resp.total_plays || 0) === 0) {
        statsEl.innerHTML += `<br><span style="color:var(--text-dim);">🤖 未训练过，自动开始首次训练…</span>`;
        await trainBandit();
      }
    } catch (e) {
      statsEl.innerHTML = `<span style="color:var(--err);">加载失败: ${e.message}</span>`;
      tbody.innerHTML = `<tr><td colspan="12" style="text-align:center;color:var(--err);">${e.message}</td></tr>`;
    }
  }

  async function trainBandit() {
    const btn = $('bandit-train');
    if (!btn) return;
    const symbol = $('eff-symbol').value;
    const interval = $('eff-interval').value;
    const limit = parseInt($('eff-limit').value || '2000', 10);
    const horizon = parseInt($('eff-horizon').value || '20', 10);
    const orig = btn.textContent;
    btn.disabled = true;
    btn.textContent = '⏳ 训练中…';
    try {
      const resp = await fetchJson(
        `/api/bandit/train?symbol=${symbol}&interval=${interval}&limit=${limit}&horizon=${horizon}&policy=thompson`,
        { method: 'POST' },
      );
      // train 响应 = state 响应超集，复用 renderBandit
      renderBandit({
        version: 1,
        total_plays: resp.after_plays,
        total_settled: resp.after_settled,
        pending: 0,
        arms: resp.arms,
      }, {
        banner: `✅ 训练完成：扫描 ${resp.triggers_scanned} 个触发点，更新 ${resp.arms_updated} 个 arm · total_plays ${resp.before_plays} → ${resp.after_plays}`,
      });
    } catch (e) {
      $('bandit-stats').innerHTML =
        `<span style="color:var(--err);">训练失败: ${e.message}</span>`;
    } finally {
      btn.disabled = false;
      btn.textContent = orig;
    }
  }

  async function resetBandit() {
    if (!confirm('确认清空 Bandit state？所有 α/β 后验会回到 Beta(1,1) 均匀先验。')) return;
    const btn = $('bandit-reset');
    const orig = btn.textContent;
    btn.disabled = true;
    btn.textContent = '⏳ …';
    try {
      await fetchJson('/api/bandit/reset', { method: 'POST' });
      await refreshBandit();
    } catch (e) {
      $('bandit-stats').innerHTML =
        `<span style="color:var(--err);">重置失败: ${e.message}</span>`;
    } finally {
      btn.disabled = false;
      btn.textContent = orig;
    }
  }

  // Wilson 95% 置信区间近似：使用 Beta(α, β) 的 Normal 近似
  // [mean - 2σ, mean + 2σ]
  function ci95(alpha, beta) {
    const s = alpha + beta;
    const mean = alpha / s;
    const variance = (alpha * beta) / (s * s * (s + 1));
    const sd = Math.sqrt(variance);
    return [Math.max(0, mean - 2 * sd), Math.min(1, mean + 2 * sd)];
  }

  function renderBandit(resp, opts = {}) {
    const tbody = $('bandit-table')?.querySelector('tbody');
    const statsEl = $('bandit-stats');
    if (!tbody || !statsEl) return;

    const banner = opts.banner
      ? `<span style="color:var(--bull);">${opts.banner}</span><br>`
      : '';
    statsEl.innerHTML = `
      ${banner}
      <strong>${resp.total_plays || 0}</strong> 次总触发 · 
      <strong>${resp.total_settled || 0}</strong> 次已结算 · 
      <strong>${resp.pending || 0}</strong> 待定 · 
      <strong>${resp.arms?.length || 0}</strong> 个 arm
    `;

    const arms = resp.arms || [];
    if (arms.length === 0) {
      tbody.innerHTML = `<tr><td colspan="12" style="text-align:center;color:var(--text-dim);padding:24px;">尚未训练。点击 🎓 训练 用历史数据填充 Beta 后验。</td></tr>`;
      return;
    }

    tbody.innerHTML = '';
    arms.forEach((a, i) => {
      const tr = document.createElement('tr');
      const mean = a.alpha / (a.alpha + a.beta);
      const [lo, hi] = ci95(a.alpha, a.beta);
      const n = a.total_wins + a.total_losses;
      const avgR = n > 0 ? a.cumulative_return_pct / n : 0;
      const meanCls = mean >= 0.6 ? 'pos' : mean < 0.4 ? 'neg' : '';
      const rCls = avgR > 0 ? 'pos' : avgR < 0 ? 'neg' : '';
      if (i === 0 && n > 5) tr.classList.add('tier-1');
      else if (i < 3 && n > 5) tr.classList.add('tier-2');

      tr.innerHTML = `
        <td>${i + 1}</td>
        <td class="arm-name">
          <code>${escapeHtml(a.name)}</code>
          <div class="arm-label">${escapeHtml(a.label || '')}</div>
        </td>
        <td><span class="eff-cat-badge ${String(a.category).toLowerCase()}">${a.category}</span></td>
        <td>${a.total_triggers}</td>
        <td>${a.total_wins}W/${a.total_losses}L${a.total_neutral ? `<span style="color:var(--text-dim);font-size:10px;"> +${a.total_neutral}N</span>` : ''}</td>
        <td>${a.alpha.toFixed(1)}</td>
        <td>${a.beta.toFixed(1)}</td>
        <td class="${meanCls}">
          <div class="mean-pill">
            <span>${(mean * 100).toFixed(1)}%</span>
            <div class="bar" style="--mean-pos: ${(mean * 100).toFixed(1)}%;"></div>
          </div>
        </td>
        <td class="ci-range">[${(lo * 100).toFixed(1)}% – ${(hi * 100).toFixed(1)}%]</td>
        <td class="${rCls}">${avgR >= 0 ? '+' : ''}${avgR.toFixed(2)}%</td>
        <td>
          <span class="pos">${a.max_return_pct >= 0 ? '+' : ''}${a.max_return_pct.toFixed(2)}%</span>
          /
          <span class="neg">${a.min_return_pct >= 0 ? '+' : ''}${a.min_return_pct.toFixed(2)}%</span>
        </td>
        <td class="eff-book">${escapeHtml(a.book_source || '—')}</td>
      `;
      tbody.appendChild(tr);
    });
  }

  // ---------- 版本信息 ----------
  async function loadVersion() {
    try {
      const v = await fetchJson('/api/version');
      $('footer-phase').textContent = `${v.name} v${v.version} · ${v.phase}`;
    } catch (e) { /* ignore */ }
  }

  // ---------- 币种 combobox：输入搜索 + 可见下拉 + 键盘导航（多交易所）----------
  // 数据源：/api/symbols（聚合 Binance + Bybit 所有 USDT 交易对）
  // 每项为 { id: 'BINANCE:BTCUSDT', exchange: 'BINANCE', symbol: 'BTCUSDT', base: 'BTC', quote: 'USDT' }
  // input.value 存储 id（如 BYBIT:BTCUSDT），裸 symbol（BTCUSDT）默认等价 BINANCE:BTCUSDT
  const symbolComboboxState = {
    // 启动默认：四交易所基本币，稍后被 /api/symbols 覆盖
    all: [
      { id: 'BINANCE:BTCUSDT', exchange: 'BINANCE', symbol: 'BTCUSDT', base: 'BTC', quote: 'USDT' },
      { id: 'BINANCE:ETHUSDT', exchange: 'BINANCE', symbol: 'ETHUSDT', base: 'ETH', quote: 'USDT' },
      { id: 'BYBIT:BTCUSDT',   exchange: 'BYBIT',   symbol: 'BTCUSDT', base: 'BTC', quote: 'USDT' },
      { id: 'BYBIT:ETHUSDT',   exchange: 'BYBIT',   symbol: 'ETHUSDT', base: 'ETH', quote: 'USDT' },
      { id: 'BITGET:BTCUSDT',  exchange: 'BITGET',  symbol: 'BTCUSDT', base: 'BTC', quote: 'USDT' },
      { id: 'BITGET:ETHUSDT',  exchange: 'BITGET',  symbol: 'ETHUSDT', base: 'ETH', quote: 'USDT' },
      { id: 'OKX:BTCUSDT',     exchange: 'OKX',     symbol: 'BTCUSDT', base: 'BTC', quote: 'USDT' },
      { id: 'OKX:ETHUSDT',     exchange: 'OKX',     symbol: 'ETHUSDT', base: 'ETH', quote: 'USDT' },
    ],
    matches: [],
    activeIdx: -1,
  };

  // 交易所视觉样式：徽章颜色 + 图标
  const EXCHANGE_STYLE = {
    BINANCE: { label: 'Binance',  color: '#F0B90B', short: 'BN' },
    BYBIT:   { label: 'Bybit',    color: '#FFC300', short: 'BY' },
    BITGET:  { label: 'Bitget',   color: '#00CEFF', short: 'BG' },
    OKX:     { label: 'OKX',      color: '#4A9EF5', short: 'OK' },
  };

  async function loadSymbolList() {
    try {
      const data = await fetchJson('/api/symbols');
      // 优先使用新 entries 结构；如果端点是旧版本只有 symbols，则降级
      let entries = Array.isArray(data?.entries) ? data.entries : null;
      if (!entries || !entries.length) {
        const legacy = Array.isArray(data?.symbols) ? data.symbols : [];
        entries = legacy.map((s) => ({
          id: `BINANCE:${s}`, exchange: 'BINANCE', symbol: s,
          base: s.replace(/USDT$/, ''), quote: 'USDT',
        }));
      }
      if (!entries.length) return;
      symbolComboboxState.all = entries;
      const byEx = entries.reduce((m, e) => { m[e.exchange] = (m[e.exchange] || 0) + 1; return m; }, {});
      console.info(`✓ 加载 ${entries.length} 个交易对，按交易所：`, byEx);
    } catch (e) {
      console.warn('加载 symbols 失败，保留默认 bootstrap:', e?.message || e);
    }
  }

  function escHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    }[c]));
  }

  /// 解析 input.value 为 {exchange, symbol}；支持带前缀或裸 symbol
  function parseSymbolInput(v) {
    const raw = (v || '').trim().toUpperCase();
    if (!raw) return { exchange: 'BINANCE', symbol: '' };
    const idx = raw.indexOf(':');
    if (idx > 0) {
      return { exchange: raw.slice(0, idx), symbol: raw.slice(idx + 1) };
    }
    return { exchange: 'BINANCE', symbol: raw };
  }

  /// Sanitize 用户输入的 symbol：清除 Chrome autocomplete 追加的垃圾
  /// 策略：
  /// 1. 大写 + trim
  /// 2. 若输入匹配 all 列表中某项的 id → 直接返回该 id
  /// 3. 若输入以 USDT 结尾附加了多余字符（如 BTCUSDTBTC），截断到第一个 USDT
  /// 4. 若不带前缀，保留（默认 Binance）
  function sanitizeSymbol(input) {
    let raw = (input || '').trim().toUpperCase();
    if (!raw) return '';
    // 规则 2：精确匹配优先
    const exact = symbolComboboxState.all.find((e) => e.id === raw);
    if (exact) return raw;
    // 规则 3：匹配 [EX:]XXXX USDT <剩余垃圾>，保留到第一个 USDT
    const m = raw.match(/^([A-Z]+:)?([A-Z0-9]+USDT)/);
    if (m) {
      const prefix = m[1] || '';
      const sym = m[2];
      const repaired = `${prefix}${sym}`;
      // 回查 all 列表，确认是合法 symbol
      if (symbolComboboxState.all.some((e) => e.id === repaired)) {
        return repaired;
      }
      // 即使不在列表里也容忍返回（可能 all 还没加载完）
      return repaired;
    }
    // 兜底：原样返回
    return raw;
  }

  // 在 entry 上执行模糊匹配 —— 支持按 base / symbol / id / exchange 命中
  function filterSymbols(query) {
    const q = (query || '').trim().toUpperCase();
    const all = symbolComboboxState.all;
    if (!q) {
      // 空查询 → 按交易所分组前 200 个（Binance 在前）
      return all.slice(0, 200);
    }
    // 支持 "BYBIT:BTC" 或 "BYBIT BTC" 形式的筛选
    const parts = q.split(/[\s:]+/).filter(Boolean);
    const starts = [];
    const contains = [];
    for (const e of all) {
      const hay = `${e.exchange} ${e.symbol} ${e.base}`;
      // 所有 parts 都能命中
      let allHit = true;
      let hasStart = false;
      for (const p of parts) {
        const idx = hay.indexOf(p);
        if (idx < 0) { allHit = false; break; }
        if (e.base.startsWith(p) || e.symbol.startsWith(p) || e.exchange.startsWith(p)) hasStart = true;
      }
      if (!allHit) continue;
      if (hasStart) starts.push(e);
      else contains.push(e);
    }
    return [...starts, ...contains].slice(0, 200);
  }

  function renderSymbolDropdown() {
    const dd = document.getElementById('symbol-dropdown');
    if (!dd) return;
    const q = ($('symbol')?.value || '').toUpperCase();
    const list = symbolComboboxState.matches;
    if (!list.length) {
      dd.innerHTML = '<div class="combobox-empty">无匹配交易对</div>';
      return;
    }
    dd.innerHTML = list.map((e, i) => {
      const active = i === symbolComboboxState.activeIdx ? ' active' : '';
      const exStyle = EXCHANGE_STYLE[e.exchange] || { label: e.exchange, color: '#888', short: '?' };
      // 高亮命中段
      const q0 = q.replace(/^[A-Z]+:/, ''); // 只对 symbol 部分做高亮（去掉前缀）
      const sym = e.symbol;
      const idx = q0 ? sym.indexOf(q0) : -1;
      let symHtml;
      if (idx >= 0 && q0) {
        symHtml = escHtml(sym.slice(0, idx)) +
          '<span class="hit">' + escHtml(sym.slice(idx, idx + q0.length)) + '</span>' +
          escHtml(sym.slice(idx + q0.length));
      } else {
        symHtml = escHtml(sym);
      }
      return `<div class="combobox-item${active}" data-value="${escHtml(e.id)}" role="option">
        <span class="cb-ex" style="background:${exStyle.color}22;color:${exStyle.color};border-color:${exStyle.color}66;" title="${escHtml(exStyle.label)}">${escHtml(exStyle.short)}</span>
        <span class="cb-sym">${symHtml}</span>
        <span class="cb-base">${escHtml(e.base)}/${escHtml(e.quote)}</span>
      </div>`;
    }).join('');
    // 滚动激活项到可视区域
    if (symbolComboboxState.activeIdx >= 0) {
      const el = dd.children[symbolComboboxState.activeIdx];
      if (el) el.scrollIntoView({ block: 'nearest' });
    }
  }

  function openSymbolDropdown() {
    const input = $('symbol');
    const dd = document.getElementById('symbol-dropdown');
    if (!dd || !input) return;
    symbolComboboxState.matches = filterSymbols(input.value);
    // activeIdx：若当前 input 精确匹配某项的 id，选中它；否则 0
    const curr = (input.value || '').toUpperCase();
    const exact = symbolComboboxState.matches.findIndex((e) => e.id === curr);
    symbolComboboxState.activeIdx = exact >= 0 ? exact : (symbolComboboxState.matches.length ? 0 : -1);
    renderSymbolDropdown();
    dd.hidden = false;
  }

  function closeSymbolDropdown() {
    const dd = document.getElementById('symbol-dropdown');
    if (dd) dd.hidden = true;
    symbolComboboxState.activeIdx = -1;
  }

  function selectSymbolByIndex(i) {
    const input = $('symbol');
    const list = symbolComboboxState.matches;
    if (!input || i < 0 || i >= list.length) return;
    const chosen = list[i];
    const chosenId = chosen && chosen.id ? chosen.id : '';
    if (!chosenId) return;
    if (input.value !== chosenId) {
      input.value = chosenId;
      // 触发 change → saveConfig + reload
      input.dispatchEvent(new Event('change', { bubbles: true }));
    }
    closeSymbolDropdown();
  }

  function setupSymbolCombobox() {
    const input = $('symbol');
    if (!input) return;
    // hidden input 场景：symbol 输入框已移除为隐藏字段，不再需要绑定小下拉 combobox
    // 保留此函数以维持 loadSymbolList 等异步数据填充到 symbolComboboxState.all 的流程
    const dd = document.getElementById('symbol-dropdown');
    if (!dd) return; // 没有下拉容器，直接返回（所有打开/筛选交互由 symbol-picker 接管）

    // 关键：点击/聚焦 input 时，优先打开全屏 picker（体验优于小下拉）
    // 若 picker 不可用（例如 HTML 没加载），回退到原小下拉
    function tryOpenPicker(ev) {
      if (typeof spOpen === 'function' && document.getElementById('symbol-picker')) {
        if (ev) ev.preventDefault();
        input.blur();
        spOpen();
        return true;
      }
      return false;
    }
    input.addEventListener('mousedown', tryOpenPicker);
    input.addEventListener('focus', (ev) => {
      if (tryOpenPicker(ev)) return;
      openSymbolDropdown();
    });
    input.addEventListener('click', (ev) => {
      if (tryOpenPicker(ev)) return;
      openSymbolDropdown();
    });
    input.addEventListener('input', () => {
      symbolComboboxState.matches = filterSymbols(input.value);
      symbolComboboxState.activeIdx = symbolComboboxState.matches.length ? 0 : -1;
      renderSymbolDropdown();
      dd.hidden = false;
    });
    input.addEventListener('keydown', (ev) => {
      if (dd.hidden) {
        if (ev.key === 'ArrowDown') { openSymbolDropdown(); ev.preventDefault(); }
        return;
      }
      const n = symbolComboboxState.matches.length;
      if (ev.key === 'ArrowDown') {
        ev.preventDefault();
        symbolComboboxState.activeIdx = (symbolComboboxState.activeIdx + 1) % Math.max(n, 1);
        renderSymbolDropdown();
      } else if (ev.key === 'ArrowUp') {
        ev.preventDefault();
        symbolComboboxState.activeIdx = (symbolComboboxState.activeIdx - 1 + n) % Math.max(n, 1);
        renderSymbolDropdown();
      } else if (ev.key === 'Enter') {
        if (symbolComboboxState.activeIdx >= 0) {
          ev.preventDefault();
          selectSymbolByIndex(symbolComboboxState.activeIdx);
        } else {
          closeSymbolDropdown();
        }
      } else if (ev.key === 'Escape') {
        closeSymbolDropdown();
      }
    });

    // 点击 item 选择
    dd.addEventListener('mousedown', (ev) => {
      const t = ev.target.closest('.combobox-item');
      if (!t) return;
      ev.preventDefault(); // 防止 input blur 先于 click 触发
      const val = t.dataset.value;
      if (!val) return;
      input.value = val;
      input.dispatchEvent(new Event('change', { bubbles: true }));
      closeSymbolDropdown();
    });

    // 失焦关闭（延迟，兼容 mousedown 已处理）
    input.addEventListener('blur', () => {
      setTimeout(closeSymbolDropdown, 120);
    });
  }

  // ============================================================
  // 交易对搜索面板（AiCoin 风全屏 picker） · 替代原小下拉的主入口
  // ============================================================
  const SYMBOL_PICKER = {
    open: false,
    filter: { exchange: '*', quote: '*', group: '*', text: '' },
    matches: [],
    activeIdx: 0,
    favorites: new Set(),
  };
  const SP_FAV_KEY = 'aura_symbol_favorites_v1';

  function spLoadFavorites() {
    try {
      const raw = localStorage.getItem(SP_FAV_KEY);
      if (!raw) return;
      const arr = JSON.parse(raw);
      if (Array.isArray(arr)) SYMBOL_PICKER.favorites = new Set(arr.filter((x) => typeof x === 'string'));
    } catch (_) {}
  }
  function spSaveFavorites() {
    try {
      localStorage.setItem(SP_FAV_KEY, JSON.stringify([...SYMBOL_PICKER.favorites]));
    } catch (_) {}
  }

  function spFilter() {
    const { exchange, quote, group, text } = SYMBOL_PICKER.filter;
    const q = (text || '').trim().toUpperCase();
    const parts = q.split(/[\s:]+/).filter(Boolean);
    const all = symbolComboboxState.all || [];
    const favs = SYMBOL_PICKER.favorites;
    const result = [];
    for (const e of all) {
      if (exchange !== '*' && e.exchange !== exchange) continue;
      if (quote !== '*' && e.quote !== quote) continue;
      if (group === 'fav' && !favs.has(e.id)) continue;
      if (parts.length) {
        const hay = `${e.exchange} ${e.symbol} ${e.base}`;
        let ok = true;
        for (const p of parts) {
          if (hay.indexOf(p) < 0) { ok = false; break; }
        }
        if (!ok) continue;
      }
      result.push(e);
    }
    // 排序：收藏优先 → Binance 优先 → base 字母
    result.sort((a, b) => {
      const af = favs.has(a.id) ? 0 : 1;
      const bf = favs.has(b.id) ? 0 : 1;
      if (af !== bf) return af - bf;
      if (a.exchange !== b.exchange) {
        if (a.exchange === 'BINANCE') return -1;
        if (b.exchange === 'BINANCE') return 1;
      }
      return a.base.localeCompare(b.base);
    });
    return result.slice(0, 500);
  }

  function spRender() {
    const list = $('sp-list');
    if (!list) return;
    SYMBOL_PICKER.matches = spFilter();
    if (SYMBOL_PICKER.activeIdx >= SYMBOL_PICKER.matches.length) {
      SYMBOL_PICKER.activeIdx = Math.max(0, SYMBOL_PICKER.matches.length - 1);
    }
    if (SYMBOL_PICKER.activeIdx < 0 && SYMBOL_PICKER.matches.length) {
      SYMBOL_PICKER.activeIdx = 0;
    }
    const cnt = $('sp-count');
    if (cnt) cnt.textContent = `${SYMBOL_PICKER.matches.length} 个匹配`;
    if (!SYMBOL_PICKER.matches.length) {
      list.innerHTML = '<div class="sp-empty">无匹配交易对，尝试修改关键字或切换筛选条件</div>';
      return;
    }
    const q = (SYMBOL_PICKER.filter.text || '').toUpperCase();
    // 纯 base 查询（不含冒号）时对 base 做高亮
    const qBase = q.indexOf(':') < 0 ? q : '';
    const favs = SYMBOL_PICKER.favorites;
    list.innerHTML = SYMBOL_PICKER.matches.map((e, i) => {
      const active = i === SYMBOL_PICKER.activeIdx ? ' active' : '';
      const favActive = favs.has(e.id) ? ' active' : '';
      const exStyle = (typeof EXCHANGE_STYLE !== 'undefined' && EXCHANGE_STYLE[e.exchange]) ||
        { label: e.exchange, color: '#888', short: '?' };
      const base = e.base || '';
      let baseHtml;
      const idx = qBase ? base.indexOf(qBase) : -1;
      if (idx >= 0 && qBase) {
        baseHtml = escHtml(base.slice(0, idx)) +
          '<span class="sp-hit">' + escHtml(base.slice(idx, idx + qBase.length)) + '</span>' +
          escHtml(base.slice(idx + qBase.length));
      } else {
        baseHtml = escHtml(base);
      }
      return `<div class="sp-item${active}" data-id="${escHtml(e.id)}" role="option">
        <button type="button" class="sp-fav${favActive}" data-fav-id="${escHtml(e.id)}" aria-label="收藏" title="收藏">★</button>
        <span class="sp-pair"><span class="base">${baseHtml}</span><span class="sep">/</span><span class="quote">${escHtml(e.quote)}</span></span>
        <span class="sp-ex">
          <span class="sp-ex-badge" style="color:${exStyle.color};border-color:${exStyle.color}66;background:${exStyle.color}22;">${escHtml(exStyle.short)}</span>
          <span>${escHtml(exStyle.label)}</span>
        </span>
        <span class="sp-base">${escHtml(e.symbol)}</span>
      </div>`;
    }).join('');
    // 滚动到激活项
    const activeEl = list.querySelector('.sp-item.active');
    if (activeEl) activeEl.scrollIntoView({ block: 'nearest' });
  }

  function spOpen() {
    const picker = $('symbol-picker');
    if (!picker) return;
    SYMBOL_PICKER.open = true;
    picker.hidden = false;
    // 延迟聚焦：避免外部 blur 干扰
    setTimeout(() => {
      const search = $('sp-search');
      if (search) {
        search.value = SYMBOL_PICKER.filter.text || '';
        search.focus();
        search.select();
      }
    }, 20);
    // 预选当前 symbol（若在列表中）
    SYMBOL_PICKER.activeIdx = 0;
    spRender();
    const currId = ($('symbol')?.value || '').toUpperCase();
    const idx = SYMBOL_PICKER.matches.findIndex((e) => e.id === currId);
    if (idx >= 0) {
      SYMBOL_PICKER.activeIdx = idx;
      spRender();
    }
  }

  function spClose() {
    const picker = $('symbol-picker');
    if (picker) picker.hidden = true;
    SYMBOL_PICKER.open = false;
  }

  function spSelect(idx) {
    const entry = SYMBOL_PICKER.matches[idx];
    if (!entry) return;
    const input = $('symbol');
    if (!input) return;
    input.value = entry.id;
    input.dispatchEvent(new Event('change', { bubbles: true }));
    spClose();
  }

  function setupSymbolPicker() {
    const picker = $('symbol-picker');
    if (!picker) return;
    spLoadFavorites();

    // 独立的 sym-badge 按钮（顶栏左侧）：点击直接打开 picker
    const badgeBtn = $('sym-badge');
    if (badgeBtn) {
      badgeBtn.addEventListener('click', (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        spOpen();
      });
    }

    // 全局 ⌘K / Ctrl+K 快捷键：打开搜索面板
    document.addEventListener('keydown', (ev) => {
      if ((ev.metaKey || ev.ctrlKey) && (ev.key === 'k' || ev.key === 'K')) {
        ev.preventDefault();
        if (SYMBOL_PICKER.open) spClose();
        else spOpen();
      }
    });

    const search = $('sp-search');
    const list = $('sp-list');

    // 关闭按钮 / 遮罩（统一由 data-sp-close 触发）
    picker.addEventListener('click', (ev) => {
      const t = ev.target.closest('[data-sp-close]');
      if (t) spClose();
    });

    // 搜索输入
    if (search) {
      search.addEventListener('input', () => {
        SYMBOL_PICKER.filter.text = search.value;
        SYMBOL_PICKER.activeIdx = 0;
        spRender();
      });
      // 键盘导航
      search.addEventListener('keydown', (ev) => {
        const n = SYMBOL_PICKER.matches.length;
        if (ev.key === 'ArrowDown') {
          ev.preventDefault();
          SYMBOL_PICKER.activeIdx = (SYMBOL_PICKER.activeIdx + 1) % Math.max(n, 1);
          spRender();
        } else if (ev.key === 'ArrowUp') {
          ev.preventDefault();
          SYMBOL_PICKER.activeIdx = (SYMBOL_PICKER.activeIdx - 1 + n) % Math.max(n, 1);
          spRender();
        } else if (ev.key === 'Enter') {
          ev.preventDefault();
          if (SYMBOL_PICKER.activeIdx >= 0) spSelect(SYMBOL_PICKER.activeIdx);
        } else if (ev.key === 'Escape') {
          ev.preventDefault();
          spClose();
        }
      });
    }

    // Tab 切换
    picker.querySelectorAll('.sp-tab').forEach((btn) => {
      btn.addEventListener('click', () => {
        const groupEl = btn.closest('.sp-tab-group');
        if (!groupEl) return;
        const group = groupEl.dataset.group;
        const val = btn.dataset.val;
        if (!group) return;
        SYMBOL_PICKER.filter[group] = val;
        groupEl.querySelectorAll('.sp-tab').forEach((b) => b.classList.toggle('active', b === btn));
        SYMBOL_PICKER.activeIdx = 0;
        spRender();
      });
    });

    // 列表：点击行选中，点击星标切换收藏
    if (list) {
      list.addEventListener('click', (ev) => {
        const favBtn = ev.target.closest('.sp-fav');
        if (favBtn) {
          ev.stopPropagation();
          const id = favBtn.dataset.favId;
          if (!id) return;
          if (SYMBOL_PICKER.favorites.has(id)) SYMBOL_PICKER.favorites.delete(id);
          else SYMBOL_PICKER.favorites.add(id);
          spSaveFavorites();
          spRender();
          return;
        }
        const item = ev.target.closest('.sp-item');
        if (!item) return;
        const id = item.dataset.id;
        const idx = SYMBOL_PICKER.matches.findIndex((e) => e.id === id);
        if (idx >= 0) spSelect(idx);
      });
      // 鼠标移入改 active
      list.addEventListener('mousemove', (ev) => {
        const item = ev.target.closest('.sp-item');
        if (!item) return;
        const id = item.dataset.id;
        const idx = SYMBOL_PICKER.matches.findIndex((e) => e.id === id);
        if (idx >= 0 && idx !== SYMBOL_PICKER.activeIdx) {
          SYMBOL_PICKER.activeIdx = idx;
          // 只更新 active class，避免整表重渲染造成抖动
          list.querySelectorAll('.sp-item.active').forEach((el) => el.classList.remove('active'));
          item.classList.add('active');
        }
      });
    }

    // 全局 Escape（模态打开时）
    document.addEventListener('keydown', (ev) => {
      if (SYMBOL_PICKER.open && ev.key === 'Escape') {
        ev.preventDefault();
        spClose();
      }
    });
  }

  // ---------- 启动 ----------
  window.addEventListener('DOMContentLoaded', () => {
    loadConfig();            // 先恢复上次的配置值
    bindConfigEvents();
    bindEvents();
    loadVersion();
    setupSymbolCombobox();   // 绑定 combobox 事件（即便 API 未返回前也可用默认 6 个）
    setupSymbolPicker();     // 绑定全屏搜索面板
    loadSymbolList();        // 异步拉所有 Binance USDT 交易对
    // 等 Lightweight Charts 脚本就绪
    if (typeof LightweightCharts === 'undefined') {
      console.error('Lightweight Charts 未加载');
      return;
    }
    initCharts();
    reload();
  });
})();
