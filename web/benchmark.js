/* =================================================================
   Aura · 基准热力图 — 前端脚本
   ================================================================= */

(() => {
  'use strict';

  const $ = (id) => document.getElementById(id);

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

  function fmtPct(v, digits = 1) {
    if (!isFinite(v)) return '—';
    const sign = v >= 0 ? '+' : '';
    return `${sign}${(v * 100).toFixed(digits)}%`;
  }
  function fmtNum(v, digits = 2) {
    if (!isFinite(v)) return '—';
    const sign = v >= 0 ? '+' : '';
    return `${sign}${v.toFixed(digits)}`;
  }

  /// 把一个数值映射到颜色（红-灰-绿渐变）
  /// value: 任意数；pivot 为中性值（默认 0）；scale 控制饱和度
  function colorFor(value, metric) {
    if (!isFinite(value)) return { bg: '#1a1f2e', fg: '#8a93ab' };
    let norm;
    if (metric === 'wf_consistency') {
      // 0..1 → -1..+1（以 0.5 为中性）
      norm = Math.max(-1, Math.min(1, (value - 0.5) * 2));
    } else if (metric === 'wf_avg_return_pct') {
      // 把 ±100% 映射到 ±1
      norm = Math.max(-1, Math.min(1, value));
    } else {
      // sharpe: ±1.5 饱和
      norm = Math.max(-1, Math.min(1, value / 1.5));
    }
    // 红（-1）→ 灰（0）→ 绿（+1）
    let r, g, b;
    if (norm >= 0) {
      // 灰(75,85,99) → 绿(20,83,45)
      const t = norm;
      r = Math.round(75 + (20 - 75) * t);
      g = Math.round(85 + (83 - 85) * t);
      b = Math.round(99 + (45 - 99) * t);
    } else {
      // 灰(75,85,99) → 红(127,29,29)
      const t = -norm;
      r = Math.round(75 + (127 - 75) * t);
      g = Math.round(85 + (29 - 85) * t);
      b = Math.round(99 + (29 - 99) * t);
    }
    const bg = `rgb(${r}, ${g}, ${b})`;
    // 根据亮度挑前景
    const lum = 0.299 * r + 0.587 * g + 0.114 * b;
    const fg = lum < 128 ? '#e8ebf4' : '#0b0d14';
    return { bg, fg };
  }

  function cellDisplay(cell, metric) {
    switch (metric) {
      case 'wf_avg_sharpe':
        return fmtNum(cell.wf_avg_sharpe);
      case 'wf_avg_return_pct':
        return fmtPct(cell.wf_avg_return_pct, 0);
      case 'wf_consistency':
        return `${(cell.wf_consistency * 100).toFixed(0)}%`;
      default:
        return '?';
    }
  }

  async function runBenchmark() {
    const btn = $('btn-bench');
    btn.disabled = true;

    const symbols = $('bm-symbols').value.split(/[,\s]+/).filter(Boolean).map((s) => s.toUpperCase());
    const intervals = $('bm-intervals').value.split(/[,\s]+/).filter(Boolean);
    const limit = parseInt($('bm-limit').value, 10) || 2000;
    const folds = parseInt($('bm-folds').value, 10) || 4;
    const metric = $('bm-color-metric').value;

    if (!symbols.length || !intervals.length) {
      setStatus('请至少填一个 symbol 和 interval', 'error');
      btn.disabled = false;
      return;
    }

    setStatus(`⏳ 运行中：${symbols.length} symbol × ${intervals.length} interval × N 体系 × ${folds} 折…`);

    try {
      const t0 = performance.now();
      const report = await fetchJson('/api/system/benchmark', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ symbols, intervals, limit, folds }),
      });
      const dt = performance.now() - t0;
      renderHeatmap(report, symbols, intervals, metric);
      setStatus(`✓ ${report.cells.length} cells 完成（${report.elapsed_ms}ms 后端 / ${dt.toFixed(0)}ms 端到端）`, 'success');
    } catch (e) {
      setStatus(`✗ 失败：${e.message}`, 'error');
    } finally {
      btn.disabled = false;
    }
  }

  function renderHeatmap(report, symbols, intervals, metric) {
    window.__lastBenchReport = report;
    const wrap = $('bm-heatmap-wrap');
    wrap.innerHTML = '';

    // 组织 cells 为 (system_id → (symbol_interval_key → cell))
    const bySystem = new Map();
    report.cells.forEach((c) => {
      if (!bySystem.has(c.system_id)) {
        bySystem.set(c.system_id, {
          name: c.system_name,
          id: c.system_id,
          cells: new Map(),
        });
      }
      const key = `${c.symbol}|${c.interval}`;
      bySystem.get(c.system_id).cells.set(key, c);
    });

    // 按 (主 WF Sharpe 均值) 给 system 排序
    const sortedSystems = Array.from(bySystem.values()).map((s) => {
      const values = Array.from(s.cells.values())
        .map((c) => c[metric])
        .filter((v) => isFinite(v));
      const avg = values.length ? values.reduce((a, b) => a + b, 0) / values.length : NaN;
      return { ...s, avg };
    });
    sortedSystems.sort((a, b) => (b.avg || -Infinity) - (a.avg || -Infinity));

    const metricLabel = $('bm-color-metric').selectedOptions[0].textContent;

    // 构造表
    const table = document.createElement('table');
    table.className = 'heatmap-table';

    // 第一行：symbol
    const thead1 = document.createElement('tr');
    thead1.innerHTML = `<th class="hm-corner" rowspan="2">体系（按 ${metricLabel} 均值降序）</th>`;
    symbols.forEach((s) => {
      thead1.innerHTML += `<th class="hm-symbol" colspan="${intervals.length}">${s}</th>`;
    });
    thead1.innerHTML += `<th class="hm-symbol" rowspan="2" title="跨 cell 均值">均值</th>`;
    table.appendChild(thead1);

    // 第二行：interval
    const thead2 = document.createElement('tr');
    symbols.forEach(() => {
      intervals.forEach((tf) => {
        thead2.innerHTML += `<th class="hm-tf">${tf}</th>`;
      });
    });
    table.appendChild(thead2);

    // 数据行
    sortedSystems.forEach((sys) => {
      const tr = document.createElement('tr');
      const isPromoted = sys.id.startsWith('promoted.');
      const icon = isPromoted ? '⭐' : '🌱';
      const nameCell = document.createElement('td');
      nameCell.className = 'hm-name';
      nameCell.innerHTML = `${icon} ${sys.name}`;
      nameCell.title = sys.id;
      nameCell.addEventListener('click', () => {
        window.open(`/system.html#${encodeURIComponent(sys.id)}`, '_blank');
      });
      tr.appendChild(nameCell);

      symbols.forEach((s) => {
        intervals.forEach((tf) => {
          const key = `${s}|${tf}`;
          const cell = sys.cells.get(key);
          const td = document.createElement('td');
          if (!cell || cell.error || !isFinite(cell[metric])) {
            td.className = 'hm-cell empty';
            td.textContent = cell && cell.error ? '✗' : '—';
            td.title = cell ? (cell.error || '无数据') : '缺失';
          } else {
            td.className = 'hm-cell';
            const { bg, fg } = colorFor(cell[metric], metric);
            td.style.background = bg;
            td.style.color = fg;
            td.textContent = cellDisplay(cell, metric);
            td.addEventListener('mouseenter', (ev) => showTooltip(ev, cell));
            td.addEventListener('mousemove', moveTooltip);
            td.addEventListener('mouseleave', hideTooltip);
            td.addEventListener('click', () => {
              window.open(`/system.html#${encodeURIComponent(sys.id)}`, '_blank');
            });
          }
          tr.appendChild(td);
        });
      });

      // 均值列
      const aggTd = document.createElement('td');
      aggTd.className = 'hm-agg';
      if (isFinite(sys.avg)) {
        const { bg, fg } = colorFor(sys.avg, metric);
        aggTd.style.background = bg;
        aggTd.style.color = fg;
        aggTd.textContent = cellDisplay(
          Object.fromEntries([[metric, sys.avg]]),
          metric,
        );
      } else {
        aggTd.textContent = '—';
      }
      tr.appendChild(aggTd);
      table.appendChild(tr);
    });

    wrap.appendChild(table);

    // 图例
    const legend = document.createElement('div');
    legend.className = 'hm-legend';
    legend.innerHTML = `
      <span>← 负</span>
      <div class="hm-legend-bar"></div>
      <span>正 →</span>
      <span style="margin-left:20px;">着色维度：${metricLabel}</span>
    `;
    wrap.appendChild(legend);

    $('bm-hint').textContent =
      `${sortedSystems.length} 体系 × ${symbols.length} symbol × ${intervals.length} 周期 = ${report.cells.length} cells，` +
      `折数=${report.folds}，后端 ${report.elapsed_ms}ms。点击单元格或体系名跳转到体系实验室。`;
  }

  // ---------- Tooltip ----------
  let tooltipEl = null;
  function ensureTooltip() {
    if (!tooltipEl) {
      tooltipEl = document.createElement('div');
      tooltipEl.className = 'hm-tooltip';
      tooltipEl.hidden = true;
      document.body.appendChild(tooltipEl);
    }
    return tooltipEl;
  }
  function showTooltip(ev, cell) {
    const t = ensureTooltip();
    t.innerHTML = `
      <div class="head">${cell.system_name}</div>
      <div class="row">${cell.symbol} · ${cell.interval}</div>
      <div class="row">WF Cons<span class="v">${(cell.wf_consistency * 100).toFixed(0)}%</span></div>
      <div class="row">WF Sharpe<span class="v">${fmtNum(cell.wf_avg_sharpe)}</span></div>
      <div class="row">WF Sharpe Std<span class="v">${fmtNum(cell.wf_sharpe_std)}</span></div>
      <div class="row">WF Avg Return<span class="v">${fmtPct(cell.wf_avg_return_pct)}</span></div>
      <div class="row">Trades<span class="v">${cell.total_trades}</span></div>
    `;
    t.hidden = false;
    moveTooltip(ev);
  }
  function moveTooltip(ev) {
    if (!tooltipEl) return;
    const pad = 14;
    let x = ev.clientX + pad;
    let y = ev.clientY + pad;
    const rect = tooltipEl.getBoundingClientRect();
    if (x + rect.width > window.innerWidth) x = ev.clientX - rect.width - pad;
    if (y + rect.height > window.innerHeight) y = ev.clientY - rect.height - pad;
    tooltipEl.style.left = `${x}px`;
    tooltipEl.style.top = `${y}px`;
  }
  function hideTooltip() {
    if (tooltipEl) tooltipEl.hidden = true;
  }

  function setStatus(msg, cls = '') {
    const el = $('bm-status');
    el.textContent = msg;
    el.className = 'run-status ' + cls;
    if (cls === 'success' || cls === 'error') {
      if (window.AuraToast) window.AuraToast.push(msg, cls);
    }
  }

  document.addEventListener('DOMContentLoaded', () => {
    $('btn-bench').addEventListener('click', runBenchmark);
    // 颜色指标切换时，重新渲染（需缓存上次 report）
    $('bm-color-metric').addEventListener('change', () => {
      if (window.__lastBenchReport) {
        const symbols = $('bm-symbols').value.split(/[,\s]+/).filter(Boolean).map((s) => s.toUpperCase());
        const intervals = $('bm-intervals').value.split(/[,\s]+/).filter(Boolean);
        renderHeatmap(window.__lastBenchReport, symbols, intervals, $('bm-color-metric').value);
      }
    });
  });

})();
