# 架构说明

本文档说明 Aura-Trade 的主要架构、模块边界和数据流，方便公开仓库读者快速理解项目。

## 总体架构

Aura-Trade 是一个本地运行的 Rust 后端 + 静态前端项目。

```text
公开行情 REST
    ↓
src/data/               K 线获取、本地缓存、交易所适配
    ↓
src/engine/             指标、形态、信号、共振评分、回测、体系实验
    ↓
src/server/             HTTP API 与静态文件服务
    ↓
web/                    浏览器工作台
```

项目不连接私有交易 API，不执行自动下单。所有功能围绕研究、复盘、回测和辅助决策展开。

## 后端分层

### `src/config.rs`

负责运行时配置，支持通过环境变量覆盖默认值。

| 配置 | 用途 |
|---|---|
| `http_bind` | HTTP 监听地址 |
| `web_root` | 静态 Web 目录 |
| `cache_dir` | 本地 K 线缓存目录 |
| `binance_base` | Binance REST 基础地址 |

### `src/data/`

行情数据层，负责：

- 拉取公开 K 线数据
- 统一 `Kline` 数据结构
- 支持本地缓存，降低重复请求
- 为不同交易所适配器预留模块边界

### `src/engine/`

核心计算层，尽量保持纯函数式输入输出，方便测试和审计。

| 模块 | 说明 |
|---|---|
| `indicator.rs` | 通用指标，如 MA、EMA、MACD、RSI、ATR 等 |
| `ma/` | 均线状态、排列、葛南维、特殊均线形态 |
| `trend/` | 摆动点、道氏趋势、趋势线、支撑压力、缺口、通道 |
| `candle/` | K 线形态、组合形态、多周期聚合 |
| `chartpattern/` | 技术图形识别与强度评估 |
| `resonance/` | 四维共振评分与交易建议计算 |
| `signal/` | 高级信号、信号路由、回放、陷阱、潜伏突破等 |
| `backtest/` | 回测执行、绩效统计、Playbook 与仓位约束 |
| `system/` | 体系实验室：组件、组合规则、Discovery、Walk-Forward |
| `rl/` | Bandit 状态、评估与持久化 |

## 四维共振数据流

四维共振是主工作台中的核心解释层。

```text
Kline[]
  ├─ ma::compute_ma_state()          → A 均线维度
  ├─ trend::compute_trend_state()    → B 趋势维度
  ├─ candle::scan()                  → C K 线形态维度
  └─ chartpattern::detect_all()      → D 技术图形维度
          ↓
resonance::compute_resonance()
          ↓
resonance::compute_suggestion()
          ↓
/api/resonance → web/app.js → 共振卡片
```

评分输出包含：

- `total`：综合分数，范围约为 `[-100, 100]`
- `stance` / `stance_label`：多空姿态
- `alignment`：非零维度方向一致性
- `dimensions`：A/B/C/D 每个维度的分数、权重与贡献项
- `suggestion`：方向、信心、止损止盈、风险金额、理由

## 回测原则

回测模块的核心目标是避免乐观偏差。

- 信号在当前 bar 形成后，交易执行使用后续可交易价格。
- 成本模型包含手续费和滑点参数。
- 绩效统计输出收益、回撤、Sharpe、胜率等指标。
- 体系实验室的 Walk-Forward 用于检查样本外稳定性。

## 前端结构

`web/` 目录包含多个静态页面。

| 文件 | 用途 |
|---|---|
| `index.html` / `app.js` / `style.css` | 主工作台 |
| `system.html` / `system.js` / `system.css` | 体系实验室 |
| `benchmark.html` / `benchmark.js` / `benchmark.css` | 基准热力图 |
| `trade.html` / `trade.js` / `trade.css` | 实时信号面板 |
| `alerts.html` / `alerts.js` / `alerts.css` | 警报页 |
| `knowledge.html` / `knowledge.js` / `knowledge.css` | 知识页 |

前端不使用构建工具，便于直接由 Rust 后端静态托管。

## 持久化与缓存

当前项目主要使用本地文件缓存：

- `data_cache/`：K 线缓存、Bandit 状态、Promoted 体系等本地运行数据
- `backtest_results/`：本地回测结果输出

这些目录默认不提交到 Git。

## 安全边界

- 不存储交易所私钥
- 不读取私有账户资产
- 不调用交易下单 API
- 不把本地缓存作为公共数据源提交
- 公开仓库前应再次检查 `.env`、密钥文件和大文件
