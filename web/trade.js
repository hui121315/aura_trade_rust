/* =================================================================
   Aura · 实时信号面板 — 前端脚本
   ================================================================= */

(() => {
  'use strict';

  const $ = (id) => document.getElementById(id);

  // ---------- 状态 ----------
  const state = {
    seeds: [],
    currentSystem: null, // 当前选中的 SystemDefinition
    pollTimer: null,
    lastScanAt: 0,
    chart: null,          // lightweight-charts 实例
    candleSeries: null,
    // M18 通知
    notificationPermission: (typeof Notification !== 'undefined') ? Notification.permission : 'unsupported',
    lastNotifiedBarIndex: -1, // 防重复
    lastNotifiedSystemId: null, // 切换体系时重置
    audioCtx: null, // WebAudio 懒初始化
  };

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
  function fmtTime(ms) {
    if (!ms) return '—';
    const d = new Date(ms);
    return d.toLocaleString('zh-CN', { hour12: false });
  }

  // ---------- 初始化 ----------
  async function init() {
    try {
      const resp = await fetchJson('/api/system/seeds');
      state.seeds = resp.seeds;
      renderSystemSelect();
      // 默认选第一个
      if (state.seeds.length > 0) {
        $('tr-system').value = state.seeds[0].id;
        onSystemChange();
      }

      initChart();
      updateNotifyButtonUI();

      $('tr-system').addEventListener('change', onSystemChange);
      $('tr-btn-start').addEventListener('click', startPolling);
      $('tr-btn-stop').addEventListener('click', stopPolling);
      $('tr-btn-notify').addEventListener('click', requestNotificationPermission);
      // symbol/interval/refresh 切换时若正在运行，自动重启
      ['tr-symbol', 'tr-interval', 'tr-limit', 'tr-refresh'].forEach((id) => {
        $(id).addEventListener('change', () => {
          if (state.pollTimer) {
            stopPolling();
            startPolling();
          }
        });
      });
    } catch (e) {
      setStatus(`初始化失败：${e.message}`, 'error');
    }
  }

  function renderSystemSelect() {
    const sel = $('tr-system');
    sel.innerHTML = '';
    state.seeds.forEach((s) => {
      const opt = document.createElement('option');
      opt.value = s.id;
      const icon = s.origin === 'Discovered' ? '⭐' : '🌱';
      opt.textContent = `${icon} ${s.name}`;
      sel.appendChild(opt);
    });
  }

  function onSystemChange() {
    const id = $('tr-system').value;
    state.currentSystem = state.seeds.find((s) => s.id === id) || null;
    // M18：切换体系时重置通知去重状态
    state.lastNotifiedBarIndex = -1;
    state.lastNotifiedSystemId = id;
    renderSystemInfo();
    // 若正在运行，切换体系后立即重新扫描
    if (state.pollTimer) {
      stopPolling();
      startPolling();
    }
  }

  function renderSystemInfo() {
    const wrap = $('tr-sys-info');
    const s = state.currentSystem;
    if (!s) {
      wrap.innerHTML = '<div class="loading">请选择体系</div>';
      return;
    }
    const rule = s.combine.type +
      (s.combine.k !== undefined ? ` (k=${s.combine.k})` : '') +
      (s.combine.window_bars !== undefined ? ` (window=${s.combine.window_bars})` : '') +
      (s.combine.threshold !== undefined ? ` (threshold=${s.combine.threshold})` : '');
    const comps = s.components.map((c) => `<span>${c}</span>`).join(' + ');
    let benchHtml = '';
    const bench = (s.meta && s.meta.last_benchmark) || [];
    if (bench.length > 0) {
      benchHtml = `<div class="key" style="margin-top:10px;">历史基准</div>` +
        bench.map((b) => {
          const sh = isFinite(b.wf_avg_sharpe) ? (b.wf_avg_sharpe >= 0 ? '+' : '') + b.wf_avg_sharpe.toFixed(2) : '—';
          return `<div>${b.symbol} ${b.interval}: Sharpe ${sh}, Cons ${(b.wf_consistency * 100).toFixed(0)}%</div>`;
        }).join('');
    }
    wrap.innerHTML = `
      <div class="key">聚合规则</div>
      <div>${rule}</div>
      <div class="key" style="margin-top:8px;">组件（${s.components.length}）</div>
      <div class="comp-list">${comps}</div>
      ${s.description ? `<div class="key" style="margin-top:8px;">描述</div><div>${s.description}</div>` : ''}
      ${benchHtml}
    `;
  }

  // ---------- Polling 核心 ----------
  function startPolling() {
    if (!state.currentSystem) {
      setStatus('请先选择体系', 'error');
      return;
    }
    $('tr-btn-start').disabled = true;
    $('tr-btn-stop').disabled = false;
    scanOnce(); // 立即跑一次
    const sec = Math.max(5, parseInt($('tr-refresh').value, 10) || 30);
    state.pollTimer = setInterval(scanOnce, sec * 1000);
    setStatus(`▶ 已启动 polling（每 ${sec}s 刷新）`, 'success');
  }
  function stopPolling() {
    if (state.pollTimer) {
      clearInterval(state.pollTimer);
      state.pollTimer = null;
    }
    $('tr-btn-start').disabled = false;
    $('tr-btn-stop').disabled = true;
    setStatus('⏸ 已暂停', '');
  }

  async function scanOnce() {
    if (!state.currentSystem) return;
    const sym = $('tr-symbol').value;
    const tf = $('tr-interval').value;
    const limit = parseInt($('tr-limit').value, 10) || 300;
    try {
      const payload = {
        definition: state.currentSystem,
        symbol: sym,
        interval: tf,
        limit,
        tail_bars: 100, // M16：放大到 100，供 K 线图使用
      };
      const t0 = performance.now();
      const r = await fetchJson('/api/system/live_scan', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const dt = performance.now() - t0;
      state.lastScanAt = Date.now();
      renderScan(r);
      maybeNotify(r); // M18：新信号推送
      $('tr-last-update').textContent = `上次扫描: ${fmtTime(state.lastScanAt)} (${dt.toFixed(0)}ms)`;
    } catch (e) {
      setStatus(`扫描失败: ${e.message}`, 'error');
    }
  }

  // ---------- M18: 浏览器通知 + 声音 ----------
  function updateNotifyButtonUI() {
    const btn = $('tr-btn-notify');
    if (!btn) return;
    btn.classList.remove('notify-granted', 'notify-denied');
    if (state.notificationPermission === 'granted') {
      btn.textContent = '🔔 通知已启用';
      btn.classList.add('notify-granted');
      btn.disabled = true;
    } else if (state.notificationPermission === 'denied') {
      btn.textContent = '🔕 通知被拒（浏览器设置）';
      btn.classList.add('notify-denied');
      btn.disabled = true;
    } else if (state.notificationPermission === 'unsupported') {
      btn.textContent = '⚠ 此浏览器不支持通知';
      btn.disabled = true;
    } else {
      btn.textContent = '🔔 启用通知';
      btn.disabled = false;
    }
  }

  async function requestNotificationPermission() {
    if (typeof Notification === 'undefined') {
      setStatus('此浏览器不支持通知', 'error');
      return;
    }
    try {
      const perm = await Notification.requestPermission();
      state.notificationPermission = perm;
      updateNotifyButtonUI();
      if (perm === 'granted') {
        // 启用时弹一条欢迎通知确认工作
        new Notification('✨ Aura 通知已启用', {
          body: '检测到新的聚合信号时会自动提醒你',
          silent: true,
        });
      }
    } catch (e) {
      setStatus(`通知权限失败：${e.message}`, 'error');
    }
  }

  /// 用 WebAudio 合成一个简短"哔"声，无需音频文件
  function playBeep(direction) {
    if (!$('tr-sound-on').checked) return;
    try {
      if (!state.audioCtx) {
        const AC = window.AudioContext || window.webkitAudioContext;
        if (!AC) return;
        state.audioCtx = new AC();
      }
      const ctx = state.audioCtx;
      // 双音通知（上扬多头 / 下沉空头）
      const freqs = direction > 0 ? [660, 990] : [440, 330];
      const now = ctx.currentTime;
      freqs.forEach((f, i) => {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = 'sine';
        osc.frequency.value = f;
        gain.gain.setValueAtTime(0.0001, now + i * 0.12);
        gain.gain.exponentialRampToValueAtTime(0.15, now + i * 0.12 + 0.02);
        gain.gain.exponentialRampToValueAtTime(0.0001, now + i * 0.12 + 0.18);
        osc.connect(gain).connect(ctx.destination);
        osc.start(now + i * 0.12);
        osc.stop(now + i * 0.12 + 0.2);
      });
    } catch (e) {
      console.warn('playBeep failed', e);
    }
  }

  /// 扫描后检测新信号并推送通知
  function maybeNotify(r) {
    if (state.notificationPermission !== 'granted') return;
    if (!state.currentSystem) return;
    const lastBar = r.bars.length ? r.bars[r.bars.length - 1] : null;
    if (!lastBar) return;

    const onlyAggregate = $('tr-notify-trigger').checked;
    // 触发条件
    const isNewBar = lastBar.bar_index !== state.lastNotifiedBarIndex;
    const hasTrigger = onlyAggregate
      ? lastBar.combined_fired
      : (lastBar.triggers.length > 0 || lastBar.combined_fired);
    if (!isNewBar || !hasTrigger) return;

    // 推送
    const dirLabel = lastBar.combined_direction > 0
      ? '📈 LONG'
      : lastBar.combined_direction < 0
        ? '📉 SHORT'
        : '🟡 触发';
    const dirClean = lastBar.combined_direction || 0;
    const fired = lastBar.triggers.map((t) => t.component_label).join(' + ') || '（无组件）';
    const title = lastBar.combined_fired
      ? `${dirLabel} · ${r.symbol} ${r.interval}`
      : `组件触发 · ${r.symbol} ${r.interval}`;
    const body = `${state.currentSystem.name}\n${fired}\n价格: ${r.latest_close}`;

    try {
      const notif = new Notification(title, {
        body,
        tag: `aura-${r.symbol}-${r.interval}`, // 同一 symbol+interval 覆盖旧通知
        requireInteraction: lastBar.combined_fired, // 聚合信号要求用户点击
      });
      notif.onclick = () => {
        window.focus();
        notif.close();
      };
      playBeep(dirClean);
      state.lastNotifiedBarIndex = lastBar.bar_index;
    } catch (e) {
      console.warn('Notification failed', e);
    }
  }

  // ---------- K 线图 ----------
  function initChart() {
    if (!window.LightweightCharts) {
      console.warn('lightweight-charts 未加载');
      return;
    }
    const el = $('tr-chart');
    state.chart = LightweightCharts.createChart(el, {
      layout: { background: { color: '#0b0d14' }, textColor: '#8a93ab' },
      grid: {
        vertLines: { color: '#252a3b' },
        horzLines: { color: '#252a3b' },
      },
      rightPriceScale: { borderColor: '#252a3b' },
      timeScale: { borderColor: '#252a3b', timeVisible: true, secondsVisible: false },
      crosshair: { mode: LightweightCharts.CrosshairMode.Normal },
    });
    state.candleSeries = state.chart.addCandlestickSeries({
      upColor: '#4ade80',
      downColor: '#f87171',
      borderUpColor: '#4ade80',
      borderDownColor: '#f87171',
      wickUpColor: '#4ade80',
      wickDownColor: '#f87171',
    });
    // 容器缩放自适应
    window.addEventListener('resize', () => {
      if (state.chart) state.chart.applyOptions({ width: el.clientWidth });
    });
    state.chart.applyOptions({ width: el.clientWidth, height: el.clientHeight });
  }

  function updateChart(r) {
    if (!state.candleSeries || !state.chart) return;
    // lightweight-charts 要求 time 单调递增且唯一；open_time 秒级
    const seen = new Set();
    const candles = [];
    r.bars.forEach((b) => {
      const time = Math.floor(b.open_time / 1000);
      if (seen.has(time)) return; // 避免重复时间戳
      seen.add(time);
      candles.push({
        time,
        open: b.open,
        high: b.high,
        low: b.low,
        close: b.close,
      });
    });
    state.candleSeries.setData(candles);

    // 触发 bar 标记
    const markers = [];
    r.bars.forEach((b) => {
      if (!b.triggers.length && !b.combined_fired) return;
      const time = Math.floor(b.open_time / 1000);
      if (b.combined_fired) {
        // 聚合信号：金色大箭头
        markers.push({
          time,
          position: b.combined_direction > 0 ? 'belowBar' : 'aboveBar',
          color: '#fbbf24',
          shape: b.combined_direction > 0 ? 'arrowUp' : 'arrowDown',
          text: b.combined_direction > 0 ? 'LONG' : 'SHORT',
        });
      } else {
        // 仅组件触发：小圆点
        const dirs = b.triggers.map((t) => t.direction);
        const hasLong = dirs.some((d) => d > 0);
        const hasShort = dirs.some((d) => d < 0);
        markers.push({
          time,
          position: hasLong ? 'belowBar' : 'aboveBar',
          color: hasShort ? '#f87171' : '#4ade80',
          shape: 'circle',
          size: 0.8,
        });
      }
    });
    // lightweight-charts 要求 markers 按 time 升序
    markers.sort((a, b) => a.time - b.time);
    state.candleSeries.setMarkers(markers);
    state.chart.timeScale().fitContent();
  }

  function renderScan(r) {
    // Hero: 价格 + 信号
    $('tr-price').textContent = fmtPrice(r.latest_close);
    $('tr-price-meta').textContent = `${r.symbol} · ${r.interval} · ${fmtTime(r.latest_close_time)}`;

    const lastBar = r.bars.length ? r.bars[r.bars.length - 1] : null;
    const signalEl = $('tr-signal-val');
    const reasonEl = $('tr-signal-reason');
    if (lastBar && lastBar.combined_fired) {
      if (lastBar.combined_direction > 0) {
        signalEl.textContent = '📈 做多';
        signalEl.className = 'tr-signal-val long';
      } else {
        signalEl.textContent = '📉 做空';
        signalEl.className = 'tr-signal-val short';
      }
      const fired = lastBar.triggers.map((t) => t.component_label).join(' + ');
      reasonEl.textContent = `触发: ${fired}`;
    } else {
      signalEl.textContent = '无信号';
      signalEl.className = 'tr-signal-val';
      const fired = lastBar && lastBar.triggers.length > 0
        ? lastBar.triggers.map((t) => t.component_label).join(' + ')
        : '（无组件触发）';
      reasonEl.textContent = `最新 bar: ${fired}`;
    }

    // 最新 bar 组件触发状态
    renderComponents(r, lastBar);

    // 最近 N bar 信号表
    renderBarsTable(r);

    // K 线图（M16）
    updateChart(r);

    $('tr-latest-bar-hint').textContent =
      `最新 bar: #${lastBar?.bar_index ?? '—'} · 扫描了 ${r.total_bars} 根 K 线 · 图表显示最近 ${r.bars.length} 根`;
  }

  function renderComponents(r, lastBar) {
    const wrap = $('tr-components');
    const s = state.currentSystem;
    if (!s || !lastBar) {
      wrap.innerHTML = '<div class="loading">—</div>';
      return;
    }
    wrap.innerHTML = '';
    s.components.forEach((cid) => {
      const trigger = lastBar.triggers.find((t) => t.component_id === cid);
      const total = r.total_triggers_by_component[cid] || 0;
      const fired = !!trigger;
      const cls = fired
        ? (trigger.direction > 0 ? 'fired-long' : 'fired-short')
        : '';
      const mark = fired ? '✓' : '○';
      const markCls = fired ? 'ok' : 'no';
      const label = trigger ? trigger.component_label : cid;
      const reason = trigger ? trigger.reason : '未触发';
      const el = document.createElement('div');
      el.className = `tr-comp-item ${cls}`;
      el.innerHTML = `
        <div class="tr-comp-head">
          <span class="tr-comp-name">${label}</span>
          <span class="tr-comp-mark ${markCls}">${mark}</span>
        </div>
        <div class="tr-comp-id">${cid}</div>
        <div class="tr-comp-reason">${reason}</div>
        <div class="tr-comp-total">整段触发: ${total} 次</div>
      `;
      wrap.appendChild(el);
    });
  }

  function renderBarsTable(r) {
    const tbody = $('tr-bars-table').querySelector('tbody');
    tbody.innerHTML = '';
    // 倒序：最新在顶；只显示最近 10 根 + 所有聚合信号 bar
    const firedBars = r.bars.filter((b) => b.combined_fired);
    const latest10 = r.bars.slice(-10);
    const merged = Array.from(new Set([...firedBars, ...latest10])); // 去重
    merged.sort((a, b) => b.bar_index - a.bar_index);
    const bars = merged;
    bars.forEach((b) => {
      const fired = b.triggers.map((t) => t.component_label).join(' + ') || '—';
      let sigBadge;
      if (b.combined_fired && b.combined_direction > 0) {
        sigBadge = '<span class="tr-sig-badge long">📈 Long</span>';
      } else if (b.combined_fired && b.combined_direction < 0) {
        sigBadge = '<span class="tr-sig-badge short">📉 Short</span>';
      } else {
        sigBadge = '<span class="tr-sig-badge none">—</span>';
      }
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>#${b.bar_index}</td>
        <td>${fmtTime(b.close_time)}</td>
        <td>O ${fmtPrice(b.open)} / C ${fmtPrice(b.close)}</td>
        <td>${fired}</td>
        <td>${sigBadge}</td>
      `;
      tbody.appendChild(tr);
    });
  }

  function setStatus(msg, cls = '') {
    const el = $('tr-status');
    el.textContent = msg;
    el.className = 'run-status ' + cls;
    if (cls === 'success' || cls === 'error') {
      if (window.AuraToast) window.AuraToast.push(msg, cls);
    }
  }

  document.addEventListener('DOMContentLoaded', init);
})();
