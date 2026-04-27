//! 全局配置
//!
//! PRD 附录 A 中所有可配置参数的默认值在此定义。
//! 后续 Phase 会扩展为可从环境变量 / 配置文件 / HTTP API 热更新。

use serde::{Deserialize, Serialize};

/// 全局运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// HTTP 监听地址
    pub http_bind: String,
    /// 静态前端资源目录（相对于二进制工作目录）
    pub web_root: String,
    /// 本地 K线缓存目录
    pub cache_dir: String,
    /// Binance REST API 基础 URL
    pub binance_base: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_bind: "127.0.0.1:3000".to_string(),
            web_root: "web".to_string(),
            cache_dir: "data_cache".to_string(),
            binance_base: "https://api.binance.com".to_string(),
        }
    }
}

impl Config {
    /// 从环境变量覆盖默认值（尚未使用的配置项保留默认值）
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(v) = std::env::var("AURA_HTTP_BIND") {
            cfg.http_bind = v;
        }
        if let Ok(v) = std::env::var("AURA_WEB_ROOT") {
            cfg.web_root = v;
        }
        if let Ok(v) = std::env::var("AURA_CACHE_DIR") {
            cfg.cache_dir = v;
        }
        if let Ok(v) = std::env::var("AURA_BINANCE_BASE") {
            cfg.binance_base = v;
        }
        cfg
    }
}
