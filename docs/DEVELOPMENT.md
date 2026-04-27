# 开发指南

本文档面向希望本地运行、测试或继续开发 Aura-Trade 的开发者。

## 本地准备

安装 Rust stable：

```bash
rustup update stable
```

克隆项目后进入仓库：

```bash
git clone https://github.com/hui121315/aura_trade_rust.git
cd aura_trade_rust
```

启动服务：

```bash
cargo run --release
```

浏览器访问：

```text
http://127.0.0.1:3000
```

## 常用命令

### 开发模式运行

```bash
cargo run
```

### Release 模式运行

```bash
cargo run --release
```

### 全量测试

```bash
cargo test --workspace --all-targets --no-fail-fast
```

### 指定模块测试

```bash
cargo test -p aura_trade engine::resonance::score::tests --lib --no-fail-fast
```

### 格式化

```bash
cargo fmt --all
```

### 格式检查

```bash
cargo fmt --all -- --check
```

### 前端 JS 语法检查

```bash
node --check web/app.js
node --check web/system.js
node --check web/trade.js
```

## 配置

运行时配置来自 `Config::from_env()`。

| 环境变量 | 用途 |
|---|---|
| `AURA_HTTP_BIND` | 修改监听地址 |
| `AURA_WEB_ROOT` | 修改静态文件目录 |
| `AURA_CACHE_DIR` | 修改缓存目录 |
| `AURA_BINANCE_BASE` | 修改 Binance REST 地址 |
| `AURA_LOG` | 修改日志级别 |

示例：

```bash
AURA_LOG=debug AURA_HTTP_BIND=127.0.0.1:8080 cargo run
```

## Git 忽略规则

默认不提交以下内容：

- `target/`
- `app-tauri/target/`
- `app-tauri/gen/`
- `data_cache/`
- `backtest_results/`
- IDE 与系统临时文件

`Cargo.lock` 应提交，以保证应用项目依赖版本可复现。

## 开发约定

### 后端

- 核心计算逻辑优先写在 `src/engine/`。
- HTTP 层只负责解析参数、组装输入、返回响应。
- 新指标或新信号应补充边界测试。
- 回测逻辑应避免未来函数和同根 K 线乐观成交。

### 前端

- 当前前端不使用构建工具。
- 修改 JS 后建议运行 `node --check`。
- 动态字段优先使用 `textContent` 或 DOM 节点赋值，减少 HTML 注入风险。
- 图表展示逻辑尽量与后端响应字段保持一一对应。

### API

- 新 API 建议返回统一 `{ ok, data, error }` envelope。
- GET 适合只读分析；POST 适合复杂 JSON 请求体。
- 参数解析失败应返回清晰错误信息。

## 测试重点

建议优先覆盖以下类型：

- 指标边界：空数组、长度不足、平盘、极端波动
- 形态识别：方向、强度、索引边界
- 共振评分：权重、归一化、未来事件过滤
- 回测执行：入场/出场价格、手续费、滑点、止损止盈
- API 一致性：输入参数是否影响输出字段

## 公开发布检查清单

公开仓库前建议检查：

- 没有 `.env`、私钥、token、交易所 API Key
- 没有大体积二进制或缓存文件
- `cargo test --workspace --all-targets --no-fail-fast` 通过
- README 能说明项目定位、启动方式、风险声明
- 仓库可见性符合预期

## 常见问题

### 启动后页面打不开

确认服务监听地址：

```bash
AURA_LOG=debug cargo run
```

默认地址是：

```text
http://127.0.0.1:3000
```

### 行情接口失败

可能原因：

- 网络无法访问交易所公开 REST
- 交易所限流
- symbol 或 interval 参数不合法
- 本地缓存目录无写权限

### 测试很快但页面数据慢

测试多为纯计算单元测试；页面数据需要拉取公开行情并进行多模块扫描，首次访问可能会创建缓存。
