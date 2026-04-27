//! Aura-Trade 核心库
//!
//! 四维共振（均线 × 趋势 × K线 × 技术图形）交易决策辅助引擎。
//!
//! 模块组织严格对齐 PRD：
//! - [`data`]      数据采集层（Binance REST + 本地缓存）
//! - [`engine`]    计算引擎层（A/B/C/D/E 五大模块）
//! - [`server`]    HTTP 服务层（`tiny_http`）
//! - [`config`]    全局配置

pub mod config;
pub mod data;
pub mod engine;
pub mod logger;
pub mod server;
