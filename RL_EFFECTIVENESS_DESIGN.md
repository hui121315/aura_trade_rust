# 指标有效性自动评估 — 技术设计文档（TDD）

> **作者**：Aura-Trade 工程组
> **版本**：v0.1（2026-04-17）
> **状态**：📐 设计中，未开始实施
> **关联代码**：`src/engine/signal/replay.rs`、`src/engine/backtest/playbook.rs`、`src/server/routes.rs`

---

## 1. 目标

用最少代码让系统具备"**自动识别当前市场哪些指标/形态/策略最靠谱**"的能力，并给出可解释的证据。

### 1.1 产品目标
| # | 目标 |
|---|---|
| G1 | 给用户一张"指标有效性排行榜"，按**胜率 × 平均 R × 样本数**综合排序 |
| G2 | 系统能根据历史表现**自动调整 Playbook 权重**，让高胜率指标获得更多话语权 |
| G3 | 决策理由能附带"这个指标最近 N 次命中率 X%"的证据 |
| G4 | 对新指标/新形态**只需实现一次**，评估与权重更新自动完成 |

### 1.2 非目标（明确不做）

- ❌ Deep RL（DQN/PPO/SAC）：样本效率低、可解释性差、不符合"3 本铁证书"的设计哲学
- ❌ 预测行情走向：本系统做的是**信号质量评估**，不是行情预测
- ❌ 替代 Playbook 铁律：Bandit 只在**多个 Playbook 都不冲突**时决定采样顺序，不推翻"断头铡刀清仓"这种硬规则

---

## 2. 核心算法选型

### 2.1 候选算法对比

| 算法 | 核心思想 | 优点 | 缺点 | 适用场景 |
|---|---|---|---|---|
| **Epsilon-Greedy** | 按 ε 概率随机探索，1-ε 选当前最优 | 最简单 | exploration 机械，不随置信度自适应 | baseline |
| **UCB1** | 置信上界 `mean + c·sqrt(ln(N)/n)` | 确定性，无随机性，适合理论分析 | 参数 c 难调，长尾回报拟合差 | 稳定环境 |
| **Thompson Sampling (TS)** | 对每个 arm 维护 Beta(α, β) 后验，每步采样 θ，选 argmax θ | 自然 explore-exploit 平衡；Bayesian；可解释 | 需要 Beta 近似二元奖励 | ⭐ 推荐 |
| **Contextual Bandit (LinUCB)** | 引入上下文特征（市场 regime、vol、RSI 区间等） | 根据市场状态动态切换最优 arm | 需要特征工程，复杂度上升一个量级 | v2 演进方向 |

### 2.2 推荐：Thompson Sampling

**动机**：
- 本项目所有指标/形态的结果可归约为二元（命中/未命中）→ 完美适配 Beta-Bernoulli 共轭
- 不需要调 hyperparameter，冷启动鲁棒
- 与 `CompositePlaybook` 按优先级排序的现有架构天然兼容：把"按固定 priority 顺序"改为"按 Thompson 采样概率"即可
- 可解释：每个 arm 的 `α/(α+β)` = 胜率估计，`α+β` = 样本数；可直接可视化

**决策规则**：
```
for each 被触发的 arm:
    θ_i ~ Beta(α_i, β_i)       // Bayesian 采样一个胜率估计
选择 argmax(θ_i) 作为当前执行的 Playbook
```

---

## 3. 数据结构设计

### 3.1 Arm 定义

**Arm = (信号类型, 参数指纹)**，按粒度由粗到细：

| 层级 | 示例 Arm 名 | 粒度 | 说明 |
|---|---|---|---|
| L1 策略层 | `playbook.guillotine` | 粗 | 与现有 `CompositePlaybook` 内的 Playbook 一一对应 |
| L2 信号层 | `signal.ma_guillotine` / `signal.macd_golden_cross` / `signal.stoch_rsi_oversold` | 中 | 单个信号事件 |
| L3 形态层 | `pattern.head_and_shoulders_top` / `pattern.bullish_engulfing` | 中 | K 线形态 |
| L4 复合层 | `confluence.ma+trend+candle` | 细 | 多维共振 |

**v1（A 阶段）**：实现 L1+L2+L3 的离线统计。
**v2（B 阶段）**：增加 L4，且所有 arm 由 Bandit 在线更新。

### 3.2 Rust 结构体

```rust
// src/engine/rl/types.rs

use serde::{Deserialize, Serialize};

/// 单个 arm 的 Beta-Bernoulli 后验 + 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmState {
    pub name: String,              // 唯一 id，如 "signal.ma_guillotine"
    pub alpha: f64,                // 成功计数 + 1（先验 1）
    pub beta: f64,                 // 失败计数 + 1（先验 1）
    pub total_triggers: u64,       // 历史总触发次数（含未结算）
    pub total_wins: u64,
    pub total_losses: u64,
    pub cumulative_r: f64,         // 累计 R-multiple（盈亏比）
    pub max_r: f64,
    pub min_r: f64,
    pub last_updated_ms: i64,      // 最后一次结算时间戳
    pub category: ArmCategory,     // L1/L2/L3/L4
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ArmCategory {
    Playbook,
    Signal,
    Pattern,
    Confluence,
}

impl ArmState {
    /// 后验均值 = 胜率估计
    pub fn mean(&self) -> f64 { self.alpha / (self.alpha + self.beta) }

    /// 后验方差（不确定度）
    pub fn variance(&self) -> f64 {
        let s = self.alpha + self.beta;
        (self.alpha * self.beta) / (s * s * (s + 1.0))
    }

    /// Thompson 采样一个 θ
    pub fn sample_theta(&self, rng: &mut impl rand::Rng) -> f64 {
        // Beta(α, β) 采样 — 实现见 §4.1
        beta_sample(self.alpha, self.beta, rng)
    }

    /// UCB1 上界（备选）
    pub fn ucb1(&self, total_n: u64, c: f64) -> f64 {
        let n = (self.total_triggers as f64).max(1.0);
        self.mean() + c * ((total_n as f64).ln() / n).sqrt()
    }
}

/// 所有 arms 的全局状态（持久化根）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BanditState {
    pub version: u32,
    pub arms: std::collections::HashMap<String, ArmState>,
    pub pending: Vec<PendingEvaluation>, // 已触发未结算的信号
}

/// 已触发但尚未到 horizon 的信号，等待结算
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingEvaluation {
    pub arm_name: String,
    pub symbol: String,
    pub interval: String,
    pub triggered_at_ms: i64,     // 触发时的 bar open_time
    pub trigger_price: f64,
    pub direction: i8,             // +1 多，-1 空，0 中性
    pub stop: Option<f64>,         // 若有 SL，则按 R-multiple 结算
    pub target: Option<f64>,
    pub horizon_bars: usize,       // 几根 K 线后强制结算（如 10）
}
```

### 3.3 持久化格式

- 位置：`data_cache/bandit_state.v1.json`
- 格式：JSON，`BanditState` 直接序列化
- 备份：每次写入先写 `.tmp` 再 rename（避免中途崩溃导致文件破损）
- 版本演进：`version` 字段控制，低版本不兼容时按默认先验重置

---

## 4. 算法细节

### 4.1 Beta 采样（无外部依赖）

Rust 社区有 `rand_distr::Beta`，但项目目前**零 rand 依赖**。用最轻量算法：

```rust
/// Beta(α, β) 采样 = Gamma(α) / (Gamma(α) + Gamma(β))
fn beta_sample(alpha: f64, beta: f64, rng: &mut impl Rng) -> f64 {
    let x = gamma_sample(alpha, rng);
    let y = gamma_sample(beta, rng);
    if x + y < 1e-12 { 0.5 } else { x / (x + y) }
}

/// Gamma 采样 — Marsaglia & Tsang 方法（对 α ≥ 1 的情形）
fn gamma_sample(alpha: f64, rng: &mut impl Rng) -> f64 {
    // 参考 "A Simple Method for Generating Gamma Variables" (2000)
    // 完整实现 ~30 行，省略
}
```

**依赖选项**：
- 方案 A：引入 `rand` + `rand_distr`（0 运行时开销，~20KB）
- 方案 B：自实现 Beta 采样（纯 Rust，零依赖）

**推荐 A**：符合社区标准，省去测试成本。

### 4.2 Reward 函数设计（最关键）

每次 `PendingEvaluation` 到 horizon 后结算一个奖励，分两种口径：

#### 4.2.1 二元奖励（给 Beta 后验用）
```rust
reward_binary = {
    if direction × (price_after - trigger_price) > 0      → 1 (胜)
    else                                                  → 0 (负)
}
// 或更严格：必须涨/跌 > cost_threshold（默认 10 bps 手续费 + 滑点）
```

#### 4.2.2 连续 R-multiple（给胜率 × 平均 R 排名用）
```rust
reward_r = {
    if stop is Some:
        realized_pnl / (trigger_price - stop).abs()      // 标准 R-multiple
    else:
        (price_after - trigger_price) / (atr × 1.5)       // 用 ATR 归一化
}
```

**决策原则**：
- **Bandit 用二元**：避免极端收益扭曲 Beta 先验
- **排行榜用 R-multiple**：给用户看"这个指标平均 1.8R"比"胜率 62%"更具指导性

### 4.3 冷启动与先验

每个新 arm 默认 `Beta(1, 1)` → uniform 先验 → 第一次触发就开始学习。

**保护措施**：
- 前 `MIN_SAMPLES = 20` 次触发前，Bandit 不生效，回退到现有 `CompositePlaybook` 固定优先级
- 防止单次异常收益（e.g., 1000R）主导后验 → R 截断到 `[-5R, +10R]`

### 4.4 遗忘因子（应对市场 regime shift）

市场环境会变（牛市/熊市切换），过去的统计不再代表未来。两个选择：

| 方案 | 实现 | 优点 | 缺点 |
|---|---|---|---|
| **α/β 指数衰减** | 每 N 天 `α ← α·γ, β ← β·γ` (γ=0.98) | 简单，算力小 | 所有 arm 同速遗忘 |
| **滑窗记忆** | 只保留最近 M 次触发记录 | 严格滑窗 | 存储成本大 |

**推荐**：指数衰减，每周 cron 执行一次，γ=0.95（半衰期约 13 周）。

---

## 5. 模块划分

```
src/engine/rl/
├── mod.rs               # 对外公共 API
├── types.rs             # ArmState / BanditState / PendingEvaluation
├── bandit.rs            # Thompson Sampling / UCB 选择逻辑
├── evaluator.rs         # 触发记录 → horizon 结算 → 更新 arm
├── persistence.rs       # JSON 读写 + 原子替换
├── decay.rs             # 遗忘因子任务（可选，v2 加）
└── tests.rs
```

**对外接口**：
```rust
pub struct BanditEngine { /* ... */ }

impl BanditEngine {
    pub fn load_or_default(path: &Path) -> Self;
    pub fn save(&self, path: &Path) -> std::io::Result<()>;

    /// 每根新 K 线调用：根据当前已触发 arms 采样并返回推荐执行的 arm
    pub fn decide(&mut self, triggered_arms: &[&str]) -> Option<String>;

    /// 信号触发时登记
    pub fn register_trigger(&mut self, eval: PendingEvaluation);

    /// 每根新 K 线调用：结算到期的 pending
    pub fn settle_expired(&mut self, current_bar_ms: i64, closes: &[f64]);

    /// 查询所有 arms 的当前状态（给 /api/effectiveness 用）
    pub fn arms_snapshot(&self) -> Vec<ArmState>;
}
```

---

## 6. 伪代码：主循环

```python
# 伪代码（Rust 实现见 §5）

on_new_bar(bar_i, ctx):
    # Step 1: 结算到期的 pending
    bandit.settle_expired(bar_i.open_time_ms, ctx.closes)

    # Step 2: 收集当前所有被触发的 arms
    triggered = []
    for playbook in composite.playbooks:
        if playbook.is_triggered(ctx):
            triggered.append(f"playbook.{playbook.name()}")
    for signal in signals.scan(ctx):
        triggered.append(f"signal.{signal.kind}")

    # Step 3: 若有触发，让 Bandit 决定执行哪个
    if len(triggered) > 1:
        chosen = bandit.decide(triggered)
    elif len(triggered) == 1:
        chosen = triggered[0]
    else:
        return  # 无信号

    # Step 4: 登记为 pending，等待 horizon 结算
    bandit.register_trigger(PendingEvaluation {
        arm_name: chosen,
        triggered_at_ms: bar_i.open_time_ms,
        trigger_price: bar_i.close,
        direction: infer_direction(chosen),
        stop: ctx.suggested_stop,
        target: ctx.suggested_target,
        horizon_bars: 10,
    })

    # Step 5: 执行对应 playbook 的 decide()
    return playbooks[chosen].decide(ctx)
```

---

## 7. API 设计

### 7.1 GET /api/effectiveness  （A 阶段：离线统计）

```
GET /api/effectiveness?symbol=BTCUSDT&interval=1d&limit=2000&horizon=10

Response:
{
  ok: true,
  data: {
    total_signals_scanned: 348,
    horizon: 10,
    rankings: [
      {
        arm: "signal.ma_guillotine",
        category: "Signal",
        n: 42,
        win_rate: 0.71,
        avg_r: 1.82,
        sharpe: 1.45,
        max_r: 4.2,
        min_r: -1.8,
        alpha_vs_market: 0.085,
        effectiveness_score: 75.3  // 综合评分：sqrt(n) × win_rate × avg_r
      },
      // ... 按 effectiveness_score 降序
    ]
  }
}
```

### 7.2 GET /api/bandit/state  （B 阶段：在线状态）

```
GET /api/bandit/state

Response:
{
  ok: true,
  data: {
    version: 1,
    last_updated_ms: 1697527680000,
    total_arms: 18,
    pending_count: 3,
    arms: [
      {
        name: "signal.ma_guillotine",
        category: "Signal",
        alpha: 31.0,            // 30 胜 + 1 先验
        beta: 13.0,             // 12 负 + 1 先验
        mean: 0.705,            // 后验胜率估计
        variance: 0.0046,
        total_triggers: 42,
        cumulative_r: 76.4,
        avg_r: 1.82,
      },
      // ...
    ]
  }
}
```

---

## 8. 前端设计

### 8.1 A 阶段：指标有效性排行榜面板

位置：`tab=realtime` 右侧面板底部，或单独 tab "📊 有效性分析"

```
┌─────────────────────────────────────────────────┐
│  📊 指标有效性排行榜（近 2000 根 K 线）         │
├─────────────────────────────────────────────────┤
│  #  指标名              样本  胜率   平均R  α    │
│  1  断头铡刀清仓       [42]  71%   1.82   +8.5% │
│  2  旱地拔葱轻仓入场   [28]  68%   1.45   +6.2% │
│  3  毒蜘蛛死叉         [35]  65%   1.23   +4.8% │
│  4  MACD 金叉          [156] 58%   0.72   +1.4% │
│  5  StochRSI 超卖      [62]  54%   0.51   +0.9% │
│  ...                                             │
│  [下载 CSV] [按周期筛选 ▾] [样本数下限 ▾]       │
└─────────────────────────────────────────────────┘
```

### 8.2 B 阶段：Bandit 实时面板

新增"🎰 Bandit 在线学习"区域：
- 每个 arm 显示 Beta(α, β) 形状小图
- 高亮本根 K 线被采样中标的 arm
- 支持手动"封禁"某 arm（管理员功能，应对真实世界大跌时暂停某策略）

---

## 9. 风险控制 & 边界情况

| 风险 | 控制措施 |
|---|---|
| **冷启动**：新 arm 样本少就被选中，输了被"打入冷宫" | `min_samples=20` 前走默认优先级 |
| **过拟合**：旧市场数据主导 Beta | 指数衰减 γ=0.95 / 周 |
| **Survivorship bias**：只回测上币的策略 | 加入 delisted symbol 列表（future） |
| **Look-ahead bias** | 严格 `index + horizon < closes.len()` 检查，已有 `replay.rs` 保证 |
| **极端奖励爆破后验** | R 截断 `[-5R, +10R]` |
| **币对切换** | Arm 按 `symbol+interval` 分别维护，或全局共享（此处选全局：跨 symbol 学习一般性规律） |
| **并发写入 bandit_state.json** | 单写线程；或 file lock；此处服务是单线程 tiny_http，暂无并发问题 |

---

## 10. 分阶段 Roadmap

### Sprint A（约 1 天）— 过渡方案，先拿数据

- [ ] **A1** 新增 `src/engine/effectiveness.rs`（复用 `replay.rs`）
  - 扫描 `candle_patterns` + `ma::scan_advanced` + `signal::scan_*` 的全部触发点
  - 对每个触发点调用 `HistoricalReplay::evaluate_signal` 得到 R-multiple
  - 按 `arm_name` 聚合成 `ReplayStats` + 综合评分
- [ ] **A2** 新增 API `/api/effectiveness`
- [ ] **A3** 前端"指标有效性排行榜"面板（复用 `big-table` 样式）
- [ ] **A4** 覆盖测试：已知数据下的胜率/Sharpe 正确

### Sprint B（约 3 天）— Bandit 在线学习

- [ ] **B1** 引入 `rand = "0.8"` + `rand_distr = "0.4"`
- [ ] **B2** 实现 `src/engine/rl/types.rs` + `bandit.rs`
- [ ] **B3** 实现 `evaluator.rs`（触发登记 + horizon 结算）
- [ ] **B4** 实现 `persistence.rs` + 单测（崩溃后恢复）
- [ ] **B5** 集成到 `CompositePlaybook`：增加 `ThompsonSamplingComposite` 变体
- [ ] **B6** API `/api/bandit/state`
- [ ] **B7** 前端 Bandit 面板
- [ ] **B8** 端到端：跑 500 根历史回放，观察 α/β 是否收敛到合理值

---

## 11. 决策点：现在需要澄清的问题

1. **Arm 粒度从哪层开始？** → 建议从 L1+L2 两层（Playbook + Signal），不含 L3 形态（形态数量多、噪声大）
2. **是否共享跨 symbol 学习？** → 建议**共享**：指标的底层物理含义是共通的，样本越多越稳
3. **Horizon 固定还是自适应？** → v1 固定（10 根），v2 可按 timeframe 差异化（1d=5, 4h=10, 1h=20）
4. **是否暴露给用户手动调节 α/β？** → v1 不暴露，v2 加管理员模式
5. **依赖 `rand` 是否可接受？** → 建议接受（行业标准，~20KB）；否则自实现 Gamma 采样 ~50 行

---

## 附录 A：综合评分公式推导

排行榜排序公式（避免小样本 arm 凭少数运气爬到顶部）：

```
effectiveness_score = sqrt(n) × (win_rate - 0.5) × avg_r × 10
                     ─────────  ──────────────  ───────
                     样本权重    纯运气校正      收益放大器
```

- **sqrt(n)**：样本越多越可信，但边际递减
- **win_rate - 0.5**：抛硬币为 0 分，只有真正跑赢随机才有分
- **× avg_r**：win_rate 高但 avg_r 低（例如胜率 90% 但平均 0.1R）不如 win_rate 低但 avg_r 高
- **× 10**：数值归一化到 0-100 区间便于阅读

---

## 附录 B：Thompson Sampling 的 Bayesian 直觉

为什么 TS 比 ε-greedy 好？

```
场景：arm_A = 10胜 10负（α=11, β=11，mean=0.5）
      arm_B = 2 胜 0 负（α=3,  β=1,  mean=0.75）

ε-greedy：arm_B mean 更高 → 总是选 arm_B → 后果严重（只 2 个样本就锁定）

Thompson：
  θ_A ~ Beta(11,11) → 通常在 [0.35, 0.65]
  θ_B ~ Beta(3, 1)  → 通常在 [0.25, 0.98]（方差大，不确定）

  有 ~40% 概率 θ_A > θ_B → 继续给 arm_A 机会
  有 ~60% 概率 θ_B > θ_A → 继续验证 arm_B

  → 自然做到 explore-exploit 平衡，无需调参。
```

**这就是 Bandit 比 Deep RL 更适合本项目的根本原因**：  
我们的信号空间小（~20 个 arms）、样本有限（每天 10-100 个触发）、需要快速从少样本学习。Bayesian 方法是量身定做的。

---

## 附录 C：不引入依赖的最小实现（备选）

若要严格零外部 crate，Gamma 采样用 Marsaglia & Tsang 方法 Pure Rust：

```rust
use std::time::{SystemTime, UNIX_EPOCH};

/// 简易 PRNG（xoshiro256**，纯 Rust，≤30 行）
pub struct Xoshiro { state: [u64; 4] }

impl Xoshiro {
    pub fn seed_from_time() -> Self { /* ... */ }
    pub fn next_u64(&mut self) -> u64 { /* ... */ }
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Box-Muller 生成 N(0,1)
    pub fn next_normal(&mut self) -> f64 { /* ... */ }
}

/// Marsaglia & Tsang Gamma 采样（α ≥ 1）
pub fn gamma_sample(alpha: f64, rng: &mut Xoshiro) -> f64 {
    assert!(alpha >= 1.0);
    let d = alpha - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    loop {
        let x = rng.next_normal();
        let v = (1.0 + c * x).powi(3);
        if v <= 0.0 { continue; }
        let u = rng.next_f64();
        if u < 1.0 - 0.0331 * x.powi(4) { return d * v; }
        if u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) { return d * v; }
    }
}
```

约 80 行 Rust 可以搞定所有采样，**完全保持项目零外部 stat 依赖**的纯度。

---

> **下一步**：确认本 TDD 方向后，按 §10 Roadmap 推进 Sprint A，预期 1 天上线 `/api/effectiveness`。
