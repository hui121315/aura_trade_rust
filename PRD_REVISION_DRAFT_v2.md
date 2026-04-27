# AURA_TRADE PRD 修订建议稿 v2

**基于**：`BOOK_REVIEW_FINDINGS.md`（轮次 1-18 累计发现 E1-E31 + R-P1/P2 共 ~45 项）
**前置**：`PRD_REVISION_DRAFT.md`（v1，覆盖 E1-E23 + R-P0-01~07）
**性质**：v1 的**增量 + 重排**，不重复 v1 内容
**当前进度**：~21% 原书精读（均线 85% / 趋势 100% ✅ / K 线 52%）
**起草日期**：Iteration 2（轮次 18 后）

---

## 文档结构

| 章节 | 内容 |
|---|---|
| 一 | 新增错误清单（E24-E31，8 项） |
| 二 | 新增 PRD 建议（R-P1-12 ~ R-P1-36 + R-P2-01，~25 项） |
| 三 | **核心决策矩阵**：原书 10 条多级趋势线策略矩阵 |
| 四 | 重新排序的 sprint 路线图 |
| 五 | v1 已有 R-P0 的执行状态盘点 |

---

## 一、新增错误清单（E24-E31）

### E24：左侧/右侧交易区分缺失（P2）

- **现象**：`backtest::evaluate` 单一胜率 KPI 评估
- **原书**（trend p.180）：左侧（高风险）vs 右侧（高确定性）应**分别评估**
- **建议**：增加 `EvaluationProfile { LeftSide, RightSide }`

---

### E25：trend/lines.rs 缺乏"修正趋势线"机制（P1）

- **现象**：`Line::fit` 只能拟合单一线，不维护多条修正
- **原书**（trend p.196）：下降趋势线可不断修正（1→2→3→...→6 条）
- **建议**：见 R-P1-23

---

### E26-E28：多级趋势线（短/中/长）的策略矩阵全部缺失（P1）

详见**第三章决策矩阵**。

---

### E29：长期趋势线未支持对数坐标系（P1）

- **现象**：`trend/lines.rs:66` 用线性价差 `(b.price - a.price) / (b.index - a.index)`
- **原书**（trend p.188, p.193）：**对数坐标系中所有当日涨跌幅相等的 K 线长度都一样**
- **建议**：
  ```rust
  pub enum CoordinateSystem { Linear, Logarithmic }
  
  impl Line {
      pub fn fit(a: SwingPoint, b: SwingPoint, coord: CoordinateSystem) -> Self {
          let (pa, pb) = match coord {
              CoordinateSystem::Linear => (a.price, b.price),
              CoordinateSystem::Logarithmic => (a.price.ln(), b.price.ln()),
          };
          // ... fit on (pa, pb)
      }
  }
  ```
- **影响范围**：长期趋势线（≥ 60 日）和量度幅度计算**必须**用对数

---

### E30：支撑压力线未实现"角色翻转"（P1）

- **现象**：`trend/sr.rs` 单向作为支撑或压力，无翻转逻辑
- **原书**（trend p.238, p.263）：**有效突破后，压力变支撑；有效跌破后，支撑变压力**
- **建议**：
  ```rust
  pub struct SrLine {
      pub price: f64,
      pub current_role: SrRole,        // Support / Resistance
      pub role_history: Vec<SrFlip>,   // 翻转历史
      pub last_flip_index: Option<usize>,
  }
  
  pub fn check_flip(line: &mut SrLine, kline: &Kline, idx: usize, threshold: f64) {
      // 突破超过 threshold (默认 3%) 触发翻转
  }
  ```

---

### E31：趋势线连接点策略不明 + 实体穿越未检测（P1）

- **原书**（trend p.158）：连接点可用收盘价（道氏正统）或最高/最低（实战），但**不可穿越 K 线实体**
- **建议**：
  ```rust
  pub enum ConnectPoint { Close, Extreme, Hybrid }
  pub fn validate_no_body_pierce(line: &TrendLine, klines: &[Kline]) -> bool;
  ```

---

## 二、新增 PRD 建议（R-P1-12 ~ R-P1-36 + R-P2-01）

### 信号识别类（候选模块新增）

| ID | 模块 | 内容 |
|---|---|---|
| R-P1-12 | `chart::round_top` | 圆顶识别（与圆底对称）|
| R-P1-13 | `ma::granville` | 各法则**绑定仓位上限**（L4 ≤ 30%）|
| R-P1-14 | `resonance` | **经典共振清单**（L3+好友反攻 / L1+晨星 / L2+锤头 等）|
| R-P1-17 | `chart::traps` | **多头陷阱 BullTrap** 识别器 |
| R-P1-19 | `ma::granville` | L8 = 诱多陷阱均线版（与 BullTrap 映射）|
| R-P1-20 | `chart::measure` | 形态量度幅度在**对数坐标系**计算 |
| R-P1-21 | `candle::patterns` | K 线**复合形态检测**（同段多形态叠加）|
| R-P1-23 | `trend::lines` | **TrendLineSeries** 多条修正线维护 |
| R-P1-24 | `chart::form` | **MultiFormIdentity** 多合一识别（趋势线+通道+旗形）|
| R-P1-27 | `chart::head_shoulders` | 复合头肩顶**分组合并**算法 |
| R-P1-28 | `chart::round_bottom` | 圆底颈线**自动选择 5 候选** + 缩量突破识别 |
| R-P1-30 | `chart::traps` | **StealthBreakout** 主力潜伏式突破识别 |
| R-P1-33 | `ma::alignment` | **周线空头排列**检测（杀伤力 > 日线）|
| R-P1-34 | `ma::convergence` | **均线收敛/发散**检测器（区分粘合 vs 交叉）|
| R-P1-36 | `chart::bottom_group` | 底部三种形态**互通映射**（V/淡友/岛形）|

### 策略/决策类

| ID | 模块 | 内容 |
|---|---|---|
| R-P1-15 | `strategy::trend_matrix` | **多级趋势线策略矩阵**（10 条买卖原则）—— **本 v2 重点** |
| R-P1-16 | `strategy::resonance_sell` | 多维共振 SellTrigger（K线+均线+图形）|
| R-P1-18 | `strategy::b4_gate` | **B4 信号需共振条件过滤**（L4 原书警告）|
| R-P1-22 | `strategy::breakout` | **3% 阈值 + HH/HL 连续判定** |
| R-P1-25 | `ma::slope` | 均线**斜率角度字段**（影响信号置信度）|
| R-P1-26 | `strategy::trend_close` | 多修正趋势线"任意跌破即清仓" |
| R-P1-29 | `strategy::critical_levels` | **120/240 日均线**作为关键压力/支撑位 |
| R-P1-31 | `strategy::channel_piercing` | 通道**穿头破脚**容忍度 |
| R-P1-32 | `strategy::progressive_exit` | 顶部多 K 线**逐渐减仓策略** |
| R-P1-35 | `strategy::weekly_resonance` | 周线乌云密布**多级共振清仓信号链** |
| R-P2-01 | `strategy::dual_ma` | **双线组合**（5 日定量 + 30 日定性）|

---

## 三、🌟🌟🌟 **核心决策矩阵：多级趋势线策略**（trend p.216 原书原版）

### 买入/加仓原则（**长期上升趋势线之上**）

| # | 情境 | 动作 | Rust 标识 |
|---|---|---|---|
| 1 | 突破长期下降趋势线，回落受**中期上升**趋势线支撑 | **Buy** | `LongDownBreak_MidUpSupport` |
| 2 | 长期上升之上，向上突破**中期下降**趋势线 | **BuyOrAdd** | `LongUp_BreakMidDown` |
| 3 | 长期上升之上，急跌后突破**短期下降**趋势线 | **BuyOrAdd** | `LongUp_QuickDip_BreakShortDown` |
| 4 | 长期上升之上，遇**长期上升**支撑止跌回升 | **BuyOrAdd** | `LongUp_MeetLongUpSupport` |
| 5 | 长期上升之上，遇**中期上升**支撑止跌回升 | **BuyOrAdd** | `LongUp_MeetMidUpSupport` |

### 卖出/空仓原则

| # | 情境 | 动作 | Rust 标识 |
|---|---|---|---|
| 1 | **跌破长期上升**趋势线 | **Close** | `LongUpBreakdown` |
| 2 | 长期上升之上，跌破**中期上升**趋势线 | **Reduce** | `LongUp_BreakMidUp` |
| 3 | 长期上升之上，急速飙升后跌破**短期上升** | **Reduce** | `LongUp_QuickRally_BreakShortUp` |
| 4 | 突破长期下降，回落跌破**中期上升** | **ReduceOrClose** | `LongDownBreak_MidUpFail` |
| 5 | 运行在**长期下降**趋势线**之下** | **StayOut** | `BelowLongDown` |

### Rust 实现骨架

```rust
pub enum TrendLevel { Long, Mid, Short }
pub enum TrendDirection { Up, Down, None }

pub struct MultiTimeframeTrendState {
    pub long: TrendDirection,
    pub mid: TrendDirection,
    pub short: TrendDirection,
}

pub enum TrendEvent {
    Breakout { level: TrendLevel, dir: TrendDirection },
    Breakdown { level: TrendLevel, dir: TrendDirection },
    Support { level: TrendLevel },
    Resistance { level: TrendLevel },
}

pub enum EntryAction {
    Buy,                // 普通买入
    BuyOrAdd,           // 可买可加（已持仓加仓）
    ReduceOrHold,       // 减仓或持股
    ReduceOrClose,      // 减仓或清仓
    Close,              // 清仓
    StayOut,            // 空仓观望
}

/// 根据原书 10 条策略矩阵映射决策
pub fn decide_action(
    state: &MultiTimeframeTrendState,
    event: TrendEvent,
) -> EntryAction;
```

### 原书警句（编码注释引用）

```rust
// trend p.221:
// "跌破长期上升趋势线 → 清仓卖出 ——
//  即便跌破之后趋势并未逆转，清仓依然是明智之举。
//  利润减少并不会有损失，而风险加大却能让交易者遭受灭顶之灾。"
```

---

## 四、重排 sprint 路线图

基于 v2 收集的 31 错误 + 45 建议，建议如下排序：

### Sprint 1（已完成 ✅）
- ✅ E5、E17、E18 P0 修复（Patch 1/2/3）
- ✅ `BacktestConfig.base_period = 60`（Patch 4）

### Sprint 2（**v2 推荐 1-2 周**）

| 工作 | 对应错误/建议 | 工作量 |
|---|---|---|
| Patch 5：`special.rs` 重分类（13 形态归位三类）| E10/E11/E12/E15/E16 | 4h |
| 实现 `MultiTimeframeTrendState` + `decide_action` | R-P1-15（**核心决策矩阵**）| 16h |
| 趋势线对数坐标 + 实体穿越检测 | E29 + E31 | 6h |
| 支撑压力角色翻转 + 通道穿头破脚 | E30 + R-P1-31 | 8h |

### Sprint 3（v2 推荐 2-3 周）

| 工作 | 对应错误/建议 | 工作量 |
|---|---|---|
| `TrendLineSeries` 多条修正线维护 | R-P1-23 + R-P1-26 | 12h |
| **共振模块全面升级**（4 信号共振：K线+均线+图形+成交量）| R-P1-14 + R-P1-16 + R-P1-35 | 16h |
| **B4 共振门控**（解决 L4 原书警告）| R-P1-18 + R-P1-13 | 8h |
| 多头陷阱 + 主力潜伏突破识别 | R-P1-17 + R-P1-30 | 12h |

### Sprint 4（v2 推荐 3-4 周）

| 工作 | 对应错误/建议 | 工作量 |
|---|---|---|
| 复合头肩顶 + 圆底自动颈线 + 多形态共存 | R-P1-21 + R-P1-24 + R-P1-27 + R-P1-28 | 20h |
| 周线分析支持 + 周线空头排列 | R-P1-33 | 8h |
| 均线收敛/发散检测器 | R-P1-34 | 8h |
| 形态量度幅度（对数坐标系）| R-P1-20 | 8h |
| 顶部逐渐减仓策略 + 双线组合策略 | R-P1-32 + R-P2-01 | 12h |

### Sprint 5+（v2 推荐 4 周后）

- 左侧/右侧分类评估（E24）
- 黄金共振案例**回归测试**（中国联合 600843 / 维维股份 600300 / 鑫茂科技 000836 / 亿安科技 000008）
- 整合所有信号到 **可视化操盘大屏**（web/）

---

## 五、v1 已有 R-P0 的执行状态盘点

| ID | v1 标题 | 状态 |
|---|---|---|
| R-P0-01 | MA20 → MA60 | ✅ 已修复（Patch 4） |
| R-P0-02 | find_crosses 同向条件 | ✅ 已修复（Patch 1） |
| R-P0-03 | B4 方向错误修复 | ✅ 已修复（Patch 3） |
| R-P0-04 | B2 触线条件修复 | ✅ 已修复（Patch 2） |
| R-P0-05 | special.rs 13 形态重分类 | ⏳ 待执行（Patch 5）|
| R-P0-06 | A5 / A6 信号定义清晰化 | ⏳ 待执行 |
| R-P0-07 | 葛南维测试用例数据更新 | ⏳ 待执行 |

**4/7 完成**（Sprint 1 阶段性结果）。

---

## 六、原书关键警句库（用于代码注释 + 单元测试断言）

下面 10 条原书警句应作为模块文档注释 + 测试 assertion message：

1. **"短期金叉死叉常常高买低卖"** —— ma p.81 → `cross.rs` doctests
2. **"L4 是逆势行为"** —— ma p.116 → `granville.rs::B4_*` doc
3. **"买进宜谨慎，卖出宜果断"** —— ma p.128 → `signal.rs::SignalKind::Sell` doc
4. **"未达 3% 价差，趋势线继续有效"** —— trend p.203 → `breakout.rs` 常量
5. **"跌破长期上升趋势线，清仓依然明智"** —— trend p.221 → `decide_action` doc
6. **"非牛市空仓"** —— trend p.225 → 全局空仓过滤器
7. **"多条相同周期趋势线，跌破任意一条即清仓"** —— trend p.213 → `TrendLineSeries::any_broken`
8. **"压力突破后变成支撑，前期压力越重，突破后支撑越强"** —— ma p.165 → `SrLine::flip` doc
9. **"周线乌云密布杀伤力极强，长中短线均应清仓"** —— ma p.304 → `weekly_resonance` doc
10. **"成交量不是判断头肩顶成立的必需依据"** —— candle p.465 → `head_shoulders_top.rs` doc

---

## 七、待补完事项（轮次 19+）

- ma Ch5-Ch6 综合实战（剩余 ~70 页）
- candle Ch7-Ch8 综合形态（剩余 ~50 页 + 600 页未读章节）
- candle Ch9 周线技术含义（重要）
- 整合 1505 页通读完整后，发布 **PRD_REVISION_DRAFT v3 = 最终冻结版**

---

## 八、回顾 v1 → v2 的进展度量

| 维度 | v1 | v2 | 增长 |
|---|---|---|---|
| 已发现错误 | 23 | 31 | +8 |
| PRD 建议 | 7 (R-P0) | 7 + 25 (R-P1) + 1 (R-P2) | +26 |
| 原书精读页 | ~70 | ~310 | +240 |
| 已修复 P0 | 0 | 4 | +4 |
| 已通读完成的书 | 0 | **1（趋势书）** | +1 ✅ |

**v2 核心交付物**：
1. 完整的"**多级趋势线策略矩阵**"（10 条买卖原则） —— 直接可编码
2. **45 项可追踪的 PRD 建议**（按模块分类）
3. **重排 sprint 路线图**（4-5 个 sprint 共 ~120 小时工作量预估）
4. **10 条原书警句库**（用于代码注释 + 测试断言）
