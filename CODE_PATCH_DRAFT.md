# AURA_TRADE P0 代码 Patch 草案 v1

**基于**：`BOOK_REVIEW_FINDINGS.md` E5 / E17 / E18 / E9 / E2-E4 / E1
**目的**：提供可直接 apply 的代码修改，修复 P0 bug
**注意**：每个 patch 后需重跑 `cargo test` 和 `PATTERN_EFFECTIVENESS_REPORT.md` 的回测
**状态**：草案，待用户 sign-off

---

## Patch 1 —— E5：修正 `find_crosses` 加入同向条件（P0）

**文件**：`src/engine/ma/alignment.rs`
**原书依据**：ma p.224 "死亡交叉必须满足：①短穿长 ②两条均线同时都在下行"

### 修改前（line 102-139）

```rust
pub enum CrossKind { Golden, Death }

pub fn find_crosses(
    fast: &[f64], slow: &[f64],
    fast_period: usize, slow_period: usize,
) -> Vec<Cross> {
    let mut out = Vec::new();
    let n = fast.len().min(slow.len());
    for i in 1..n {
        let (p_fast, p_slow, c_fast, c_slow) = (fast[i-1], slow[i-1], fast[i], slow[i]);
        if !(p_fast.is_finite() && ... ) { continue; }
        if p_fast <= p_slow && c_fast > c_slow {
            out.push(Cross { ..., kind: Golden });
        } else if p_fast >= p_slow && c_fast < c_slow {
            out.push(Cross { ..., kind: Death });
        }
    }
    out
}
```

### 修改后

```rust
pub enum CrossKind {
    Golden,        // 黄金交叉：短穿长 + 两线同时上行
    Death,         // 死亡交叉：短破长 + 两线同时下行
    PlainUp,       // 普通交叉（向上）：短穿长但某条方向不符 → 无信号意义
    PlainDown,     // 普通交叉（向下）：短破长但某条方向不符 → 无信号意义
}

impl CrossKind {
    /// 是否为原书明确的交易信号（黄金/死亡交叉）
    pub fn is_signal(&self) -> bool {
        matches!(self, CrossKind::Golden | CrossKind::Death)
    }
}

pub fn find_crosses(
    fast: &[f64], slow: &[f64],
    fast_period: usize, slow_period: usize,
) -> Vec<Cross> {
    find_crosses_with_slope(fast, slow, fast_period, slow_period, 5)
}

/// 新签名：通过 slope_lookback 回望 N 根，自行计算方向
pub fn find_crosses_with_slope(
    fast: &[f64], slow: &[f64],
    fast_period: usize, slow_period: usize,
    slope_lookback: usize,
) -> Vec<Cross> {
    let mut out = Vec::new();
    let n = fast.len().min(slow.len());
    for i in slope_lookback.max(1)..n {
        let (p_fast, p_slow, c_fast, c_slow) = (fast[i-1], slow[i-1], fast[i], slow[i]);
        if !(p_fast.is_finite() && p_slow.is_finite() && c_fast.is_finite() && c_slow.is_finite()) {
            continue;
        }
        // 方向：基于 slope_lookback 根回望
        let fast_up = c_fast > fast[i - slope_lookback];
        let slow_up = c_slow > slow[i - slope_lookback];

        let up_cross = p_fast <= p_slow && c_fast > c_slow;
        let down_cross = p_fast >= p_slow && c_fast < c_slow;

        let kind = match (up_cross, down_cross, fast_up, slow_up) {
            (true,  _,    true,  true)  => CrossKind::Golden,
            (true,  _,    _,     _)     => CrossKind::PlainUp,
            (_,     true, false, false) => CrossKind::Death,
            (_,     true, _,     _)     => CrossKind::PlainDown,
            _ => continue,
        };
        out.push(Cross { index: i, fast_period, slow_period, kind });
    }
    out
}
```

### 调用方影响
- `src/engine/ma/special.rs` 里 `cross_bars` 参数可改为只保留 `kind.is_signal() == true` 的索引
- `src/engine/resonance/` 中使用交叉事件的位置需 filter 掉 Plain
- 测试：新增一组普通交叉样本（已在原书图 3-120 给出丹甫股份），确保其返回 `PlainUp/PlainDown`

---

## Patch 2 —— E17：修正葛南维 B2 条件（P0）

**文件**：`src/engine/ma/granville.rs`
**原书依据**：ma p.84-85 L2 = "回调时**未跌破**均线"

### 修改前（line 124）

```rust
// B2：均线上行，价格回踩至均线附近获支撑后上行
if s.is_finite() && s > p.slope_eps && c > m && c_prev <= m_prev * 1.001 && c >= c_prev {
    out.push(GranvilleSignal { index: i, rule: B2PullbackBuy });
    continue;
}
```

### 修改后

```rust
// B2 回踩买入：价格必须 **未跌破均线**（L2 原书定义），区别于 B3 假跌破
// c_prev >= m_prev（未破），但靠近均线（以回踩）
if s.is_finite() && s > p.slope_eps && c > m 
   && c_prev >= m_prev                              // 关键：未跌破
   && c_prev <= m_prev * (1.0 + p.touch_band)       // 靠近均线
   && c >= c_prev {                                 // 当前反弹
    out.push(GranvilleSignal { index: i, rule: B2PullbackBuy });
    continue;
}
```

### 配置新增
```rust
pub struct GranvilleParams {
    // ...
    pub touch_band: f64,  // 回踩带宽：默认 0.02（= 2%）；原书未明确数值，需 backtest 调优
}
```

### S2 同样修正
```rust
if s < -p.slope_eps && c < m 
   && c_prev <= m_prev                              // 未站上均线
   && c_prev >= m_prev * (1.0 - p.touch_band)
   && c <= c_prev {
    out.push(GranvilleSignal { index: i, rule: S2ReboundSell });
    continue;
}
```

---

## Patch 3 —— E18：修正葛南维 B4/S4 方向（**P0 最严重**）

**文件**：`src/engine/ma/granville.rs`
**原书依据**：ma p.83 L4 = "均线**下行**，股价在均线之下，暴跌远离均线 → 反弹买入"

### 修改前（line 152-160）

```rust
// B4：均线上行 + 乖离率深度负  ← 完全反了！
if s.is_finite() && s > p.slope_eps && b.is_finite() && b < -p.bias_thresh {
    out.push(GranvilleSignal { index: i, rule: B4DivergenceBuy });
    continue;
}
// S4：均线下行 + 乖离率深度正  ← 同样反了
if s.is_finite() && s < -p.slope_eps && b.is_finite() && b > p.bias_thresh {
    out.push(GranvilleSignal { index: i, rule: S4DivergenceSell });
    continue;
}
```

### 修改后

```rust
// B4 乖离买入（逆势反弹，轻仓）：
// - 均线下行（下降趋势）
// - 股价在均线之下
// - 深度负乖离（暴跌远离均线）
if s.is_finite() && s < -p.slope_eps 
   && c < m
   && b.is_finite() && b < -p.bias_thresh {
    out.push(GranvilleSignal { index: i, rule: B4DivergenceBuy });
    continue;
}

// S4 乖离卖出（超涨回落）：
// - 均线上行（上升趋势）
// - 股价在均线之上
// - 深度正乖离（暴涨远离均线）
if s.is_finite() && s > p.slope_eps 
   && c > m
   && b.is_finite() && b > p.bias_thresh {
    out.push(GranvilleSignal { index: i, rule: S4DivergenceSell });
    continue;
}
```

### ⚠️ 重要：此修复可能大幅改变回测结果

- 当前代码的 B4 在**牛市强势上涨**中触发（超卖抄底型），大概率盈利
- 修复后 B4 在**熊市暴跌**中触发（逆势反弹），大概率**亏损或仅小幅反弹**
- 原书 L4 明确要求"**仓位一定要轻，快进快出**"（因在下降趋势中）

**强制配套**：
- B4 触发时 `position_size_limit` 应限制 ≤ 30%
- B4 信号必须附带 `stop_loss` 设置
- 测试需验证 B4 在熊市样本中的表现

---

## Patch 4 —— E9：葛南维基准均线 MA20 → MA60（P0）

**文件**：`src/engine/ma/granville.rs`
**原书依据**：ma p.155 "沪深股市葛南维八大法则运用 60 日均线最为有效"

### 修改前（line 82）

```rust
impl Default for GranvilleParams {
    fn default() -> Self {
        Self {
            period: 20,
            // ...
        }
    }
}
```

### 修改后

```rust
impl Default for GranvilleParams {
    fn default() -> Self {
        Self {
            period: 60,  // 原书 ma p.155 明确：沪深最有效为 60 日
            // ...
        }
    }
}

/// 葛南维法则预设三套参数
impl GranvilleParams {
    /// 原书沪深市场推荐（60 日季线）—— 默认
    pub fn cn_default() -> Self { Self { period: 60, ..Default::default() } }
    /// 葛南维原版美股（200 日）
    pub fn us_classic() -> Self { Self { period: 200, ..Default::default() } }
    /// 短期附加（20 日，虚假信号多，仅用于印证）
    pub fn short_confirm() -> Self { Self { period: 20, ..Default::default() } }
}
```

### 测试影响
- `tests/` 中所有以 MA20 为基准的 granville 测试需更新为 MA60
- backtest benchmark 需用 MA60 重跑

---

## Patch 5 —— E2/E3：`special.rs` 重分类 + 删除凭空形态（P0）

**文件**：`src/engine/ma/special.rs`

### 删除以下凭空枚举项（原书无）
```rust
// 以下 5 项应删除或迁移到 "AURA 扩展" 模块
RapidUp,           // 快速上升 —— 原书无此独立形态
RapidDown,         // 快速下降 —— 同上
Mire,              // 烂泥潭 —— 原书无
BullBearBoundary,  // 牛熊分界 —— 原书无
CycleSwap,         // 周期轮换 —— 原书无
```

### 迁移到 `alignment_forms.rs`（Ch3·3 排列章节）
```rust
UphillClimb,       // 上山爬坡 → 属 Ch3·3·1（与多头排列同节）
DownhillSlide,     // 下山滑坡 → 属 Ch3·3·2
WaveUp,            // 逐浪上升 → 同 Ch3·3·1
WaveDown,          // 逐浪下降 → 同 Ch3·3·2
BullArrangement,   // 多头排列 → Ch3·3·1
BearArrangement,   // 空头排列 → Ch3·3·2
MaBond,            // 均线粘合 → Ch3·3·5
```

### 保留在 `special.rs` 的 3 项（原书 Ch4·1 特殊形态）
```rust
AcceleratingUp,    // 加速上行（原书 Ch4·1·1）
AcceleratingDown,  // 加速下行（原书 Ch4·1·2）
SilverValley,      // 银山谷（原书 Ch4·1·7）
GoldenValley,      // 金山谷（原书 Ch4·1·8）
DeathValley,       // 死亡谷（原书 Ch4·1·9）
```

### 需新增的 11 项（原书 Ch4·1 剩余）
```rust
pub enum MaSpecialKind {
    // ...
    FighterJetLaunch,      // 战机起航（Ch4·1·3）
    DiveBomb,              // 俯冲式下降（Ch4·1·4）
    LongRainbow,           // 气贯长虹（Ch4·1·5）
    FireBrigade,           // 火烧连营（Ch4·1·6）
    Skull,                 // 骷髅头（Ch4·1·10）
    StaircaseUp,           // 阶梯式上升（Ch4·1·11）
    FishLeap,              // 鱼跃龙门（Ch4·1·12）
    GroundUproot,          // 旱地拔葱（Ch4·1·13）
    DeathLeap,             // 绝命跳（Ch4·1·14）
    GoldenSpider,          // 金蜘蛛（Ch4·1·15）
    ToxicSpider,           // 毒蜘蛛（Ch4·1·16）
}
```

各形态识别规则见 `BOOK_REVIEW_FINDINGS.md` 轮次 1-2。

---

## Patch 6 —— E4：新增 Ch3·3·6-9 四个文件（P0）

**新文件**：
- `src/engine/ma/converge_diverge.rs`
- `src/engine/ma/obedience.rs`
- `src/engine/ma/divergence.rs`
- `src/engine/ma/repair.rs`

### 6.1 `converge_diverge.rs` 核心签名

```rust
//! Ch3·3·6 均线收敛与发散

#[derive(Debug, Clone, Copy)]
pub enum ConvergeDivergeEvent {
    /// 均线收敛：间距由大变小
    Converging,
    /// 多头发散：股价之上方的均线由收敛开始向两侧扩大
    BullishDiverging,
    /// 空头发散：股价之下方的均线由收敛开始向两侧扩大
    BearishDiverging,
}

pub fn detect(
    ma_stack: &[&[f64]],       // 多条均线序列（按周期升序）
    closes: &[f64],
    i: usize,
    lookback: usize,           // 计算"间距变化"的窗口（默认 20）
) -> Option<ConvergeDivergeEvent>;
```

### 6.2 `obedience.rs` 核心签名

```rust
//! Ch3·3·7 均线服从与扭转

pub enum ObedienceState {
    Obedient,       // 服从：短周期方向 == 长周期方向
    Inverting,      // 扭转中：短周期刚改变方向，长周期尚未跟随
    Inverted,       // 扭转完成：短周期带动长周期改变方向
}

pub fn classify(
    short: &[f64], long: &[f64], i: usize, slope_window: usize,
) -> ObedienceState;
```

### 6.3 `divergence.rs` 核心签名

```rust
//! Ch3·3·8 均线背离

pub struct MaDivergence {
    pub index: usize,
    pub kind: DivergenceKind,
    pub preceded_by_extreme: bool,  // 必须在暴涨/暴跌之后（原书要求）
}

pub enum DivergenceKind {
    BullishDiv,    // 长期均线继续上行，短期已下穿长期（暴涨后矫枉过正）
    BearishDiv,    // 长期继续下行，短期已上穿长期
}
```

### 6.4 `repair.rs` 核心签名

```rust
//! Ch3·3·9 均线修复

pub enum RepairMode {
    ActiveRepair,  // 主动修复：股价急速回归均线
    PassiveRepair, // 被动修复：股价横盘，等均线追上来
}

pub fn classify(
    closes: &[f64], ma: &[f64], i: usize, window: usize,
) -> Option<RepairMode>;
```

---

## 执行顺序与测试计划

| 顺序 | Patch | 测试 | 预估影响 |
|---|---|---|---|
| 1 | **Patch 3**（E18 B4 方向） | `cargo test granville` + 回测 | 🔴 **重大**：backtest 收益可能显著变化 |
| 2 | **Patch 1**（E5 find_crosses 同向） | `cargo test alignment` + 样本 `丹甫股份 002366` | 减少假交叉信号 |
| 3 | Patch 2（E17 B2/B3 条件） | `cargo test granville` | B2 信号数下降 |
| 4 | Patch 4（E9 MA20 → MA60） | 全量回归 | backtest 基准线变化 |
| 5 | Patch 5（special.rs 重分类） | `cargo test ma_special_test` | 枚举改名，测试更新 |
| 6 | Patch 6（新增 4 文件） | 新写单元测试 | 增加新信号 |

---

## 回归测试要求

1. 用 `examples/aggregate_effectiveness.rs` 跑 10 只代表股票，对比 **修复前 vs 修复后** 的：
   - B4 信号数量变化（预计暴跌 >90%，因方向反转）
   - 黄金/死亡交叉信号数量变化（预计下降 30-50%）
   - 整体 backtest alpha 变化
2. 重点关注：**修复 Patch 3 后 B4 胜率是否从 ">50%" 降到 "<50%"**
   - 如果是：证明修复正确，原书"轻仓快进快出"是对的
   - 如果否：说明 B4 在中国市场可能依然有效，需进一步分析（但仍应按原书标记为 `BuyWarning`）

---

## 未纳入本次草案（P1 后续）

- R-P1-01 至 R-P1-11（见 `PRD_REVISION_DRAFT.md`）
- 缺失的 K 线形态补全（~40 种）
- SignalLevel 4 分级
- 阴阳哲学层
