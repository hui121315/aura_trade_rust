# HTTP API 速览

Aura-Trade 使用 `tiny_http` 提供本地 HTTP API。所有业务接口通常返回统一 envelope：

```json
{
  "ok": true,
  "data": {},
  "error": null
}
```

启动服务后，默认访问：

```text
http://127.0.0.1:3000
```

## 通用查询参数

多数行情相关 GET 接口支持以下参数：

| 参数 | 示例 | 说明 |
|---|---|---|
| `symbol` | `BTCUSDT` | 交易对 |
| `interval` | `1d` / `4h` / `1h` | K 线周期 |
| `limit` | `1000` | K 线数量 |

## 系统接口

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/api/ping` | 健康检查 |
| `GET` | `/api/version` | 版本信息 |
| `GET` | `/api/symbols` | 交易对列表 |

示例：

```bash
curl 'http://127.0.0.1:3000/api/ping'
```

## 行情与基础分析

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/api/klines` | K 线数据 |
| `GET` | `/api/ma_state` | 均线状态、排列、BIAS、交叉等 |
| `GET` | `/api/candle_patterns` | K 线形态扫描 |
| `GET` | `/api/trend_state` | 趋势状态、道氏阶段、通道、缺口等 |
| `GET` | `/api/chart_patterns` | 技术图形识别 |
| `GET` | `/api/indicators/series` | 指标序列 |

示例：

```bash
curl 'http://127.0.0.1:3000/api/klines?symbol=BTCUSDT&interval=1d&limit=200'
```

## 四维共振

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/api/resonance` | 四维共振评分与交易建议 |

常用参数：

| 参数 | 默认含义 | 说明 |
|---|---|---|
| `w_ma` | 均线维度权重 | A 维度权重 |
| `w_trend` | 趋势维度权重 | B 维度权重 |
| `w_candle` | K 线形态维度权重 | C 维度权重 |
| `w_chart` | 技术图形维度权重 | D 维度权重 |
| `equity` | 账户权益 | 建议计算器使用 |
| `max_risk` | 单笔最大风险比例 | 如 `0.02` 表示 2% |
| `rr` | 盈亏比目标 | 如 `2` 表示 1:2 |
| `atr_mult` | ATR 止损倍数 | 用于止损距离 |

示例：

```bash
curl 'http://127.0.0.1:3000/api/resonance?symbol=BTCUSDT&interval=1d&limit=500&w_ma=0.3&w_trend=0.3&w_candle=0.2&w_chart=0.2&equity=10000&max_risk=0.02&rr=2&atr_mult=1.5'
```

核心响应字段：

| 字段 | 说明 |
|---|---|
| `score.total` | 综合分数 |
| `score.stance` | 姿态枚举 |
| `score.stance_label` | 中文姿态标签 |
| `score.alignment` | 维度方向一致性 |
| `score.dimensions` | A/B/C/D 维度分数、权重、贡献项 |
| `suggestion.direction` | 建议方向：`1` 多、`-1` 空、`0` 观望 |
| `suggestion.confidence` | 信心系数 |
| `suggestion.entry_price` | 参考入场价 |
| `suggestion.stop_loss` | 参考止损价 |
| `suggestion.take_profit` | 参考止盈价 |
| `suggestion.rationale` | 解释文本 |

## 高级信号与决策

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/api/signals` | 高级信号聚合 |
| `GET` | `/api/decision` | 决策聚合输出 |
| `GET` | `/api/effectiveness` | 信号有效性统计 |

## Backtest

| Method | Path | 说明 |
|---|---|---|
| `GET` / `POST` | `/api/backtest/run` | 基础回测 |
| `GET` / `POST` | `/api/backtest/playbook` | Playbook 回测 |

GET 示例：

```bash
curl 'http://127.0.0.1:3000/api/backtest/run?symbol=BTCUSDT&interval=1d&limit=1000'
```

## Bandit / 学习状态

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/api/bandit/state` | 当前 Bandit 状态 |
| `GET` / `POST` | `/api/bandit/train` | 训练 / 更新 |
| `POST` | `/api/bandit/reset` | 重置状态 |
| `GET` | `/api/bandit/decide` | 获取 Bandit 决策 |

## 体系实验室

| Method | Path | 说明 |
|---|---|---|
| `GET` | `/api/system/components` | 列出全部组件 |
| `GET` | `/api/system/seeds` | 列出种子体系和 promoted 体系 |
| `POST` | `/api/system/run` | 运行单个体系回测 |
| `POST` | `/api/system/walkforward` | Walk-Forward 验证 |
| `POST` | `/api/system/discover` | 自动发现组合 |
| `POST` | `/api/system/benchmark` | 跑基准矩阵 |
| `POST` | `/api/system/live_scan` | 实时扫描 |
| `POST` | `/api/system/promote` | 晋升体系 |
| `POST` | `/api/system/demote` | 移除 promoted 体系 |

## 静态页面

除 API 外，服务也托管 `web/` 静态文件：

| URL | 页面 |
|---|---|
| `/` | 主工作台 |
| `/system.html` | 体系实验室 |
| `/benchmark.html` | 基准热力图 |
| `/trade.html` | 实时信号面板 |
| `/alerts.html` | 警报页 |
| `/knowledge.html` | 知识页 |
