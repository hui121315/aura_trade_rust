# AURA_TRADE PRD 修订建议稿

**基于**：`BOOK_REVIEW_FINDINGS.md`（轮次 1-6 累计发现 E1-E23）
**生效前置**：需通读全部 1505 页原书（当前进度 ~7%）+ 用户 sign-off
**修订日期**：Iteration 1（轮次 6 起草）

---

## 一、P0 必修错误（7 项，必须修复才能发布）

### R-P0-01：葛南维基准均线 —— MA20 → **MA60**

**PRD 现状**：
```
基准均线 = MA20
```

**原书依据**（ma p.155, p.90）：
> "葛南维八大买卖法则在沪深股市运用 **60 日均线最为有效**。"
> "本书默认 = 60 日均线，除特别注明。"

**建议修订**：
```
基准均线 = MA60（季线）
可选配置：MA200（葛南维原版，美股适用）
短期/附加信号：MA20 / MA30（虚假信号多，仅用于印证）
```

**代码影响**：
- `@/Users/shiyizhidahui/Desktop/交易/aura_trade_rust/src/engine/ma/granville.rs:82` 改 `period: 60`
- 测试 case `tests/` 基准数据重新生成

---

### R-P0-02：修复 `find_crosses` —— 加入"同向条件"

**PRD 现状**：A5 黄金交叉/死亡交叉仅要求数值穿越。

**原书依据**（ma p.224）：
> "死亡交叉必须满足两个条件：①短期均线由上而下穿越长期均线；**②短期均线和长期均线同时都在下行**"

**建议修订**：
```rust
// 新增 slope 参数
pub fn find_crosses(
    fast: &[f64], slow: &[f64],
    fast_slope: &[f64], slow_slope: &[f64],  // 新增
    fast_period: usize, slow_period: usize,
) -> Vec<Cross> {
    // ...
    if p_fast <= p_slow && c_fast > c_slow 
       && fast_slope[i] > 0.0 && slow_slope[i] > 0.0 {  // 同向上行
        out.push(Cross { ..., kind: CrossKind::Golden });
    } else if p_fast >= p_slow && c_fast < c_slow 
              && fast_slope[i] < 0.0 && slow_slope[i] < 0.0 {  // 同向下行
        out.push(Cross { ..., kind: CrossKind::Death });
    }
    // 不满足同向 → "普通交叉"，不发出买卖信号
}
```

**代码影响**：`src/engine/ma/alignment.rs:119-139`

---

### R-P0-03：修复葛南维 B4 方向错误（L4 = 均线**下行** + 暴跌反弹买入）

**PRD 现状**：B4 乖离买入 — 方向未明确说明。

**代码 bug**：`@/Users/shiyizhidahui/Desktop/交易/aura_trade_rust/src/engine/ma/granville.rs:152-154` 要求 `s > slope_eps`（均线上行）— **完全反了**。

**原书依据**（ma p.83, 第四大法则）：
> "均线**下行**，股价/指数在均线**之下**运行，随后突然暴跌，距离均线**很远**，极有可能向均线靠近，可以进场买入。"

**建议修订**：
```rust
// B4：均线下行 + 股价深度负乖离 → 超跌反弹轻仓买入
if s.is_finite() && s < -p.slope_eps && b < -p.bias_thresh && c < m {
    out.push(GranvilleSignal { index: i, rule: B4DivergenceBuy });
}
// S4 同理：均线上行 + 深度正乖离 + 价格高于均线 → 超涨回落
if s.is_finite() && s > p.slope_eps && b > p.bias_thresh && c > m {
    out.push(GranvilleSignal { index: i, rule: S4DivergenceSell });
}
```

**额外 PRD 新增约束**：
- B4 触发时仓位必须 ≤ 30%（原书"逆势反弹"风险大）
- B4 信号必须设置止损，不可持仓至下降趋势延续

---

### R-P0-04：修复葛南维 B2 ≠ B3

**代码 bug**：`@/Users/shiyizhidahui/Desktop/交易/aura_trade_rust/src/engine/ma/granville.rs:124` 允许前根跌破均线 0.1%（`c_prev <= m_prev * 1.001`），这是 **L3** 条件不是 L2。

**原书依据**（ma p.84-85）：
- L2：均线上行，股价在均线之上，**回调时未跌破均线**
- L3：股价在均线之上，**回调时跌破均线**，但均线继续上行，股价很快重回

**建议修订**：
```rust
// B2 回踩买入：前根不得跌破均线
if s > p.slope_eps && c > m 
   && c_prev >= m_prev * 0.999 && c_prev < m_prev * 1.02  // 触及但未破
   && c >= c_prev {
    out.push(GranvilleSignal { rule: B2PullbackBuy, ... });
}
// B3 假跌破买入：前 N 根内有跌破
if s > p.slope_eps && c > m && (lo..i).any(|k| closes[k] < ma[k]) {
    out.push(GranvilleSignal { rule: B3FalseBreakBuy, ... });
}
```

---

### R-P0-05：特殊形态重分类（17 项 → 16 项原书形态）

**PRD 现状 A6**：17 "特殊形态"混合章节（Ch3·3 排列 + Ch4·1 特殊）。

**原书结构**：Ch4·1 "特殊形态" **恰好 16 种**（见 FINDINGS 轮次 1 详表）。

**建议修订**：
```
A6：16 大特殊形态（严格对应原书 Ch4·1）
  1 加速上行  2 加速下行  3 战机起航  4 俯冲式下降
  5 气贯长虹  6 火烧连营  7 银山谷   8 金山谷
  9 死亡谷   10 骷髅头   11 阶梯式上升 12 鱼跃龙门
  13 旱地拔葱 14 绝命跳   15 金蜘蛛   16 毒蜘蛛

A5-排列：单独章节
  - 多头排列 / 空头排列
  - 上山爬坡 / 下山滑坡（均线温和斜率形态）
  - 逐浪上升 / 逐浪下降（股价浪形形态）
  - 均线粘合（Ch3·3·5）
  - 🆕 均线收敛/发散（Ch3·3·6，独立事件）
  - 🆕 均线服从/扭转（Ch3·3·7，独立事件）
  - 🆕 均线背离（Ch3·3·8，独立事件）
  - 🆕 均线修复（Ch3·3·9，独立事件）
```

**代码影响**：
- 删除 `special.rs` 中 `UphillClimb/DownhillSlide/WaveUp/WaveDown/RapidUp/RapidDown/BullArrangement/BearArrangement/MaBond/Mire/BullBearBoundary/CycleSwap` 12 项（迁移或删除）
- 新增 `src/engine/ma/` 下 4 个文件：`divergence.rs` / `repair.rs` / `obedience.rs` / `converge_diverge.rs`
- 新增 12 种特殊形态识别（目前代码只有约 3/16）

---

### R-P0-06：删除凭空新增的 5 项（非原书）

`special.rs` 中以下项**原书完全没有**：
- `RapidUp` / `RapidDown`（快速上升/下降）
- `Mire`（烂泥潭）
- `BullBearBoundary`（牛熊分界）
- `CycleSwap`（周期轮换）

**建议**：
- 删除，或明确在注释中标注为 **"AURA 工程扩展，非原书形态"**，并在输出信号中加前缀 `[AURA-EXT]`

---

### R-P0-07：补全 Ch3·3·6-9 四节（收敛发散/服从扭转/背离/修复）

详见 E4。这 4 节是原书**趋势反转识别的核心工具**。

**交付文件**：
```
src/engine/ma/converge_diverge.rs   // 收敛（spread 缩窄）/发散（spread 扩大）事件
src/engine/ma/obedience.rs           // 服从（短 = 长方向）/扭转（短改变方向带动长）
src/engine/ma/divergence.rs          // 背离（暴涨后短长反向交叉）
src/engine/ma/repair.rs              // 主动修复（急速回归均线）/被动修复（横盘等均线）
```

---

## 二、P1 建议改进（10 项）

### R-P1-01：A3 双线组合加入"定性线/定量线"语义

**现状**：PRD 仅罗列双线组合周期。

**建议修订**：
```
每个双线组合包含两条线：
- 定性线（较长周期）：判断趋势方向
- 定量线（较短周期）：触发买卖点

规则：信号必须 "定性线方向 + 定量线穿越" 同时满足
例：短期组合 = 5/10，定性线 = 10 日，定量线 = 5 日
```

### R-P1-02：A10 均线操纵拆分为 3 种模式 + 置信度

```
原 A10：操纵识别（一句话）

建议修订为：
A10-1 ManipulationAbsent    无控盘，K 线/均线杂乱
A10-2 StrongHolderRally     强庄拉升：均线平滑多头排列 + 小阴小阳
A10-3 DistributionSellOff   主力出货：多头排列→背离→死叉→粘合→空头排列

每种模式输出：
- confidence: u8 (0-100)
- 作为交易信号的全局调节系数
```

### R-P1-03：B1-S4 加入仓位约束表

```
L1/L2/L3 买入：牛市中 → 可满仓
L4 逆势反弹：必须轻仓（≤ 30%），快进快出
L5/L6/L7/L8 卖出：熊市中 → 坚决清仓
```

### R-P1-04：K 线非对称交易原则

```
买入信号（见底形态）→ 等待 2-3 根 K 线确认
卖出信号（见顶形态）→ 立即触发（不等确认）
理由：candle p.45 原书"见底可缓，见顶须急"
```

### R-P1-05：旗形识别器加前提条件

```
Flag 检测需同时满足：
- 整理前 N=10 根 K 线累计涨跌幅 > X%（"急速" 前情）
- 整理时间 10-240 日（原书 10 几天 ~ 几个月）
- 超过 240 日：视为不规则圆顶/圆底，不当旗形处理
- 突破有效性：价格超越旗形边线 ≥ 3%
```

### R-P1-06：扩散三角形 = 操纵洗盘信号

```
Broadening pattern 的置信度调节：
- 如果处在长期上升趋势中 → "过顶吸筹" 主力操纵特征 → 置信度 +20
- 常伴均线粘合 / 股价快速上涨前兆
```

### R-P1-07：新增 "通道稳定度" 字段

```rust
pub struct Channel {
    // 现有字段...
    pub touches: Vec<TouchEvent>,
    pub pierce_count: usize,           // 历史刺穿次数
    pub stability: f64,                // = 1 - (pierce / total_touches)
}
```

### R-P1-08：新增通道逆推（Ch5·4 独创）

见 FINDINGS 轮次 5 具体结构。

### R-P1-09：多图形共振检测

```
ChartPatternResonance：
- 检测同一时间窗口内同向图形（如 圆底 + 下降三角形破位 + 头肩底）
- 共振信号权重 = Σ 各图形权重 × 同向因子
```

### R-P1-11：K 线形态引入 **SignalLevel** 4 分级（轮次 7 新增）

**原书依据**（candle p.220）：
> "曙光初现和好友反攻一样，不是买入信号。"

**当前代码问题**：`BullishCounterAttack` / `PiercingLine` / `BearishCounterAttack` / `DarkCloudCover` 等形态的 `direction: i8` 都被简单标记为 ±1，等同于交易触发信号。**与原书不符**。

**建议修订**：
```rust
pub enum SignalLevel {
    BuyTrigger,    // 直接触发买入：旭日东升/多方尖兵/上涨两颗星/红三兵/穿头破脚(阳包阴)
    BuyWarning,    // 见底预警，需二次确认：好友反攻/曙光初现
    SellTrigger,   // 直接触发卖出：倾盆大雨/乌云盖顶/高开出逃/徐缓下降/黑三兵
    SellWarning,   // 见顶预警：升势停顿/升势受阻/淡友反攻
    Neutral,       // 中继或方向不明确
}

impl PatternKind {
    pub fn signal_level(&self) -> SignalLevel { /* ... */ }
}
```

**关键原书力度排序**：
- 见顶：倾盆大雨 > 乌云盖顶 > 淡友反攻
- 见底：旭日东升 > 曙光初现 > 好友反攻
- 红三兵：三个白色武士 > 普通红三兵
- 头肩底：颈线向上倾斜 > 水平 > 向下倾斜

---

### R-P1-10：代码命名双轨 label（邱书正名 + 西方术语）

```rust
impl PatternKind {
    pub fn label(&self) -> &'static str { /* 现有中文 */ }
    pub fn qiu_name(&self) -> &'static str {
        match self {
            BullishEngulfing => "穿头破脚（阳包阴）",
            BullishHarami => "身怀六甲（阳）",
            PiercingLine => "曙光初现",
            DarkCloudCover => "乌云盖顶",
            // ...
        }
    }
}
```

---

## 三、P2 长期增强（4 项）

### R-P2-01：阴阳哲学层

引入全局"阴阳度"0-100 作为市场情绪调节器（见 E21）。

### R-P2-02：道氏三大假设说明

PRD 前言加入趋势书 Ch1·2·4 的"三大假设 = 客观事实"论述。

### R-P2-03：年线 = 240 日 / 前复权

数据层明确约定：前复权 + 年线 = 240 日。

### R-P2-04：补全 K 线书缺失形态（~40 种）

按 P0 → P1 优先级逐步实现：

**P0 缺失 10 种**：
- 向上/向下加速度线
- 倾盆大雨（更强版乌云盖顶）
- 连续跳空三阳线 / 三阴线
- 多方尖兵（仙人指路）/ 空方尖兵
- 升势/降势鹤鸦缺口
- 揉搓线 / 尽头线
- 下跌三连阴
- 双飞乌鸦

**P1 缺失 ~15 种**：
- 冉冉上升 / 稳步上涨 / 徐缓上升 / 上升抵抗 / 下降弧形线 / 升势受阻 / 升势停顿 / 绵绵阴跌 / 下跌不止 / 倒三阳 等"行情节奏"类

---

## 四、摘要统计

| 级别 | 数量 | 覆盖范围 |
|---|---|---|
| P0 必修 | 7 项 | 葛南维 + 交叉 + 特殊形态 + Ch3·3·6-9 |
| P1 建议 | 10 项 | 双线语义 / 操纵 / 通道 / 共振 / 命名 |
| P2 长期 | 4 项 | 哲学层 / 假设 / 数据约定 / K 线补全 |
| **合计** | **21 项** | — |

---

## 五、执行建议

1. **先修 P0**（估 5-8 工作日），重跑全部 `tests/` 回归
2. 验证 E3/E5/E17/E18 修复后 `PATTERN_EFFECTIVENESS_REPORT.md` 胜率是否改善
3. **P1 分批上线**，每批验证 backtest KPI
4. P2 按需处理，不影响主链路

**当前风险**：
- E18（B4 方向错）是**直接发错买入信号**的 bug，可能导致真实回测收益被**高估**（因为 B4 在上升中触发本来就大概率盈利，与原书"逆势抄底"语义完全不同）
- 建议**立即**修复 E18 后重新评估 PATTERN_EFFECTIVENESS
