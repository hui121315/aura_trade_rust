/* =================================================================
   Aura · 体系实验室 — 独立前端脚本
   =================================================================
   职责：
   1. 从后端 API 拉组件列表 / 种子体系
   2. 管理用户对组件的选择 + 聚合规则配置
   3. 提交 POST /api/system/run 跑回测
   4. 渲染结果：KPI 网格 / 组件归因 / 交易列表
   ================================================================= */

(() => {
  'use strict';

  // ---------- 状态 ----------
  const state = {
    components: [],              // 全部 32 个组件
    componentsByDim: {},         // 按维度分组
    seeds: [],                    // 全部 8 个种子体系
    selected: new Set(),          // 已选组件 ID
    weights: {},                  // 各组件权重（仅 WeightedScore 用）
    seedSortKey: 'default',       // M13：种子列表排序键
    lastDiscoverySections: null,  // M14：缓存最近一次 Discovery 结果供导出报告
  };

  // 维度中文标签
  const DIM_LABELS = {
    MaSignal: '均线信号（葛南维）',
    MaAdvanced: '均线高级形态',
    MaSpecial: '均线特殊形态',
    CandlePattern: 'K 线形态',
    ChartPattern: '技术图形',
    TrendStructure: '趋势结构（道氏）',
  };

  // ---------- 工具 ----------
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

  function fmtPct(v, digits = 2) {
    if (!isFinite(v)) return '—';
    const sign = v >= 0 ? '+' : '';
    return `${sign}${(v * 100).toFixed(digits)}%`;
  }
  function fmtNum(v, digits = 2) {
    if (!isFinite(v)) return '—';
    const sign = v >= 0 ? '+' : '';
    return `${sign}${v.toFixed(digits)}`;
  }

  // ---------- 初始化 ----------
  async function init() {
    try {
      const [comps, seedsResp] = await Promise.all([
        fetchJson('/api/system/components'),
        fetchJson('/api/system/seeds'),
      ]);
      state.componentsByDim = comps.by_dimension;
      // 平铺一份便于查找
      state.components = Object.values(comps.by_dimension).flat();
      state.seeds = seedsResp.seeds;

      renderSeedList();
      renderComponentSelector();
      updateSelectedCount();

      // 绑定聚合规则切换
      $('combine-type').addEventListener('change', onCombineTypeChange);
      $('btn-run').addEventListener('click', runBacktest);
      $('btn-walkforward').addEventListener('click', runWalkForward);
      $('btn-discover').addEventListener('click', runDiscovery);
      $('btn-discover-both').addEventListener('click', runDiscoveryBoth);
      // M13: 种子列表排序
      $('seed-sort').addEventListener('change', (ev) => {
        state.seedSortKey = ev.target.value;
        renderSeedList();
      });
      // M14: 导出 Markdown 报告
      $('btn-export-report').addEventListener('click', exportMarkdownReport);
      // 打磨：清空组件
      $('btn-clear-components').addEventListener('click', () => {
        state.selected.clear();
        state.weights = {};
        renderComponentSelector();
        setStatus('已清空组件选择');
      });

      // 默认加载第一个种子（金山谷·蛟龙出海：冠军体系）
      const champion = state.seeds.find((s) => s.id === 'seed.golden_dragon');
      if (champion) loadSeed(champion);
    } catch (e) {
      $('run-status').textContent = `初始化失败：${e.message}`;
      $('run-status').classList.add('error');
    }
  }

  // ---------- M14: 导出 Markdown 报告 ----------
  /// 汇总当前所有数据（种子 + benchmark + 最近 Discovery）生成 Markdown 下载
  async function exportMarkdownReport() {
    const btn = $('btn-export-report');
    btn.disabled = true;
    setStatus('⏳ 正在组装研究报告…');
    try {
      // 拉最新全局 benchmark（3 symbol × 1d）
      let bench = null;
      try {
        bench = await fetchJson('/api/system/benchmark', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            symbols: ['BTCUSDT', 'ETHUSDT', 'SOLUSDT'],
            intervals: ['4h', '1d'],
            limit: 2000,
            folds: 4,
          }),
        });
      } catch (e) {
        // 失败也继续，只是报告不带 benchmark 段
        console.warn('benchmark failed', e);
      }

      const md = renderReportMarkdown(state.seeds, bench, state.lastDiscoverySections);
      const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      downloadText(`aura-report-${ts}.md`, md);
      setStatus(`✓ 报告已下载（${(md.length / 1024).toFixed(1)} KB）`, 'success');
    } catch (e) {
      setStatus(`✗ 导出失败：${e.message}`, 'error');
    } finally {
      btn.disabled = false;
    }
  }

  function downloadText(filename, text) {
    const blob = new Blob([text], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    setTimeout(() => {
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    }, 100);
  }

  /// 把各份数据组装成一份 Markdown 文档（中文友好，GitHub 风格表格）
  function renderReportMarkdown(seeds, benchReport, discoverySections) {
    const now = new Date();
    const iso = now.toISOString();

    let md = `# Aura · 交易体系研究报告\n\n`;
    md += `> 自动生成于 ${iso}\n\n`;
    md += `## 1. 概览\n\n`;
    md += `- 种子体系总数：**${seeds.length}** 个\n`;
    md += `  - 硬编码 Seed：${seeds.filter((s) => s.origin === 'Seed').length}\n`;
    md += `  - 已入库 Discovered：${seeds.filter((s) => s.origin === 'Discovered').length}\n`;
    if (benchReport) {
      md += `- 基准矩阵：${benchReport.cells.length} cells（${benchReport.folds} 折 WF，后端 ${benchReport.elapsed_ms}ms）\n`;
    }
    if (discoverySections) {
      const combosTotal = discoverySections.reduce((a, s) => a + (s.report.total_combinations_tried || 0), 0);
      md += `- 最近一次 Discovery：${discoverySections.length} 方向，合计试过 ${combosTotal} 组合\n`;
    }
    md += `\n`;

    // 2. 种子体系排行（按 benchmark 平均 Sharpe）
    md += `## 2. 种子体系排行（按 last_benchmark 平均 Sharpe 降序）\n\n`;
    md += `| 排名 | 体系 | Origin | 聚合规则 | 组件数 | 平均 Sharpe | 平均 Cons | BTC Sharpe |\n`;
    md += `|---:|---|---|---|---:|---:|---:|---:|\n`;
    const ranked = seeds.slice().sort(
      (a, b) => seedSortValue(b, 'avg_sharpe') - seedSortValue(a, 'avg_sharpe'),
    );
    ranked.forEach((s, i) => {
      const avgSh = seedSortValue(s, 'avg_sharpe');
      const avgCo = seedSortValue(s, 'avg_cons');
      const btcSh = seedSortValue(s, 'btc_sharpe');
      const rule = s.combine.type + (s.combine.k !== undefined ? ` k=${s.combine.k}` : '');
      const fmt = (v, pct = false) => {
        if (!isFinite(v)) return '—';
        return pct ? `${(v * 100).toFixed(0)}%` : (v >= 0 ? '+' : '') + v.toFixed(2);
      };
      md += `| ${i + 1} | ${s.name} | ${s.origin} | ${rule} | ${s.components.length} | ${fmt(avgSh)} | ${fmt(avgCo, true)} | ${fmt(btcSh)} |\n`;
    });
    md += `\n`;

    // 2.5 Promoted 体系详情
    const promoted = seeds.filter((s) => s.origin === 'Discovered');
    if (promoted.length > 0) {
      md += `## 3. Promoted（已入库）体系详情\n\n`;
      promoted.forEach((s) => {
        md += `### ${s.name}\n\n`;
        md += `- **ID**: \`${s.id}\`\n`;
        md += `- **聚合规则**: ${s.combine.type}${s.combine.k !== undefined ? ` (k=${s.combine.k})` : ''}${s.combine.window_bars !== undefined ? ` (window=${s.combine.window_bars})` : ''}${s.combine.threshold !== undefined ? ` (threshold=${s.combine.threshold})` : ''}\n`;
        md += `- **组件**: ${s.components.map((c) => `\`${c}\``).join(' + ')}\n`;
        if (s.description) md += `- **描述**: ${s.description}\n`;
        const bench = (s.meta && s.meta.last_benchmark) || [];
        if (bench.length > 0) {
          md += `- **基准快照**:\n\n`;
          md += `  | Symbol | Interval | Cons | Sharpe | Return | Trades |\n`;
          md += `  |---|---|---:|---:|---:|---:|\n`;
          bench.forEach((b) => {
            const ret = isFinite(b.wf_avg_return_pct) ? ((b.wf_avg_return_pct >= 0 ? '+' : '') + (b.wf_avg_return_pct * 100).toFixed(1) + '%') : '—';
            const sh = isFinite(b.wf_avg_sharpe) ? ((b.wf_avg_sharpe >= 0 ? '+' : '') + b.wf_avg_sharpe.toFixed(2)) : '—';
            md += `  | ${b.symbol} | ${b.interval} | ${(b.wf_consistency * 100).toFixed(0)}% | ${sh} | ${ret} | ${b.total_trades} |\n`;
          });
        }
        md += `\n`;
      });
    }

    // 4. 全局 Benchmark 热力图数据
    if (benchReport && benchReport.cells && benchReport.cells.length > 0) {
      md += `## 4. 全局基准矩阵（${benchReport.folds} 折 WF）\n\n`;
      md += `| 体系 | Symbol | Interval | Cons | Sharpe | Return | Trades |\n`;
      md += `|---|---|---|---:|---:|---:|---:|\n`;
      benchReport.cells.forEach((c) => {
        if (c.error) {
          md += `| ${c.system_name} | ${c.symbol} | ${c.interval} | — | — | — | 错误: ${c.error} |\n`;
          return;
        }
        const sh = isFinite(c.wf_avg_sharpe) ? ((c.wf_avg_sharpe >= 0 ? '+' : '') + c.wf_avg_sharpe.toFixed(2)) : '—';
        const ret = isFinite(c.wf_avg_return_pct) ? ((c.wf_avg_return_pct >= 0 ? '+' : '') + (c.wf_avg_return_pct * 100).toFixed(1) + '%') : '—';
        md += `| ${c.system_name} | ${c.symbol} | ${c.interval} | ${(c.wf_consistency * 100).toFixed(0)}% | ${sh} | ${ret} | ${c.total_trades} |\n`;
      });
      md += `\n`;
    }

    // 5. 最近 Discovery 结果
    if (discoverySections && discoverySections.length > 0) {
      md += `## 5. 最近一次 Discovery\n\n`;
      discoverySections.forEach(({ direction, report }) => {
        const dirLabel = direction > 0 ? '📈 多头 (Long)' : '📉 空头 (Short)';
        md += `### ${dirLabel}\n\n`;
        md += `- 试过 **${report.total_combinations_tried || 0}** 组合，耗时 ${report.elapsed_ms || 0}ms\n`;
        md += `- Top ${(report.top_k || []).length}:\n\n`;
        md += `| # | 体系组件 | 规则 | Composite | 主 Sharpe | 跨市场 Sharpe | 跨市场 Cons |\n`;
        md += `|---:|---|---|---:|---:|---:|---:|\n`;
        (report.top_k || []).forEach((c) => {
          const comps = c.definition.components.map((x) => x.split('.', 2)[1] || x).join(' + ');
          const rule = c.definition.combine.type + (c.definition.combine.k !== undefined ? ` k=${c.definition.combine.k}` : '');
          const fmt = (v, pct = false) => {
            if (!isFinite(v)) return '—';
            return pct ? `${(v * 100).toFixed(0)}%` : (v >= 0 ? '+' : '') + v.toFixed(2);
          };
          md += `| ${c.rank} | ${comps} | ${rule} | ${fmt(c.composite_score)} | ${fmt(c.wf_avg_sharpe)} | ${fmt(c.cross_sharpe_mean)} | ${fmt(c.cross_consistency_mean, true)} |\n`;
        });
        md += `\n`;

        // 组件频度统计
        const freq = {};
        (report.top_k || []).forEach((c) => {
          c.definition.components.forEach((cid) => {
            freq[cid] = (freq[cid] || 0) + 1;
          });
        });
        const ranked = Object.entries(freq).sort((a, b) => b[1] - a[1]);
        if (ranked.length > 0) {
          md += `**${dirLabel} Top-K 中的组件频度**（出现次数越多，说明该组件越是核心）:\n\n`;
          ranked.forEach(([cid, n]) => {
            md += `- \`${cid}\`: **${n}** 次\n`;
          });
          md += `\n`;
        }
      });
    }

    // 6. 建议
    md += `## 6. 作战建议\n\n`;
    const recommended = seeds.filter((s) => {
      const bench = (s.meta && s.meta.last_benchmark) || [];
      if (bench.length === 0) return false;
      const vals = bench.map((b) => b.wf_avg_sharpe).filter(isFinite);
      const conss = bench.map((b) => b.wf_consistency).filter(isFinite);
      if (vals.length === 0) return false;
      const avgSh = vals.reduce((a, b) => a + b, 0) / vals.length;
      const avgCo = conss.reduce((a, b) => a + b, 0) / conss.length;
      return avgSh >= 0.5 && avgCo >= 0.75;
    });
    if (recommended.length > 0) {
      md += `**建议重点关注**（平均 Sharpe ≥ 0.5 且 一致性 ≥ 75%）：\n\n`;
      recommended.forEach((s) => {
        md += `- **${s.name}** (\`${s.id}\`)\n`;
      });
    } else {
      md += `当前尚无同时满足 *Sharpe ≥ 0.5* 且 *一致性 ≥ 75%* 的体系。建议：\n`;
      md += `- 用 Discovery 继续挖掘高 composite 组合\n`;
      md += `- 对候选体系用 Walk-Forward 交叉验证（包含 4h / 1w 多周期）\n`;
      md += `- 入库 composite ≥ 0.5 的体系，长期跟踪\n`;
    }
    md += `\n`;

    md += `---\n\n_由 Aura Trade 自动生成_\n`;
    return md;
  }

  // ---------- 种子体系列表 ----------
  /// 从 last_benchmark 中提取排序键值（NaN 视为最低，排最后）
  function seedSortValue(seed, key) {
    const bench = (seed.meta && seed.meta.last_benchmark) || [];
    if (key === 'name') return seed.name.toLowerCase();
    if (bench.length === 0) return -Infinity;
    if (key === 'avg_sharpe') {
      const vals = bench.map((b) => b.wf_avg_sharpe).filter(isFinite);
      return vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : -Infinity;
    }
    if (key === 'btc_sharpe') {
      const btc = bench.find((b) => b.symbol === 'BTCUSDT');
      return btc && isFinite(btc.wf_avg_sharpe) ? btc.wf_avg_sharpe : -Infinity;
    }
    if (key === 'avg_cons') {
      const vals = bench.map((b) => b.wf_consistency).filter(isFinite);
      return vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : -Infinity;
    }
    return 0;
  }

  function renderSeedList() {
    const wrap = $('seed-list');
    wrap.innerHTML = '';
    // M13：按配置排序
    let sorted = state.seeds.slice();
    const key = state.seedSortKey;
    if (key === 'name') {
      sorted.sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'));
    } else if (key !== 'default') {
      sorted.sort((a, b) => seedSortValue(b, key) - seedSortValue(a, key));
    }
    sorted.forEach((s) => {
      const row = document.createElement('div');
      row.className = 'seed-item';
      const originIcon = s.origin === 'Discovered' ? '⭐' : s.origin === 'User' ? '👤' : '🌱';
      const isPromoted = s.origin === 'Discovered' && s.id.startsWith('promoted.');

      // M10: 渲染 benchmark 摘要（若有）
      let benchHtml = '';
      const bench = (s.meta && s.meta.last_benchmark) || [];
      if (bench.length > 0) {
        const parts = bench.map((b) => {
          const symShort = b.symbol.replace('USDT', '');
          const sh = b.wf_avg_sharpe;
          const cls = sh >= 0.5 ? 'bench-good' : sh >= 0 ? 'bench-ok' : 'bench-bad';
          const txt = isFinite(sh) ? (sh >= 0 ? '+' : '') + sh.toFixed(2) : '—';
          return `<span class="${cls}" title="${b.symbol} ${b.interval}: cons ${(b.wf_consistency * 100).toFixed(0)}% · avg_ret ${(b.wf_avg_return_pct * 100).toFixed(1)}% · ${b.total_trades} trades">${symShort} ${txt}</span>`;
        }).join(' ');
        benchHtml = `<div class="seed-bench">${parts}</div>`;
      }

      row.innerHTML = `
        <div class="seed-main">
          <span class="name" title="${s.description || ''}">${originIcon} ${s.name}</span>
          <span class="seed-actions">
            <span class="badge">${s.combine.type} · ${s.components.length}</span>
            ${isPromoted ? '<button class="seed-demote-btn" type="button" title="从本地入库中移除">🗑</button>' : ''}
          </span>
        </div>
        ${benchHtml}
      `;
      row.addEventListener('click', () => loadSeed(s));
      if (isPromoted) {
        row.querySelector('.seed-demote-btn').addEventListener('click', (ev) => {
          ev.stopPropagation();
          demoteSeed(s.id);
        });
      }
      wrap.appendChild(row);
    });
  }

  function loadSeed(seed) {
    // 1) 清空当前选择
    state.selected.clear();
    seed.components.forEach((cid) => state.selected.add(cid));
    // 2) 填聚合规则
    const ct = seed.combine.type;
    $('combine-type').value = ct;
    if (ct === 'MajorityK') $('combine-k').value = seed.combine.k ?? 2;
    if (ct === 'WeightedScore') $('combine-threshold').value = seed.combine.threshold ?? 1.5;
    if (ct === 'SequentialCascade') $('combine-window').value = seed.combine.window_bars ?? 10;
    // 3) 填风控
    $('stop-atr').value = seed.risk.stop_atr_mult;
    $('target-r').value = seed.risk.target_r;
    $('max-hold').value = seed.risk.max_hold_bars;
    // 4) 保留权重
    state.weights = { ...(seed.weights || {}) };
    // 5) 刷新 UI
    onCombineTypeChange();
    renderComponentSelector();
    updateSelectedCount();
    setStatus(`已加载：${seed.name}（${seed.components.length} 个组件）`, 'success');
  }

  // ---------- 组件选择器 ----------
  function renderComponentSelector() {
    const wrap = $('component-selector');
    wrap.innerHTML = '';
    const dimOrder = [
      'MaSignal', 'MaSpecial', 'MaAdvanced',
      'TrendStructure', 'CandlePattern', 'ChartPattern',
    ];
    for (const dim of dimOrder) {
      const list = state.componentsByDim[dim];
      if (!list) continue;
      const group = document.createElement('div');
      group.className = 'dim-group';
      const title = document.createElement('h3');
      title.textContent = `${DIM_LABELS[dim] || dim}  ·  ${list.length} 个`;
      group.appendChild(title);

      const chips = document.createElement('div');
      chips.className = 'comp-chip-list';
      list.forEach((c) => {
        const chip = document.createElement('button');
        chip.type = 'button';
        chip.className = 'comp-chip';
        if (state.selected.has(c.id)) chip.classList.add('selected');
        const dirCls = c.direction_bias > 0 ? 'dir-up' : (c.direction_bias < 0 ? 'dir-dn' : '');
        chip.innerHTML = `<span class="${dirCls}"></span>${c.label}`;
        chip.title = `${c.id}\n${c.book_source}`;
        chip.addEventListener('click', () => toggleComponent(c.id));
        chips.appendChild(chip);
      });
      group.appendChild(chips);
      wrap.appendChild(group);
    }
    const info = document.createElement('div');
    info.className = 'selected-count';
    info.innerHTML = `已选 <span class="n">${state.selected.size}</span> / 5 个组件`;
    wrap.appendChild(info);
  }

  function toggleComponent(cid) {
    if (state.selected.has(cid)) {
      state.selected.delete(cid);
    } else {
      if (state.selected.size >= 5) {
        setStatus('最多只能选 5 个组件', 'error');
        return;
      }
      state.selected.add(cid);
    }
    renderComponentSelector();
    updateSelectedCount();
  }

  function updateSelectedCount() {
    // already rendered inside selector; left as placeholder for future
  }

  // ---------- 聚合规则 ----------
  function onCombineTypeChange() {
    const t = $('combine-type').value;
    $('row-k').hidden = t !== 'MajorityK';
    $('row-threshold').hidden = t !== 'WeightedScore';
    $('row-window').hidden = t !== 'SequentialCascade';
  }

  function buildCombine() {
    const t = $('combine-type').value;
    if (t === 'AllAligned') return { type: 'AllAligned' };
    if (t === 'MajorityK')
      return { type: 'MajorityK', k: parseInt($('combine-k').value, 10) || 1 };
    if (t === 'WeightedScore')
      return { type: 'WeightedScore', threshold: parseFloat($('combine-threshold').value) || 1.0 };
    if (t === 'SequentialCascade')
      return { type: 'SequentialCascade', window_bars: parseInt($('combine-window').value, 10) || 10 };
    return { type: 'AllAligned' };
  }

  // ---------- 跑回测 ----------
  async function runBacktest() {
    if (state.selected.size === 0) {
      setStatus('请至少选择一个组件', 'error');
      return;
    }
    const btn = $('btn-run');
    btn.disabled = true;
    setStatus('⏳ 正在跑回测（拉数据 + 扫描 + 回测）…');

    const payload = {
      definition: {
        id: 'custom.interactive',
        name: '交互式自定义体系',
        origin: 'User',
        description: null,
        components: Array.from(state.selected),
        combine: buildCombine(),
        weights: state.weights || {},
        risk: {
          stop_atr_mult: parseFloat($('stop-atr').value) || 2.0,
          target_r: parseFloat($('target-r').value) || 3.0,
          max_hold_bars: parseInt($('max-hold').value, 10) || 30,
          max_position_pct: 0.5,
        },
        backtest: {
          warmup_bars: 60,
          cost_model: { mode: 'Zero' },
        },
        meta: {
          schema_version: 1,
          created_at_ms: Date.now(),
          last_backtested_ms: null,
          total_trades: 0,
          win_rate: null,
          total_return_pct: null,
          sharpe: null,
          max_drawdown_pct: null,
        },
      },
      symbol: $('symbol').value,
      interval: $('interval').value,
      limit: parseInt($('limit').value, 10) || 1000,
    };

    try {
      const t0 = performance.now();
      const result = await fetchJson('/api/system/run', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const dt = performance.now() - t0;
      renderResults(result);
      setStatus(`✓ 完成（${dt.toFixed(0)} ms），${result.performance.total_trades} 笔交易`, 'success');
    } catch (e) {
      setStatus(`✗ 失败：${e.message}`, 'error');
    } finally {
      btn.disabled = false;
    }
  }

  // ---------- Walk-Forward ----------
  async function runWalkForward() {
    if (state.selected.size === 0) {
      setStatus('请至少选择一个组件', 'error');
      return;
    }
    const btn = $('btn-walkforward');
    const btnRun = $('btn-run');
    btn.disabled = true;
    btnRun.disabled = true;
    const folds = parseInt($('wf-folds').value, 10) || 4;
    const limit = parseInt($('wf-limit').value, 10) || 2000;
    setStatus(`⏳ 跑 Walk-Forward（${folds} 折 × ${limit} 根）…`);

    const def = buildDefinition();
    const payload = {
      definition: def,
      symbol: $('symbol').value,
      interval: $('interval').value,
      limit,
      folds,
      prewarm_bars: 0,
    };
    try {
      const t0 = performance.now();
      const report = await fetchJson('/api/system/walkforward', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const dt = performance.now() - t0;
      renderWalkForward(report);
      const ratio = (report.aggregate.consistency_ratio * 100).toFixed(0);
      setStatus(`✓ Walk-Forward 完成（${dt.toFixed(0)} ms），盈利折 ${ratio}%`, 'success');
    } catch (e) {
      setStatus(`✗ Walk-Forward 失败：${e.message}`, 'error');
    } finally {
      btn.disabled = false;
      btnRun.disabled = false;
    }
  }

  /// 用当前 UI 状态构造 SystemDefinition JSON
  function buildDefinition() {
    return {
      id: 'custom.interactive',
      name: '交互式自定义体系',
      origin: 'User',
      description: null,
      components: Array.from(state.selected),
      combine: buildCombine(),
      weights: state.weights || {},
      risk: {
        stop_atr_mult: parseFloat($('stop-atr').value) || 2.0,
        target_r: parseFloat($('target-r').value) || 3.0,
        max_hold_bars: parseInt($('max-hold').value, 10) || 30,
        max_position_pct: 0.5,
      },
      backtest: { warmup_bars: 60, cost_model: { mode: 'Zero' } },
      meta: {
        schema_version: 1,
        created_at_ms: Date.now(),
        last_backtested_ms: null,
        total_trades: 0,
        win_rate: null,
        total_return_pct: null,
        sharpe: null,
        max_drawdown_pct: null,
      },
    };
  }

  function renderWalkForward(report) {
    const card = $('result-walkforward');
    card.hidden = false;
    // 隐藏单次回测专属卡片（避免混淆）
    $('result-placeholder').hidden = true;

    $('wf-hint').textContent =
      `${report.folds.length} 折 × ${report.symbol} ${report.interval} · ` +
      `每折独立 warmup + 独立回测 · 总交易 ${report.aggregate.total_trades}`;

    const agg = report.aggregate;
    const consClass = agg.consistency_ratio >= 0.75 ? 'pos'
                    : agg.consistency_ratio >= 0.5 ? ''
                    : 'neg';
    const aggEls = [
      { label: '盈利折 (一致性)', value: `${(agg.consistency_ratio * 100).toFixed(0)}%`, cls: consClass },
      { label: '平均 Sharpe', value: fmtNum(agg.avg_sharpe), cls: agg.avg_sharpe >= 0 ? 'pos' : 'neg' },
      { label: 'Sharpe 标准差', value: fmtNum(agg.sharpe_std), cls: '' },
      { label: '平均总收益', value: fmtPct(agg.avg_return_pct), cls: agg.avg_return_pct >= 0 ? 'pos' : 'neg' },
      { label: '平均胜率', value: `${(agg.avg_win_rate * 100).toFixed(1)}%`, cls: agg.avg_win_rate >= 0.5 ? 'pos' : '' },
      { label: '平均最大回撤', value: `${(agg.avg_max_dd_pct * 100).toFixed(2)}%`, cls: 'neg' },
    ];
    const grid = $('wf-aggregate');
    grid.innerHTML = '';
    aggEls.forEach((k) => {
      const el = document.createElement('div');
      el.className = 'kpi-item';
      el.innerHTML = `<div class="label">${k.label}</div><div class="value ${k.cls}">${k.value}</div>`;
      grid.appendChild(el);
    });

    // 各折表
    const tbody = $('wf-folds-table').querySelector('tbody');
    tbody.innerHTML = '';
    report.folds.forEach((f) => {
      const p = f.performance;
      const retCls = p.total_return_pct >= 0 ? 'pnl-pos' : 'pnl-neg';
      const shCls = p.sharpe >= 0 ? 'pnl-pos' : 'pnl-neg';
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>#${f.fold_index + 1}</td>
        <td>${f.start_bar} → ${f.end_bar}</td>
        <td>${p.total_trades}</td>
        <td>${(p.win_rate * 100).toFixed(1)}%</td>
        <td class="${retCls}">${fmtPct(p.total_return_pct)}</td>
        <td class="${shCls}">${fmtNum(p.sharpe)}</td>
        <td>${(p.max_drawdown_pct * 100).toFixed(2)}%</td>
      `;
      tbody.appendChild(tr);
    });
  }

  // ---------- Discovery ----------
  /// 构造 Discovery 请求 payload（提取为工具函数，供单向/双向共用）
  function buildDiscoveryPayload(direction) {
    const maxSize = parseInt($('dc-max-size').value, 10);
    const topK = parseInt($('dc-top-k').value, 10) || 10;
    const wfFolds = parseInt($('dc-wf-folds').value, 10);
    const limit = parseInt($('wf-limit').value, 10) || 2000;

    const crossRawS = ($('dc-cross-symbols').value || '').trim();
    const crossSymbols = crossRawS
      ? crossRawS.split(/[,\s]+/).map((s) => s.trim().toUpperCase()).filter(Boolean)
      : [];
    const crossRawT = ($('dc-cross-intervals').value || '').trim();
    const crossIntervals = crossRawT
      ? crossRawT.split(/[,\s]+/).map((s) => s.trim().toLowerCase()).filter(Boolean)
      : [];

    return {
      symbol: $('symbol').value,
      cross_symbols: crossSymbols,
      cross_intervals: crossIntervals,
      interval: $('interval').value,
      limit,
      direction,
      min_size: 2,
      max_size: maxSize,
      top_k: topK,
      wf_folds: wfFolds,
      enable_majority: true,
      enable_all_aligned: true,
    };
  }

  async function runDiscovery() {
    const btn = $('btn-discover');
    const btnBoth = $('btn-discover-both');
    btn.disabled = true;
    btnBoth.disabled = true;
    const direction = parseInt($('dc-direction').value, 10);
    const dirLabel = direction > 0 ? 'Long' : 'Short';
    setStatus(`⏳ 自动发现中（direction=${dirLabel}）…`);

    const payload = buildDiscoveryPayload(direction);
    try {
      const t0 = performance.now();
      const r = await fetchJson('/api/system/discover', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const dt = performance.now() - t0;
      renderDiscovery([{ direction, report: r }]);
      setStatus(`✓ ${dirLabel} 发现 ${r.top_k.length} 个体系（试了 ${r.total_combinations_tried} 组合，${dt.toFixed(0)} ms）`, 'success');
    } catch (e) {
      setStatus(`✗ Discovery 失败：${e.message}`, 'error');
    } finally {
      btn.disabled = false;
      btnBoth.disabled = false;
    }
  }

  /// M12：并行发两次 discover 请求（Long + Short），结果垂直平铺
  async function runDiscoveryBoth() {
    const btn = $('btn-discover');
    const btnBoth = $('btn-discover-both');
    btn.disabled = true;
    btnBoth.disabled = true;
    setStatus('⏳ 双向发现中（Long + Short 并行）…');

    const payloadLong = buildDiscoveryPayload(1);
    const payloadShort = buildDiscoveryPayload(-1);
    try {
      const t0 = performance.now();
      const [rLong, rShort] = await Promise.all([
        fetchJson('/api/system/discover', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payloadLong),
        }),
        fetchJson('/api/system/discover', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payloadShort),
        }).catch((e) => ({ error: e.message, top_k: [], total_combinations_tried: 0, elapsed_ms: 0 })),
      ]);
      const dt = performance.now() - t0;
      renderDiscovery([
        { direction: 1, report: rLong },
        { direction: -1, report: rShort },
      ]);
      setStatus(
        `✓ 双向完成（Long ${rLong.top_k.length} / Short ${rShort.top_k ? rShort.top_k.length : 0}，${dt.toFixed(0)} ms）`,
        'success',
      );
    } catch (e) {
      setStatus(`✗ 双向发现失败：${e.message}`, 'error');
    } finally {
      btn.disabled = false;
      btnBoth.disabled = false;
    }
  }

  /// M12: renderDiscovery 接受 `[{direction, report}, ...]`，按分段垂直平铺
  function renderDiscovery(sections) {
    state.lastDiscoverySections = sections; // M14: 缓存供导出用
    $('result-placeholder').hidden = true;
    const card = $('result-discovery');
    card.hidden = false;
    const container = $('dc-sections');
    container.innerHTML = '';

    // 汇总 hint
    const totalCombos = sections.reduce((a, s) => a + (s.report.total_combinations_tried || 0), 0);
    const totalElapsed = sections.reduce((a, s) => a + (s.report.elapsed_ms || 0), 0);
    const firstReport = sections[0] && sections[0].report;
    const crossPts = firstReport && firstReport.top_k && firstReport.top_k[0] && firstReport.top_k[0].cross_validation
      ? firstReport.top_k[0].cross_validation.length
      : 0;
    $('dc-hint').textContent =
      `${sections.length} 方向 · 试过 ${totalCombos} 组合 · 后端合计 ${totalElapsed}ms` +
      (crossPts > 0 ? ` · 每候选在 ${crossPts + 1} 个验证点重跑 WF` : '') +
      ` · 按 composite 降序 · 点击条目加载为当前体系`;

    // 分段渲染
    sections.forEach(({ direction, report }) => {
      const section = document.createElement('div');
      section.className = 'dc-section';
      const dirLabel = direction > 0 ? '📈 多头 (Long)' : '📉 空头 (Short)';
      const dirCls = direction > 0 ? 'dc-section-long' : 'dc-section-short';
      section.classList.add(dirCls);

      const topK = (report && report.top_k) || [];
      section.innerHTML = `
        <div class="dc-section-head">
          <span class="dc-section-title">${dirLabel}</span>
          <span class="dc-section-meta">${topK.length} 个体系 · 试过 ${report.total_combinations_tried || 0} · ${report.elapsed_ms || 0}ms</span>
        </div>
      `;
      const list = document.createElement('div');
      list.className = 'discovery-list';

      if (topK.length === 0) {
        const empty = document.createElement('div');
        empty.className = 'loading';
        empty.textContent = report && report.error
          ? `错误：${report.error}`
          : '（未找到有效组合，可能是此方向组件少）';
        list.appendChild(empty);
      } else {
        topK.forEach((c) => list.appendChild(buildCandidateEl(c)));
      }
      section.appendChild(list);
      container.appendChild(section);
    });
  }

  /// 构造一个 Discovery 候选体系的 DOM 卡片
  function buildCandidateEl(c) {
    const d = c.definition;
    const rule = d.combine.type === 'MajorityK'
      ? `MajorityK (k=${d.combine.k})`
      : d.combine.type;
    const comps = d.components.map((id) => {
      const meta = state.components.find((x) => x.id === id);
      return meta ? meta.label : id;
    }).join(' + ');
    const rankCls = c.rank === 1 ? 'top1' : c.rank <= 3 ? 'top3' : '';
    const scoreCls = c.composite_score >= 0 ? 'pnl-pos' : 'pnl-neg';

    let crossHtml = '';
    if (c.cross_validation && c.cross_validation.length > 0) {
      const rows = c.cross_validation.map((cv) => {
        const shCls = cv.wf_avg_sharpe >= 0 ? 'pnl-pos' : 'pnl-neg';
        const consCls = cv.wf_consistency >= 0.75 ? 'pnl-pos' : cv.wf_consistency >= 0.5 ? '' : 'pnl-neg';
        return `
          <tr>
            <td>${cv.symbol} · ${cv.interval || '—'}</td>
            <td class="${consCls}">${isFinite(cv.wf_consistency) ? (cv.wf_consistency * 100).toFixed(0) + '%' : '—'}</td>
            <td class="${shCls}">${isFinite(cv.wf_avg_sharpe) ? fmtNum(cv.wf_avg_sharpe) : '—'}</td>
            <td>${isFinite(cv.wf_avg_return_pct) ? fmtPct(cv.wf_avg_return_pct, 1) : '—'}</td>
            <td>${cv.total_trades}</td>
          </tr>
        `;
      }).join('');
      crossHtml = `
        <table class="dc-cross-table">
          <thead>
            <tr><th>验证点</th><th>Cons</th><th>Sharpe</th><th>Return</th><th>Trades</th></tr>
          </thead>
          <tbody>${rows}</tbody>
        </table>
      `;
    }

    const el = document.createElement('div');
    el.className = 'discovery-item';
    el.innerHTML = `
      <div class="dc-head">
        <span class="dc-rank ${rankCls}">#${c.rank}</span>
        <span class="dc-rule">${rule}</span>
        <span class="dc-score ${scoreCls}">composite ${fmtNum(c.composite_score)}</span>
        <button class="dc-load-btn" type="button">加载为当前体系</button>
        <button class="dc-promote-btn" type="button" title="持久化入库，下次启动自动出现在种子列表">⭐ 入库</button>
      </div>
      <div class="dc-components">${comps}</div>
      <div class="dc-metrics">
        <span class="m"><span class="k">Single Sharpe</span>${fmtNum(c.single_sharpe)}</span>
        <span class="m"><span class="k">Return</span>${fmtPct(c.single_return_pct, 1)}</span>
        <span class="m"><span class="k">Trades</span>${c.single_trades}</span>
        <span class="m"><span class="k">DD</span>${(c.single_max_dd_pct * 100).toFixed(1)}%</span>
        <span class="m"><span class="k">主 WF Cons</span>${isFinite(c.wf_consistency) ? (c.wf_consistency * 100).toFixed(0) + '%' : '—'}</span>
        <span class="m"><span class="k">主 WF Sharpe</span>${isFinite(c.wf_avg_sharpe) ? fmtNum(c.wf_avg_sharpe) : '—'}</span>
        <span class="m"><span class="k">跨市场 Cons</span>${isFinite(c.cross_consistency_mean) ? (c.cross_consistency_mean * 100).toFixed(0) + '%' : '—'}</span>
        <span class="m"><span class="k">跨市场 Sharpe</span>${isFinite(c.cross_sharpe_mean) ? fmtNum(c.cross_sharpe_mean) : '—'}</span>
      </div>
      ${crossHtml}
    `;
    el.querySelector('.dc-load-btn').addEventListener('click', (ev) => {
      ev.stopPropagation();
      loadSeed(d);
    });
    el.querySelector('.dc-promote-btn').addEventListener('click', async (ev) => {
      ev.stopPropagation();
      await promoteDefinition(d, c);
    });
    el.addEventListener('click', () => loadSeed(d));
    return el;
  }

  /// 入库一个 Discovery 体系到本地 vault。附带自动命名。
  async function promoteDefinition(def, cand) {
    const defaultName = def.name || '自动发现的冠军';
    const suggested = prompt('给这个体系起个名字（将保存到本地）：', defaultName);
    if (!suggested) return;
    const idSeed = (suggested + '-' + Date.now().toString(36))
      .replace(/\s+/g, '_')
      .toLowerCase();
    const payload = {
      definition: {
        ...def,
        id: idSeed,
        name: suggested,
        description: `从 Discovery 入库 · BTC composite=${fmtNum(cand.composite_score)} · ` +
          `cross_sharpe=${fmtNum(cand.cross_sharpe_mean)} · cross_cons=${(cand.cross_consistency_mean * 100).toFixed(0)}%`,
      },
    };
    try {
      const r = await fetchJson('/api/system/promote', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      setStatus(`✓ 已入库：${r.definition.id}`, 'success');
      // 刷新种子列表（从 API 重新拉）
      await reloadSeeds();
    } catch (e) {
      setStatus(`✗ 入库失败：${e.message}`, 'error');
    }
  }

  /// 重新从 API 拉种子列表并刷新左栏
  async function reloadSeeds() {
    try {
      const resp = await fetchJson('/api/system/seeds');
      state.seeds = resp.seeds;
      renderSeedList();
    } catch (e) {
      // 静默
    }
  }

  /// 移除已入库体系
  async function demoteSeed(id) {
    if (!confirm(`确认移除已入库体系 ${id}?`)) return;
    try {
      const r = await fetchJson('/api/system/demote', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id }),
      });
      if (r.removed) {
        setStatus(`✓ 已移除：${id}`, 'success');
        await reloadSeeds();
      } else {
        setStatus(`未找到 ${id}`, 'error');
      }
    } catch (e) {
      setStatus(`✗ 移除失败：${e.message}`, 'error');
    }
  }

  // ---------- 结果渲染 ----------
  function renderResults(r) {
    $('result-placeholder').hidden = true;
    $('result-kpi').hidden = false;
    $('result-contrib').hidden = false;
    $('result-trades').hidden = false;

    const p = r.performance;
    const kpis = [
      { label: '交易数', value: p.total_trades, cls: '' },
      { label: '胜率', value: `${(p.win_rate * 100).toFixed(1)}%`, cls: p.win_rate >= 0.5 ? 'pos' : '' },
      { label: '总收益', value: fmtPct(p.total_return_pct), cls: p.total_return_pct >= 0 ? 'pos' : 'neg' },
      { label: '年化', value: fmtPct(p.annualized_return_pct), cls: p.annualized_return_pct >= 0 ? 'pos' : 'neg' },
      { label: 'Sharpe', value: fmtNum(p.sharpe), cls: p.sharpe >= 0 ? 'pos' : 'neg' },
      { label: 'Sortino', value: fmtNum(p.sortino), cls: p.sortino >= 0 ? 'pos' : 'neg' },
      { label: '最大回撤', value: `${(p.max_drawdown_pct * 100).toFixed(2)}%`, cls: 'neg' },
      { label: '期望 R', value: fmtNum(p.expectancy_r), cls: p.expectancy_r >= 0 ? 'pos' : 'neg' },
      { label: 'Profit Factor', value: fmtNum(p.profit_factor), cls: p.profit_factor >= 1 ? 'pos' : 'neg' },
      { label: '平均胜 R', value: fmtNum(p.avg_win_r ?? 0), cls: 'pos' },
      { label: '平均负 R', value: fmtNum(p.avg_loss_r ?? 0), cls: 'neg' },
      { label: '平均持仓', value: `${(p.avg_hold_bars ?? 0).toFixed(1)} 根`, cls: '' },
    ];
    const grid = $('kpi-grid');
    grid.innerHTML = '';
    kpis.forEach((k) => {
      const el = document.createElement('div');
      el.className = 'kpi-item';
      el.innerHTML = `
        <div class="label">${k.label}</div>
        <div class="value ${k.cls}">${k.value}</div>
      `;
      grid.appendChild(el);
    });

    // 归因
    const contribBody = $('contrib-table').querySelector('tbody');
    contribBody.innerHTML = '';
    (r.component_contributions || []).forEach((c) => {
      const rate = c.trigger_count > 0 ? c.traded_count / c.trigger_count : 0;
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>${c.component_id}</td>
        <td>${c.trigger_count}</td>
        <td>${c.traded_count}</td>
        <td>${(rate * 100).toFixed(1)}%</td>
      `;
      contribBody.appendChild(tr);
    });

    // 交易列表
    const tbody = $('trades-table').querySelector('tbody');
    tbody.innerHTML = '';
    const trades = (r.trades || []).slice(0, 20);
    trades.forEach((t) => {
      const pnlCls = t.pnl_pct >= 0 ? 'pnl-pos' : 'pnl-neg';
      const rCls = t.r_multiple >= 0 ? 'pnl-pos' : 'pnl-neg';
      const tr = document.createElement('tr');
      tr.innerHTML = `
        <td>#${t.id}</td>
        <td>${t.side === 'Long' ? '🟢 多' : '🔴 空'}</td>
        <td>${t.hold_bars}b</td>
        <td>${t.entry_bar} → ${t.exit_bar}</td>
        <td>${t.entry_price.toFixed(2)}</td>
        <td>${t.exit_price.toFixed(2)}</td>
        <td class="${pnlCls}">${fmtPct(t.pnl_pct)}</td>
        <td class="${rCls}">${fmtNum(t.r_multiple)}</td>
        <td>${t.exit_reason}</td>
      `;
      tbody.appendChild(tr);
    });
  }

  function setStatus(msg, cls = '') {
    const el = $('run-status');
    el.textContent = msg;
    el.className = 'run-status ' + cls;
    // 打磨：浮动 toast 提示（仅 success/error 真正浮出）
    if (cls === 'success' || cls === 'error') {
      if (window.AuraToast) window.AuraToast.push(msg, cls);
    }
  }

  // 启动
  document.addEventListener('DOMContentLoaded', init);
})();
