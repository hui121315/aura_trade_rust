# Aura-Trade (灵气交易系统)

基于邱立波《趋势交易技术》三本典籍（均线 / 趋势 / K线）理论体系的 **交易决策辅助 + 历史回测** 工作台。

- **产品定位**：基于网页的决策辅助工具，**不自动下单**
- **后端**：纯 Rust（`tiny_http` + `ureq`）
- **前端**：原生 HTML/CSS/JS + [Lightweight Charts](https://www.tradingview.com/lightweight-charts/)
- **理论根基**：四维共振（均线 × 趋势 × K线 × 技术图形）
- **详细设计**：见 [`../AURA_TRADE_PRD.md`](../AURA_TRADE_PRD.md)

## 快速启动

```bash
cargo run --release
```

然后浏览器访问 `http://127.0.0.1:3000`。

## 项目结构

```
aura_trade_rust/
├── Cargo.toml
├── src/
│   ├── main.rs              # 二进制入口
│   ├── lib.rs               # 库根
│   ├── config.rs            # 全局配置
│   ├── server/              # HTTP 服务 (tiny_http)
│   ├── data/                # Binance 数据采集 + 本地缓存
│   └── engine/              # 四维共振计算引擎
│       ├── ma/              # 模块 A：均线引擎
│       ├── trend/           # 模块 B：趋势引擎
│       ├── candle/          # 模块 C：K线引擎
│       ├── chart/           # 模块 D：技术图形引擎
│       └── backtest/        # 模块 E：回测引擎
└── web/                     # 前端工作台 (静态文件)
    ├── index.html
    ├── style.css
    └── app.js
```

## 开发进度（按 PRD 路线图 6 Phase）

- [x] Phase 0：项目骨架
- [ ] Phase 1：均线 + K线基础 + Web 骨架
- [ ] Phase 2：回测引擎 MVP
- [ ] Phase 3：趋势引擎 + 回测验证
- [ ] Phase 4：K线形态全量 + 技术图形
- [ ] Phase 5：均线 17 大特殊形态 + 四维共振 + 建议计算器
- [ ] Phase 6：Walk-Forward / 参数敏感性 / WebSocket

## 设计原则

- **只读数据源**：仅使用 Binance 公开 REST/WebSocket 行情，不接入任何交易 API
- **纯 Rust 后端**：所有计算逻辑用 Rust 实现，保证性能与可审计性
- **回测-实盘一致**：同一套引擎既用于实时分析也用于回测，消除一致性偏差
- **术语忠于原书**：采用"葛南维"、"银山谷"、"蛟龙出海"等书中原始术语
