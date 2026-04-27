# 体系实验室（System Lab）— 技术设计文档（TDD）

> **作者**：Aura-Trade 工程组
> **版本**：v0.1（初稿）
> **状态**：📐 设计中，未开始实施
> **定位**：Aura-Trade 的第三个核心工作台（继实时共振、单指标有效性排行榜之后）
> **关联代码**：`src/engine/{candle,ma,trend,chartpattern}/`、`src/engine/effectiveness.rs`、`src/engine/backtest/*`、`src/engine/rl/*`
> **关联文档**：`AURA_TRADE_PRD.md`、`RL_EFFECTIVENESS_DESIGN.md`、`PATTERN_EFFECTIVENESS_REPORT.md`

---

## 0. 概览（TL;DR）

把三本典籍里散落的信号识别器（均线葛南维、17 种均线特殊形态、50+ K 线形态、15+ 图形、道氏结构等）**组件化**，允许用户：

1. **手动勾选组件**拼成一个「交易体系（Trading System）」，实时看回测表现；
2. **系统自动探索**当前市场下 Top-K 胜率最高的组件组合（Strategy Discovery）；
3. 把体系**应用到图表**，信号按维度着色标注；
4. **多体系并排对比**（权益曲线叠加 + 指标雷达图）。

**关键词**：Component Registry / SystemDefinition / CombineRule / Walk-Forward / Greedy Forward Selection / Strategy Discovery

**非目标**（严格对齐 `AURA_TRADE_PRD.md` §非功能需求 + `RL_EFFECTIVENESS_DESIGN.md` §1.2）：

- ❌ 不做深度学习 / 神经网络 / 行情预测
- ❌ 不替代现有 Playbook 铁律（断头铡刀清仓等硬规则依旧优先）
- ❌ 不做连续参数的无限搜索空间（所有组件 ≤ `MAX_K=5`，保持可解释）

---

## 1. 目录

- [2. 产品愿景与用户故事](#2-产品愿景与用户故事)
- [3. 核心概念与术语](#3-核心概念与术语)
- [4. 数据模型（Rust 类型定义）](#4-数据模型rust-类型定义)
- [5. 聚合规则（CombineRule）](#5-聚合规则combinerule)
- [6. 回测执行器（SystemRunner）](#6-回测执行器systemrunner)
- [7. Walk-Forward 验证框架](#7-walk-forward-验证框架)
- [8. 组合发现器（Strategy Discovery）](#8-组合发现器strategy-discovery)
- [9. 评分函数（Scoring）](#9-评分函数scoring)
- [10. 种子体系（Seed Systems）— 8 个原书经典](#10-种子体系seed-systems-8-个原书经典)
- [11. API 设计](#11-api-设计)
- [12. 前端设计](#12-前端设计)
- [13. 与现有模块的关系](#13-与现有模块的关系)
- [14. 风险与边界情况](#14-风险与边界情况)
- [15. 分阶段 Roadmap](#15-分阶段-roadmap)
- [16. 附录 A：默认成本模型](#16-附录-a默认成本模型)
- [17. 附录 B：术语表](#17-附录-b术语表)
- [18. 附录 C：FAQ](#18-附录-c-faq)

---

## 2. 产品愿景与用户故事

### 2.1 愿景

> **让每个用户都能在 5 分钟内造出一个「胜率 60%+ 的交易体系」，且每一步都对应原书某页、都能用历史数据证伪。**

这个愿景服务三类用户：

| 用户 | 典型场景 | 痛点 |
|---|---|---|
| **新手** | 看了三本书想试试哪些形态真的能赚钱 | 不知道怎么把 17 种均线特殊形态 + 50+ K 线形态组合成有效体系 |
| **老交易员** | 有自己的老方法，想用数据验证 | 缺一个"把我手写规则跑回历史数据"的工具 |
| **系统研究者** | 想探索新组合 | 手动试 N² 种组合太慢，需要自动化探索 |

### 2.2 核心用户故事

**US-1**：作为一个均线派用户，我可以勾选 `[MA20 葛南维 B2 + 多头排列过滤 + BIAS < 8%]`，点击"回测"，立刻看到该组合在 BTC/1d 过去 4 年的胜率、Sharpe 和权益曲线。

**US-2**：作为一个探索者，我可以点击"发现高胜率组合"按钮，系统在 1-2 分钟内返回 10 个它自己找到的高分组合，每个都带 walk-forward OOS 指标。我挑一个保存为"我的体系 #7"。

**US-3**：作为一个对比控，我可以同时选中「K 线反转系统」和「四维共振系统」，在图表上看它们的信号是否重合、权益曲线谁陡，哪个最大回撤小。

**US-4**：作为一个实盘用户，我可以把我保存的体系应用到实时面板，当下一根 K 线产生信号时，系统根据该体系的规则告诉我"买/卖/持有"。

---

## 3. 核心概念与术语

| 术语 | 英文 | 含义 |
|---|---|---|
| **组件** | Component | 最小可复用的信号识别单元，如 `ma.granville.b2_pullback`、`candle.morning_star`、`chart.head_and_shoulders_top` |
| **聚合规则** | CombineRule | 多组件同时触发时的决策规则：AllAligned / MajorityK / WeightedScore / SequentialCascade |
| **交易体系** | TradingSystem / SystemDefinition | 组件集合 + 聚合规则 + 风控参数 + 元数据 |
| **种子体系** | SeedSystem | 从原书提炼的 8 个经典组合，作为探索器的起点或用户起始模板 |
| **组合发现** | Strategy Discovery | 自动搜索组件空间，找到高评分的组合 |
| **Walk-Forward** | — | 把历史切成 `[训练窗 → 验证窗]` 滚动，训练窗决定组件/参数，验证窗产出 OOS 指标 |
| **评分** | Effectiveness Score | 综合胜率/平均收益/样本数/复杂度的单一数字，用于排序 |
| **OOS** | Out-Of-Sample | 训练集外数据的表现，真正可信的评估口径 |

---

## 4. 数据模型（Rust 类型定义）

### 4.1 `Component`（组件）

```rust
// src/engine/system/component.rs

use crate::data::Kline;

/// 单个信号识别单元的元数据 + 触发函数指针
pub struct Component {
    /// 唯一 ID，命名空间 `<dimension>.<kind>.<variant>`
    pub id: &'static str,

    /// 人类可读标签（中文）
    pub label: &'static str,

    /// 原书出处，如 "均线 §2.1 p.48"
    pub book_source: &'static str,

    /// 维度分类
    pub dimension: ComponentDimension,

    /// 方向偏好：+1 只看多 / -1 只看空 / 0 双向（由 trigger 内部决定）
    pub direction_bias: i8,

    /// 历史 alpha（从 `effectiveness.rs` 报告注入，用于 UI 排序）
    pub historical_alpha_pct: Option<f64>,
    pub historical_winrate: Option<f64>,

    /// 触发函数：给一段 K 线 + 当前索引，返回是否触发 + 方向
    pub trigger: fn(&[Kline], usize, &ComponentParams) -> Option<TriggerEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentDimension {
    MaSignal,         // 葛南维、均线交叉、BIAS 等
    MaSpecial,        // 金山谷 / 死亡谷 / 断头铡刀 / 旱地拔葱 等 17 种
    CandlePattern,    // K 线形态
    ChartPattern,     // 图形（头肩、三角、楔形等）
    TrendStructure,   // HH/HL 结构、趋势线突破
    Confluence,       // 跨维度派生（v2 才加）
}

pub struct TriggerEvent {
    pub direction: i8,   // +1 多 / -1 空
    pub confidence: f64, // [0, 1]，部分识别器可给强度分
    pub reason: String,  // 诊断用，如 "20日均线向上翘且价格回踩不破"
}

pub struct ComponentParams {
    pub ma_periods: Vec<usize>,
    pub atr_period: usize,
    pub body_ratio_min: f64,
    // ... 其他需要的参数
}
```

### 4.2 全局组件注册表

```rust
// src/engine/system/registry.rs

use once_cell::sync::Lazy;

pub static COMPONENTS: Lazy<Vec<Component>> = Lazy::new(|| vec![
    // ========= MaSignal (7 个) =========
    Component {
        id: "ma.granville.b1_breakout",
        label: "葛南维 B1 均线由下转上 + 价格上穿",
        book_source: "均线 §2.1 p.45",
        dimension: ComponentDimension::MaSignal,
        direction_bias: 1,
        historical_alpha_pct: Some(3.2),
        historical_winrate: Some(0.58),
        trigger: triggers::ma_granville_b1,
    },
    Component {
        id: "ma.granville.b2_pullback",
        label: "葛南维 B2 回踩均线不破",
        book_source: "均线 §2.1 p.48",
        // ...
        trigger: triggers::ma_granville_b2,
    },
    // B3, B4, S1, S2, S3, S4 略

    // ========= MaSpecial (17 个) =========
    Component {
        id: "ma_special.golden_valley",
        label: "金山谷",
        book_source: "均线 §4.2 p.121",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: 1,
        historical_alpha_pct: Some(2.13),
        historical_winrate: Some(0.553),
        trigger: triggers::ma_special_golden_valley,
    },
    Component {
        id: "ma_special.death_valley",
        label: "死亡谷",
        book_source: "均线 §4.2 p.125",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: -1,
        historical_alpha_pct: Some(19.03),  // 周线极强
        historical_winrate: Some(0.587),
        trigger: triggers::ma_special_death_valley,
    },
    Component {
        id: "ma_special.guillotine",
        label: "断头铡刀",
        book_source: "均线 §4.3 p.380",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: -1,
        // ...
        trigger: triggers::ma_special_guillotine,
    },
    // 其他 14 个略（蛟龙出海、旱地拔葱、毒蜘蛛、气贯长虹、战机起航 等）

    // ========= CandlePattern (50+ 个) =========
    Component {
        id: "candle.light_legs_bear",
        label: "光脚阴线",
        book_source: "K线 §3.1 p.82",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: -1,
        historical_alpha_pct: Some(12.54),  // 周线
        historical_winrate: Some(0.60),
        trigger: triggers::candle_light_legs_bear,
    },
    // ...

    // ========= ChartPattern (15+ 个) =========
    Component {
        id: "chart.diamond_top",
        label: "菱形顶",
        book_source: "K线 §7.2 p.420",
        dimension: ComponentDimension::ChartPattern,
        direction_bias: -1,
        historical_alpha_pct: Some(11.87),  // 日线
        historical_winrate: Some(0.857),
        trigger: triggers::chart_diamond_top,
    },
    // ...

    // ========= TrendStructure (6 个) =========
    Component {
        id: "trend.dow_uptrend",
        label: "道氏上升趋势（HH + HL 至少 2 组）",
        book_source: "趋势 §1.2 p.32",
        dimension: ComponentDimension::TrendStructure,
        direction_bias: 1,
        trigger: triggers::trend_dow_uptrend,
        // ...
    },
    // ...
]);

pub fn find_component(id: &str) -> Option<&'static Component> {
    COMPONENTS.iter().find(|c| c.id == id)
}

pub fn components_by_dimension(dim: ComponentDimension) -> Vec<&'static Component> {
    COMPONENTS.iter().filter(|c| c.dimension == dim).collect()
}
```

### 4.3 `SystemDefinition`（交易体系定义）

```rust
// src/engine/system/definition.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDefinition {
    /// 体系唯一 ID，如 "seed.ma_skeleton" / "user.my_favorite_2025-04"
    pub id: String,

    /// 显示名
    pub name: String,

    /// 来源：Seed（预设）/ User（用户自定义）/ Discovered（自动发现）
    pub origin: SystemOrigin,

    /// 描述（可选，从原书章节摘要）
    pub description: Option<String>,

    /// 组件 ID 列表（有序，某些 CombineRule 关心顺序）
    pub components: Vec<String>,

    /// 聚合规则
    pub combine: CombineRule,

    /// 每个组件的权重（仅 WeightedScore 使用，其他规则可忽略）
    #[serde(default)]
    pub weights: std::collections::HashMap<String, f64>,

    /// 风控参数
    pub risk: RiskParams,

    /// 回测参数
    pub backtest: BacktestParams,

    /// 元数据：创建时间、来源版本等
    pub meta: SystemMeta,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SystemOrigin { Seed, User, Discovered }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CombineRule {
    /// 所有组件必须同向触发（最严格）
    AllAligned,

    /// 至少 k 个组件同向触发
    MajorityK { k: usize },

    /// 加权分数超过阈值：Σ(weight_i × direction_i) ≥ threshold
    WeightedScore { threshold: f64 },

    /// 级联：必须按顺序逐个触发（组件 0 先触发，T 根内组件 1 触发，...）
    SequentialCascade { window_bars: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskParams {
    pub stop_atr_mult: f64,     // 止损 = entry ± ATR × mult
    pub target_r: f64,           // 目标止盈 = R × target_r
    pub max_hold_bars: usize,    // 最大持仓 K 线数（超过强制离场）
    pub max_position_pct: f64,   // 最大仓位占资金比例（0.0-1.0）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestParams {
    pub horizon_bars: usize,     // 固定 horizon 评估模式（不走 ATR 止损时用）
    pub cost_model: CostModel,   // 见 §16
    pub warmup_bars: usize,      // 前多少根 K 线不算入回测（让均线预热）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CostModel {
    Zero,                                    // 纯价格，无成本
    Fixed { fee_pct: f64, slip_pct: f64 },   // 固定（MVP 默认）
    Dynamic { base_fee_pct: f64 },           // 根据 volume/ATR 动态估滑点（v2）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMeta {
    pub created_at_ms: i64,
    pub last_backtested_ms: Option<i64>,
    pub last_backtest_symbol: Option<String>,
    pub last_backtest_interval: Option<String>,
    pub schema_version: u32,
}
```

### 4.4 回测结果

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBacktestResult {
    pub system_id: String,
    pub symbol: String,
    pub interval: String,
    pub bars: usize,
    pub cost_model: CostModel,

    pub performance: Performance,  // 复用 backtest/types.rs 中的 Performance

    /// 权益曲线，每根 K 线一个点
    pub equity: Vec<EquityPoint>,

    /// 所有交易
    pub trades: Vec<Trade>,

    /// 每个组件对触发事件的贡献统计（诊断用）
    pub component_contribution: Vec<ComponentContrib>,

    /// 若启用 walk-forward，记录每个窗口的 OOS 表现
    pub walk_forward_windows: Option<Vec<WalkForwardWindow>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentContrib {
    pub component_id: String,
    pub triggers: usize,
    pub matched_system_entries: usize,  // 实际促成体系开仓的次数
    pub lone_triggers: usize,            // 仅此组件触发但其他组件不匹配 → 被聚合规则过滤掉的次数
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardWindow {
    pub train_start_ms: i64,
    pub train_end_ms: i64,
    pub oos_start_ms: i64,
    pub oos_end_ms: i64,
    pub oos_performance: Performance,
    pub oos_trades: usize,
}
```

---

## 5. 聚合规则（CombineRule）

### 5.1 `AllAligned`（最严格）

所有组件在当根 K 线必须同向触发。

```
foreach bar t:
    events = [c.trigger(klines, t) for c in components]
    if all(e.direction == +1 for e in events):  emit Buy
    elif all(e.direction == -1 for e in events): emit Sell
```

**适合**：四维共振系统、稳健低频体系
**问题**：触发极稀疏，样本少

### 5.2 `MajorityK`（多数派）

k 个或更多组件同向即可触发。

```
foreach bar t:
    events = [c.trigger(klines, t) for c in components]
    up = count(e.direction == +1)
    dn = count(e.direction == -1)
    if up >= k: emit Buy
    elif dn >= k: emit Sell
```

**适合**：允许部分组件失效的稳健体系
**推荐**：`k = ceil(n_components / 2)`

### 5.3 `WeightedScore`（加权投票）

每个组件有 `weight`，合计方向分数超阈值即触发。权重可由 `historical_winrate` 预填。

```
score = Σ(weight_i × direction_i × confidence_i)
if score >=  threshold: emit Buy
if score <= -threshold: emit Sell
```

**适合**：把"多维度但不要求全对齐"编码得更精细
**默认**：`threshold = 0.5 × Σ|weight|`

### 5.4 `SequentialCascade`（级联确认）

组件必须按定义顺序在 `window_bars` 根 K 线内依次触发。

```
例：[断头铡刀 → 毒蜘蛛] with window=5
   bar t   : 断头铡刀 ✓
   bar t+2 : 毒蜘蛛   ✓   → emit Sell at t+2
   bar t+6 : 毒蜘蛛   ✓   → 窗口已过，不触发
```

**适合**：先 A 后 B 的因果关系（例如旱地拔葱后金山谷）
**复杂度**：状态机，每个组件维护"最近多少根内触发过"

### 5.5 实现接口

```rust
// src/engine/system/combine.rs

pub trait CombineEvaluator {
    fn evaluate(
        &self,
        trigger_events: &[(String, Option<TriggerEvent>)],
        weights: &HashMap<String, f64>,
    ) -> Option<CombinedSignal>;
}

pub struct CombinedSignal {
    pub direction: i8,
    pub confidence: f64,
    pub contributing_components: Vec<String>,  // 实际参与的组件
}
```

---

## 6. 回测执行器（SystemRunner）

### 6.1 主流程

```rust
// src/engine/system/runner.rs

pub fn run(
    def: &SystemDefinition,
    klines: &[Kline],
    symbol: &str,
    interval: &str,
) -> SystemBacktestResult {
    // 1. 预计算每个组件在每根 K 线的触发事件
    let components: Vec<_> = def.components.iter()
        .map(|id| find_component(id).expect("unknown component"))
        .collect();
    let params = ComponentParams::default();

    let n = klines.len();
    let mut events: Vec<Vec<Option<TriggerEvent>>> = vec![vec![]; components.len()];
    for (ci, c) in components.iter().enumerate() {
        events[ci] = (0..n).map(|i| (c.trigger)(klines, i, &params)).collect();
    }

    // 2. ATR（用于止损计算）
    let atr = compute_atr(klines, 14);

    // 3. 主循环：按聚合规则扫描
    let evaluator = build_combine_evaluator(&def.combine);
    let mut position: Option<OpenTrade> = None;
    let mut trades: Vec<Trade> = Vec::new();
    let mut equity = vec![EquityPoint { ts: klines[0].open_time, value: 1.0 }];

    for t in def.backtest.warmup_bars..n {
        // 3.1 收集当根所有组件事件
        let per_comp: Vec<(String, Option<TriggerEvent>)> = components
            .iter()
            .enumerate()
            .map(|(ci, c)| (c.id.to_string(), events[ci][t].clone()))
            .collect();

        // 3.2 聚合
        let combined = evaluator.evaluate(&per_comp, &def.weights);

        // 3.3 持仓管理
        if let Some(open) = &position {
            if should_exit(open, &klines[t], atr[t], &def.risk, t) {
                let trade = close_trade(open, &klines[t], &def.backtest.cost_model);
                equity.push(EquityPoint {
                    ts: klines[t].close_time,
                    value: equity.last().unwrap().value * (1.0 + trade.pnl_pct),
                });
                trades.push(trade);
                position = None;
            }
        }

        // 3.4 开仓
        if position.is_none() {
            if let Some(sig) = combined {
                position = Some(open_trade(&klines[t], sig, atr[t], &def.risk, &def.backtest.cost_model));
            }
        }

        // 3.5 权益曲线 mark-to-market（若持仓）
        // ...
    }

    // 4. 统计指标
    let perf = metrics::summarize(&trades, &equity, klines);
    let contrib = compute_contribution(&events, &trades);

    SystemBacktestResult {
        system_id: def.id.clone(),
        symbol: symbol.into(),
        interval: interval.into(),
        bars: n,
        cost_model: def.backtest.cost_model.clone(),
        performance: perf,
        equity,
        trades,
        component_contribution: contrib,
        walk_forward_windows: None,  // Walk-forward 由 §7 包装器添加
    }
}
```

### 6.2 风控出场逻辑

按优先级依次判断：

1. **断头铡刀铁律**：任意时刻若出现 `MaAdvancedKind::Guillotine`，强制清仓（不因体系规则而赦免）
2. **止损**：价格突破 `entry ± ATR × stop_atr_mult`
3. **止盈**：价格达到 `entry ± R × target_r`（R = |entry - stop|）
4. **时间退出**：持仓超过 `max_hold_bars` 根
5. **反向信号**：聚合器输出与持仓反向的信号

---

## 7. Walk-Forward 验证框架

### 7.1 动机

**绝不能**在全部历史数据上一次性跑出指标就相信它。例子：

> 某组合在 2020-2025 全段回测胜率 75%，但如果把数据切成 `[2020-2023 训练 → 2024H1 验证 → 2024H2 训练 → 2025H1 验证]` 滚动测试，可能真实 OOS 胜率只有 48%。

走 walk-forward 才是诚实的"持续强化"口径。

### 7.2 参数

```rust
pub struct WalkForwardConfig {
    pub train_bars: usize,     // 训练窗大小（根）
    pub oos_bars: usize,       // 验证窗大小（根）
    pub step_bars: usize,      // 滚动步长（根），通常 = oos_bars
    pub min_trades_per_window: usize,  // 窗口内交易少于此数 → 丢弃窗口
}

pub const DEFAULT_WALKFORWARD: WalkForwardConfig = WalkForwardConfig {
    train_bars: 730,   // 约 2 年（日线）
    oos_bars: 183,     // 约 6 个月
    step_bars: 183,
    min_trades_per_window: 5,
};
```

### 7.3 算法

```
for window_start in 0..(n - train_bars - oos_bars) step step_bars:
    train_slice = klines[window_start..window_start + train_bars]
    oos_slice   = klines[window_start + train_bars..window_start + train_bars + oos_bars]

    # Discovery 模式：在 train_slice 上做组合搜索，得到最优体系 def*
    def_star = discover_best(train_slice)

    # 评估模式：在 oos_slice 上跑 def_star
    oos_result = runner::run(&def_star, oos_slice, ...)

    record_window(def_star, train_perf, oos_result)

# 聚合所有窗口的 OOS 指标
aggregated = aggregate_windows(all_windows)
```

### 7.4 报告口径

对外始终用 **OOS 平均** + **窗口间稳定性**，绝不用"全段一次性回测"：

```
Rank 1: [金山谷 + MA20 B2 + BIAS<8%]
  OOS Sharpe avg = 1.82 (std 0.34 across 6 windows)
  OOS Win Rate = 0.623 (std 0.08)
  OOS Max DD = -14.2% (worst 21.8%)
  Trades per window (avg) = 18
```

### 7.5 "训练"这个词的范围限定

本项目里的"训练"**只包含两种东西**：

1. **组件选择** — 哪几个组件组合成体系
2. **聚合规则选择** — AllAligned / MajorityK / ...

**不训练**的东西（避免过拟合）：

- 组件内部的识别阈值（如 `body_ratio_min`）
- 止损 ATR 倍数、target_r（用固定默认）
- K 线形态的具体判定逻辑

这些参数若要调，走单独的"参数敏感性分析"工具，而不是混在体系搜索里。

---

## 8. 组合发现器（Strategy Discovery）

### 8.1 核心算法：贪心前向选择（Greedy Forward Selection）

```
S = {}
score_history = []
for step in 1..MAX_K:
    best_c, best_score = None, score(S)
    for c in COMPONENTS where c not in S:
        score_new = walk_forward_score(S ∪ {c}, klines)
        if score_new > best_score:
            best_c, best_score = c, score_new
    if best_c is None or best_score - score(S) < MIN_IMPROVEMENT:
        break
    S = S ∪ {best_c}
    score_history.append((step, best_c.id, best_score))
return S, score_history
```

**复杂度**：`O(MAX_K × |COMPONENTS| × cost_of_walkforward)`

**量级估算**：
- MAX_K = 5
- |COMPONENTS| ≈ 80（全部）
- walk_forward on 2000 bars ≈ 200ms（cargo release）
- **总时间 ≈ 5 × 80 × 200ms × 6 个窗口 = 480 秒 ≈ 8 分钟**
- 用前端 SSE 流式返回每一步结果，用户不会感到卡死

### 8.2 复杂度惩罚（防过拟合）

```rust
fn penalized_score(s: &SystemDefinition, perf: &Performance) -> f64 {
    let raw = perf.sharpe;  // 或者 effectiveness_score（§9）
    let penalty = LAMBDA * (s.components.len() as f64 - 1.0);
    raw - penalty
}
```

**默认 LAMBDA = 0.1**：加一个组件必须让 Sharpe 提升 ≥ 0.1 才划算。

### 8.3 聚合规则搜索

对每个 `S`（组件集合），尝试所有可行聚合规则：

- `|S| = 1` → 只有 `AllAligned`（等价于单组件触发）
- `|S| = 2` → `AllAligned` / `MajorityK{k=1}`（= OR） / `SequentialCascade`
- `|S| = 3..5` → 全部四种

外层循环：
```
for rule in all_feasible_rules(S):
    def = SystemDefinition { components: S, combine: rule, ... }
    score[rule] = walk_forward_score(def)
pick rule* = argmax score
```

### 8.4 多样性保证（可选）

一个贪心搜索若只跑一次，结果会完全确定。为了输出 Top-K 不同风格的组合：

```
for iter in 0..K:
    S_i, score_i = greedy_with_exclusion(excluded = prev_S_union)
    yield (S_i, score_i)
```

每次从剩余组件空间里启动新一轮贪心，确保 K 个组合**不完全相同**。

### 8.5 进阶扩展路径（不在 MVP）

| 算法 | 何时加 | 复杂度 | 产出差异 |
|---|---|---|---|
| **Sequential Backward Elimination** | Phase 2 | 与 Forward 相当 | 互补：Forward 易局部最优，Backward 可纠偏 |
| **Genetic Algorithm** | Phase 3 | 高（~30 分钟） | 能找到 Forward 错过的高维组合 |
| **Bandit-driven Exploration** | Phase 3 | 可摊销 | 在线学习用户偏好 |

---

## 9. 评分函数（Scoring）

### 9.1 单组合 effectiveness_score

复用 `effectiveness.rs` 已有公式，升级为考虑多维因素：

```rust
fn effectiveness_score(perf: &Performance, n_components: usize) -> f64 {
    let n = perf.total_trades as f64;
    if n < 5.0 { return 0.0; }  // 样本不足直接 0

    let win_rate_delta = (perf.win_rate - 0.5).max(0.0);  // 抛硬币 = 0
    let avg_r = perf.expectancy_r.max(0.0);

    let base = (n.sqrt()) * win_rate_delta * avg_r * 10.0;

    // Sharpe 加成
    let sharpe_bonus = perf.sharpe.max(0.0) * 2.0;

    // 最大回撤惩罚
    let dd_penalty = (perf.max_drawdown_pct.abs() / 100.0).min(1.0) * 5.0;

    // 复杂度惩罚
    let complexity_penalty = (n_components as f64 - 1.0) * 2.0;

    (base + sharpe_bonus - dd_penalty - complexity_penalty).max(0.0)
}
```

### 9.2 多窗口聚合（walk-forward）

```rust
fn aggregated_score(windows: &[WalkForwardWindow]) -> AggregatedScore {
    let scores: Vec<f64> = windows.iter()
        .map(|w| effectiveness_score(&w.oos_performance, /* components */))
        .collect();

    let mean = mean(&scores);
    let std = stddev(&scores);
    let min = scores.iter().copied().fold(f64::INFINITY, f64::min);

    AggregatedScore {
        oos_mean: mean,
        oos_std: std,
        oos_worst: min,
        stability: 1.0 - (std / mean.abs().max(1e-6)).min(1.0), // 越接近 1 越稳
    }
}
```

**探索器用 `oos_worst` 而非 `oos_mean` 作为主排序键**（保守），`stability` 作为 tie-break。

---

## 10. 种子体系（Seed Systems）— 8 个原书经典

> 这些不是"固定"的，而是**用户起始模板 + 探索器的起点参考**。用户可以克隆、改、替换组件。

### 10.1 完整列表

| # | ID | 名称 | 组件 | 聚合 | 方向 | 原书出处 |
|---|---|---|---|---|---|---|
| 1 | `seed.ma_skeleton` | 均线骨架系统 | [B1, B2, B3] 葛南维 + [多头排列] | MajorityK{k=2} | 多 | 均线 §1-2 |
| 2 | `seed.golden_dragon` | 金山谷·蛟龙出海 | [金山谷, 蛟龙出海, 断头铡刀] | SequentialCascade | 多转空 | 均线 §4.2 |
| 3 | `seed.candle_reversal` | K线反转系统 | [曙光初现, 乌云盖顶, 镊子底, 镊子顶, 红三兵, 三只乌鸦] | MajorityK{k=1} | 双向 | K线 §3-4 |
| 4 | `seed.dow_trend` | 道氏趋势系统 | [道氏上升, 趋势线突破, MTF 共振] | AllAligned | 双向 | 趋势 §1-2 |
| 5 | `seed.resonance_4d` | 四维共振系统 | [MA, Trend, Candle, Chart 各至少 1] | AllAligned | 双向 | PRD §B8 |
| 6 | `seed.pattern_endgame` | 形态终局系统 | [头肩顶/底, 菱形顶/底, 双顶/底, 颈线突破] | MajorityK{k=2} | 双向 | K线 §5-7 |
| 7 | `seed.guillotine_risk` | 断头铡刀风控体系 | [断头铡刀] + 入场组 [旱地拔葱, 光脚阳线] | WeightedScore | 纯空清仓 + 多 | 均线 §4.3 |
| 8 | `seed.main_surge` | 主升浪追踪体系 | [气贯长虹, 战机起航, 连续跳空上扬, 主升浪] | MajorityK{k=2} | 多 | 均线 §4.4 |

### 10.2 种子体系 #5（四维共振）详细定义示例

```rust
SystemDefinition {
    id: "seed.resonance_4d".into(),
    name: "四维共振系统".into(),
    origin: SystemOrigin::Seed,
    description: Some("均线、趋势、K线、图形四个维度同向触发才开仓。触发稀疏但胜率稳定。".into()),
    components: vec![
        "ma.granville.b2_pullback".into(),
        "trend.dow_uptrend".into(),
        "candle.light_legs_bull".into(),
        "chart.ascending_triangle_confirmed".into(),
    ],
    combine: CombineRule::AllAligned,
    weights: Default::default(),
    risk: RiskParams {
        stop_atr_mult: 2.0,
        target_r: 3.0,
        max_hold_bars: 30,
        max_position_pct: 0.5,
    },
    backtest: BacktestParams {
        horizon_bars: 10,
        cost_model: CostModel::Fixed { fee_pct: 0.10, slip_pct: 0.05 },
        warmup_bars: 60,
    },
    meta: /* ... */,
}
```

---

## 11. API 设计

### 11.1 GET `/api/systems/components`

返回全部可选组件元数据。

```json
{
  "ok": true,
  "data": {
    "total": 82,
    "by_dimension": {
      "MaSignal": [ { "id": "ma.granville.b1_breakout", "label": "葛南维 B1", "book_source": "均线 §2.1 p.45", "direction_bias": 1, "historical_alpha_pct": 3.2, "historical_winrate": 0.58 }, ... ],
      "MaSpecial": [ ... ],
      "CandlePattern": [ ... ],
      "ChartPattern": [ ... ],
      "TrendStructure": [ ... ]
    }
  }
}
```

### 11.2 POST `/api/systems/backtest`

入参：

```json
{
  "definition": { /* SystemDefinition, 完整 JSON */ },
  "symbol": "BTCUSDT",
  "interval": "1d",
  "bars": 2000,
  "walk_forward": { "train_bars": 730, "oos_bars": 183, "step_bars": 183 } // 可选
}
```

响应：`SystemBacktestResult` 完整 JSON（含 equity/trades，约 100-500 KB）。

### 11.3 GET `/api/systems`

列用户保存的 + 种子体系。

### 11.4 POST `/api/systems/save`

```json
{
  "definition": { /* SystemDefinition */ },
  "overwrite": false
}
```

存至 `data_cache/user_systems.v1.json`（同 Bandit state 的持久化方式）。

### 11.5 POST `/api/systems/compare`

```json
{
  "system_ids": ["seed.ma_skeleton", "seed.resonance_4d", "user.my_combo_1"],
  "symbol": "BTCUSDT",
  "interval": "1d",
  "bars": 2000
}
```

响应：并排对比结果，只返回聚合指标 + 压缩的权益曲线（downsample 到 200 点）以保持响应小。

### 11.6 POST `/api/systems/discover`（SSE 流式）

入参：

```json
{
  "symbol": "BTCUSDT",
  "interval": "1d",
  "bars": 2000,
  "max_k": 5,
  "top_k": 10,
  "walk_forward": { /* ... */ }
}
```

响应流（SSE）：

```
event: progress
data: { "step": 1, "candidates_tried": 12, "candidates_total": 80, "best_so_far": { "components": ["ma.granville.b2"], "score": 5.3 } }

event: progress
data: { "step": 2, "candidates_tried": 25, ... }

...

event: result
data: { "top_combinations": [ {...}, {...}, ... ] }
```

---

## 12. 前端设计

### 12.1 整体布局（新 Tab "🧪 体系实验室"）

```
┌───────────────┬────────────────────────────────────┬─────────────────┐
│ 左：组件调色板 │  中：组合构造区 + 回测指标           │  右：K 线图     │
│ (30% 宽)      │  (40% 宽)                          │  (30% 宽)       │
├───────────────┼────────────────────────────────────┼─────────────────┤
│ [搜索框]      │ 当前组合：                          │ [图表]          │
│               │  ┌──────────────────┐              │                 │
│ ▾ 均线信号    │  │ ✓ 葛南维 B2       │ [✕]          │  带信号标注     │
│   □ 葛南维 B1 │  │ ✓ 多头排列        │ [✕]          │  按维度着色     │
│   ☑ 葛南维 B2 │  │ ✓ 金山谷          │ [✕]          │                 │
│   □ 葛南维 B3 │  └──────────────────┘              │                 │
│   ...         │                                    │                 │
│               │ 聚合规则: [MajorityK k=2 ▾]        │                 │
│ ▾ 均线特殊    │                                    │                 │
│   ☑ 金山谷    │ 风控: ATR×[2.0] R目标×[3.0]       │                 │
│   □ 死亡谷    │       最大持仓[30]根              │                 │
│   ...         │                                    │                 │
│               │ 成本口径: [固定 0.1% + 0.05% ▾]    │                 │
│ ▾ K 线形态    │                                    │                 │
│   ...         │ [🔄 回测] [💾 保存为体系]         │                 │
│               │                                    │                 │
│ ▾ 图形        │ ═══ 结果 ═══                       │                 │
│   ...         │  胜率: 68.2%                       │                 │
│               │  Sharpe: 1.82                      │                 │
│ ▾ 趋势结构    │  最大回撤: -14.5%                  │                 │
│   ...         │  交易数: 42                        │                 │
│               │                                    │                 │
└───────────────┤  [权益曲线 sparkline]              │                 │
                │                                    │                 │
                └────────────────────────────────────┴─────────────────┘

底部按钮栏：
[📂 我的体系] [🌱 种子体系] [✨ 自动发现 Top 10] [⚖️ 对比模式]
```

### 12.2 关键交互

**组件勾选**：左侧 accordion 按维度分组，每个组件卡片：

```
┌─────────────────────────────────────┐
│ ☑ 葛南维 B2 回踩                    │
│   均线 §2.1 p.48                    │
│   历史 α: +3.2%  |  胜率: 58%       │
└─────────────────────────────────────┘
```

勾选即加入右侧组合区，取消勾选即移除。点击组件卡片展开看详细说明。

**实时回测**：组合变化时，500ms 防抖后自动重新回测（触发 `/api/systems/backtest`），不需要点按钮。指标卡片平滑过渡。

**自动发现**：点击 `✨ 自动发现 Top 10` → 弹出侧边栏，展示 SSE 进度：

```
🔍 正在搜索高胜率组合...
   进度: 3/5 (候选试过 187/400)

当前最佳:
  [金山谷 + MA20 B2] Sharpe 1.45

Top 3 已确定:
  1. [金山谷 + MA20 B2 + BIAS<8%] Sharpe 1.82  [👁 查看]
  2. [断头铡刀 + 毒蜘蛛] Sharpe 1.45          [👁 查看]
  3. [菱形顶 + 红三兵止损] Sharpe 1.38        [👁 查看]

[⏹ 停止] [⏭ 跳过此窗口]
```

完成后用户可以点 `[👁 查看]` 把该组合加载回左侧 + 中间，或者点 `[💾 保存]` 直接存为 `discovered.xxx`。

**对比模式**：顶部切换 → 左侧变多选 → 选中的体系各画一条权益曲线到中间大图，右侧 K 线图的信号按体系颜色叠加。

**图表信号着色**：
- 均线维度：蓝色
- 均线特殊：深蓝
- K 线形态：橙色
- 图形：紫色
- 趋势结构：绿色

每个标注 hover 显示："来自 `seed.ma_skeleton` · 葛南维 B2 回踩 · 均线 p.48"。

---

## 13. 与现有模块的关系

```
                         ┌──────────────────────────────┐
                         │  engine/effectiveness.rs     │
                         │  (单组件有效性评估)           │
                         └────────────┬─────────────────┘
                                      │
                  给 Component 的 historical_alpha 注入
                                      ▼
┌────────────────────┐    ┌──────────────────────────────┐    ┌─────────────────┐
│ engine/candle/     │    │ engine/system/               │    │ engine/rl/      │
│ engine/ma/         │───▶│   component.rs (注册)         │    │ (Phase 2 挂入)  │
│ engine/chartpattern│    │   definition.rs              │    │                 │
│ engine/trend/      │    │   combine.rs                 │    │ 每个 System 作为│
│                    │    │   runner.rs                  │    │  一个 L1 arm    │
│ (触发器 trigger fn) │    │   discover.rs                │───▶│                 │
└────────────────────┘    │   registry.rs (种子体系)      │    └─────────────────┘
                          │   compare.rs                 │
                          └────────────┬─────────────────┘
                                       │
                                       ▼
                          ┌──────────────────────────────┐
                          │ engine/backtest/             │
                          │ (Performance / Trade 类型复用)│
                          └──────────────────────────────┘
```

**关键边界**：

- **不重写** `engine/candle/ma/chartpattern/trend/`，只**引用**它们的触发器
- **不重写** `engine/backtest/metrics.rs`，直接复用 `Performance / Trade` 类型
- **不改** `engine/effectiveness.rs`，它的单组件报告反向喂给 Component 的元数据
- Bandit 挂入是 **Phase 2 的选项**，Phase 1 不依赖 RL

---

## 14. 风险与边界情况

| 风险 | 缓解 |
|---|---|
| **小样本过拟合** | 走 walk-forward + `min_trades_per_window` 过滤 + 复杂度惩罚 |
| **组件识别 bug 污染全部体系** | PATTERN_EFFECTIVENESS_REPORT.md 已点名的 V 形/双底 W/旗形等必须**先修复再参与探索**；在 Component 注册表加 `quality_flag: Beta/Stable/Deprecated` 字段 |
| **探索器耗时过长** | SSE 流式 + 最大 `MAX_K=5` + `MIN_IMPROVEMENT` 早停 + 时间预算（默认 5 分钟硬截止） |
| **多聚合规则组合爆炸** | |S|≥3 时每次只试 4 种规则，不枚举子集 |
| **前端渲染大量信号标注卡顿** | 标注数 > 500 时自动聚合成"信号密度热力图" |
| **断头铡刀铁律被用户覆盖** | 硬编码：`should_exit` 内无论体系配置如何，检测到 `MaAdvancedKind::Guillotine` 一律清仓 |
| **用户存储组合数量爆炸** | 单用户上限 100 个自定义体系；超过提示删除 |
| **成本模型默认值偏离实盘** | 前端下拉允许切换 3 种成本口径（含"零成本"供快速验证） |
| **跨 symbol 共享 vs 分离** | 默认每次回测按指定 symbol+interval 独立评估。未来可加"多 symbol 聚合"模式 |
| **look-ahead bias** | runner.rs 中严格只用 `klines[..=t]`；组件的 trigger 函数也必须遵守 |

---

## 15. 分阶段 Roadmap

### Phase 1 — MVP（约 7 个工作日）

目标：用户能勾选组件 → 看到回测 → 保存体系 → 对比图表。

| 里程碑 | 时长 | 内容 | 验收 |
|---|---|---|---|
| **M1** | 2 天 | `component.rs` + `definition.rs` + 20 个核心组件注册 + `runner.rs` 基础版 | `cargo run --example system_backtest` 跑通一个种子体系 |
| **M2** | 1 天 | `combine.rs` 4 种规则 + 单测 | combine 模块测试绿 |
| **M3** | 1 天 | 种子体系注册（8 个） + `examples/seed_systems_benchmark.rs` | 8 个种子体系在 BTC 1d 2000 根 K 线上都跑通，指标合理 |
| **M4** | 1 天 | HTTP API：`/api/systems/components`, `/api/systems/backtest`, `/api/systems`, `/api/systems/save` | curl 测试通过 |
| **M5** | 2 天 | 前端 Tab "🧪 体系实验室"（左+中区域，无对比模式，无自动发现） | 用户能交互完成一次"勾选 → 回测 → 保存"流程 |

### Phase 2 — 发现器 + 对比模式（约 5 个工作日）

| 里程碑 | 时长 | 内容 |
|---|---|---|
| **M6** | 2 天 | `discover.rs` 贪心前向选择 + 复杂度惩罚 + 多样性输出 |
| **M7** | 1 天 | Walk-forward 包装器 + 多窗口聚合 |
| **M8** | 1 天 | SSE 流式 API `/api/systems/discover` |
| **M9** | 1 天 | 前端自动发现面板 + 对比模式 UI |

### Phase 3 — 深度集成（约 5 个工作日，按需）

| 里程碑 | 时长 | 内容 |
|---|---|---|
| **M10** | 2 天 | Bandit 集成：每个体系作为 L1 arm，实时面板给出推荐 |
| **M11** | 1 天 | 遗传算法探索器（作为贪心的补充） |
| **M12** | 1 天 | 动态成本模型（按 volume/ATR 估算滑点） |
| **M13** | 1 天 | 导出 CSV / 分享 URL / 体系版本管理 |

### Phase 4 — 长期（>1 个月）

- WebSocket 实时行情接入，体系在新 K 线 close 时自动检测触发
- Paper Trading 模式：选定体系在真实时间流上跑虚拟账户，累计 1 个月后对照 OOS 指标
- 多 symbol 聚合评估：同一体系在 10 个币种上的 OOS 指标矩阵

---

## 16. 附录 A：默认成本模型

### 16.1 三种模型对比

| 模型 | 公式 | 适用 | 默认值 |
|---|---|---|---|
| **Zero** | 无成本 | 快速验证识别器 | — |
| **Fixed**（MVP 默认） | `cost = (fee_pct + slip_pct) × 2`（双边） | 大部分场景 | fee=0.1%, slip=0.05% |
| **Dynamic** | `slip = base_fee + k × ATR / volume_pct`（volume_pct = 当根 volume / 均值） | 高频 / 大仓位 | base=0.05%, k=0.3 |

### 16.2 成本应用位置

- 开仓：`entry_price × (1 + cost_pct)`（买）或 `× (1 - cost_pct)`（卖）
- 平仓：同理反向
- 权益曲线：每笔交易后扣减 `2 × cost_pct × notional`

---

## 17. 附录 B：术语表

| 术语 | 简述 |
|---|---|
| Arm | Bandit 术语：一个决策臂 |
| ATR | Average True Range，真实波动幅度均值 |
| BIAS | 价格相对均线的偏离度 |
| CAGR | 年化复合收益率 |
| Confluence | 多维度共振 |
| MAE | Max Adverse Excursion，持仓期间最大浮亏 |
| MFE | Max Favorable Excursion，持仓期间最大浮盈 |
| OOS | Out-Of-Sample，样本外 |
| R-multiple | 盈亏相对于初始风险的倍数 |
| Sharpe | 夏普比，风险调整后收益 |
| SSE | Server-Sent Events，HTTP 单向流式 |
| TDD | 技术设计文档 |
| Walk-Forward | 滚动时间窗验证法 |

---

## 18. 附录 C：FAQ

**Q1：为什么不直接用深度强化学习端到端训练？**
A：三条理由：
1. 样本极少（每天 10-100 个触发）→ DRL 无法收敛
2. 不可解释 → 违反"每一步都要追溯原书某页"的项目哲学
3. 现有 Bandit（Thompson Sampling）在本规模下数学上严格优于 DRL
详见 `RL_EFFECTIVENESS_DESIGN.md` §2。

**Q2：为什么 MAX_K=5？组件再多不是能学更精细？**
A：第 6 个组件边际收益通常很小，且组合数量随 K 指数膨胀。`K=5` 在复杂度、样本数、可解释性三者间平衡最佳。想突破可用遗传算法（Phase 3）。

**Q3：用户保存的体系会被系统自动修改吗？**
A：不会。用户体系是**只读模板**，系统只读不写。探索器产出的新体系另存为 `discovered.xxx` 命名空间，不覆盖用户的。

**Q4：种子体系的权重可以调吗？**
A：MVP 阶段不暴露权重调节 UI，但用户可以克隆种子 → 改组件 → 保存为自己的。Phase 3 会加 WeightedScore 规则的权重滑块。

**Q5：多 symbol / 多 timeframe 怎么处理？**
A：MVP 固定"一个体系 × 一个 symbol × 一个 interval"的回测单元。多 symbol 聚合评估在 Phase 4。每个体系可以带 `applicable_timeframes: Vec<Timeframe>` 提示用户它适用哪些周期（基于原书说明 + 历史有效性）。

**Q6：回测结果可信度如何？**
A：
- MVP 默认的单段回测 = "乐观上界"，适合快速筛选
- Walk-forward OOS = "真实表现"，上线前必看
- Paper trading = "实盘预演"，Phase 4 才有
前端明确标注每个数字是哪个口径出来的，不混淆。

**Q7：如何防止探索器过拟合历史数据？**
A：三重保护：
1. Walk-forward OOS 评估
2. 复杂度惩罚（`LAMBDA × n_components`）
3. `min_trades_per_window` 硬阈值
4. 排序用 `oos_worst` 而非 `oos_mean`

**Q8：组件识别 bug 会不会污染所有体系？**
A：会。所以 MVP 第一步必须先修 `PATTERN_EFFECTIVENESS_REPORT.md` 点名的 V 形/双底 W/旗形识别器，否则含这些组件的体系全部不可信。参见 §14。

---

## 19. 决策点：开工前需要澄清

| # | 问题 | MVP 默认选择 | 需要用户确认 |
|---|---|---|---|
| 1 | 是否允许用户自定义组合 | **允许**（JSON 存储，无 UI 编辑器） | ✅ 已确认 |
| 2 | 多体系图表显示模式 | 单体系切换 + 「对比模式」开关 | ⚠ 待确认 |
| 3 | 默认成本模型 | Fixed (fee=0.1%, slip=0.05%) | ⚠ 待确认 |
| 4 | 是否挂入 Bandit | Phase 1 不挂，Phase 3 才挂 | ⚠ 待确认 |
| 5 | 单用户保存体系上限 | 100 个 | 建议默认 |
| 6 | 探索器时间预算 | 5 分钟硬截止，用户可打断 | 建议默认 |
| 7 | MAX_K（组件数上限） | 5 | 建议默认 |
| 8 | Walk-forward 默认窗 | 日线 2y/6m，4h 6mo/1mo，周线 5y/1y | 建议默认 |
| 9 | 种子体系数量 | 8 个（§10.1） | ⚠ 若要增减需确认 |

---

> **下一步**：用户确认本 TDD 后，进入 `Phase 1 → M1`：搭建 `src/engine/system/` 模块骨架 + 20 个核心组件 + 一个种子体系的可跑回测。预期 2 天内能在 `cargo run --example system_backtest` 看到第一个端到端的回测结果。
