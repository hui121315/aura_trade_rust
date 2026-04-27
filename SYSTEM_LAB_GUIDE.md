# Aura 体系实验室 · 完整指南

> **从零到量化交易研究平台**：18 个里程碑 (M1-M18) + 打磨，477 测试绿，4 个交互面板。
>
> 本文档是 Aura 体系实验室的**用户指南 + 架构总结**。设计文档见 `SYSTEM_LAB_DESIGN.md`。

---

## 目录

- [1. 项目定位](#1-项目定位)
- [2. 快速开始](#2-快速开始)
- [3. 4 个前端面板](#3-4-个前端面板)
- [4. 核心概念](#4-核心概念)
- [5. 完整 API 清单](#5-完整-api-清单)
- [6. 里程碑回顾（M1-M18）](#6-里程碑回顾m1-m18)
- [7. 数据驱动的关键发现](#7-数据驱动的关键发现)
- [8. 最佳实践工作流](#8-最佳实践工作流)
- [9. 技术栈与性能](#9-技术栈与性能)

---

## 1. 项目定位

Aura 体系实验室是一个**交易体系研发平台**，在已有 Aura-Trade 核心引擎之上提供：

- **组件化体系搭建**：把葛南维八法 / K 线形态 / 技术图形 / 均线特殊形态等 32 个经典技术信号，作为可组合的组件
- **四种聚合规则**：`AllAligned` / `MajorityK` / `WeightedScore` / `SequentialCascade`（级联状态机）
- **自动化挖掘**：贪心 Beam Search 自动发现 Top-K 高 Sharpe 组合
- **稳健性验证**：Walk-Forward OOS + 跨 symbol + 跨周期交叉验证
- **实时执行**：组件级实时扫描 + 浏览器通知

**核心原则**：
- ❌ 不做 Deep RL / 黑盒预测
- ❌ 不做单一参数优化
- ✅ 所有组件可溯源到经典技术分析文献
- ✅ 所有结果可解释（组件归因 + 分 fold 指标）

---

## 2. 快速开始

### 启动服务

```bash
cd aura_trade_rust
cargo run --release
# → http://127.0.0.1:3000
```

启动时后台自动完成：
1. 拉取 BTC/ETH/SOL × 1d × 2000 根 K 线（若本地缓存失效）
2. 为 8 个硬编码种子体系跑一次 walk-forward 基准
3. 结果写入内存，供 `/api/system/seeds` 即时返回

### 访问面板

| 面板 | URL | 用途 |
|---|---|---|
| 主界面 | `/` | 通用回测 / 学习 / 指标 |
| **体系实验室** | `/system.html` | 搭建 + Discovery + WF + 入库 |
| **基准热力图** | `/benchmark.html` | 全局 WF 矩阵热力图 |
| **实时信号面板** | `/trade.html` | Polling + 通知推送 |

所有面板顶栏互相可达。

---

## 3. 4 个前端面板

### 3.1 主界面（`/`）

保留原 Aura 的图表 / 回测 / 学习 tab。顶栏新增 3 个外链按钮跳到其他面板。

### 3.2 体系实验室（`/system.html`）

**最核心的研发面板**。分 5 步：

1. **1️⃣ 种子体系**：8 hardcoded + 所有 Promoted，按 benchmark Sharpe 排序（可切换）
2. **2️⃣ 组件选择**：32 个组件按维度分组，最多 5 个，带"清空"按钮
3. **3️⃣ 聚合规则**：4 种规则，带动态参数（K / 阈值 / 窗口）
4. **4️⃣ 市场/风控**：Symbol / Interval / 止损 ATR × / 目标 R / 最长持仓
5. **5️⃣ 🔍 Discovery**：
   - Direction（多头/空头）+ max_size + Top-K + WF 折数
   - 交叉验证 symbols + intervals（逗号分隔）
   - 🔍 发现当前方向 / ⚖️ 双向发现

**操作按钮**：
- 🚀 运行回测 → 单次完整回测（KPI + 归因 + 交易列表）
- 🧪 Walk-Forward → 折数 + 总 K 线数（滚动窗口验证）
- 📄 导出报告 → 一键生成 Markdown 研究报告

### 3.3 基准热力图（`/benchmark.html`）

输入 Symbols + Intervals + WF 折数，跑完整矩阵：

- **行** = 体系（按选中指标降序，Promoted 带 ⭐）
- **列** = `symbol × interval` 组合
- **单元格** = 红-灰-绿渐变色块
- **着色指标**：Sharpe / Return / Consistency（下拉即时切换）
- Hover 显示 per-cell 完整 WF 指标
- 点击 cell / 体系名跳转到 `/system.html`

### 3.4 实时信号面板（`/trade.html`）

- **配置**：体系 / Symbol / Interval / K 线数 / 刷新间隔（秒）
- **控制**：▶ 开始监听 / ⏸ 暂停 / 🔔 启用通知 / 🔊 声音开关
- **Hero 卡片**：最新价 + 24h 时间 + 大号信号徽章（绿/红/脉动动画）
- **K 线图**：lightweight-charts 深色主题，最近 100 根 + Marker（聚合信号金色箭头 / 组件触发小圆点）
- **组件触发状态网格**：每个组件的 ✓/○ + 整段触发次数
- **最近 10 bar + 所有聚合信号 bar**：时间倒序表格

**通知行为**：
- 新 bar 触发信号 → 桌面通知 + WebAudio 双音哔声（多头上扬 / 空头下沉）
- 聚合信号要求用户主动关闭（`requireInteraction`）
- 同 symbol+interval 使用相同 tag → 新通知自动覆盖旧的

---

## 4. 核心概念

### 4.1 SystemDefinition

```rust
pub struct SystemDefinition {
    pub id: String,
    pub name: String,
    pub origin: SystemOrigin,       // Seed / User / Discovered
    pub components: Vec<String>,    // 组件 ID 列表（≤ 5）
    pub combine: CombineRule,       // 聚合规则
    pub weights: HashMap<String, f64>,
    pub risk: RiskParams,           // ATR 止损 / 目标 R / 最长持仓
    pub backtest: BacktestParams,   // warmup / 成本模型
    pub meta: SystemMeta,           // 含 last_benchmark: Vec<BenchmarkSnapshot>
}
```

### 4.2 四种聚合规则

| 规则 | 行为 |
|---|---|
| `AllAligned` | 所有组件必须同向触发（最严格） |
| `MajorityK { k }` | 至少 k 个组件同向（民主多数派） |
| `WeightedScore { threshold }` | 加权得分 ≥ 阈值 |
| `SequentialCascade { window_bars }` | 组件**按顺序**在 N bar 内依次触发（级联状态机） |

### 4.3 Direction Bias

每个组件声明多/空偏好：
- `direction_bias: +1` 仅能产生多头信号（如 `ma_special.bull_arrangement`）
- `direction_bias: -1` 仅能产生空头信号（如 `candle.evening_star`）

Discovery 按方向筛选组件，避免 `AllAligned` 永不触发。

### 4.4 组件归因

回测结果包含 `component_contribution: Vec<ComponentContrib>`：
- `trigger_count`: 整段触发次数
- `traded_count`: 实际进入交易的次数
- 比率揭示哪个组件是信号"瓶颈"

---

## 5. 完整 API 清单

### 体系管理
- `GET  /api/system/components` — 32 个组件按维度分组
- `GET  /api/system/seeds` — Seeds + Promoted 合并，带 benchmark 数据
- `POST /api/system/run` — 单次完整回测
- `POST /api/system/walkforward` — 滚动窗口 WF
- `POST /api/system/discover` — Beam Search 自动发现 + 跨 symbol / 跨周期验证
- `POST /api/system/benchmark` — 多体系 × 多 symbol × 多 interval 矩阵
- `POST /api/system/live_scan` — 轻量实时扫描，返回最近 N bar 每组件触发

### 持久化
- `POST /api/system/promote` — 入库（自动跑 BTC/ETH/SOL × 1d benchmark 填充 meta）
- `POST /api/system/demote` — 按 id 移除已入库体系

### 通用
- `GET  /api/klines` — 拉 Binance K 线（60s TTL 缓存）
- `GET  /api/ping` — 健康检查

---

## 6. 里程碑回顾（M1-M18）

| M | 交付 |
|---|---|
| **M1** | 核心引擎：`Component` / `SystemDefinition` / `Combine` / `Runner`（断头铡刀硬覆盖） |
| **M2** | 32 组件（MA × 13 / Candle × 6 / Chart × 5 / Trend × 2 / MA-Special × 6） |
| **M3** | 8 种子体系（覆盖 4 种聚合规则 + 多/空双方向） |
| **M4** | HTTP API（3 基础端点） |
| **M5** | 前端：system.html 独立页面 |
| **M6** | Discovery 贪心 Beam Search + 跨 symbol 验证 |
| **M7** | Walk-Forward 滚动窗口（揭示"金山谷"仅 75% fold 盈利） |
| **M8** | rayon 并行化（Discovery 4.7× 提速）|
| **Vault** | Promoted 持久化到 `promoted_seeds.json` |
| **M9** | 基准热力图（48 cells × 4 folds = 22ms） |
| **M10** | Promoted 入库自动 benchmark → meta |
| **M11** | Discovery 支持跨周期验证（9 验证点 / 90ms） |
| **M12** | 双向 Discovery 并行展示 |
| **M13** | 种子列表排序下拉 |
| **M14** | Markdown 研究报告一键导出 |
| **M15** | `/trade.html` 实时信号面板 |
| **M16** | 实时面板 K 线图 + Marker |
| **M17** | 启动时自动为所有 Seed 跑 benchmark（内存） |
| **M18** | 浏览器通知 + WebAudio 声音推送 |
| **打磨** | 主界面顶栏跨面板导航 + 组件清空按钮 + 全局 Toast |

---

## 7. 数据驱动的关键发现

### 7.1 最佳种子体系（BTC/ETH/SOL × 1d × 4 folds）

| 排名 | 体系 | 平均 Sharpe | 平均 Consistency |
|---|---|---|---|
| 🥇 | **主升浪追踪** | **+0.49** | **83%** |
| 🥈 | 金山谷·蛟龙出海 | +0.40 | 75% |
| 🥉 | 形态终局（顶部反转） | +0.08 | 50% |
| 4 | 断头铡刀风控体系 | 0.00 | 0%（AllAligned 太苛刻） |
| 5 | 四维共振系统 | -0.22 | 8% |
| 6 | 道氏趋势系统 | -0.37 | 17% |
| 7 | 均线骨架系统 | -0.38 | 17% |
| 8 | K 线底部反转 | -0.52 | 25% |

### 7.2 Discovery 挖掘的"共识冠军"

`granville.b1_breakout + accelerating_up + bull_arrangement` [MK k=2]
- BTC 1d: **100% cons / +1.15 Sharpe**
- ETH 1d: 75% / +0.71
- SOL 1d: 75% / +0.36
- cross_composite = **+0.65**（比手动种子高 74%）

极简双核 `accelerating_up + bull_arrangement`（AllAligned）也能达 cross_composite +0.57，说明这两个组件是加密市场牛市的**核心引擎**。

### 7.3 反直觉发现

- **K 线反转形态在加密市场几乎全线溃败**（K 线底部反转 -0.52 Sharpe）
- **过去 2000 日加密市场整体偏牛**，空头体系最佳 composite 仅 +0.11
- **4h 周期比 1d 噪声大得多**：共识冠军在 ETH 4h 上 Sharpe 跌到 -1.40
- **断头铡刀风控**（旱地拔葱空头检测）在整段牛市零触发 → 0 笔交易

---

## 8. 最佳实践工作流

### 场景 A：发现新体系

1. 打开 `/system.html` → 左下角 "5️⃣ Discovery"
2. 配置：
   - Direction：多头
   - Max size：3
   - Top-K：10
   - WF 折数：4
   - 交叉验证 symbols：`ETHUSDT, SOLUSDT`
   - 交叉验证 intervals：`4h, 1w`（跨周期验证）
3. 点击 🔍 自动发现
4. 等待 ~500ms
5. 查看结果：`cross_consistency_mean ≥ 75% 且 cross_sharpe_mean ≥ 0.5` 的体系才值得入库
6. 点击 "⭐ 入库"，起名，自动跑 benchmark 填 meta
7. 下次启动仍在

### 场景 B：实时监听

1. 打开 `/trade.html`
2. 选中刚入库的冠军体系（带 ⭐ 图标）
3. Symbol: BTCUSDT, Interval: 4h, Refresh: 30s
4. 点击 🔔 启用通知 → 允许
5. 点击 ▶ 开始监听
6. 浏览器回后台也会收到 toast / 声音提示

### 场景 C：整体验证

1. 打开 `/benchmark.html`
2. Symbols: `BTCUSDT, ETHUSDT, SOLUSDT, BNBUSDT`
3. Intervals: `4h, 1d`
4. 🔥 运行基准矩阵
5. 查看热力图：深绿色单元格 = 该体系在该市场稳定
6. 切换着色指标（Sharpe / Return / Consistency）观察不同视角
7. 点击名称跳回 `/system.html` 深入研究

### 场景 D：导出研究报告

1. `/system.html` 顶栏点击 "📄 导出报告"
2. 等待 ~6s（拉 benchmark）
3. 浏览器下载 `aura-report-YYYY-MM-DDTHH-MM-SS.md`
4. 包含：概览 / 种子排行 / Promoted 详情 / 全局基准 / 最新 Discovery / 作战建议

---

## 9. 技术栈与性能

### 后端（Rust 纯后端，零异步依赖）
- `tiny_http` — HTTP 服务器
- `ureq` — Binance 同步客户端
- `serde` + `serde_json` — 序列化
- `rayon` — 数据并行（Discovery + WF + Benchmark 多层并行）
- **无** tokio / async-std / 深度学习库

### 前端（零打包，零框架）
- `lightweight-charts@4.2.0` — K 线图
- 手写 Vanilla JS + CSS（变量化深色主题）
- 每个页面独立文件，互不干扰

### 测试
```bash
cargo test --lib  # 477 passed
```

### 性能基线（M8 并行后）
| 操作 | 耗时 |
|---|---|
| 单次 `runner::run`（2000 根） | ~15ms |
| Walk-Forward（4 折） | ~30ms |
| Discovery（3 sym × 316 combos × 4 folds） | **114ms** |
| Benchmark 矩阵（8 体系 × 3 sym × 2 interval × 4 folds = 48 cells） | **22ms** |
| Live Scan（300 bar + 100 tail） | **35ms** |

### 测试数量与覆盖
- **单元测试 477 个**全绿
- 覆盖：组件扫描 / 聚合规则 / 回测主循环 / WF / Discovery / Vault / Benchmark

---

## 10. 项目结构

```
aura_trade_rust/
├── src/
│   ├── engine/
│   │   ├── system/
│   │   │   ├── component.rs        # 32 组件注册表
│   │   │   ├── definition.rs       # SystemDefinition + SystemMeta
│   │   │   ├── registry.rs         # 8 种子体系
│   │   │   ├── combine.rs          # 4 种聚合规则
│   │   │   ├── runner.rs           # 回测主循环 + 断头铡刀
│   │   │   ├── scan.rs             # 预扫描所有组件触发
│   │   │   ├── walkforward.rs      # 滚动窗口 WF
│   │   │   ├── discovery.rs        # Beam Search + cross validation
│   │   │   ├── benchmark.rs        # 矩阵基准
│   │   │   └── vault.rs            # Promoted 持久化
│   │   ├── ma/                     # 均线引擎（葛南维 / 特殊形态 / 高级）
│   │   ├── candle/                 # K 线形态
│   │   ├── chartpattern/           # 技术图形
│   │   ├── trend/                  # 道氏趋势
│   │   └── backtest/               # 基础回测（types / metrics / runner）
│   ├── server/
│   │   ├── server.rs               # HTTP 主循环 + 后台 seed_benchmark 线程
│   │   ├── routes.rs               # 路由分发
│   │   ├── system_routes.rs        # 7 个体系 API handler
│   │   └── response.rs             # JSON envelope
│   └── data/                       # KlineCache + Binance client
├── web/
│   ├── index.html / app.js / style.css    # 主界面
│   ├── system.html / system.js / system.css    # 体系实验室
│   ├── benchmark.html / benchmark.js / benchmark.css    # 基准热力图
│   ├── trade.html / trade.js / trade.css    # 实时信号面板
│   └── toast.js                    # 全局 toast 组件
├── Cargo.toml
├── SYSTEM_LAB_DESIGN.md            # 早期设计文档
└── SYSTEM_LAB_GUIDE.md             # 本文档
```

---

_本指南由 Aura 体系实验室自动生成。最后更新：M18 + 打磨后._
