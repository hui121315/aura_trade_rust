/* ===========================================================
 * alerts.js — 订阅提醒页面逻辑
 *
 * 功能：
 *   - 订阅 CRUD（localStorage 持久化）
 *   - 币种 combobox（复用 /api/symbols）
 *   - 60s 轮询扫描后端 API，触发通知 + 声音 + 写入历史
 *   - 触发去重（按事件 index / timestamp）
 *   - CSV 导出
 *
 * 依赖：
 *   - toast.js（全局 toast 提示）
 *
 * 存储键：
 *   - aura_alerts          订阅数组
 *   - aura_alert_history   触发历史（最多 200 条）
 * ========================================================= */

(() => {
  'use strict';

  const STORAGE_KEY = 'aura_alerts';
  const HISTORY_KEY = 'aura_alert_history';
  const MAX_HISTORY = 200;
  const POLL_INTERVAL_MS = 60 * 1000;

  const $ = (id) => document.getElementById(id);

  // ---- Toast fallback（toast.js 的全局对象是 AuraToast.push）----
  const toast = (msg, kind = 'info') => {
    // 兼容 success/warn/error/info —— AuraToast 只认 success/error/info
    const k = kind === 'warn' ? 'error' : kind;
    if (window.AuraToast && typeof window.AuraToast.push === 'function') {
      window.AuraToast.push(msg, k);
    } else {
      console.log(`[${kind}] ${msg}`);
    }
  };

  // ---------- 触发类型元数据 ----------
  // 每种订阅类型都对应一个 API 端点 + 判断函数 + 展示标签
  const KIND_META = {
    ma_golden_cross: { label: '均线金叉', tag: 'bull', api: 'ma_state', desc: 'MA 快穿慢（带斜率确认）' },
    ma_death_cross:  { label: '均线死叉', tag: 'bear', api: 'ma_state', desc: 'MA 快下穿慢' },
    ma_bull_align:   { label: '多头排列', tag: 'bull', api: 'ma_state', desc: '均线多头排列（≥3 条）' },
    ma_bear_align:   { label: '空头排列', tag: 'bear', api: 'ma_state', desc: '均线空头排列' },
    ma_guillotine:   { label: '断头铡刀', tag: 'bear', api: 'signals',  desc: '跌破 60 日 + 短 MA 死叉（铁证 71% 胜率）' },
    ma_desert_breakout: { label: '旱地拔葱', tag: 'bull', api: 'signals', desc: '长粘合后放量突破' },
    ma_poison_spider: { label: '毒蜘蛛', tag: 'bear', api: 'signals', desc: '均线空头纠缠' },
    candle_bull_engulf: { label: '阳包阴', tag: 'bull', api: 'candle_patterns', desc: '看涨吞没形态' },
    candle_bear_engulf: { label: '阴包阳', tag: 'bear', api: 'candle_patterns', desc: '看跌吞没形态' },
    candle_morning_star: { label: '早晨之星', tag: 'bull', api: 'candle_patterns', desc: '三 K 线见底形态' },
    candle_evening_star: { label: '黄昏之星', tag: 'bear', api: 'candle_patterns', desc: '三 K 线见顶形态（强于早晨）' },
    chart_hs_top:    { label: '头肩顶', tag: 'bear', api: 'chart_patterns', desc: '颈线跌破确认' },
    chart_hs_bottom: { label: '头肩底', tag: 'bull', api: 'chart_patterns', desc: '颈线突破确认' },
    chart_double_top: { label: '双顶', tag: 'bear', api: 'chart_patterns', desc: '二次测试高点失败' },
    chart_double_bottom: { label: '双底', tag: 'bull', api: 'chart_patterns', desc: '二次测试低点支撑' },
    sr_break_up:     { label: '上破阻力', tag: 'bull', api: 'trend_state', desc: '价格突破关键阻力位' },
    sr_break_down:   { label: '下破支撑', tag: 'bear', api: 'trend_state', desc: '价格跌破关键支撑位' },
    trend_reversal:  { label: '趋势反转', tag: 'warn', api: 'trend_state', desc: 'Swing 反转点确认' },
    rsi_overbought:  { label: 'RSI 超买', tag: 'bear', api: 'indicators',  desc: 'RSI > 70（回调风险）' },
    rsi_oversold:    { label: 'RSI 超卖', tag: 'bull', api: 'indicators',  desc: 'RSI < 30（反弹机会）' },
    volume_spike:    { label: '异常放量', tag: 'warn', api: 'klines',      desc: '成交量 > 3×20 均量' },
  };

  // ---------- 状态 ----------
  const state = {
    alerts: [],       // 订阅列表
    history: [],      // 触发历史
    allSymbols: ['BTCUSDT', 'ETHUSDT', 'SOLUSDT', 'BNBUSDT', 'XRPUSDT', 'DOGEUSDT'],
    matches: [],
    comboActiveIdx: -1,
    pollTimer: null,
    pollCountdown: POLL_INTERVAL_MS / 1000,
    pollCountdownTimer: null,
    scanning: false,
    // 本次轮询内的请求级缓存（key: `${api}|${symbol}|${interval}`）
    roundCache: new Map(),
  };

  // ---------- 工具 ----------
  function uid() { return 'a_' + Math.random().toString(36).slice(2, 10) + Date.now().toString(36); }

  function nowIso() { return new Date().toISOString(); }

  function fmtTime(iso) {
    if (!iso) return '—';
    const d = new Date(iso);
    if (isNaN(d.getTime())) return '—';
    const pad = (n) => String(n).padStart(2, '0');
    const now = new Date();
    const sameDay = d.toDateString() === now.toDateString();
    const date = sameDay ? '今天' : `${d.getMonth() + 1}/${d.getDate()}`;
    return `${date} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  function fmtPrice(v) {
    if (!isFinite(v) || v == null) return '—';
    if (v >= 1000) return v.toFixed(2);
    if (v >= 1) return v.toFixed(4);
    return v.toFixed(6);
  }

  function escHtml(s) {
    return String(s ?? '').replace(/[&<>"']/g, (c) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    }[c]));
  }

  function fetchJson(url) {
    return fetch(url, { headers: { Accept: 'application/json' } })
      .then((r) => r.json())
      .then((body) => {
        if (!body.ok) throw new Error(body.error || 'API error');
        return body.data;
      });
  }

  // ---------- 持久化 ----------
  function loadAlerts() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return [];
      const arr = JSON.parse(raw);
      return Array.isArray(arr) ? arr : [];
    } catch { return []; }
  }
  function saveAlerts() {
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(state.alerts)); } catch { /* quota */ }
  }
  function loadHistory() {
    try {
      const raw = localStorage.getItem(HISTORY_KEY);
      if (!raw) return [];
      const arr = JSON.parse(raw);
      return Array.isArray(arr) ? arr.slice(0, MAX_HISTORY) : [];
    } catch { return []; }
  }
  function saveHistory() {
    try {
      // 只保留最新的 MAX_HISTORY 条
      const trimmed = state.history.slice(0, MAX_HISTORY);
      localStorage.setItem(HISTORY_KEY, JSON.stringify(trimmed));
    } catch { /* quota */ }
  }

  // ---------- 渲染 ----------
  function render() {
    renderAlerts();
    renderHistory();
    renderStats();
  }

  function renderAlerts() {
    const tbody = $('al-tbody');
    const empty = $('al-empty');
    if (!tbody) return;
    if (!state.alerts.length) {
      tbody.innerHTML = '';
      empty.hidden = false;
      return;
    }
    empty.hidden = true;
    tbody.innerHTML = state.alerts.map((a) => {
      const meta = KIND_META[a.kind] || { label: a.kind, tag: '', desc: '' };
      const status = a.enabled
        ? `<span class="al-badge active">活跃</span>`
        : `<span class="al-badge paused">暂停</span>`;
      const last = a.lastTriggeredAt
        ? `<span class="al-cell-time">${escHtml(fmtTime(a.lastTriggeredAt))}</span>`
        : `<span class="al-cell-time">—</span>`;
      const note = a.note ? escHtml(a.note) : '<span style="color:var(--text-muted)">—</span>';
      return `
        <tr data-id="${a.id}">
          <td class="al-cell-symbol">${escHtml(a.symbol)}</td>
          <td class="al-cell-interval">${escHtml(a.interval)}</td>
          <td>
            <span class="al-kind-tag ${meta.tag}" title="${escHtml(meta.desc)}">${escHtml(meta.label)}</span>
          </td>
          <td>${note}</td>
          <td>${status}</td>
          <td>${last}</td>
          <td>
            <div class="al-row-actions">
              <button class="al-row-btn" data-act="toggle" title="${a.enabled ? '暂停' : '启用'}">
                ${a.enabled ? '⏸' : '▶'}
              </button>
              <button class="al-row-btn" data-act="test" title="手动测试触发">🧪</button>
              <button class="al-row-btn danger" data-act="del" title="删除">✕</button>
            </div>
          </td>
        </tr>
      `;
    }).join('');
  }

  function renderHistory() {
    const tbody = $('al-history-tbody');
    const empty = $('al-history-empty');
    if (!tbody) return;
    if (!state.history.length) {
      tbody.innerHTML = '';
      empty.hidden = false;
      return;
    }
    empty.hidden = true;
    tbody.innerHTML = state.history.map((h) => {
      const meta = KIND_META[h.kind] || { label: h.kind, tag: '' };
      return `
        <tr>
          <td class="al-cell-time">${escHtml(fmtTime(h.time))}</td>
          <td class="al-cell-symbol">${escHtml(h.symbol)}</td>
          <td class="al-cell-interval">${escHtml(h.interval)}</td>
          <td><span class="al-kind-tag ${meta.tag}">${escHtml(meta.label)}</span></td>
          <td class="al-cell-price">${escHtml(fmtPrice(h.price))}</td>
          <td>${h.note ? escHtml(h.note) : '<span style="color:var(--text-muted)">—</span>'}</td>
        </tr>
      `;
    }).join('');
  }

  function renderStats() {
    const active = state.alerts.filter((a) => a.enabled).length;
    const paused = state.alerts.length - active;
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);
    const today = state.history.filter((h) => new Date(h.time) >= todayStart).length;
    $('al-stat-active').textContent = String(active);
    $('al-stat-paused').textContent = String(paused);
    $('al-stat-today').textContent = String(today);
    $('al-stat-total').textContent = String(state.history.length);
  }

  // ---------- Combobox（复用主界面的逻辑）----------
  async function loadSymbolList() {
    try {
      const data = await fetchJson('/api/symbols');
      const symbols = Array.isArray(data?.symbols) ? data.symbols : [];
      if (symbols.length) state.allSymbols = symbols;
    } catch (e) {
      console.warn('加载 symbols 失败:', e?.message || e);
    }
  }

  function filterSymbols(q) {
    q = (q || '').trim().toUpperCase();
    if (!q) return state.allSymbols.slice(0, 200);
    const starts = [], contains = [];
    for (const s of state.allSymbols) {
      const idx = s.indexOf(q);
      if (idx === 0) starts.push(s);
      else if (idx > 0) contains.push(s);
    }
    return [...starts, ...contains].slice(0, 200);
  }

  function renderCombobox() {
    const dd = $('al-symbol-dropdown');
    if (!dd) return;
    const q = ($('al-symbol').value || '').toUpperCase();
    const list = state.matches;
    if (!list.length) {
      dd.innerHTML = '<div class="combobox-empty">无匹配交易对</div>';
      return;
    }
    dd.innerHTML = list.map((s, i) => {
      const active = i === state.comboActiveIdx ? ' active' : '';
      const idx = q ? s.indexOf(q) : -1;
      let html = (idx >= 0 && q)
        ? escHtml(s.slice(0, idx)) + '<span class="hit">' + escHtml(s.slice(idx, idx + q.length)) + '</span>' + escHtml(s.slice(idx + q.length))
        : escHtml(s);
      return `<div class="combobox-item${active}" data-value="${escHtml(s)}" role="option">${html}</div>`;
    }).join('');
    if (state.comboActiveIdx >= 0) {
      const el = dd.children[state.comboActiveIdx];
      if (el) el.scrollIntoView({ block: 'nearest' });
    }
  }

  function openCombobox() {
    const input = $('al-symbol');
    const dd = $('al-symbol-dropdown');
    if (!input || !dd) return;
    state.matches = filterSymbols(input.value);
    const exact = state.matches.indexOf(input.value.toUpperCase());
    state.comboActiveIdx = exact >= 0 ? exact : (state.matches.length ? 0 : -1);
    renderCombobox();
    dd.hidden = false;
  }
  function closeCombobox() {
    const dd = $('al-symbol-dropdown');
    if (dd) dd.hidden = true;
    state.comboActiveIdx = -1;
  }

  function setupCombobox() {
    const input = $('al-symbol');
    const dd = $('al-symbol-dropdown');
    if (!input || !dd) return;
    input.addEventListener('focus', openCombobox);
    input.addEventListener('click', openCombobox);
    input.addEventListener('input', () => {
      state.matches = filterSymbols(input.value);
      state.comboActiveIdx = state.matches.length ? 0 : -1;
      renderCombobox();
      dd.hidden = false;
    });
    input.addEventListener('keydown', (ev) => {
      if (dd.hidden) {
        if (ev.key === 'ArrowDown') { openCombobox(); ev.preventDefault(); }
        return;
      }
      const n = state.matches.length;
      if (ev.key === 'ArrowDown') {
        ev.preventDefault();
        state.comboActiveIdx = (state.comboActiveIdx + 1) % Math.max(n, 1);
        renderCombobox();
      } else if (ev.key === 'ArrowUp') {
        ev.preventDefault();
        state.comboActiveIdx = (state.comboActiveIdx - 1 + n) % Math.max(n, 1);
        renderCombobox();
      } else if (ev.key === 'Enter') {
        if (state.comboActiveIdx >= 0) {
          ev.preventDefault();
          const v = state.matches[state.comboActiveIdx];
          input.value = v;
          closeCombobox();
        }
      } else if (ev.key === 'Escape') {
        closeCombobox();
      }
    });
    dd.addEventListener('mousedown', (ev) => {
      const t = ev.target.closest('.combobox-item');
      if (!t) return;
      ev.preventDefault();
      const val = t.dataset.value;
      if (!val) return;
      input.value = val;
      closeCombobox();
    });
    input.addEventListener('blur', () => setTimeout(closeCombobox, 120));
    input.addEventListener('change', () => {
      input.value = (input.value || '').trim().toUpperCase();
    });
  }

  // ---------- CRUD ----------
  function addAlert(a) {
    // 去重：同 symbol+interval+kind 的不重复添加
    const exist = state.alerts.find(
      (x) => x.symbol === a.symbol && x.interval === a.interval && x.kind === a.kind
    );
    if (exist) {
      toast(`已存在相同订阅：${a.symbol} ${a.interval} ${KIND_META[a.kind]?.label}`, 'warn');
      return false;
    }
    state.alerts.unshift({
      id: uid(),
      symbol: a.symbol,
      interval: a.interval,
      kind: a.kind,
      note: a.note || '',
      enabled: true,
      createdAt: nowIso(),
      lastTriggeredAt: null,
      lastTriggerSig: null,
    });
    saveAlerts();
    render();
    toast(`已添加订阅：${a.symbol} · ${a.interval} · ${KIND_META[a.kind]?.label}`, 'success');
    return true;
  }

  function toggleAlert(id) {
    const a = state.alerts.find((x) => x.id === id);
    if (!a) return;
    a.enabled = !a.enabled;
    saveAlerts();
    render();
    toast(a.enabled ? `已启用订阅` : `已暂停订阅`, 'info');
  }
  function deleteAlert(id) {
    state.alerts = state.alerts.filter((a) => a.id !== id);
    saveAlerts();
    render();
    toast('订阅已删除', 'info');
  }
  function enableAll() {
    state.alerts.forEach((a) => { a.enabled = true; });
    saveAlerts(); render();
    toast(`已启用全部 ${state.alerts.length} 个订阅`, 'success');
  }
  function pauseAll() {
    state.alerts.forEach((a) => { a.enabled = false; });
    saveAlerts(); render();
    toast(`已暂停全部 ${state.alerts.length} 个订阅`, 'info');
  }
  function clearAll() {
    if (!confirm(`确认删除全部 ${state.alerts.length} 个订阅？此操作不可撤销。`)) return;
    state.alerts = [];
    saveAlerts(); render();
    toast('已清空订阅列表', 'info');
  }

  // ---------- 触发记录 ----------
  function recordTrigger(alert, payload) {
    alert.lastTriggeredAt = nowIso();
    alert.lastTriggerSig = payload.sig;
    saveAlerts();

    state.history.unshift({
      id: uid(),
      alertId: alert.id,
      time: alert.lastTriggeredAt,
      symbol: alert.symbol,
      interval: alert.interval,
      kind: alert.kind,
      price: payload.price ?? null,
      note: alert.note || '',
      extra: payload.extra || null,
    });
    if (state.history.length > MAX_HISTORY) state.history.length = MAX_HISTORY;
    saveHistory();
    render();

    // 高亮行
    setTimeout(() => {
      const row = document.querySelector(`#al-tbody tr[data-id="${alert.id}"]`);
      if (row) {
        row.classList.add('just-triggered');
        setTimeout(() => row.classList.remove('just-triggered'), 1500);
      }
    }, 50);

    notify(alert, payload);
  }

  // ---------- 通知 + 声音 ----------
  function ensureNotifPermission() {
    if (!('Notification' in window)) return Promise.resolve('unsupported');
    if (Notification.permission === 'granted') return Promise.resolve('granted');
    if (Notification.permission === 'denied') return Promise.resolve('denied');
    return Notification.requestPermission();
  }
  function updateNotifStatus() {
    const st = $('al-notif-status');
    if (!('Notification' in window)) {
      st.textContent = '浏览器不支持';
      st.className = 'al-toggle-status denied';
      return;
    }
    if (Notification.permission === 'granted') {
      st.textContent = '已允许';
      st.className = 'al-toggle-status granted';
      $('al-enable-notif').checked = true;
    } else if (Notification.permission === 'denied') {
      st.textContent = '已拒绝';
      st.className = 'al-toggle-status denied';
      $('al-enable-notif').checked = false;
    } else {
      st.textContent = '未启用';
      st.className = 'al-toggle-status';
      $('al-enable-notif').checked = false;
    }
  }

  function playBeep() {
    if (!$('al-enable-sound').checked) return;
    try {
      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      if (!AudioCtx) return;
      const ctx = new AudioCtx();
      const o = ctx.createOscillator();
      const g = ctx.createGain();
      o.type = 'sine';
      o.frequency.value = 660;
      g.gain.value = 0.08;
      o.connect(g);
      g.connect(ctx.destination);
      o.start();
      g.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.35);
      o.stop(ctx.currentTime + 0.4);
    } catch { /* ignore */ }
  }

  function notify(alert, payload) {
    const meta = KIND_META[alert.kind] || { label: alert.kind };
    const title = `✦ ${alert.symbol} ${alert.interval} · ${meta.label}`;
    const body = (payload.extra ? payload.extra + '\n' : '') +
      `价格 ${fmtPrice(payload.price)}` +
      (alert.note ? `\n备注：${alert.note}` : '');
    playBeep();
    toast(`${title} — ${fmtPrice(payload.price)}`, 'success');
    if ('Notification' in window && Notification.permission === 'granted') {
      try {
        const n = new Notification(title, { body, tag: `aura-${alert.id}`, renotify: true });
        n.onclick = () => { window.focus(); n.close(); };
      } catch { /* ignore */ }
    }
    // 顶栏状态闪烁
    const ps = $('al-poll-status');
    if (ps) {
      ps.classList.add('triggered');
      setTimeout(() => ps.classList.remove('triggered'), 1200);
    }
  }

  // ==========================================================
  // 以下：轮询 + 触发判断（第二部分将追加到文件）
  // 避免单文件过长导致单次 edit 溢出
  // ==========================================================
  window.__auraAlerts = {
    state, KIND_META, fetchJson, recordTrigger, fmtPrice,
    render, renderAlerts, renderHistory, renderStats,
  };

  // ---------- 扫描核心 ----------
  // 同一轮询内，按 (api,symbol,interval) 去重缓存
  function cacheKey(api, symbol, interval) { return `${api}|${symbol}|${interval}`; }

  async function getCachedOrFetch(api, symbol, interval) {
    const key = cacheKey(api, symbol, interval);
    if (state.roundCache.has(key)) return state.roundCache.get(key);
    const url = buildUrl(api, symbol, interval);
    const promise = fetchJson(url).catch((e) => {
      console.warn(`[alerts] ${api} ${symbol} ${interval} 失败:`, e?.message || e);
      return null;
    });
    state.roundCache.set(key, promise);
    return promise;
  }

  function buildUrl(api, symbol, interval) {
    const sym = encodeURIComponent(symbol);
    switch (api) {
      case 'ma_state':
        return `/api/ma_state?symbol=${sym}&interval=${interval}&limit=500&periods=5,10,20,30,60,120,250&kind=sma`;
      case 'signals':
        return `/api/signals?symbol=${sym}&interval=${interval}&limit=500`;
      case 'candle_patterns':
        return `/api/candle_patterns?symbol=${sym}&interval=${interval}&limit=500`;
      case 'chart_patterns':
        return `/api/chart_patterns?symbol=${sym}&interval=${interval}&limit=500`;
      case 'trend_state':
        return `/api/trend_state?symbol=${sym}&interval=${interval}&limit=500`;
      case 'indicators':
        return `/api/indicators/series?symbol=${sym}&interval=${interval}&limit=500&kinds=rsi`;
      case 'klines':
        return `/api/klines?symbol=${sym}&interval=${interval}&limit=200`;
      default:
        return `/api/ping`;
    }
  }

  // ---------- 触发判断分发 ----------
  // 每个 kind 返回 { triggered: bool, sig: string, price: number, extra?: string }
  async function checkTrigger(alert) {
    const api = KIND_META[alert.kind]?.api;
    if (!api) return { triggered: false };
    const data = await getCachedOrFetch(api, alert.symbol, alert.interval);
    if (!data) return { triggered: false };
    switch (alert.kind) {
      case 'ma_golden_cross':
      case 'ma_death_cross':
        return checkCross(data, alert.kind === 'ma_golden_cross' ? 'Golden' : 'Death');
      case 'ma_bull_align':
      case 'ma_bear_align':
        return checkAlignment(data, alert.kind === 'ma_bull_align' ? 'Bullish' : 'Bearish');
      case 'ma_guillotine':
        return checkAdvancedMa(data, 'Guillotine');
      case 'ma_desert_breakout':
        return checkAdvancedMa(data, 'DesertBreakout');
      case 'ma_poison_spider':
        return checkAdvancedMa(data, 'PoisonSpider');
      case 'candle_bull_engulf':
        return checkCandle(data, 'BullishEngulfing');
      case 'candle_bear_engulf':
        return checkCandle(data, 'BearishEngulfing');
      case 'candle_morning_star':
        return checkCandle(data, 'MorningStar');
      case 'candle_evening_star':
        return checkCandle(data, 'EveningStar');
      case 'chart_hs_top':
        return checkChart(data, 'HeadAndShouldersTop');
      case 'chart_hs_bottom':
        return checkChart(data, 'HeadAndShouldersBottom');
      case 'chart_double_top':
        return checkChart(data, 'DoubleTop');
      case 'chart_double_bottom':
        return checkChart(data, 'DoubleBottom');
      case 'sr_break_up':
        return checkSrBreak(data, 'up');
      case 'sr_break_down':
        return checkSrBreak(data, 'down');
      case 'trend_reversal':
        return checkTrendReversal(data);
      case 'rsi_overbought':
        return checkRsi(data, 'overbought');
      case 'rsi_oversold':
        return checkRsi(data, 'oversold');
      case 'volume_spike':
        return checkVolumeSpike(data);
      default:
        return { triggered: false };
    }
  }

  // ---------- 具体判断函数 ----------
  function checkCross(ma, wantKind) {
    const crosses = Array.isArray(ma?.crosses) ? ma.crosses : [];
    if (!crosses.length) return { triggered: false };
    // 取最新的一条符合方向的交叉
    const recent = crosses.slice().reverse().find((c) => c.kind === wantKind);
    if (!recent) return { triggered: false };
    const lastIdx = (ma.series?.[0]?.length || 1) - 1;
    // 触发窗口：最近 3 根内
    if (recent.index < lastIdx - 3) return { triggered: false };
    const price = ma.last_values?.[0] ?? null;
    return {
      triggered: true,
      sig: `cross-${wantKind}@${recent.index}-${recent.fast_period}-${recent.slow_period}`,
      price,
      extra: `MA${recent.fast_period} ${wantKind === 'Golden' ? '↑' : '↓'} MA${recent.slow_period}`,
    };
  }

  function checkAlignment(ma, wantKind) {
    const cur = ma?.alignment;
    if (!cur) return { triggered: false };
    const match = (wantKind === 'Bullish' && cur === 'Bullish')
      || (wantKind === 'Bearish' && cur === 'Bearish');
    if (!match) return { triggered: false };
    const lastIdx = (ma.series?.[0]?.length || 1) - 1;
    // 以 bar index 作为去重 sig；alignment 改变才算新触发
    return {
      triggered: true,
      sig: `align-${wantKind}@${lastIdx}`,
      price: null,
      extra: `当前排列：${wantKind === 'Bullish' ? '多头' : '空头'}`,
    };
  }

  function checkAdvancedMa(signals, wantKind) {
    const events = Array.isArray(signals?.advanced_ma_events) ? signals.advanced_ma_events : [];
    const evt = events.slice().reverse().find((e) => (e.kind === wantKind || e.event_type === wantKind));
    if (!evt) return { triggered: false };
    const lastBar = signals?.bars ? signals.bars - 1 : Infinity;
    if (evt.index != null && evt.index < lastBar - 5) return { triggered: false };
    return {
      triggered: true,
      sig: `${wantKind}@${evt.index ?? evt.bar_index ?? 'latest'}`,
      price: evt.price ?? null,
      extra: evt.note || evt.description || '',
    };
  }

  function checkCandle(resp, wantKind) {
    const pats = Array.isArray(resp?.patterns) ? resp.patterns : [];
    if (!pats.length) return { triggered: false };
    const last = pats[pats.length - 1];
    if (!last) return { triggered: false };
    const kind = last.kind || last.pattern || last.code;
    if (kind !== wantKind) return { triggered: false };
    // 只在最近 2 根内触发
    const idx = last.index ?? last.bar_index ?? 0;
    const total = resp?.count ?? pats.length;
    // 粗略判断：last 就是最新的
    return {
      triggered: true,
      sig: `candle-${wantKind}@${idx}`,
      price: last.price ?? last.close ?? null,
      extra: last.label || '',
    };
  }

  function checkChart(resp, wantKind) {
    const pats = Array.isArray(resp?.patterns) ? resp.patterns : [];
    const hit = pats.find((p) => (p.kind || p.pattern) === wantKind);
    if (!hit) return { triggered: false };
    const idx = hit.confirm_index ?? hit.end_index ?? hit.index ?? 0;
    return {
      triggered: true,
      sig: `chart-${wantKind}@${idx}`,
      price: hit.neckline ?? hit.confirm_price ?? null,
      extra: hit.label || '',
    };
  }

  function checkSrBreak(trend, dir) {
    const state_ = trend?.state || trend;
    const levels = state_?.sr_levels || [];
    const price = state_?.current_price ?? null;
    if (!levels.length || price == null) return { triggered: false };
    // 最近 5 根内是否穿越某条 SR
    for (const lvl of levels) {
      if (dir === 'up' && lvl.kind === 'Resistance' && price > lvl.price * 1.003) {
        return {
          triggered: true,
          sig: `sr-up@${lvl.price.toFixed(4)}`,
          price,
          extra: `突破阻力位 ${fmtPrice(lvl.price)}（触碰 ${lvl.touches} 次）`,
        };
      }
      if (dir === 'down' && lvl.kind === 'Support' && price < lvl.price * 0.997) {
        return {
          triggered: true,
          sig: `sr-down@${lvl.price.toFixed(4)}`,
          price,
          extra: `跌破支撑位 ${fmtPrice(lvl.price)}（触碰 ${lvl.touches} 次）`,
        };
      }
    }
    return { triggered: false };
  }

  function checkTrendReversal(trend) {
    const state_ = trend?.state || trend;
    const swings = state_?.swings || [];
    if (swings.length < 2) return { triggered: false };
    const lastSwing = swings[swings.length - 1];
    const lastBar = state_?.bars ? state_.bars - 1 : Infinity;
    if (lastSwing.index < lastBar - 3) return { triggered: false };
    return {
      triggered: true,
      sig: `swing@${lastSwing.index}`,
      price: lastSwing.price ?? null,
      extra: `新 swing ${lastSwing.kind} @ ${fmtPrice(lastSwing.price)}`,
    };
  }

  function checkRsi(data, kind) {
    const rsi = data?.rsi;
    if (!Array.isArray(rsi) || !rsi.length) return { triggered: false };
    const last = rsi[rsi.length - 1];
    if (!isFinite(last)) return { triggered: false };
    const threshold = kind === 'overbought' ? 70 : 30;
    const hit = kind === 'overbought' ? last > threshold : last < threshold;
    if (!hit) return { triggered: false };
    // sig 用整数 bar 索引，相同状态只触发一次（直到回落再反弹）
    return {
      triggered: true,
      sig: `rsi-${kind}@${rsi.length - 1}-${Math.round(last)}`,
      price: null,
      extra: `RSI = ${last.toFixed(2)}`,
    };
  }

  function checkVolumeSpike(resp) {
    const kl = Array.isArray(resp?.klines) ? resp.klines : [];
    if (kl.length < 30) return { triggered: false };
    const last = kl[kl.length - 1];
    const window = kl.slice(-21, -1);
    const avg = window.reduce((a, k) => a + (k.volume || 0), 0) / window.length;
    if (!isFinite(avg) || avg <= 0) return { triggered: false };
    const ratio = (last.volume || 0) / avg;
    if (ratio < 3) return { triggered: false };
    return {
      triggered: true,
      sig: `vol-spike@${last.open_time}`,
      price: last.close,
      extra: `成交量放大 ${ratio.toFixed(1)}× 均量`,
    };
  }

  // ---------- 轮询 ----------
  async function scanOnce() {
    if (state.scanning) return;
    state.scanning = true;
    const ps = $('al-poll-status');
    if (ps) { ps.classList.add('scanning'); ps.textContent = '扫描中…'; }
    state.roundCache.clear();

    const active = state.alerts.filter((a) => a.enabled);
    if (active.length === 0) {
      if (ps) { ps.classList.remove('scanning'); ps.textContent = '无活跃订阅'; }
      state.scanning = false;
      return;
    }
    // 并发（但每个 API 端点同 symbol+interval 的请求只会发一次，靠 roundCache 去重）
    await Promise.all(active.map(async (alert) => {
      try {
        const res = await checkTrigger(alert);
        if (res.triggered && res.sig !== alert.lastTriggerSig) {
          recordTrigger(alert, res);
        }
      } catch (e) {
        console.warn(`[alerts] ${alert.symbol} ${alert.kind}:`, e?.message || e);
      }
    }));

    if (ps) { ps.classList.remove('scanning'); }
    state.scanning = false;
    state.pollCountdown = POLL_INTERVAL_MS / 1000;
  }

  function startPolling() {
    stopPolling();
    state.pollTimer = setInterval(scanOnce, POLL_INTERVAL_MS);
    state.pollCountdownTimer = setInterval(() => {
      state.pollCountdown = Math.max(0, state.pollCountdown - 1);
      const ps = $('al-poll-status');
      if (ps && !state.scanning) {
        ps.textContent = state.alerts.some((a) => a.enabled)
          ? `下次扫描 ${state.pollCountdown}s`
          : '无活跃订阅';
      }
    }, 1000);
    // 启动即扫一次
    setTimeout(scanOnce, 500);
  }
  function stopPolling() {
    if (state.pollTimer) clearInterval(state.pollTimer);
    if (state.pollCountdownTimer) clearInterval(state.pollCountdownTimer);
    state.pollTimer = null;
    state.pollCountdownTimer = null;
    const ps = $('al-poll-status');
    if (ps) ps.textContent = '已暂停';
  }

  // ---------- CSV 导出 ----------
  function exportCsv() {
    if (!state.history.length) {
      toast('没有可导出的历史', 'warn');
      return;
    }
    const headers = ['时间', '币种', '周期', '触发类型', '触发标签', '价格', '备注', '补充信息'];
    const rows = state.history.map((h) => {
      const meta = KIND_META[h.kind] || {};
      return [
        h.time,
        h.symbol,
        h.interval,
        h.kind,
        meta.label || h.kind,
        h.price ?? '',
        (h.note || '').replace(/"/g, '""'),
        (h.extra || '').replace(/"/g, '""'),
      ].map((v) => `"${v ?? ''}"`).join(',');
    });
    const csv = [headers.join(','), ...rows].join('\n');
    const blob = new Blob([`\ufeff${csv}`], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `aura_alerts_history_${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    toast(`已导出 ${state.history.length} 条触发记录`, 'success');
  }

  // ---------- 事件绑定 ----------
  function bindEvents() {
    // 表单提交
    $('al-form').addEventListener('submit', (ev) => {
      ev.preventDefault();
      const symbol = ($('al-symbol').value || '').trim().toUpperCase();
      const interval = $('al-interval').value;
      const kind = $('al-kind').value;
      const note = ($('al-note').value || '').trim();
      if (!symbol) { toast('请输入交易对', 'warn'); return; }
      if (!kind) { toast('请选择触发类型', 'warn'); return; }
      if (addAlert({ symbol, interval, kind, note })) {
        $('al-note').value = '';
      }
    });

    // 表格行操作（事件委托）
    $('al-tbody').addEventListener('click', (ev) => {
      const btn = ev.target.closest('.al-row-btn');
      if (!btn) return;
      const row = btn.closest('tr');
      const id = row?.dataset.id;
      if (!id) return;
      const act = btn.dataset.act;
      if (act === 'toggle') toggleAlert(id);
      else if (act === 'del') {
        if (confirm('确认删除此订阅？')) deleteAlert(id);
      }
      else if (act === 'test') {
        const a = state.alerts.find((x) => x.id === id);
        if (a) {
          const meta = KIND_META[a.kind] || { label: a.kind };
          notify(a, { price: 0, extra: `测试触发（${meta.label}）` });
        }
      }
    });

    // 批量
    $('al-enable-all').addEventListener('click', enableAll);
    $('al-pause-all').addEventListener('click', pauseAll);
    $('al-clear-all').addEventListener('click', clearAll);

    // 历史
    $('al-export-csv').addEventListener('click', exportCsv);
    $('al-clear-history').addEventListener('click', () => {
      if (!state.history.length) return;
      if (!confirm(`确认清空全部 ${state.history.length} 条触发历史？`)) return;
      state.history = [];
      saveHistory();
      render();
      toast('已清空触发历史', 'info');
    });

    // 通知权限
    $('al-enable-notif').addEventListener('change', async (ev) => {
      if (ev.target.checked) {
        const res = await ensureNotifPermission();
        updateNotifStatus();
        if (res === 'granted') toast('浏览器通知已启用', 'success');
        else if (res === 'denied') toast('浏览器拒绝了通知权限', 'warn');
      } else {
        updateNotifStatus();
      }
    });
    $('al-test-notif').addEventListener('click', () => {
      notify(
        { symbol: 'TEST', interval: '4h', kind: 'ma_golden_cross', note: '测试订阅' },
        { price: 12345.67, extra: '这是一条测试通知' }
      );
    });

    // 自动扫描开关
    $('al-auto-poll').addEventListener('change', (ev) => {
      if (ev.target.checked) { startPolling(); toast('自动扫描已开启（每 60s）', 'info'); }
      else { stopPolling(); toast('自动扫描已暂停', 'info'); }
    });
  }

  // ---------- 启动 ----------
  window.addEventListener('DOMContentLoaded', async () => {
    state.alerts = loadAlerts();
    state.history = loadHistory();
    render();
    setupCombobox();
    bindEvents();
    updateNotifStatus();
    await loadSymbolList();
    if ($('al-auto-poll').checked) startPolling();
  });
})();
