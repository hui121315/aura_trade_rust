//! K线本地文件缓存 + 多交易所路由
//!
//! 策略：
//! - 每个 `(exchange, symbol, timeframe)` 一个 JSON 文件，存全量 K线
//! - TTL 过期后才重新拉取；未过期直接读文件
//! - 拉取失败时：若本地有旧缓存（哪怕已过 TTL），回退到旧缓存，避免前端白屏
//! - 支持按 symbol 前缀路由到不同交易所：
//!     * 无前缀 → Binance（向后兼容）
//!     * `BINANCE:xxx` → Binance
//!     * `BYBIT:xxx`   → Bybit
//!     * `BITGET:xxx`  → Bitget
//!     * `OKX:xxx`     → OKX
//!
//! 文件命名：
//! - Binance: `<cache_dir>/<SYMBOL>_<TIMEFRAME>.json`（保持向后兼容）
//! - 其他:    `<cache_dir>/<EXCHANGE>_<SYMBOL>_<TIMEFRAME>.json`

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::binance::Binance;
use super::bitget::Bitget;
use super::bybit::Bybit;
use super::kline::{Kline, Timeframe};
use super::okx::Okx;

/// 缓存文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    pub symbol: String,
    pub timeframe: String,
    /// 缓存最后更新的 Unix 时间（秒）
    pub updated_at: i64,
    /// K线数组（open_time 升序）
    pub klines: Vec<Kline>,
}

/// 交易所枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Exchange {
    Binance,
    Bybit,
    Bitget,
    Okx,
}

impl Exchange {
    pub fn as_str(&self) -> &'static str {
        match self {
            Exchange::Binance => "BINANCE",
            Exchange::Bybit => "BYBIT",
            Exchange::Bitget => "BITGET",
            Exchange::Okx => "OKX",
        }
    }

    /// 遍历所有交易所（用于 /api/symbols 聚合）
    pub fn all() -> [Exchange; 4] {
        [Exchange::Binance, Exchange::Bybit, Exchange::Bitget, Exchange::Okx]
    }

    /// 从用户输入的 symbol 解析交易所前缀
    ///
    /// 支持格式：
    /// - `BINANCE:BTCUSDT` → (Binance, "BTCUSDT")
    /// - `BYBIT:BTCUSDT`   → (Bybit, "BTCUSDT")
    /// - `BITGET:BTCUSDT`  → (Bitget, "BTCUSDT")
    /// - `OKX:BTCUSDT`     → (Okx, "BTCUSDT")
    /// - `BTCUSDT`         → (Binance, "BTCUSDT") （向后兼容）
    pub fn parse_symbol(input: &str) -> (Exchange, String) {
        let up = input.to_uppercase();
        if let Some(rest) = up.strip_prefix("BINANCE:") {
            (Exchange::Binance, rest.to_string())
        } else if let Some(rest) = up.strip_prefix("BYBIT:") {
            (Exchange::Bybit, rest.to_string())
        } else if let Some(rest) = up.strip_prefix("BITGET:") {
            (Exchange::Bitget, rest.to_string())
        } else if let Some(rest) = up.strip_prefix("OKX:") {
            (Exchange::Okx, rest.to_string())
        } else {
            (Exchange::Binance, up)
        }
    }
}

pub struct KlineCache {
    root: PathBuf,
    binance: Binance,
    bybit: Bybit,
    bitget: Bitget,
    okx: Okx,
    /// 缓存有效期（秒）。过期后会重新拉取。
    ttl_secs: i64,
}

impl KlineCache {
    /// 构造：传入四个交易所客户端
    pub fn new(
        root: impl Into<PathBuf>,
        binance: Binance,
        bybit: Bybit,
        bitget: Bitget,
        okx: Okx,
    ) -> Self {
        Self {
            root: root.into(),
            binance,
            bybit,
            bitget,
            okx,
            ttl_secs: 60,
        }
    }

    pub fn with_ttl(mut self, secs: i64) -> Self {
        self.ttl_secs = secs;
        self
    }

    /// 获取 K 线：命中缓存则直接返回，否则按 symbol 前缀路由到对应交易所。
    ///
    /// `symbol` 可带前缀（`BINANCE:` / `BYBIT:`）或不带（默认 Binance）
    ///
    /// 返回的 K 线数量 = min(缓存数量, limit)
    pub fn get(&self, symbol: &str, tf: Timeframe, limit: usize) -> Result<Vec<Kline>, String> {
        let (exchange, native_symbol) = Exchange::parse_symbol(symbol);
        self.get_for(exchange, &native_symbol, tf, limit)
    }

    /// 显式指定交易所的获取方法
    pub fn get_for(
        &self,
        exchange: Exchange,
        native_symbol: &str,
        tf: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, String> {
        let path = self.path_for(exchange, native_symbol, tf);
        let now = now_secs();

        // 1. 尝试读取已有缓存
        if let Some(cache) = self.read_existing(&path) {
            let fresh = now - cache.updated_at < self.ttl_secs;
            let enough = cache.klines.len() >= limit;
            if fresh && enough {
                log::debug!(
                    "缓存命中 {} {} {} (n={}, age={}s)",
                    exchange.as_str(),
                    native_symbol,
                    tf.as_str(),
                    cache.klines.len(),
                    now - cache.updated_at
                );
                return Ok(tail(&cache.klines, limit));
            }
        }

        // 2. 按交易所路由拉取
        let fetch_result: Result<Vec<Kline>, String> = if limit <= 1000 {
            log::info!(
                "拉取 {} {} {} limit={}",
                exchange.as_str(),
                native_symbol,
                tf.as_str(),
                limit
            );
            match exchange {
                Exchange::Binance => self.binance.klines(native_symbol, tf, limit.max(500)),
                Exchange::Bybit => self.bybit.klines(native_symbol, tf, limit.max(500)),
                Exchange::Bitget => self.bitget.klines(native_symbol, tf, limit.max(500)),
                Exchange::Okx => self.okx.klines(native_symbol, tf, limit.min(300).max(100)),
            }
        } else {
            // 估算起点：向前多取 10% 做冗余
            let span_ms = (limit as i64) * tf.interval_ms();
            let end_ms = now_ms();
            let start_ms = (end_ms - (span_ms + span_ms / 10)).max(0);
            log::info!(
                "分页拉取 {} {} {} limit={} (start={} end={})",
                exchange.as_str(),
                native_symbol,
                tf.as_str(),
                limit,
                start_ms,
                end_ms
            );
            match exchange {
                Exchange::Binance => self
                    .binance
                    .klines_range(native_symbol, tf, start_ms, end_ms)
                    .map(|all| tail(&all, limit)),
                Exchange::Bybit => self
                    .bybit
                    .klines_range(native_symbol, tf, start_ms, end_ms)
                    .map(|all| tail(&all, limit)),
                Exchange::Bitget => self
                    .bitget
                    .klines_range(native_symbol, tf, start_ms, end_ms)
                    .map(|all| tail(&all, limit)),
                Exchange::Okx => self
                    .okx
                    .klines_range(native_symbol, tf, start_ms, end_ms)
                    .map(|all| tail(&all, limit)),
            }
        };

        let fetched = match fetch_result {
            Ok(k) => k,
            Err(e) => {
                // 降级：若本地有旧缓存（哪怕过期），先用起来避免前端白屏
                if let Some(cache) = self.read_existing(&path) {
                    if !cache.klines.is_empty() {
                        log::warn!(
                            "{} 拉取失败（{}）, 降级到旧缓存 {} {} (n={}, age={}s)",
                            exchange.as_str(),
                            e,
                            native_symbol,
                            tf.as_str(),
                            cache.klines.len(),
                            now - cache.updated_at,
                        );
                        return Ok(tail(&cache.klines, limit));
                    }
                }
                return Err(e);
            }
        };

        // 3. 写入缓存
        let file = CacheFile {
            symbol: native_symbol.to_uppercase(),
            timeframe: tf.as_str().to_string(),
            updated_at: now,
            klines: fetched.clone(),
        };
        if let Err(e) = self.write_file(&path, &file) {
            log::warn!("写入缓存失败: {}", e);
        }

        Ok(tail(&fetched, limit))
    }

    /// 暴露给上层：获取对应交易所的 symbol 列表（Spot USDT）
    pub fn list_symbols(&self, exchange: Exchange) -> Result<Vec<String>, String> {
        match exchange {
            Exchange::Binance => self.binance.exchange_info_usdt_symbols(),
            Exchange::Bybit => self.bybit.instruments_info_usdt_symbols(),
            Exchange::Bitget => self.bitget.symbols_spot_usdt(),
            Exchange::Okx => self.okx.instruments_spot_usdt(),
        }
    }

    fn path_for(&self, exchange: Exchange, symbol: &str, tf: Timeframe) -> PathBuf {
        // Binance 保持原有命名（向后兼容已有 JSON 缓存文件）
        // 其他交易所以 `<EXCHANGE>_<SYMBOL>_<TIMEFRAME>.json` 命名
        let name = match exchange {
            Exchange::Binance => {
                format!("{}_{}.json", symbol.to_uppercase(), tf.as_str())
            }
            _ => {
                format!(
                    "{}_{}_{}.json",
                    exchange.as_str(),
                    symbol.to_uppercase(),
                    tf.as_str()
                )
            }
        };
        self.root.join(name)
    }

    fn read_existing(&self, path: &Path) -> Option<CacheFile> {
        let bytes = fs::read(path).ok()?;
        serde_json::from_slice::<CacheFile>(&bytes).ok()
    }

    fn write_file(&self, path: &Path, cache: &CacheFile) -> std::io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(cache)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn tail(v: &[Kline], n: usize) -> Vec<Kline> {
    if n >= v.len() {
        v.to_vec()
    } else {
        v[v.len() - n..].to_vec()
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
