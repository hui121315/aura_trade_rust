# Aura-Trade

Aura-Trade 是一个基于 Rust 的交易研究与决策辅助工作台，围绕“均线、趋势、K 线形态、技术图形”四个维度构建可解释的技术分析、共振评分、历史回测与体系实验能力。

> **重要声明**：本项目仅用于技术分析研究、策略复盘与教学演示，不提供投资建议，不自动下单，也不保证任何收益。交易存在风险，请独立判断。

## 功能概览

- **四维共振评分**：将均线、趋势、K 线形态、技术图形汇总为可解释的综合评分与交易建议。
- **指标与形态引擎**：覆盖 MA/EMA、MACD、RSI、ATR、葛南维规则、均线特殊形态、K 线组合、技术图形等模块。
- **历史回测**：支持基础信号、Playbook、体系组合与风控参数回测。
- **体系实验室**：把经典技术信号抽象为组件，支持组合、Walk-Forward、Discovery 与基准热力图。
- **实时信号面板**：基于公开行情数据进行轮询分析、组件触发展示与浏览器通知。
- **纯本地运行**：后端为 Rust，前端为静态 HTML/CSS/JS，不需要数据库或私有交易所 API Key。

## 技术栈

| 层级 | 技术 |
|---|---|
| 后端 | Rust 2021, `tiny_http`, `ureq`, `serde`, `rayon` |
| 前端 | 原生 HTML/CSS/JavaScript, Lightweight Charts |
| 数据 | 公开行情 REST，本地 K 线缓存 |
| 测试 | Rust unit/integration tests |

## 快速开始

### 环境要求

- Rust stable
- macOS / Linux / Windows 均可运行 Rust 后端
- 浏览器访问本地 Web UI

### 启动服务

```bash
cargo run --release
```

默认监听：

```text
http://127.0.0.1:3000
```

### 常用环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `AURA_HTTP_BIND` | `127.0.0.1:3000` | HTTP 监听地址 |
| `AURA_WEB_ROOT` | `web` | 静态前端目录 |
| `AURA_CACHE_DIR` | `data_cache` | 本地 K 线缓存目录 |
| `AURA_BINANCE_BASE` | `https://api.binance.com` | Binance REST 基础地址 |
| `AURA_LOG` | `info` | 日志级别 |

示例：

```bash
AURA_HTTP_BIND=127.0.0.1:8080 cargo run --release
```

## Web 面板

| 页面 | 地址 | 用途 |
|---|---|---|
| 主工作台 | `/` | 图表、指标、共振、回测与学习入口 |
| 体系实验室 | `/system.html` | 组件化体系搭建、Discovery、Walk-Forward |
| 基准热力图 | `/benchmark.html` | 多体系、多市场、多周期的基准矩阵 |
| 实时信号面板 | `/trade.html` | 实时轮询、信号展示、浏览器通知 |
| 警报页 | `/alerts.html` | 指标警报配置与展示 |
| 知识页 | `/knowledge.html` | 技术分析知识与说明页面 |

## 项目结构

```text
aura_trade_rust/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs                 # 二进制入口
│   ├── lib.rs                  # 库入口
│   ├── config.rs               # 运行时配置
│   ├── data/                   # 行情适配器与本地缓存
│   ├── server/                 # HTTP 路由、响应与静态文件服务
│   └── engine/                 # 核心计算引擎
│       ├── indicator.rs        # 通用指标
│       ├── ma/                 # 均线、葛南维、特殊形态
│       ├── trend/              # 道氏趋势、摆动点、趋势线、支撑压力、缺口
│       ├── candle/             # K 线形态与组合
│       ├── chartpattern/       # 技术图形识别
│       ├── resonance/          # 四维共振评分与建议
│       ├── signal/             # 高级信号与路由
│       ├── backtest/           # 回测引擎
│       ├── system/             # 体系实验室
│       └── rl/                 # Bandit 评估与持久化
├── tests/                      # 集成测试
├── examples/                   # 研究/验证示例
├── web/                        # 静态前端
└── docs/                       # 公开文档
```

## 文档

- [架构说明](docs/ARCHITECTURE.md)
- [HTTP API 速览](docs/API.md)
- [开发指南](docs/DEVELOPMENT.md)
- [体系实验室完整指南](SYSTEM_LAB_GUIDE.md)
- [项目实施总结](PROJECT_IMPLEMENTATION_SUMMARY.md)

## 测试

运行全部测试：

```bash
cargo test --workspace --all-targets --no-fail-fast
```

运行共振评分相关测试：

```bash
cargo test -p aura_trade engine::resonance::score::tests --lib --no-fail-fast
```

格式检查：

```bash
cargo fmt --all -- --check
```

## 设计原则

- **不自动交易**：仅输出分析、评分、回测与建议，不连接私有交易 API。
- **可解释优先**：评分、信号和建议尽量保留贡献项与原因。
- **回测一致性**：实时分析与回测共享核心引擎，减少前后不一致。
- **避免未来函数**：信号与回测按时间顺序消费数据，避免使用未来 K 线。
- **可审计实现**：核心逻辑集中在 Rust 模块中，并通过测试覆盖边界行为。

## 许可证

本项目使用 [MIT License](LICENSE)。
