# AURA UI/UX 改进方案

> **视角**：顶级交易员 + 产品设计师
> **基于**：Sprint 0-18 完整交付的当前 UI 真实截图评估
> **发布时间**：2026-04-17
> **目标**：从"技术分析工具"进化为"**交易员日常决策助手**"

## ✅ P0 实施进度（2026-04-17 更新）

| 项 | 状态 | 文件 |
|---|---|---|
| **P0-1** AI 决策横条（**已改紧凑版**） | ✅ 已完成 | 主行 ~30px 单行：图标+标签+置信度条+风险+首条理由+▸展开；详情区默认折叠 · `app.js::renderDecision`+`toggleDecisionDetails` |
| **P0-2** K 线标注聚合 | ✅ 已完成 | `app.js::applyPatternMarkers`（5 根窗口 cluster + 方向分色 + 强度 opacity） |
| **P0-3** 当前价大号显示 | ✅ 已完成 | `price-display` + `updatePriceDisplay`（WS tick 实时） |
| **P0-4** Playbook 对比模式 | ✅ 已完成 | `app.js::runPlaybookCompare`（5 策略并发 + 排名表） |
| **P0-5** 空白引导组件 | ✅ 已完成 | `#bt-equity-empty` + `applyBacktestPreset` |
| **指标副图** | ✅ 已完成 | 新 API `/api/indicators/series`（`routes.rs::handle_indicators_series`）；前端 `#indicator-subpanel` + tab（MACD/RSI/Volume）+ 光标联动 + RSI 70/30 参考线 + MACD hist 涨跌色 · `app.js::applyIndicators/renderIndicator/switchIndicator` |

### 关于指标 SDK 的结论

| 来源 | 提供指标？ | 备注 |
|---|---|---|
| Binance REST/WS | ❌ | 只提供原始 K 线/深度/成交 |
| TradingView Lightweight Charts（项目当前用） | ❌ | 纯渲染库，无指标计算 |
| TradingView Advanced Charts | ✅ 100+ | 闭源 CL，个人/研究免费，商用需申请 |
| TradingView Widget（iframe） | ✅ | 弹性差，无法接入业务数据 |
| **本项目后端** | ✅ | `engine/indicator.rs` 已实现 RSI/MACD/EMA；`engine/ma/*` 实现粘合/葛南维/特殊形态等数十个；可直接扩展新端点 |

---

## 📋 目录

- [1. 当前状态评估](#1-当前状态评估)
- [2. 核心理念：5 秒决策原则](#2-核心理念5-秒决策原则)
- [3. P0 改进（立刻做）](#3-p0-改进立刻做)
- [4. P1 改进（一周内）](#4-p1-改进一周内)
- [5. P2 改进（v1.5 版本）](#5-p2-改进v15-版本)
- [6. 战略级增强（v2.0）](#6-战略级增强v20)
- [7. 视觉系统规范](#7-视觉系统规范)
- [8. 实施路线图](#8-实施路线图)

---

## 1. 当前状态评估

### 1.1 做得极好的（保留并强化）

✅ **AURA 金色主题**：辨识度强，专业感
✅ **信息密度**：一屏 8+ 模块，业界领先
✅ **形态直接标注 K 线**：很多竞品做不到
✅ **参数配置算法白盒**：书源 + 权重都可调，**极其稀有**
✅ **原书铁证追溯**：每个信号都有页码
✅ **双回测引擎**：pattern + Playbook 策略

### 1.2 关键问题（致命）

🔴 **P0-1 K 线标注过载**：1d×800 根 + 5 星密度 → 红黄绿文字叠在一起读不清
🔴 **P0-2 没有 AI 决策条**：输入参数在顶部，但**输出决策去哪了**？
🔴 **P0-3 当前价不醒目**：底部状态栏小字 `75,689.99` → 交易员第一眼找不到
🔴 **P0-4 Playbook 卡片右侧大片空白**：浪费屏幕空间
🔴 **P0-5 权益曲线空白无引导**：新用户进去一片空白不知所措

---

## 2. 核心理念：5 秒决策原则

**顶级交易员工作流**：

```
开盘 → 扫一眼屏幕（5 秒）→ 做出决定（买/卖/持）→ 下单
```

**当前 UI 的 5 秒测试**：

| 问题 | 当前状态 | 应达到 |
|---|---|---|
| 当前价是多少？ | ⏱️ 2-3 秒（底部小字）| ⏱️ 0.3 秒（顶部大号）|
| 现在该买还是卖？ | ⏱️ 30+ 秒（看多个卡片推断）| ⏱️ 2 秒（顶部 AI 横条）|
| 风险大吗？ | ⏱️ 找不到 | ⏱️ 1 秒（红黄绿条）|
| 关键位（止损/目标）？ | ⏱️ 找不到 | ⏱️ K 线图直接画虚线 |

**设计目标**：把每个关键问题的响应时间压缩到 **≤ 1 秒**。

---

## 3. P0 改进（立刻做）

### P0-1：顶部 AI 决策横条 ⭐⭐⭐⭐⭐

**位置**：工具栏正下方，K 线图上方。

**线框图**：

```
┌──────────────────────────────────────────────────────────────┐
│  🎯 当前建议：⚠️ 持股观望                     风险等级：🟡中  │
│  置信度：65% ██████░░░░                                       │
│                                                              │
│  ✅ 理由（3 条）：                                            │
│    • 长期上升趋势（MA240 上行 +1.2%）                         │
│    • 刚触及 MA60 支撑（91,184）                               │
│    • ⚠️ 最近 5 根出现毒蜘蛛（空头警告）                       │
│                                                              │
│  💡 操作建议：[持股] [减仓 30%] [清仓] [下调止损→ 88,500]    │
│                                                              │
│  📖 原书依据：ma p.200（双线 6 条规则 Rule 6 持股）           │
└──────────────────────────────────────────────────────────────┘
```

**实现逻辑**：
```javascript
// app.js 新增
async function loadDecision() {
  const sig = await fetchJson(`/api/signals?...`);
  const playbook = await fetchJson(`/api/backtest/playbook?...&strategy=default`);

  // 汇总当前所有信号 → Playbook decide
  const topSignal = sig.advanced_ma_events[sig.advanced_ma_events.length - 1];
  const confidence = computeConfidence(sig);
  const reasons = extractReasons(sig);
  const actions = suggestActions(sig);

  renderDecisionBar({
    direction: topSignal?.kind || 'Hold',
    confidence,
    risk: sig.l4_warning || 'low',
    reasons,
    actions,
    bookSource: topSignal?.book_source
  });
}
```

**后端扩展**：新增 `/api/decision` 端点，专门返回聚合决策：
```rust
#[derive(serde::Serialize)]
struct DecisionResp {
    action: String,           // "buy" | "sell" | "hold" | "watch"
    confidence: f64,          // 0-100
    risk_level: String,       // "low" | "medium" | "high"
    reasons: Vec<String>,     // 3-5 条
    suggested_actions: Vec<ActionBtn>,
    book_sources: Vec<String>,
}
```

**工作量**：1 天（400 行代码 + 1 端点 + HTML/CSS/JS）

---

### P0-2：K 线标注聚合 + 淡化 ⭐⭐⭐⭐⭐

**问题**：Image 1 中"洗盘震仓"/"光脚光脚"等标签**严重重叠**。

**改进三步**：

**Step 1：按强度分级**
```javascript
// 根据 strength 决定 opacity
const opacityByStrength = (s) => ({
  1: 0.25, 2: 0.4, 3: 0.6, 4: 0.8, 5: 1.0, 6: 1.0
})[s] || 0.5;

// 低强度淡化（5 星密度时，1-3 星标注 opacity < 0.5）
```

**Step 2：重叠聚合为圆点**
```javascript
// 5 根 K 线内 ≥3 个标注 → 聚合为 ● 图标
function aggregateOverlappingMarkers(markers, windowBars = 5) {
  const clusters = [];
  // ... 聚类算法
  return clusters.map(c =>
    c.length === 1 ? c[0] : {
      type: 'cluster',
      icon: '●',
      count: c.length,
      members: c, // hover 时展开
    }
  );
}
```

**Step 3：按方向分色**
```css
.marker-bull {  color: var(--signal-bull); }   /* 绿色，看多 */
.marker-bear {  color: var(--signal-bear); }   /* 红色，看空 */
.marker-neutral { color: var(--signal-neutral); opacity: 0.4; } /* 灰，中性 */
```

**用户控件**：工具栏形态密度从 5 星默认改为 **3 星**；5 星保留为"专家模式"。

**工作量**：0.5 天

---

### P0-3：当前价大号显示（Big Number）⭐⭐⭐⭐

**位置**：顶部右侧，紧贴 Tab 按钮。

**线框图**：
```
│  ... [实时分析] [回测] [配置]  │  ¥75,689.99  ▲ +2.34% │
                                   ━━━━━━━━━━━━   ━━━━━━
                                   36px bold      14px
                                   🟢 绿色         🟢 绿色
```

**实现**：
```html
<div class="price-display">
  <span class="price-big" id="current-price">75,689.99</span>
  <span class="price-change" id="price-change">▲ +2.34%</span>
</div>
```

```css
.price-big {
  font-size: 36px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  transition: color 0.3s;
}
.price-big.up { color: var(--signal-bull); }
.price-big.down { color: var(--signal-bear); }
```

实时更新（WebSocket 已有），每次更新**闪烁 200ms**：
```css
@keyframes price-flash {
  0% { background: var(--signal-bull); }
  100% { background: transparent; }
}
.price-big.flashing { animation: price-flash 0.2s; }
```

**工作量**：0.2 天

---

### P0-4：Playbook 对比模式 ⭐⭐⭐⭐

**当前问题**：Image 2 中策略卡片只显示 1 个策略结果，右侧大片空白。

**改进**：新增"对比模式"切换：

**线框图（对比模式）**：
```
┌─ 🎭 原书策略回测（对比模式） ────────────────────────────────┐
│                                                              │
│ 策略                  │ 总收益 │ 胜率 │ 回撤 │ Sharpe │ α    │
├───────────────────────┼───────┼──────┼──────┼───────┼──────┤
│ 📊 买入持有（基线）    │ +45.2%│  -   │ 28%  │ 0.85  │  -   │
│ 🔥 断头铡刀清仓        │ +78.1%│ 71%  │ 12%  │ 2.10⭐│+32.9%│
│ 🌱 旱地拔葱轻仓        │ +12.3%│ 45%  │  8%  │ 0.40  │-32.9%│
│ 📉 三次减仓            │ +65.0%│ 63%  │ 15%  │ 1.80  │+19.8%│
│ 🎯 多级趋势矩阵        │ +58.4%│ 55%  │ 18%  │ 1.50  │+13.2%│
│ 🎭 组合策略（默认）    │ +82.3%│ 68%  │ 10%  │ 2.30⭐│+37.1%│
│                                                              │
│ ✅ 推荐：组合策略（α=37.1% 最高，Sharpe 2.3 最稳）            │
└──────────────────────────────────────────────────────────────┘
```

**实现**：
```javascript
async function runPlaybookComparison() {
  const strategies = ['default', 'guillotine', 'scallions', 'staged_exit', 'trend_matrix'];
  const results = await Promise.all(
    strategies.map(s => fetchJson(`/api/backtest/playbook?...&strategy=${s}`))
  );
  // 额外添加 buy & hold 基线（原 run 端点）
  const baseline = await fetchJson(`/api/backtest/run?...&strategy=buy_hold`);

  renderComparisonTable([baseline, ...results]);
}
```

**工作量**：1 天

---

### P0-5：权益曲线空白引导 ⭐⭐⭐

**当前**：Image 2 权益曲线区一片空白，只有 TradingView logo。

**改进**：
```html
<div id="equity-empty-state" class="empty-state">
  <svg class="empty-icon">...</svg>
  <h3>点击"运行回测"开始</h3>
  <p>选择策略，回测将展示：</p>
  <ul>
    <li>📈 权益曲线（对比买入持有）</li>
    <li>📊 回撤曲线</li>
    <li>🏆 所有交易记录</li>
  </ul>
  <div class="quick-presets">
    <button>🚀 默认跑 BTC 1d 回测</button>
    <button>🎯 跑组合策略</button>
  </div>
</div>
```

---

## 4. P1 改进（一周内）

### P1-1：仓位规划器浮窗

**位置**：右下角固定，可折叠。

```
┌─ 💰 仓位规划 ──────────┐
│ 账户权益：  ¥100,000   │
│ 单笔风险：  2%         │
│ 入场价：    ¥75,700    │
│ 止损价：    ¥73,500    │
│ ────────────────────── │
│ 📊 建议股数：291       │
│    占账户：  22%       │
│                        │
│ 🚨 L4 警告生效：≤ 30%  │
│                        │
│ [一键下单]  [发送给 API] │
└────────────────────────┘
```

### P1-2：止损线直接画在 K 线图

当前图上**没有画止损线**。应该：
- 入场时在 K 线图添加**红色水平虚线**（止损）
- 绿色水平虚线（目标）
- 两线之间用**半透明区域**标示风险带

### P1-3：信号列表 hover 详情

右侧葛南维信号列表（Image 1）**hover 显示**：
```
B2 回踩买入
━━━━━━━━━━━━━━━━━━
📖 原书：ma p.100 葛南维第二法则
🕐 触发：2026-04-13 08:00 @¥78,693
⭐ 真实胜率：60%（BTC 日线样本）
💰 期望收益：+1.85%
🎯 建议：均线上方回踩未破，可买 70%
⚠️ 失效条件：跌破 MA60 > 3%

[查看历史触发]  [加入回测]
```

### P1-4：tab 命名交易员化

| 当前 | 建议 |
|---|---|
| 📊 实时分析 | 📊 盯盘 |
| 🧪 回测实验室 | 🧪 验策略 |
| ⚙️ 参数配置 | ⚙️ 设置 |

或增加第 4 个：`📋 交易日志`

### P1-5：夜盘模式

顶部右上加 🌙/☀️ 切换按钮。
- 白天：当前金色主题
- 夜晚：深蓝/绿色低饱和，更护眼

### P1-6：参数配置一键预设

Image 3 的 4 维权重配置，加 4 个快捷按钮：
```
[🛡️ 保守] [⚖️ 平衡] [🚀 激进] [↩️ 恢复默认]
```

### P1-7：新用户引导

首次访问 localStorage 未设 → 4 步 tour：
```
Step 1 → "左上输入交易对"
Step 2 → "右侧看 AI 建议"
Step 3 → "切到回测实验室验证策略"
Step 4 → "参数配置个性化"
```

---

## 5. P2 改进（v1.5 版本）

### P2-1：实时通知系统

强信号（断头铡刀 / SELL-1 / 多合一 ≥3 类）触发：
- 右上 **toast 通知**（3 秒自动消失）
- **浏览器桌面通知**（可选开关）
- **声音提醒**（可选开关）

### P2-2：事件回放

K 线图下方加**时间轴拖动条**：
- 拖到历史某点 → UI 完整还原该时刻所有信号
- 可**逐根 K 线步进**（→ 键）

### P2-3：多标的对比

Symbol 选择器改为**多选**：
```
[BTCUSDT × ETHUSDT × SOLUSDT]
```
K 线图上叠加 3 条价格曲线（对齐起点 100%），展示相对强弱。

### P2-4：导出交易日志

任何回测结果 → **一键导出 CSV / JSON / PDF**：
- 交易清单
- 绩效指标
- 权益曲线
- 当时触发的所有信号快照

### P2-5：学习模式

点击任何技术术语（"断头铡刀"/"倒春寒"）→ 弹出：
- 原书原文（PDF 截图）
- 案例图（历史真实触发）
- 3 分钟讲解视频（未来）

---

## 6. 战略级增强（v2.0）

### 6.1 策略自定义（DSL）

让用户用 YAML 写自己的 Playbook：
```yaml
name: 我的稳健策略
book_source: 我的实战心得

rules:
  - when:
      ma_advanced_kind: Guillotine
    then:
      action: Sell
      target_position: 0.0
      reason: "断头铡刀立即清仓"

  - when:
      all_of:
        - granville_rule: B1BreakoutBuy
        - long_trend: Up
        - ma_relation: Diverging
    then:
      action: Buy
      target_position: 0.7
      reason: "牛市 B1 突破，重仓"
```

### 6.2 社交层

- 用户可分享策略（Playbook YAML）
- 策略排行榜（按真实数据 α）
- 订阅他人策略实时信号

### 6.3 账户托管 / 自动下单

集成 Binance / OKX API，支持：
- 观察账户
- 手动一键下单（签名）
- 完全自动化（白名单策略）

### 6.4 移动端

响应式 H5 / PWA / 原生 App：
- 行情监控
- 信号推送（push）
- 简化下单

---

## 7. 视觉系统规范

### 7.1 颜色语义

```css
:root {
  /* 信号方向 */
  --signal-strong-bull:  #10b981;  /* 强烈看涨 */
  --signal-weak-bull:    #34d399;  /* 轻微看涨 */
  --signal-neutral:      #6b7280;  /* 中性 */
  --signal-weak-bear:    #f87171;  /* 轻微看跌 */
  --signal-strong-bear:  #ef4444;  /* 强烈看跌 */

  /* 特殊标记 */
  --signal-iron-evidence: #fbbf24; /* 原书铁证 */
  --signal-warning:       #f59e0b; /* 警告（L4 共振）*/
  --signal-critical:      #dc2626; /* 严重（清仓）*/

  /* 风险等级 */
  --risk-low:    var(--signal-weak-bull);
  --risk-med:    #f59e0b;
  --risk-high:   var(--signal-strong-bear);
}
```

### 7.2 字体系统

```css
body { font-family: 'Inter', -apple-system, sans-serif; }

.number-tabular { font-variant-numeric: tabular-nums; }

/* 大号价格 */
.price-large { font-size: 36px; font-weight: 700; }
/* 关键指标 */
.metric-value { font-size: 24px; font-weight: 600; }
/* 标签 */
.label { font-size: 12px; font-weight: 500; opacity: 0.6; }
```

### 7.3 间距系统（8px grid）

```css
:root {
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;
}
```

### 7.4 动画

```css
/* 状态切换：平滑 */
.transition-fast { transition: all 0.15s ease; }
.transition-medium { transition: all 0.3s ease; }

/* 新信号出现：脉冲 */
@keyframes pulse-new {
  0% { transform: scale(1); opacity: 0; }
  50% { transform: scale(1.05); opacity: 1; }
  100% { transform: scale(1); opacity: 1; }
}
.new-signal { animation: pulse-new 0.6s ease-out; }

/* 价格变化：闪烁 */
@keyframes flash-up {
  0% { background: rgba(16, 185, 129, 0.3); }
  100% { background: transparent; }
}
```

### 7.5 响应式断点

```css
/* 桌面（默认）*/
@media (min-width: 1200px) { ... }

/* 小桌面 / 笔记本 */
@media (max-width: 1199px) { 右侧面板折叠为汉堡菜单 }

/* 平板 */
@media (max-width: 768px) {
  单列布局
  Tab 下移到底部
}
```

---

## 8. 实施路线图

### Week 1：P0 核心改进（4 项）

| 项 | 工时 | 交付 |
|---|---|---|
| P0-1 AI 决策横条 | 1 天 | `/api/decision` + 顶部组件 |
| P0-2 K 线标注聚合 | 0.5 天 | `aggregateMarkers()` + CSS 分级 |
| P0-3 当前价大号 | 0.2 天 | 顶部 header 改造 |
| P0-4 Playbook 对比模式 | 1 天 | 多策略并发 + 对比表格 |
| P0-5 空白引导 | 0.3 天 | empty-state 组件 |

**Week 1 合计：3 天**

### Week 2：P1 体验打磨（7 项）

| 项 | 工时 |
|---|---|
| P1-1 仓位规划器 | 1 天 |
| P1-2 止损线绘制 | 0.5 天 |
| P1-3 信号 hover | 0.5 天 |
| P1-4 Tab 命名 | 0.1 天 |
| P1-5 夜盘模式 | 0.5 天 |
| P1-6 预设按钮 | 0.3 天 |
| P1-7 新用户引导 | 0.5 天 |

**Week 2 合计：3.4 天**

### Week 3：视觉系统规范

- 替换全部颜色为 CSS 变量
- 统一字体 / 字号 / 间距
- 响应式断点处理

**Week 3 合计：2-3 天**

### Week 4+：P2 + v2.0（按需）

---

## 9. 成功指标（KPI）

### 9.1 用户体验指标

| 指标 | 当前 | 目标 |
|---|---|---|
| 新用户 5 秒内知道"该做什么" | ❌ | ✅ |
| 专业交易员日均使用时长 | — | 30+ 分钟 |
| 信号→操作转化率 | — | 15%+ |
| 回测运行频次 / 会话 | — | 3+ |

### 9.2 技术指标

| 指标 | 目标 |
|---|---|
| 页面首屏 | < 1s |
| K 线图 800 根渲染 | < 200ms |
| 信号 API 响应 | < 500ms |
| 浏览器内存 | < 200MB |

---

## 10. 结论

### 10.1 核心洞察

AURA 目前是一个**信息完整度业界顶级**的工具，但**决策辅助性**有待提升。

**问题本质**：把算法黑盒白盒化是**工程胜利**，但把白盒结果转化为**5 秒可决策信号**才是**产品胜利**。

### 10.2 一句话建议

> **"顶部 AI 决策条 + K 线标注聚合"** 是最小代价（1.5 天）最大体验提升（⭐⭐⭐⭐⭐）的组合拳 —— 立即做。

### 10.3 差异化定位

AURA 应该定位为：

**"唯一内置原书铁证 + 真实数据验证 + 前后端一体的中国式技术分析决策助手"**

—— 不是炫技产品，而是**让交易员每天都要打开的工具**。

---

*AURA UI/UX 改进方案 v1 · 2026-04-17 · 顶级交易员 + 产品设计师视角*
