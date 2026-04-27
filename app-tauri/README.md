# Aura Trade · macOS 桌面壳（Tauri v2）

把 `aura_trade` 后端 + `web/` 前端打包成一个独立的 macOS App / DMG。

> 产物：`Aura Trade.app` 约 20–30 MB，双击即用，无需终端、无需浏览器。

## 架构

```
┌───────────────────────────────────────────┐
│              Aura Trade.app               │
│                                           │
│   ┌─────────────┐     ┌──────────────┐    │
│   │   WKWebView │ ──▶ │ 127.0.0.1:PORT│   │
│   └─────────────┘     └──────┬───────┘    │
│                              │             │
│                    ┌─────────▼─────────┐  │
│                    │  tiny_http 服务   │  │
│                    │ (aura_trade crate)│  │
│                    └───────────────────┘  │
└───────────────────────────────────────────┘
```

- 启动时自动选择一个空闲端口（避免占用 3000）
- `web_root` → Bundle Resources 内嵌的 `web/`
- `cache_dir` → `~/Library/Application Support/com.aura.trade/cache/`
- WebView 指向 `http://127.0.0.1:<port>/`，前端零修改

## 首次准备（一次性）

### 1. 安装 Tauri CLI

```bash
cargo install tauri-cli --version "^2.0"
```

### 2. 生成占位图标（或替换为自己的 logo）

```bash
# 从系统 Stocks App 拷贝占位图标
bash app-tauri/scripts/gen_placeholder_icons.sh

# 或者使用自己的 1024x1024 PNG 生成全套
cargo tauri icon path/to/your-logo.png --output app-tauri/icons
```

## 开发模式（热重载后端 + DevTools）

```bash
cd app-tauri && cargo tauri dev
```

窗口打开后按 **Cmd+Option+I** 打开 WebKit DevTools。Rust 代码改动会自动 rebuild。

## 生产打包（推荐：一键脚本）

```bash
bash app-tauri/scripts/build_release.sh
```

自动完成：
1. `cargo tauri build --bundles app` 生成 `.app`
2. `codesign` ad-hoc 签名（无需证书，本机免警告）
3. `hdiutil` 用**已签名的 .app** 重打 DMG
4. `xattr -cr` 清理 quarantine 属性
5. 把 `.app` 和 `.dmg` 拷贝到 `~/Desktop/`

### 使用正式开发者证书签名

```bash
export AURA_SIGN_ID="Developer ID Application: Your Name (TEAMID)"
bash app-tauri/scripts/build_release.sh
```

产物位置（workspace 共享 `target/`，在**项目根**下）：

```
target/release/bundle/
├── macos/Aura Trade.app               ← 双击即用（~9 MB，已 ad-hoc 签名）
└── dmg/Aura Trade 0.1.0.dmg           ← 分发用 DMG（~4 MB）
~/Desktop/
├── Aura Trade.app                     ← 复本（快捷使用）
└── Aura Trade 0.1.0.dmg               ← 复本（分发）
```

### 手动打包（不需要签名）

```bash
cd app-tauri && cargo tauri build
```

产物在 `target/release/bundle/macos/Aura Trade.app` 和 `target/release/bundle/dmg/*.dmg`，未签名、不会自动拷贝到桌面。

> **本项目仅构建 Apple Silicon (aarch64)**。如需 Intel 通用二进制，加 `--target universal-apple-darwin`，但本 repo 的目标机型不含 Intel。

## 数据目录

| 用途 | 路径 |
|------|------|
| K 线缓存 / bandit 状态 / vault seeds | `~/Library/Application Support/com.aura.trade/cache/` |
| 日志 | stderr（`Console.app` 中按进程名 `Aura Trade` 过滤） |

> **环境变量 `AURA_CACHE_DIR`** 仍生效：开发时可以指向仓库内的 `data_cache/` 复用历史缓存。

## 常见问题

### 启动时白屏

首次启动时后端要加载 bandit warm-up（约 20–40 秒），期间 API 已可用，前端会一直请求直到就绪。观察 `Console.app`。

### 端口冲突

代码会自动选择空闲端口，不依赖 3000。如果还是失败，检查防火墙是否阻止了本机 TCP 连接。

### 代码签名 / 公证

默认未签名，Gatekeeper 会拦截。开发自用可右键→打开绕过；分发需：

```bash
# 配置签名身份
# 在 tauri.conf.json 的 bundle.macOS.signingIdentity 填入开发者证书名
# 然后 cargo tauri build --bundles dmg
```

公证（notarization）需要 Apple Developer 账号与 `xcrun notarytool`，超出本 README 范围。

## 目录结构

```
app-tauri/
├── Cargo.toml              # 依赖 tauri v2 + 主 crate
├── tauri.conf.json         # Bundle 配置
├── build.rs                # tauri-build 入口
├── src/main.rs             # 桌面壳：起后端 + 创建 WebView
├── capabilities/default.json   # Tauri v2 权限声明
├── icons/                  # App 图标（gen_placeholder_icons.sh 生成）
└── scripts/
    └── gen_placeholder_icons.sh
```

## 卸载清理

```bash
# 删除 App
rm -rf "/Applications/Aura Trade.app"

# 清除数据目录
rm -rf "$HOME/Library/Application Support/com.aura.trade"
```
